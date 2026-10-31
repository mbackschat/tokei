"""Judges Clojure, ClojureScript and ClojureC files by the Clojure reader's
lexical rules: reads paths on stdin and prints, per file, the comment and
literal ranges in UTF-16 offsets after a byte order mark and whether a
string, regular expression or character ran to the end of the file, as
judgecmp's stream expects.

The rules are those of `src/jvm/clojure/lang/LispReader.java` at clojure
commit 33fb064dee. A `;` opens a comment to the end of the line, as `#!`
does at the start of a token (CommentReader); `"` opens a string and `#"`
a regular expression, in both of which a backslash takes the next
character (StringReader, RegexReader); a backslash outside them takes the
next character, whatever it is, as a character literal (CharacterReader).
Any other token runs to whitespace, a comma or a terminating macro
character, all macro characters but `#`, `'` and `%` (readToken,
isTerminatingMacro), so a `#` inside a symbol starts nothing.

    python3 -I clojure/judge.py
"""

import json
import sys

MACROS = set("\";'@^`~()[]{}\\%#")
TERMINATING = MACROS - set("#'%")


def ends_token(ch):
    return ch.isspace() or ch == "," or ch in TERMINATING


def quoted_end(text, i):
    """The end of the string or regular expression whose body starts at `i`."""
    n = len(text)
    while i < n:
        if text[i] == "\\":
            i += 2
        elif text[i] == '"':
            return i + 1
        else:
            i += 1
    return None


def judge(path):
    with open(path, "rb") as handle:
        data = handle.read()
    if data.startswith(b"\xef\xbb\xbf"):
        data = data[3:]
    try:
        text = data.decode("utf-8")
    except UnicodeDecodeError:
        return
    comments, literals, errors = [], [], 0
    i, n = 0, len(text)
    while i < n:
        ch = text[i]
        if ch == ";" or text.startswith("#!", i):
            end = i
            while end < n and text[end] not in "\r\n":
                end += 1
            comments.append([i, end])
            i = end
        elif ch == '"' or text.startswith('#"', i):
            start = i + (ch == "#")
            end = quoted_end(text, start + 1)
            if end is None:
                errors, end = 1, n
            literals.append([start, end])
            i = end
        elif ch == "\\":
            if i + 1 == n:
                errors = 1
            i += 2
        elif ch.isspace() or ch == "," or ch in MACROS:
            i += 1
        else:
            i += 1
            while i < n and not ends_token(text[i]):
                i += 1
    units = [len(c.encode("utf-16-le")) // 2 for c in text]
    prefix = [0]
    for u in units:
        prefix.append(prefix[-1] + u)
    convert = lambda ranges: [[prefix[a], prefix[b]] for a, b in ranges]
    result = {"path": path, "units": "utf16", "errors": errors, "comments": convert(comments), "literals": convert(literals)}
    print(json.dumps(result), flush=True)


for line in sys.stdin:
    if line.strip():
        judge(line.rstrip("\n"))
