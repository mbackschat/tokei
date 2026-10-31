"""Judges Module-Definition files by the lexer of LLVM 21.1.0's
`llvm/lib/Object/COFFModuleDefinition.cpp`, which lld-link and llvm-dlltool
read them with: outside a quoted name, `;` starts a comment that runs to the
end of its line, even right after a word, which ends at `;`; `"` starts a
name that runs to the next `"`, with no escapes, across lines if need be, or
to the end of the file.

Reads paths on stdin and prints, per file, the comment and literal ranges in
byte offsets, as judgecmp's stream expects.
"""
import json
import sys


def judge_bytes(data):
    comments, literals = [], []
    at = 0
    while True:
        semicolon, quote = data.find(b";", at), data.find(b'"', at)
        if quote >= 0 and (semicolon < 0 or quote < semicolon):
            end = data.find(b'"', quote + 1)
            end = len(data) if end < 0 else end + 1
            literals.append([quote, end])
            at = end
        elif semicolon >= 0:
            end = data.find(b"\n", semicolon)
            end = len(data) if end < 0 else end
            comments.append([semicolon, end])
            at = end
        else:
            return comments, literals


def main():
    for line in sys.stdin:
        path = line.rstrip("\n")
        if path:
            with open(path, "rb") as f:
                comments, literals = judge_bytes(f.read())
            print(json.dumps({"path": path, "units": "bytes", "errors": 0, "comments": comments, "literals": literals}))


if __name__ == "__main__":
    main()
