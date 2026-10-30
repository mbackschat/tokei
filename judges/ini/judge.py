"""Judges INI files as Python's configparser reads them.

INI has no standard; configparser is the parser that reads them in Python's
standard library, and Tokei's `;` and `#` comments are its default comment
prefixes. A line is a comment when, stripped, it starts with a prefix, even
inside a value continued over indented lines (`_Line._strip_full` in
CPython 3.13's `Lib/configparser.py`); inline comments are off by default, so
the rest of a line holding a value is code. A file that `RawConfigParser`
rejects, with duplicates allowed and no interpolation, is invalid. Reads
paths on stdin and prints, per file, the comment ranges in byte offsets, as
judgecmp's stream expects.
"""

import configparser
import json
import sys

PREFIXES = ("#", ";")


def judge(path):
    with open(path, "rb") as f:
        data = f.read()
    try:
        text = data.decode("utf-8-sig")
    except UnicodeDecodeError:
        return None
    bom = len(data) - len(text.encode("utf-8"))
    errors = 0
    try:
        configparser.RawConfigParser(strict=False, allow_no_value=True).read_string(text)
    except configparser.Error:
        errors = 1
    comments, offset = [], bom
    for raw in data[bom:].split(b"\n"):
        line = raw.decode("utf-8")
        if line.strip().startswith(PREFIXES):
            comments.append([offset, offset + len(raw.rstrip(b"\r"))])
        offset += len(raw) + 1
    return {"path": path, "units": "bytes", "errors": errors, "comments": comments, "literals": []}


for entry in sys.stdin:
    judged = judge(entry.rstrip("\n"))
    if judged is not None:
        print(json.dumps(judged))
