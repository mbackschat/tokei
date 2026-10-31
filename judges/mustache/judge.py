"""Judges Mustache templates by the Mustache specification v1.4.3
(`specs/comments.yml`, `specs/delimiters.yml`) and the mustache(5) manual:
a `{{! … }}` comment runs to the closing delimiter, across lines if need be;
`{{=<% %>=}}` sets the delimiters for the rest of the template, comments
included; Mustache has no strings, so the template holds no literals.

Reads paths on stdin and prints, per file, the comment ranges in byte
offsets, as judgecmp's stream expects. A tag that does not close is an
error.
"""
import json
import sys


class Unclosed(Exception):
    pass


def judge_bytes(data):
    comments = []
    open_tag, close_tag = b"{{", b"}}"
    at = 0
    while (start := data.find(open_tag, at)) >= 0:
        inner = start + len(open_tag)
        sigil = data[inner:inner + 1]
        if sigil == b"=":
            end = data.find(b"=" + close_tag, inner + 1)
            if end < 0:
                raise Unclosed
            # The old closing delimiter ends the tag that sets the new ones.
            at = end + 1 + len(close_tag)
            open_tag, close_tag = data[inner + 1:end].split()
            continue
        if sigil == b"{" and open_tag == b"{{":
            end = data.find(b"}" + close_tag, inner + 1)
            length = len(close_tag) + 1
        else:
            end = data.find(close_tag, inner)
            length = len(close_tag)
        if end < 0:
            raise Unclosed
        if sigil == b"!":
            comments.append([start, end + length])
        at = end + length
    return comments


def judge(path):
    with open(path, "rb") as f:
        data = f.read()
    try:
        comments, errors = judge_bytes(data), 0
    except (Unclosed, ValueError):
        comments, errors = [], 1
    return {"path": path, "units": "bytes", "errors": errors, "comments": comments, "literals": []}


def main():
    for line in sys.stdin:
        path = line.rstrip("\n")
        if path:
            print(json.dumps(judge(path)))


if __name__ == "__main__":
    main()
