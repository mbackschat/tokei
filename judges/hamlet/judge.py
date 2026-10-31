"""Judges Hamlet templates by the line rules of shakespeare 2.1.7.1's parser,
`Text.Hamlet.Parse.parseLine`, ported since the module exports no function
that keeps line positions:

- A line whose text after its indentation of spaces opens with `$#` is a
  comment to its end (`comment`).
- A line whose text opens with `<!--` drops everything up to the next `-->`,
  across lines if need be, and then each further `<!-- … -->` of the rest of
  its line (`htmlComment`, `nonComments`).

Hamlet passes a `<!--` later in a line on to the page as text; the page holds
it as an HTML comment, as a browser reads it, so the judge reads it as one
too, running to the next `-->`. Tabs in indentation are an error, so such a
file is invalid. Hamlet has no strings that span lines; attribute values end
with their line, so a file holds no literals.

Reads paths on stdin and prints, per file, the comment ranges in byte offsets
after a byte order mark, as judgecmp's stream expects.
"""
import json
import re
import sys

LINE = re.compile(rb"[^\n]*(?:\n|$)")


def judge(text):
    comments = []
    pos = 0
    errors = 0
    while pos < len(text):
        end = LINE.match(text, pos).end()
        line = text[pos:end].rstrip(b"\r\n")
        indent = len(line) - len(line.lstrip(b" "))
        if line[indent:indent + 1] == b"\t":
            errors += 1
        if line.startswith(b"$#", indent):
            comments.append([pos + indent, pos + len(line)])
            pos = end
            continue
        at = pos + indent
        line_end = pos + len(line)
        while (open_ := text.find(b"<!--", at, line_end)) != -1:
            close = text.find(b"-->", open_ + 4)
            close = len(text) if close == -1 else close + 3
            comments.append([open_, close])
            at = close
            if close > line_end:
                line_end = text.find(b"\n", close)
                line_end = len(text) if line_end == -1 else line_end
                if text[line_end - 1:line_end] == b"\r":
                    line_end -= 1
        pos = text.find(b"\n", max(at, pos), len(text))
        pos = len(text) if pos == -1 else pos + 1
    return comments, errors


for path in sys.stdin.read().splitlines():
    with open(path, "rb") as f:
        text = f.read()
    if text.startswith(b"\xef\xbb\xbf"):
        text = text[3:]
    comments, errors = judge(text)
    print(json.dumps({"path": path, "units": "bytes", "errors": errors, "comments": comments, "literals": []}))
