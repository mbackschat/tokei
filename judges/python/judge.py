"""Judges Python files with CPython's own tokenizer, `tokenize`.

Reads paths on stdin and prints, per file, the comment and literal ranges in
byte offsets and the number of tokenizer errors, as judgecmp's stream expects.
Strings, docstrings included, and the text parts of f-strings and t-strings
are literals; replacement fields are code with their own strings and comments
(PEP 701). A file the tokenizer rejects counts its error, keeping the tokens
read before it.
"""

import json
import sys
import tokenize

LITERALS = {tokenize.STRING}
for name in ("FSTRING_START", "FSTRING_MIDDLE", "FSTRING_END", "TSTRING_START", "TSTRING_MIDDLE", "TSTRING_END"):
    if hasattr(tokenize, name):
        LITERALS.add(getattr(tokenize, name))


def judge(path):
    with open(path, "rb") as f:
        data = f.read()
    try:
        text = data.decode("utf-8")
    except UnicodeDecodeError:
        return None
    if text.startswith("﻿"):
        text = text[1:]
    # Byte offset of each line start; tokenize counts columns in characters.
    lines = text.splitlines(keepends=True)
    starts = [0]
    for line in lines:
        starts.append(starts[-1] + len(line.encode("utf-8")))

    def offset(row, col):
        if row - 1 >= len(lines):
            return starts[-1]
        return starts[row - 1] + len(lines[row - 1][:col].encode("utf-8"))

    comments, literals, errors = [], [], 0
    readline = iter(text.splitlines(keepends=True)).__next__
    try:
        for token in tokenize.generate_tokens(readline):
            if token.type == tokenize.COMMENT:
                comments.append([offset(*token.start), offset(*token.end)])
            elif token.type in LITERALS:
                literals.append([offset(*token.start), offset(*token.end)])
            elif token.type == tokenize.ERRORTOKEN:
                errors += 1
    except (tokenize.TokenError, SyntaxError, StopIteration):
        errors += 1
    return {"path": path, "units": "bytes", "errors": errors, "comments": comments, "literals": literals}


for line in sys.stdin:
    judged = judge(line.rstrip("\n"))
    if judged is not None:
        print(json.dumps(judged))
