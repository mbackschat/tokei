"""Judges C, C++, Objective-C and Objective-C++ files with clang's raw lexer.

Reads paths on stdin and prints, per file, the comment and literal ranges in
byte offsets after a byte order mark, as judgecmp's stream expects.
`clang -cc1 -dump-raw-tokens`, which prints to stderr, lexes without
preprocessing, as the counts read the text the program's author wrote, and
keeps whitespace as tokens, so each token ends where the next begins. A quote that closes nowhere on
its line is an `unknown` token, code to the end of the line, as clang reads
it in skipped blocks and `#error` text; only an unterminated block comment
makes a file invalid. Line splices belong to no token. Headers (`.h`) are lexed as C++, whose raw strings C
lacks; the other forms of the two languages lex alike.
"""

import json
import os
import re
import subprocess
import sys
from concurrent.futures import ThreadPoolExecutor

MODES = {
    ".c": ["-x", "c", "-std=c23"],
    ".ec": ["-x", "c", "-std=c23"],
    ".pgc": ["-x", "c", "-std=c23"],
    ".m": ["-x", "objective-c"],
    ".mm": ["-x", "objective-c++", "-std=c++2c"],
}
CPP = ["-x", "c++", "-std=c++2c"]
SPLICE = re.compile(rb"\\[ \t\f\v]*\r?\n")
LITERALS = re.compile(r"(wide_|utf8_|utf16_|utf32_)?(string_literal|char_constant)$")


def judge(path):
    with open(path, "rb") as f:
        data = f.read()
    try:
        data.decode("utf-8")
    except UnicodeDecodeError:
        return None
    bom = 3 if data.startswith(b"\xef\xbb\xbf") else 0
    mode = MODES.get(os.path.splitext(path)[1].lower(), CPP)
    run = subprocess.run(["clang", "-cc1", "-dump-raw-tokens", *mode, path], capture_output=True)
    if run.returncode != 0:
        return {"path": path, "units": "bytes", "errors": 1, "comments": [], "literals": []}
    starts = [0]
    for i, b in enumerate(data):
        if b == 0x0A:
            starts.append(i + 1)
    anchor = re.compile(rb"Loc=<" + re.escape(path.encode()) + rb":(\d+):(\d+)>\n")
    tokens, begin = [], 0
    for m in anchor.finditer(run.stderr):
        kind = run.stderr[begin : m.start()].split(b" ", 1)[0].decode()
        tokens.append((kind, starts[int(m[1]) - 1] + int(m[2]) - 1))
        begin = m.end()
    comments, literals, errors = [], [], 0
    for i, (kind, start) in enumerate(tokens):
        end = tokens[i + 1][1] if i + 1 < len(tokens) else len(data)
        # clang starts a token at the line splices before it, which
        # translation phase 2 deletes (C23 §5.1.1.2), so they belong to no
        # token.
        while m := SPLICE.match(data, start, end):
            start = m.end()
        if kind == "comment":
            comments.append([start - bom, end - bom])
        elif LITERALS.match(kind):
            literals.append([start - bom, end - bom])
        elif kind == "unknown" and data.startswith(b"/*", start):
            errors += 1
    return {"path": path, "units": "bytes", "errors": errors, "comments": comments, "literals": literals}


paths = [line.rstrip("\n") for line in sys.stdin]
with ThreadPoolExecutor(os.cpu_count()) as pool:
    for judged in pool.map(judge, paths):
        if judged is not None:
            print(json.dumps(judged))
