"""Judges COBOL files with GnuCOBOL's preprocessor: reads paths on stdin and
prints, per file, the comment lines in byte offsets after a byte order mark
and whether cobc reported an error, as judgecmp's stream expects.

`cobc -E` (GnuCOBOL 3.2) applies the reference format and writes one line
for each source line, `#line` markers around copied text: a line it leaves
empty held no program text, as a comment line, a `*>` floating comment, a
debugging line outside debugging mode, a comment entry after `AUTHOR.` and
its kind, or text only in the sequence or identification areas; a line it
keeps, even as a single space where a `COPY` or `REPLACE` statement was,
held code; a directive it passes on, such as `#OPTION` for a `$SET`, is a
line of its own, not counted. Two kinds of line it leaves empty are code
all the same: a continuation line, whose text it joins to the line before,
and a compiler directive, such as `>>SOURCE`, `$SET` or an IBM `CBL` or
`PROCESS` line of compiler options. A line is a comment when it is not
blank and held no code.

GnuCOBOL 3.2 reads fixed form unless told otherwise; the judge starts each
file in the form GnuCOBOL's development branch chooses by default, free
when the first line that reaches column 7 has no fixed-form indicator there
(`ppopen_get_file` and `is_fixed_indicator` in `cobc/pplex.l` at gnucobol
`eda8905e44`), and also free when a `*>` starts in columns 1 to 6, which
fixed form leaves to the sequence number but only files built with
`cobc -free` were found to hold. A `>>SOURCE FORMAT` directive or a
`$SET SOURCEFORMAT` one switches the form from the next line on, as cobc
does.

Before cobc reads the file, conditional compilation directives are blanked
out, so that every branch is read: the text of an inactive branch is code
written in the file, as the other judges count an inactive `#if` branch.
The words `COPY` and `REPLACE` are changed to words of the same length that
cobc does not know, as a line's class does not depend on a copybook's text
or a replacement, while cobc prints nothing for the later lines of a
statement it consumes, such as the operands of a `REPLACING` phrase.

    python3 -I cobol/judge.py
"""

import json
import os
import re
import subprocess
import sys
import tempfile

TAB_WIDTH = 8
FIXED_INDICATORS = set(" -dD*/\\$")
MARKER = re.compile(r'#line (\d+) "(.*)"$')
CONDITIONAL = re.compile(r"(>>\s*(IF|ELIF|ELSE|END-IF|EVALUATE|WHEN|END-EVALUATE)|\$\s*(IF|ELIF|ELSE|END))\b", re.I)
SOURCE_FORMAT = re.compile(
    r"(?:>>\s*SOURCE\s+(?:FORMAT\s+)?(?:IS\s+)?|\$\s*SET\b.*\bSOURCE-?FORMAT\W*)(FREE|FIXED)\b", re.I
)
OPTIONS_LINE = re.compile(r"(CBL|PROCESS)([ ,;]|$)", re.I)
PREPROCESSOR_STATEMENT = re.compile(r"\b(COPY|REPLACE)\b", re.I)
EARLY_FLOATING_COMMENT = re.compile(r"^ {0,5}\*>")
DEBUGGING_LINE = re.compile(r">>D(?!EFINE\b|ISPLAY\b)", re.I)


def expand_tabs(line):
    return line.expandtabs(TAB_WIDTH)


def starts_free(lines):
    if any(EARLY_FLOATING_COMMENT.match(expand_tabs(line)) for line in lines):
        return True
    for line in lines:
        line = expand_tabs(line)
        if len(line) >= 7:
            return line[6] not in FIXED_INDICATORS
    return False


def directive(line, free):
    """The text of the compiler directive `line` holds, or `None`; a `>>D`
    that starts no longer directive starts a debugging line instead (pplex's
    `">>D"` rule)."""
    text = expand_tabs(line)
    if free:
        text = text.lstrip()
    elif text[6:7] == "$":
        return text[6:]
    else:
        text = text[6:].lstrip()
    if text.startswith("$") and text[1:2] not in ("", " "):
        return text
    if DEBUGGING_LINE.match(text):
        return None
    return text if text.startswith(">>") or OPTIONS_LINE.match(text) else None


def code_lines(output, path):
    """The numbers of the lines of `path` for which cobc kept text."""
    kept, current = set(), None
    for out in output.split("\n"):
        marker = MARKER.match(out)
        if marker:
            current = int(marker[1]) if marker[2] == path else None
            continue
        if current is not None and out.startswith("#"):
            continue
        if current is not None:
            if out:
                kept.add(current)
            current += 1
    return kept


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
    forms, free = [], starts_free(lines)
    for line in lines:
        forms.append(free)
        command = directive(line, free)
        if command and SOURCE_FORMAT.match(command):
            free = SOURCE_FORMAT.match(command)[1].upper() == "FREE"
    unconditional = []
    for line, free in zip(lines, forms):
        command = directive(line, free)
        if command and CONDITIONAL.match(command):
            line = ""
        unconditional.append(PREPROCESSOR_STATEMENT.sub(lambda m: "X" + m[1][1:], line))
    with tempfile.TemporaryDirectory() as scratch:
        # A fixed name, as cobc rejects a long base name or one with spaces.
        copy = os.path.join(scratch, "source.cob")
        with open(copy, "w", encoding="utf-8") as handle:
            handle.write("\n".join(unconditional))
        form = "-free" if forms[0] else "-fixed"
        command = ["cobc", "-E", form, copy]
        run = subprocess.run(command, capture_output=True, text=True, errors="replace")
        kept = code_lines(run.stdout, copy)
    comments, offset = [], 0
    for number, (line, free) in enumerate(zip(lines, forms), 1):
        body = line.rstrip("\r")
        if body.strip() and number not in kept and not directive(body, free):
            continuation = not free and expand_tabs(body)[6:7] == "-"
            if not continuation:
                comments.append([offset, offset + len(body.encode("utf-8"))])
        offset += len(line.encode("utf-8")) + 1
    result = {"path": path, "units": "bytes", "errors": int(run.returncode != 0), "comments": comments, "literals": []}
    print(json.dumps(result), flush=True)


for line in sys.stdin:
    path = line.rstrip("\n")
    if path:
        judge(path)
