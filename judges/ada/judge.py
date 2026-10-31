"""Judges Ada files by the Ada reference manual's lexical rules, as GNAT's
scanner reads them: reads paths on stdin and prints, per file, the comment and
literal ranges in byte offsets after a byte order mark and whether a string or
character literal was left open, as judgecmp's stream expects.

No Ada toolchain at hand exposes its tokens, so this is a reference judge
with documented limits, checked against GNAT's syntax check instead. It reads
`--` comments (RM 2.7), `"…"` strings with `""` escapes and GNAT's `%…%`
replacement strings (RM 2.6, J.2), and character literals (RM 2.5). An
apostrophe after an identifier, `all`, `delta`, `digits`, `project`, `@`,
`)`, `]` or a literal is an attribute's or a qualified expression's tick
instead, as GNAT decides by the token before it (gcc `gcc/ada/scng.adb` at
`3f35c3d2d1`, `Char_Literal_Case`).

    python3 -I ada/judge.py
"""

import json
import re
import sys

# Reserved words after which an apostrophe is a tick, by GNAT's look-back.
TICK_WORDS = {"all", "delta", "digits", "project"}
RESERVED = set(
    """abort abs abstract accept access aliased all and array at begin body
    case constant declare delay delta digits do else elsif end entry exception
    exit for function generic goto if in interface is limited loop mod new not
    null of or others out overriding package parallel pragma private procedure
    protected raise range record rem renames requeue return reverse select
    separate some subtype synchronized tagged task terminate then type until
    use when while with xor""".split()
)
IDENTIFIER = re.compile(r"[^\W\d]\w*")
NUMBER = re.compile(r"\d[\d_]*(#[\w.]*#|\.[\d_]+)?([eE][+-]?\d+)?")


def judge(path):
    with open(path, "rb") as handle:
        data = handle.read()
    if data.startswith(b"\xef\xbb\xbf"):
        data = data[3:]
    try:
        text = data.decode("utf-8")
    except UnicodeDecodeError:
        return
    # Byte offsets of each character, as the text is read in characters.
    offsets = [0]
    for char in text:
        offsets.append(offsets[-1] + len(char.encode("utf-8")))
    comments, literals = [], []
    errors = 0
    # Whether the token before is one after which `'` is a tick.
    ticks = False
    i, n = 0, len(text)
    while i < n:
        char = text[i]
        if char.isspace():
            i += 1
            continue
        if text.startswith("--", i):
            end = text.find("\n", i)
            end = n if end < 0 else end
            last = end - 1 if text[end - 1] == "\r" else end
            comments.append([offsets[i], offsets[last]])
            i = end
            continue
        if char in "\"%":
            j = i + 1
            while True:
                if j >= n or text[j] == "\n":
                    errors = 1
                    break
                if text[j] == char:
                    if text.startswith(char * 2, j):
                        j += 2
                        continue
                    j += 1
                    break
                j += 1
            literals.append([offsets[i], offsets[j]])
            i, ticks = j, True
            continue
        if char == "'":
            if not ticks and text[i + 2 : i + 3] == "'" and text[i + 1] != "\n":
                literals.append([offsets[i], offsets[i + 3]])
                i, ticks = i + 3, True
            else:
                if not ticks:
                    errors = 1
                i, ticks = i + 1, False
            continue
        match = IDENTIFIER.match(text, i)
        if match:
            word = match.group().lower()
            ticks = word in TICK_WORDS or word not in RESERVED
            i = match.end()
            continue
        match = NUMBER.match(text, i)
        if match:
            i, ticks = match.end(), True
            continue
        ticks = char in ")]@"
        i += 1
    print(
        json.dumps(
            {"path": path, "units": "bytes", "errors": errors, "comments": comments, "literals": literals}
        ),
        flush=True,
    )


for path in sys.stdin.read().splitlines():
    judge(path)
