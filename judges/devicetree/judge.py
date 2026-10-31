"""Judges Device Tree sources by dtc 1.8.1's lexer rules.

dtc exports no tokenizer, so this ports the rules of `dtc-lexer.l` that
decide where comments and literals start and end, in every start condition:

- `STRING`, `"` with backslash escapes, may span lines; `CHAR_LITERAL`, `'`
  with `\\'` escapes, may too.
- `COMMENT` runs from `/*` to the next `*/`, and `LINECOMMENT` from `//` to
  the end of its line.
- A new-style path reference, `&{` path characters `}`, is one token, so a
  `/*` inside it, as in `&{/soc/*}`, opens no comment.

Sources such as Linux's are run through the C preprocessor first
(`cmd_dtc_cpp` in `scripts/Makefile.lib`), which reads comments and strings
by the same rules, so its directive lines are read as dtc reads any line. Two
places where the preprocessor reads differently are taken from it: a `//`
comment ends a file that ends without a newline, and a backslash at the end
of a `//` comment's line continues the comment onto the next (C17 5.1.1.2,
translation phase 2).

Reads paths on stdin and prints, per file, the comment and literal ranges in
byte offsets, as judgecmp's stream expects. A string, character literal or
comment that does not close is an error.
"""
import json
import re
import sys

STRING = re.compile(rb'"(?:[^\\"]|\\[\s\S])*"')
CHAR_LITERAL = re.compile(rb"'(?:\\'|[^'])*'")
PATH_REFERENCE = re.compile(rb"&\{[a-zA-Z0-9,._+*#?@/-]*\}")
SPECIAL = re.compile(rb"[\"'/&]")


class Unclosed(Exception):
    pass


def line_comment_end(data, at):
    """The end of the `//` comment at `at`, before the newline that ends it."""
    while True:
        newline = data.find(b"\n", at)
        if newline < 0:
            return len(data)
        if not data[:newline].rstrip(b"\r").endswith(b"\\"):
            return newline
        at = newline + 1


def judge_bytes(data):
    comments, literals = [], []
    at = 0
    while True:
        m = SPECIAL.search(data, at)
        if not m:
            return comments, literals
        at = m.start()
        b = data[at:at + 1]
        if b in (b'"', b"'"):
            literal = (STRING if b == b'"' else CHAR_LITERAL).match(data, at)
            if not literal:
                raise Unclosed
            literals.append([at, literal.end()])
            at = literal.end()
        elif data.startswith(b"/*", at):
            end = data.find(b"*/", at + 2)
            if end < 0:
                raise Unclosed
            comments.append([at, end + 2])
            at = end + 2
        elif data.startswith(b"//", at):
            end = line_comment_end(data, at)
            comments.append([at, end])
            at = end
        elif b == b"&" and (path := PATH_REFERENCE.match(data, at)):
            at = path.end()
        else:
            at += 1


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
