"""Judges Jinja2 templates with Jinja2 3.1.6's lexer, in its default syntax,
which Ansible and Salt use too: reads paths on stdin and prints, per file,
the comment and literal ranges in byte offsets, as judgecmp's stream expects.
`{# … #}` comments are comments and strings inside `{{ … }}` and `{% … %}`
are literals; template text, `{% raw %}` blocks included, is code.

`Lexer.tokeniter` yields each token's source text with its line but no
offset, so each is found in order in the source, after the previous one;
text a `-` modifier strips is never yielded and is skipped by the search.
The lexer reads `\r\n` and `\r` as `\n`, so the source is read that way
and offsets are mapped back to the bytes of the file. A template the lexer
rejects counts as one error.
"""
import json
import sys

from jinja2 import Environment
from jinja2.exceptions import TemplateSyntaxError

LEXER = Environment().lexer


def byte_offsets(raw):
    """The source as the lexer reads it, with the byte offset in `raw` of
    each of its characters and of its end."""
    chars, offsets, at = [], [], 0
    text = raw.decode("utf-8")
    for i, c in enumerate(text):
        if c == "\r" and text[i + 1:i + 2] == "\n":
            at += 1
            continue
        chars.append("\n" if c == "\r" else c)
        offsets.append(at)
        at += len(c.encode("utf-8"))
    offsets.append(at)
    return "".join(chars), offsets


def judge_text(text):
    comments, literals = [], []
    at = 0
    comment_start = None
    for _, kind, value in LEXER.tokeniter(text, None):
        start = text.index(value, at)
        at = start + len(value)
        if kind == "comment_begin":
            comment_start = start
        elif kind == "comment_end":
            comments.append([comment_start, at])
        elif kind == "string":
            literals.append([start, at])
    return comments, literals


def main():
    for line in sys.stdin:
        path = line.rstrip("\n")
        if not path:
            continue
        judged = {"path": path, "units": "bytes", "errors": 0, "comments": [], "literals": []}
        with open(path, "rb") as f:
            raw = f.read().removeprefix(b"\xef\xbb\xbf")
        try:
            text, offsets = byte_offsets(raw)
            comments, literals = judge_text(text)
        except (TemplateSyntaxError, UnicodeDecodeError):
            judged["errors"] = 1
        else:
            judged["comments"] = [[offsets[a], offsets[b]] for a, b in comments]
            judged["literals"] = [[offsets[a], offsets[b]] for a, b in literals]
        print(json.dumps(judged))


if __name__ == "__main__":
    main()
