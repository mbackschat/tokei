"""Judges Solidity files with the compiler's own parser: reads paths on stdin
and prints, per file, the comment and literal ranges in byte offsets after a
byte order mark and whether the parser rejected the file, as judgecmp's
stream expects.

solc parses the files in batches, stopping after parsing so that imports
need not resolve; as it then returns no tree when any file of a batch fails,
a failing batch is split until each file that fails stands alone. Its
abstract syntax tree gives each string, hex and unicode literal, in Solidity
and in inline assembly, with its byte range. Comments are the `//` and
`/* */` that open outside those literals, which do not nest (Solidity
documentation, Layout of a Solidity Source File, Comments). Version pragmas
are blanked out before parsing, as solc rejects a file that asks for another
compiler version, which says nothing of its tokens. SOLC names the
command that runs solc, by default the `ethereum/solc:0.8.30` image without
network.

    python3 -I solidity/judge.py
"""

import json
import os
import re
import shlex
import subprocess
import sys

SOLC = shlex.split(os.environ.get("SOLC", "docker run --rm -i --network none ethereum/solc:0.8.30")) + [
    "--standard-json"
]
BATCH = 400
VERSION_PRAGMA = re.compile(r"pragma\s+solidity\b[^;]*;")


def literal_ranges(node, ranges):
    if isinstance(node, dict):
        kind = node.get("nodeType")
        if (kind == "Literal" and node.get("kind") in ("string", "hexString", "unicodeString")) or (
            kind == "YulLiteral" and node.get("kind") == "string"
        ):
            start, length, _ = map(int, node["src"].split(":"))
            ranges.append([start, start + length])
        for value in node.values():
            literal_ranges(value, ranges)
    elif isinstance(node, list):
        for value in node:
            literal_ranges(value, ranges)


def comment_ranges(data, literals):
    comments, i, n = [], 0, len(data)
    literals = iter(sorted(literals) + [[n + 1, n + 1]])
    literal = next(literals)
    while i < n:
        if i >= literal[0]:
            i = max(i, literal[1])
            literal = next(literals)
            continue
        if data.startswith(b"//", i):
            end = data.find(b"\n", i)
            end = n if end < 0 else end
            comments.append([i, end - (data[end - 1 : end] == b"\r")])
            i = end
        elif data.startswith(b"/*", i):
            end = data.find(b"*/", i + 2)
            end = n if end < 0 else end + 2
            comments.append([i, end])
            i = end
        else:
            i += 1
    return comments


def without_version(text):
    """`text` with each version pragma replaced by blanks, keeping offsets."""
    return VERSION_PRAGMA.sub(lambda match: re.sub(r"[^\n]", " ", match.group()), text)


def parse(sources):
    """solc's output for `sources`, a mapping of paths to texts."""
    request = {
        "language": "Solidity",
        "sources": {path: {"content": without_version(text)} for path, text in sources.items()},
        "settings": {"stopAfter": "parsing", "outputSelection": {"*": {"": ["ast"]}}},
    }
    return json.loads(subprocess.run(SOLC, input=json.dumps(request).encode(), capture_output=True, check=True).stdout)


def fails(output):
    return any(error.get("severity") == "error" for error in output.get("errors", []))


def report(sources, output):
    for path, text in sources.items():
        data = text.encode("utf-8")
        literals = []
        literal_ranges(output.get("sources", {}).get(path, {}).get("ast"), literals)
        result = {
            "path": path,
            "units": "bytes",
            "errors": int(fails(output)),
            "comments": comment_ranges(data, literals),
            "literals": sorted(literals),
        }
        print(json.dumps(result), flush=True)


def judge_sources(sources):
    output = parse(sources)
    if not fails(output) or len(sources) == 1:
        report(sources, output)
        return
    items = list(sources.items())
    judge_sources(dict(items[: len(items) // 2]))
    judge_sources(dict(items[len(items) // 2 :]))


def judge(paths):
    sources = {}
    for path in paths:
        with open(path, "rb") as handle:
            data = handle.read()
        if data.startswith(b"\xef\xbb\xbf"):
            data = data[3:]
        try:
            sources[path] = data.decode("utf-8")
        except UnicodeDecodeError:
            pass
    judge_sources(sources)


paths = sys.stdin.read().splitlines()
for start in range(0, len(paths), BATCH):
    judge(paths[start : start + BATCH])
