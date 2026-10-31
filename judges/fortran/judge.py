"""Judges Fortran files with fparser's reader, the one fparser's Fortran 2008
parser and PSyclone read sources with: reads paths on stdin and prints, per
file, the comment ranges in byte offsets after a byte order mark and whether
the reader rejected the file, as judgecmp's stream expects.

No Fortran compiler exposes its tokens, so fparser is the named reference.
It reports each comment, whole-line or after code, as an item spanning its
line, whose text ends that line, with tabs expanded. Its source form is set
here rather than by fparser, which reads it from content alone: a `.f90`
file is free form, as compilers read it, and any other is free form when
fparser's rule finds a free-form line among those that are neither
preprocessor directives nor `*` comment lines. fparser counts a `#if` line,
gone before the compiler reads the source, as free form, and a `*` comment
line too when it ends with `&`, as LAPACK's do. A string cannot hold a blank line, even when continued,
so the literals need no ranges for judgecmp to class every line.

    uv run --with fparser==0.2.5 python fortran/judge.py
"""

import json
import os
import sys

from fparser.common.readfortran import Comment, FortranFileReader
from fparser.common.sourceinfo import _FREE_FORMAT_START, FortranFormat

FREE_EXTENSIONS = {".f03", ".f08", ".f90", ".f95"}


def is_free(path, lines):
    """fparser's `get_source_info_str` rule over non-directive lines."""
    if os.path.splitext(path)[1].lower() in FREE_EXTENSIONS:
        return True
    tally = 10000
    for line in lines:
        line = line.rstrip()
        if not line or line[0] in "!#*":
            continue
        if line[0] != "\t" and _FREE_FORMAT_START(line[:5]) or line[-1:] == "&":
            return True
        tally -= 1
        if tally == 0:
            break
    return False


def raw_index(line, column):
    """Index in `line` of the character at `column` once tabs are expanded."""
    width = 0
    for index, char in enumerate(line):
        if width >= column:
            return index
        width = (width // 8 + 1) * 8 if char == "\t" else width + 1
    return len(line)


def judge(path):
    with open(path, "rb") as handle:
        data = handle.read()
    if data.startswith(b"\xef\xbb\xbf"):
        data = data[3:]
    try:
        text = data.decode("utf-8")
    except UnicodeDecodeError:
        return
    lines = text.split("\n")
    starts = [0]
    for line in lines:
        starts.append(starts[-1] + len(line.encode("utf-8")) + 1)
    comments = []
    errors = 0
    try:
        reader = FortranFileReader(path, ignore_comments=False)
        reader.set_format(FortranFormat(is_free(path, lines), False))
        for item in reader:
            if not isinstance(item, Comment) or not item.comment.strip():
                continue
            first, last = item.span
            for number in range(first, last + 1):
                line = lines[number - 1].rstrip()
                text_of = item.comment.split("\n")[number - first].strip()
                at = line.expandtabs().replace("\xa0", " ").rfind(text_of)
                if at < 0:
                    continue
                start = starts[number - 1] + len(line[: raw_index(line, at)].encode("utf-8"))
                comments.append([start, starts[number - 1] + len(line.encode("utf-8"))])
    # A source the reader cannot read is invalid, whatever it raises; on some,
    # such as a makefile named `.f`, it calls `sys.exit`.
    except (Exception, SystemExit):
        errors = 1
    print(
        json.dumps(
            {"path": path, "units": "bytes", "errors": errors, "comments": comments, "literals": []}
        ),
        flush=True,
    )


for path in sys.stdin.read().splitlines():
    judge(path)
