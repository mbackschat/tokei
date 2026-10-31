"""Judges `.asm` files, NASM's and MASM's, by their assemblers' lexical
rules: reads paths on stdin and prints, per file, the comment and literal
ranges in byte offsets after a byte order mark, as judgecmp's stream
expects.

NASM's rules are those of the NASM 2.16 manual, sections 3.1 and 3.4.2, and
of `nasm -E` (NASM 2.16.03), which settles what the manual leaves open: a
`;` outside a string opens a comment to the end of the line; `'…'` and
`"…"` strings hold their text verbatim, so a backslash escapes nothing, and
`` `…` `` strings take C-style backslash escapes; a string that does not
close on its line ends there, with a warning; a backslash directly before
the newline joins the next line to the line, also in a comment. MASM's
`COMMENT` directive, which NASM lacks, makes a comment of the text from the
directive to the end of the line holding the next occurrence of the first
character after it (MASM 6.1 Programmer's Guide, chapter 1, "Comments").
A file with lines that start with `//` and none that start with `;` is in
another syntax that shares the extension, such as AMD's GPU shader sources,
and is left out.

    python3 -I asm/judge.py | judgecmp stream Assembly
"""

import json
import re
import sys

COMMENT_DIRECTIVE = re.compile(rb"[ \t]*(?i:comment)[ \t]+(\S)")
SLASH_COMMENT_LINE = re.compile(rb"^[ \t]*//", re.M)
SEMICOLON_COMMENT_LINE = re.compile(rb"^[ \t]*;", re.M)


def line_end(data, i):
    end = data.find(b"\n", i)
    return len(data) if end < 0 else end


def joins_next(data, end):
    """Whether the line ending at `end` ends in a backslash that joins the
    next line to it."""
    text = data[:end].removesuffix(b"\r")
    return text.endswith(b"\\") and end < len(data)


def judge(path):
    with open(path, "rb") as handle:
        data = handle.read()
    if data.startswith(b"\xef\xbb\xbf"):
        data = data[3:]
    try:
        data.decode("utf-8")
    except UnicodeDecodeError:
        return
    if SLASH_COMMENT_LINE.search(data) and not SEMICOLON_COMMENT_LINE.search(data):
        return
    comments, literals = [], []
    i, n, line_start = 0, len(data), True
    while i < n:
        if line_start:
            line_start = False
            directive = COMMENT_DIRECTIVE.match(data, i)
            if directive:
                start = i + len(directive[0]) - len(directive[0].lstrip(b" \t"))
                close = data.find(directive[1], directive.end())
                end = line_end(data, n if close < 0 else close)
                comments.append([start, end])
                i = end
                continue
        c = data[i : i + 1]
        if c == b"\n":
            i += 1
            line_start = True
        elif c == b";":
            end = line_end(data, i)
            while joins_next(data, end):
                end = line_end(data, end + 1)
            comments.append([i, end])
            i = end
        elif c in (b"'", b'"', b"`"):
            end, j = None, i + 1
            while j < n and data[j : j + 1] != b"\n":
                if c == b"`" and data[j : j + 1] == b"\\" and data[j + 1 : j + 2] != b"\n":
                    j += 2
                    continue
                if data[j : j + 1] == c:
                    end = j + 1
                    break
                j += 1
            end = end or line_end(data, i)
            literals.append([i, end])
            i = end
        else:
            i += 1
    result = {"path": path, "units": "bytes", "errors": 0, "comments": comments, "literals": literals}
    print(json.dumps(result), flush=True)


for line in sys.stdin:
    if line.strip():
        judge(line.rstrip("\n"))
