"""Judges Meson build files with Meson's own lexer.

Runs `mesonbuild.mparser.Lexer`, the lexer `meson setup` parses
`meson.build` and `meson.options` files with. Strings, f-strings and their
`'''` forms are literals; `#` comments are comments, including one after a
`\\` line continuation, which the lexer reads as part of that token. A file
the lexer rejects, such as one using double quotes, is invalid. Reads paths
on stdin and prints, per file, the comment and literal ranges in byte
offsets after any byte order mark, as judgecmp's stream expects; the lexer
works in characters, so offsets are converted.
"""

import json
import sys

from mesonbuild import mparser

LITERALS = {"string", "fstring", "multiline_string", "multiline_fstring"}


def judge(path):
    with open(path, "rb") as f:
        data = f.read().removeprefix(b"\xef\xbb\xbf")
    try:
        text = data.decode("utf-8")
    except UnicodeDecodeError:
        return None
    # Byte offset of each character, and of the end.
    offsets, at = [], 0
    for char in text:
        offsets.append(at)
        at += len(char.encode("utf-8"))
    offsets.append(at)
    comments, literals, errors = [], [], 0
    try:
        for token in mparser.Lexer(text).lex(path):
            start, end = token.bytespan
            if token.tid in LITERALS:
                literals.append([offsets[start], offsets[end]])
            elif token.tid == "comment" or token.tid == "whitespace" and "#" in text[start:end]:
                start = text.index("#", start) if token.tid == "whitespace" else start
                end = text.find("\n", start, end) if token.tid == "whitespace" else end
                if text[end - 1] == "\r":
                    end -= 1
                comments.append([offsets[start], offsets[end]])
    except mparser.ParseException:
        errors = 1
    return {"path": path, "units": "bytes", "errors": errors, "comments": comments, "literals": literals}


for entry in sys.stdin:
    judged = judge(entry.rstrip("\n"))
    if judged is not None:
        print(json.dumps(judged))
