"""Judges CSS files with tinycss2, an implementation of CSS Syntax Level 3.

Reads paths on stdin and prints, per file, the comment and literal ranges in
byte offsets and the number of tokenizer errors, as judgecmp's stream
expects. Comments are comments; strings and unquoted `url(…)` tokens are
literals. The tokenizer reports where each token starts; a token's end is
read from the text by the rules that ended it (CSS Syntax Level 3, §4.3.2
comments, §4.3.5 strings, §4.3.6 URLs). A bad string or URL, or a comment or
string that the end of the file cuts off, is an error. CSS has no `//`
comments, so they are code.

    uv run --no-project --with tinycss2==1.5.1 python -I judge.py
"""

import json
import sys

import tinycss2


def string_end(text, start):
    """The end of the string at `text[start]` and whether the file ends it."""
    quote, i = text[start], start + 1
    while i < len(text):
        c = text[i]
        if c == "\\":
            i += 2
        elif c == quote:
            return i + 1, False
        else:
            i += 1
    return len(text), True


def judge(path):
    with open(path, "rb") as f:
        data = f.read()
    try:
        text = data.decode("utf-8")
    except UnicodeDecodeError:
        return None
    if text.startswith("﻿"):
        text = text[1:]
    lines = text.split("\n")
    starts = [0]
    for line in lines:
        starts.append(starts[-1] + len(line) + 1)

    def index(token):
        return starts[token.source_line - 1] + token.source_column - 1

    comments, literals, errors = [], [], 0

    def walk(tokens):
        nonlocal errors
        for token in tokens:
            if token.type == "comment":
                start = index(token)
                end = text.find("*/", start + 2)
                if end < 0:
                    errors += 1
                comments.append([start, len(text) if end < 0 else end + 2])
            elif token.type == "string":
                start = index(token)
                end, cut = string_end(text, start)
                errors += cut
                literals.append([start, end])
            elif token.type == "url":
                start = index(token)
                end = text.find(")", start)
                if end < 0:
                    errors += 1
                literals.append([start, len(text) if end < 0 else end + 1])
            elif token.type == "error" and token.kind in ("bad-string", "bad-url"):
                errors += 1
            elif token.type in ("() block", "[] block", "{} block"):
                walk(token.content)
            elif token.type == "function":
                walk(token.arguments)

    walk(tinycss2.parse_component_value_list(text, skip_comments=False))
    if text.isascii():
        offsets = range(len(text) + 1)
    else:
        offsets = [0]
        for c in text:
            offsets.append(offsets[-1] + len(c.encode("utf-8")))
    to_bytes = lambda ranges: [[offsets[a], offsets[b]] for a, b in ranges]
    return {"path": path, "units": "bytes", "errors": errors,
            "comments": to_bytes(comments), "literals": to_bytes(literals)}


for line in sys.stdin:
    judged = judge(line.rstrip("\n"))
    if judged is not None:
        print(json.dumps(judged))
