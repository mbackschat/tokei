"""Judges PlantUML sources by PlantUML v1.2026.9's preprocessor, which drops
comments line by line before any diagram reads its text; PlantUML has no
tokenizer to call, so these rules are ported from it. `TContext` chains three
iterators over the lines, in this order:

- `CodeIteratorLongComment`: a line `TLineType` reads as `COMMENT_LONG_START`,
  one opening with `/'` after blanks and holding no `'/`, starts a comment of
  whole lines that ends with the first line, this one included, whose trimmed
  text ends with `'/`, or with the file.
- `CodeIteratorShortComment`: a line opening with `'` after blanks, or holding
  nothing but `/' … '/` and blanks, is a comment (`COMMENT_SIMPLE`).
- `CodeIteratorInnerComment`, `StringLocated.removeInnerComment`: of a line
  whose trimmed text opens with `/'`, the text up to its first `'/` is a
  comment; otherwise, of a line whose trimmed text ends with `'/`, the text
  from its last `/'` on is; and `/'''name'''/` markers are comments anywhere.
  A `/' … '/` with text on both sides is no comment.

Blanks are Java's `\\s` and U+00A0 in the line patterns (`%s` in
`Pattern2`), characters up to U+0020 and U+00A0 when `StringUtils.trin`
trims, and characters up to U+0020 when `removeInnerComment` trims. Lines end
at `\\n`, `\\r\\n` or `\\r`, as `BufferedReader.readLine` reads them. PlantUML
has no string literals the preprocessor reads across lines, so a file holds
no literals and none is an error.

Reads paths on stdin and prints, per file, the comment ranges in byte
offsets after a byte order mark, as judgecmp's stream expects.
"""
import json
import re
import sys

BLANKS = "\\s\u00a0"
COMMENT_SIMPLE = re.compile(f"[{BLANKS}]*(?:'|/'.*'/[{BLANKS}]*$)")
COMMENT_LONG_START = re.compile(f"[{BLANKS}]*/'")
LINE = re.compile(r"[^\r\n]*(?:\r\n|\r|\n|$)")
SPECIAL_INNER = re.compile(r"/'''[-A-Za-z0-9_]*'''/")


def trin(s):
    return s.strip("".join(map(chr, range(0x21))) + "\u00a0")


def java_trim(s):
    return s.strip("".join(map(chr, range(0x21))))


def lines(text):
    """Each line's start offset and its text without the line end."""
    for m in LINE.finditer(text):
        if m.start() == len(text) and text:
            return
        yield m.start(), m.group().rstrip("\r\n")


def inner_comments(line):
    """The comment spans `removeInnerComment` drops from a line, as offsets in it."""
    trim = java_trim(line.replace("\t", " "))
    spans = []
    rest_from, rest_to = 0, len(line)
    if trim.startswith("/'") and (end := line.find("'/")) != -1:
        spans.append((line.find("/'"), end + 2))
        rest_from = end + 2
    elif trim.endswith("'/") and (start := line.rfind("/'")) != -1:
        spans.append((start, len(line)))
        rest_to = start
    elif not ("/'''" in trim and "'''/" in trim):
        return spans
    rest = line[rest_from:rest_to]
    if "/'''" in rest and "'''/" in rest:
        spans += [(rest_from + m.start(), rest_from + m.end()) for m in SPECIAL_INNER.finditer(rest)]
    return sorted(spans)


def judge_text(text):
    """Comment ranges in character offsets."""
    comments = []
    in_long = False
    for start, line in lines(text):
        if in_long:
            in_long = not trin(line).endswith("'/")
            comments[-1][1] = start + len(line)
        elif COMMENT_LONG_START.match(line) and "'/" not in line:
            in_long = True
            comments.append([start, start + len(line)])
        elif COMMENT_SIMPLE.match(line):
            comments.append([start, start + len(line)])
        else:
            comments += [[start + a, start + b] for a, b in inner_comments(line)]
    return comments


def judge(path):
    with open(path, "rb") as f:
        data = f.read()
    text = data.removeprefix(b"\xef\xbb\xbf").decode("utf-8", errors="surrogateescape")
    byte = [0]
    for c in text:
        byte.append(byte[-1] + len(c.encode("utf-8", errors="surrogateescape")))
    comments = [[byte[a], byte[b]] for a, b in judge_text(text) if a < b]
    return {"path": path, "units": "bytes", "errors": 0, "comments": comments, "literals": []}


def main():
    for line in sys.stdin:
        path = line.rstrip("\n")
        if path:
            print(json.dumps(judge(path)))


if __name__ == "__main__":
    main()
