"""Judges gettext PO files by GNU gettext's reading rules.

Ports the tokenizer of `gettext-tools/src/read-po-lex.c`: outside a string,
`#` starts a comment that runs to the end of its line, and `"…"` strings,
with backslash escapes, are literals. The lexer reads the `#~` lines of an
obsolete entry and the `#|` lines of a previous string as entries, but only
to check them; the gettext manual calls obsolete entries "commented out",
lists `#|` lines among an entry's comments (sections "The Format of PO
Files" and "Obsolete Entries"), and msgfmt compiles neither, so they are
comments here. A string left open at the end of its line is an error, as
the lexer reports it. Reads paths on stdin and prints, per file, the
comment and literal ranges in byte offsets after any byte order mark, as
judgecmp's stream expects.
"""

import json
import sys


def judge(path):
    with open(path, "rb") as f:
        data = f.read().removeprefix(b"\xef\xbb\xbf")
    comments, literals, errors = [], [], 0
    i, n = 0, len(data)
    while i < n:
        byte = data[i]
        if byte == ord("#"):
            end = data.find(b"\n", i)
            end = n if end < 0 else end
            if end > i and data[end - 1] == 0x0D:
                end -= 1
            comments.append([i, end])
            i = end
        elif byte == ord('"'):
            j = i + 1
            while j < n and data[j] not in b'"\n':
                j += 2 if data[j] == 0x5C else 1
            if j < n and data[j] == ord('"'):
                literals.append([i, j + 1])
                i = j + 1
            else:
                errors += 1
                literals.append([i, j])
                i = j
        else:
            i += 1
    return {"path": path, "units": "bytes", "errors": errors, "comments": comments, "literals": literals}


for entry in sys.stdin:
    print(json.dumps(judge(entry.rstrip("\n"))))
