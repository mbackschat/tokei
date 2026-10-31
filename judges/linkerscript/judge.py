"""Judges linker scripts as their builds read them: through the C
preprocessor, then GNU ld 2.45's lexer (`ld/ldlex.l`).

The projects whose scripts were measured, Zephyr, U-Boot and NuttX among
them, run every linker script through the C preprocessor before ld reads it,
so its directive lines, the only lines starting with `#` there, are code, and
the preprocessor's comments come first: `/* */` and `//`, which a final
backslash continues onto the next line (C17 5.1.1.2, translation phases 2
and 3). ld then reads what is left, where `"` starts a name that runs to the
next `"` without escapes, across lines if need be (the `"\""[^\"]*"\""`
rule).

Reads paths on stdin and prints, per file, the comment and literal ranges in
byte offsets, as judgecmp's stream expects. A comment or string that does not
close is an error.
"""
import json
import re
import sys

SPECIAL = re.compile(rb'["/]')


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
    while m := SPECIAL.search(data, at):
        at = m.start()
        if data[at] == ord('"'):
            end = data.find(b'"', at + 1)
            if end < 0:
                raise Unclosed
            literals.append([at, end + 1])
            at = end + 1
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
        else:
            at += 1
    return comments, literals


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
