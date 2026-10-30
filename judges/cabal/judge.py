"""Judges Cabal package descriptions by Cabal-syntax's lexer rules.

Cabal-syntax 3.18.2.0's `Distribution/Fields/Lexer.x` drops comments
instead of emitting them, so this ports its comment rules: at the start of a
line, in every start code, `$spacetab* "--" $comment*` is a comment, also
among a field's continuation lines; within a section line, `"--"` starts a
comment that ends the line, while in a field's value it is text. There are
no block comments. Reads paths on stdin and prints, per file, the comment
ranges in byte offsets after any byte order mark, as judgecmp's stream
expects; Cabal has no lexical errors at this level.
"""

import json
import re
import sys

COMMENT_LINE = re.compile(rb"[ \t]*--")


def judge(path):
    with open(path, "rb") as f:
        data = f.read().removeprefix(b"\xef\xbb\xbf")
    comments, offset = [], 0
    for raw in data.split(b"\n"):
        line = raw[:-1] if raw.endswith(b"\r") else raw
        match = COMMENT_LINE.match(line)
        if match:
            comments.append([offset + match.end() - 2, offset + len(line)])
        offset += len(raw) + 1
    return {"path": path, "units": "bytes", "errors": 0, "comments": comments, "literals": []}


for entry in sys.stdin:
    print(json.dumps(judge(entry.rstrip("\n"))))
