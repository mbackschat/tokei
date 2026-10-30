"""Judges SQL files with PostgreSQL's own scanner, scan.l, through pglast.

Reads paths on stdin and prints, per file, the comment and literal ranges in
byte offsets and the scan errors, as judgecmp's stream expects. String
constants and quoted identifiers are literals; `--` and nested `/* */`
comments are comments. As psql reads a script, a backslash outside SQL's
literals and comments starts a meta-command, such as `\set x 'a\'b'`, that
its own lexer reads to the end of the line (src/bin/psql/psqlscanslash.l), so
the line's rest is code. A dollar-quoted string whose text scans as SQL is read
as SQL, as PostgreSQL reads the PL/pgSQL and SQL function bodies and `DO`
blocks it is mostly used for: PL/pgSQL's scanner is the core one
(src/pl/plpgsql/src/pl_scanner.c, `core_yylex`). Any other is a literal. A
file the scanner rejects is an error.

    uv run --no-project --with pglast==8.7 python -I sql/judge.py < paths
"""

import json
import sys

from pglast.parser import ParseError, scan

COMMENTS = {"SQL_COMMENT", "C_COMMENT"}
LITERALS = {"SCONST", "USCONST", "BCONST", "XCONST"}
# Quoted identifiers, `"…"` and `U&"…"`, whose text holds no comment either.
IDENTIFIERS = {"IDENT", "UIDENT"}


def judge(path):
    with open(path, "rb") as f:
        data = f.read()
    try:
        text = data.decode("utf-8")
    except UnicodeDecodeError:
        return None
    if text.startswith("\ufeff"):
        text = text[1:]
    judged = {"path": path, "units": "bytes", "errors": 0, "comments": [], "literals": []}
    comments, literals = [], []
    try:
        classify_script(text, comments, literals)
    except ParseError:
        judged["errors"] = 1
        return judged
    # The scanner gives character positions; judgecmp takes byte offsets
    # after the byte order mark.
    offsets = [0]
    for char in text:
        offsets.append(offsets[-1] + len(char.encode("utf-8")))
    judged["comments"] = [[offsets[a], offsets[b]] for a, b in comments]
    judged["literals"] = [[offsets[a], offsets[b]] for a, b in literals]
    return judged


def classify_script(text, comments, literals):
    """Classifies a psql script: its SQL, up to each meta-command, and from
    the line after it."""
    start = 0
    while True:
        command = meta_command(text, start)
        classify(text[start:command], start, comments, literals)
        if command == len(text):
            return
        end = text.find("\n", command)
        start = len(text) if end < 0 else end


def meta_command(text, start):
    """Where the first meta-command after `start` begins, or the end of the
    text: the first backslash the SQL before it leaves outside any literal or
    comment."""
    at = text.find("\\", start)
    while at >= 0:
        try:
            tokens = scan(text[start:at])
        except ParseError:
            # The backslash is inside an unterminated literal or comment.
            pass
        else:
            last = tokens[-1] if tokens else None
            if last is None or last.name != "SQL_COMMENT" or start + last.end + 1 < at:
                return at
        at = text.find("\\", at + 1)
    return len(text)


def classify(text, base, comments, literals):
    """Adds the comment and literal ranges of `text`, which starts at
    character `base` of the file, as end-exclusive character ranges."""
    for token in scan(text):
        start, end = token.start, token.end + 1
        span = [base + start, base + end]
        if token.name in COMMENTS:
            comments.append(span)
        elif token.name == "SCONST" and text[start] == "$":
            # The body lies between the opening `$tag$` and its repetition.
            body = text.index("$", start + 1) + 1
            body_comments, body_literals = [], []
            try:
                classify(text[body : end - (body - start)], base + body, body_comments, body_literals)
            except ParseError:
                literals.append(span)
            else:
                comments.extend(body_comments)
                literals.extend(body_literals)
        elif token.name in LITERALS or (token.name in IDENTIFIERS and text[end - 1] == '"'):
            literals.append(span)


def main():
    for line in sys.stdin:
        judged = judge(line.rstrip("\n"))
        if judged is not None:
            print(json.dumps(judged))


if __name__ == "__main__":
    main()
