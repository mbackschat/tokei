"""Judges Nix files by Nix 2.35.4's lexer.

Nix exports no tokenizer, so this ports the rules of `src/libexpr/lexer.l`
that decide where comments and strings start and end, with flex's choice of
the longest match and, among equally long ones, the first rule:

- Outside strings, `#` comments run to the end of the line and `/* */`
  comments do not nest. Paths such as `/nix/store`, `a/b` and `./${x}`,
  URIs such as `https://example.org/*`, which may hold `*` and `'`, and
  identifiers such as `x''`, which may hold `'`, are read before a comment
  or string could start inside them, as is `//`, the update operator, so
  `//*` opens no comment.
- `"…"` strings take backslash escapes; `''…''` strings take the `'''`,
  `''$` and `''\` escapes. In both, `${…}` holds code, while a `$` takes the
  next character as text, so `$${` is a `$` and a brace.
- `{` and `${` push the default state and `}` pops it, which is how the end
  of a `${…}` hole returns to its string.

Reads paths on stdin and prints, per file, the comment and literal ranges in
byte offsets, as judgecmp's stream expects. A string or comment that does not
close, and a path ending in a slash, are errors, as Nix's parser rejects them.
"""
import json
import re
import sys

PATH_CHAR = rb"[a-zA-Z0-9._\-+]"
PATH = rb"%s*(?:/%s+)+/?" % (PATH_CHAR, PATH_CHAR)
PATH_SEG = rb"%s*/" % PATH_CHAR
PATH_PIECE = re.compile(rb"%s|%s|%s+" % (PATH, PATH_SEG, PATH_CHAR))

# The default state's rules in lexer.l's order, each with the action this
# judge needs. Keywords are identifiers here, since only lengths matter.
RULES = [
    (rb"\.\.\.|==|!=|<=|>=|&&|\|\||->|//|\+\+|<\||\|>", "code"),
    (rb"[a-zA-Z_][a-zA-Z0-9_'\-]*", "code"),
    (rb"[0-9]+", "code"),
    (rb"(?:[1-9][0-9]*\.[0-9]*|0?\.[0-9]+)(?:[Ee][+-]?[0-9]+)?", "code"),
    (rb"\$\{", "push"),
    (rb"\}", "pop"),
    (rb"\{", "push"),
    (rb'"', "string"),
    (rb"''(?: *\n)?", "indented"),
    (rb"(?:%s|~/)\$\{" % PATH_SEG, "segment"),
    (PATH, "path"),
    (rb"~(?:/%s+)+/?" % PATH_CHAR, "path"),
    (rb"<%s+(?:/%s+)*>" % (PATH_CHAR, PATH_CHAR), "code"),
    (rb"[a-zA-Z][a-zA-Z0-9+\-.]*:[a-zA-Z0-9%/?:@&=+$,\-_.!~*']+", "code"),
    (rb"/\*(?:[^*]|\*+[^*/])*\*+/", "comment"),
    (rb"[ \t\r\n]+", "code"),
    (rb"#[^\r\n]*", "comment"),
]
RULES = [(re.compile(pattern), action) for pattern, action in RULES]


class Unclosed(Exception):
    pass


def longest(data, at):
    """The longest default-state match at `at`, the first rule on ties."""
    best = (0, "code")
    for pattern, action in RULES:
        m = pattern.match(data, at)
        if m and m.end() - at > best[0]:
            best = (m.end() - at, action)
    # Any other character is a token of its own.
    return best if best[0] else (1, "code")


def string_end(data, at, indented):
    """Where the string body from `at` ends: at its closer or at a hole."""
    n = len(data)
    while at < n:
        b = data[at]
        if indented:
            if data.startswith(b"''", at):
                if at + 2 < n and data[at + 2] in b"$'":
                    at += 3
                elif at + 2 < n and data[at + 2] == ord("\\"):
                    at += 4
                else:
                    return at, "close"
            elif data.startswith(b"${", at):
                return at, "hole"
            elif b == ord("$"):
                at += 1 if at + 1 < n and data[at + 1] == ord("'") else 2
            elif b == ord("'"):
                at += 1 if at + 1 < n and data[at + 1] == ord("$") else 2
            else:
                at += 1
        else:
            if b == ord('"'):
                return at, "close"
            if b == ord("\\"):
                at += 2
            elif data.startswith(b"${", at):
                return at, "hole"
            elif b == ord("$"):
                nxt = data[at + 1] if at + 1 < n else None
                at += {ord('"'): 1, ord("\\"): 3}.get(nxt, 2)
            else:
                at += 1
    raise Unclosed


def judge_bytes(data):
    comments, literals = [], []
    # The stack holds, for each `{` or `${`, what its `}` returns to: None
    # for code, '"' or "''" for a string, "path" for a path.
    stack = []
    at, n = 0, len(data)
    in_path = False
    while at < n:
        if in_path:
            if data.startswith(b"${", at):
                stack.append("path")
                in_path = False
                at += 2
                continue
            m = PATH_PIECE.match(data, at)
            if m:
                at = m.end()
                continue
            in_path = False
            continue
        length, action = longest(data, at)
        if action == "comment":
            comments.append([at, at + length])
        elif action == "push":
            stack.append(None)
        elif action == "pop" and stack:
            back = stack.pop()
            if back == "path":
                in_path = True
            elif back is not None:
                at = resume(data, at + 1, back, literals, stack)
                continue
        elif action == "segment":
            # lexer.l rewinds a segment followed by `${` and reads the
            # segment alone, so the hole opens after it.
            length -= 2
            in_path = True
        elif action == "path":
            if data[at + length - 1] == ord("/") and not data.startswith(b"${", at + length):
                raise Unclosed
            in_path = True
        elif action in ("string", "indented"):
            at = resume(data, at + length, '"' if action == "string" else "''", literals, stack, at)
            continue
        elif data.startswith(b"/*", at):
            raise Unclosed
        at += length
    if any(kind in ('"', "''") for kind in stack):
        raise Unclosed
    return comments, literals


def resume(data, at, kind, literals, stack, start=None):
    """Reads the string body from `at`, recording it as a literal, and returns
    where the default state resumes: after the closer, or inside a hole."""
    start = at if start is None else start
    end, how = string_end(data, at, kind == "''")
    if how == "close":
        end += len(kind)
        literals.append([start, end])
        return end
    literals.append([start, end])
    stack.append(kind)
    return end + 2


def judge(path):
    with open(path, "rb") as f:
        data = f.read()
    try:
        comments, literals = judge_bytes(data)
        errors = 0
    except Unclosed:
        comments, literals, errors = [], [], 1
    return {"path": path, "units": "bytes", "errors": errors, "comments": comments, "literals": literals}


def main():
    for line in sys.stdin:
        path = line.rstrip("\n")
        if path:
            print(json.dumps(judge(path)))


if __name__ == "__main__":
    main()
