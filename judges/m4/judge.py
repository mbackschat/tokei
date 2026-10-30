"""Judges m4 and Autoconf files by GNU m4's tokenizer.

m4 expands macros as it reads, so this ports how GNU m4 1.4.19's
`next_token` (`src/input.c`) splits the input before any expansion: a word
is `[_A-Za-z][_A-Za-z0-9]*`, and the word `dnl` discards the rest of its line
(m4 manual, "Deleting whitespace in input"); `#` starts a comment that runs
to the end of its line; and a quoted string runs from the left quote to its
matching right quote, nesting, while neither comments nor `dnl` are
recognized inside it. Quotes are `` ` `` and `'` by default; Autoconf files,
which `m4sugar` reads after `changequote([, ])`, quote with `[` and `]`
(Autoconf manual, "M4 Quotation"). Comments and `dnl` are comments, quoted
strings literals. Text a later expansion rescans is not followed, so a `#`
line in a quoted macro argument is quoted text here; a file that changes
its quotes or comment delimiters is invalid. Reads paths on stdin and
prints, per file, the comment and literal ranges in byte offsets after any
byte order mark, as judgecmp's stream expects; with `--autoconf`, files are
read with Autoconf's quotes.
"""

import json
import re
import sys

WORD = re.compile(rb"[_A-Za-z][_A-Za-z0-9]*")
CHANGES_DELIMITERS = re.compile(rb"\b(?:m4_)?change(?:quote|com)\b")


def judge(path, left, right):
    with open(path, "rb") as f:
        data = f.read().removeprefix(b"\xef\xbb\xbf")
    comments, literals = [], []
    errors = 1 if CHANGES_DELIMITERS.search(data) else 0
    i, n = 0, len(data)
    while i < n:
        byte = data[i:i + 1]
        if byte == left:
            depth, j = 1, i + 1
            while j < n and depth:
                if data[j:j + 1] == left:
                    depth += 1
                elif data[j:j + 1] == right:
                    depth -= 1
                j += 1
            if depth:
                errors += 1
            literals.append([i, j])
            i = j
            continue
        word = WORD.match(data, i)
        if word:
            if word.group() == b"dnl":
                end = data.find(b"\n", i)
                end = n if end < 0 else end
                comments.append([i, end - 1 if end > i and data[end - 1:end] == b"\r" else end])
                i = end
            else:
                i = word.end()
            continue
        if byte == b"#":
            end = data.find(b"\n", i)
            end = n if end < 0 else end
            comments.append([i, end - 1 if end > i and data[end - 1:end] == b"\r" else end])
            i = end
            continue
        i += 1
    return {"path": path, "units": "bytes", "errors": errors, "comments": comments, "literals": literals}


left, right = (b"[", b"]") if "--autoconf" in sys.argv else (b"`", b"'")
for entry in sys.stdin:
    print(json.dumps(judge(entry.rstrip("\n"), left, right)))
