"""Judges batch files by cmd.exe's reading rules.

cmd.exe has no tokenizer to call, and its source is closed, so this follows
Microsoft's documentation of the commands and, where it is silent, Wine's
reimplementation in `programs/cmd/wcmdmain.c`, citing it by function:

- Before a command, `lexer_strip_for_command_start` skips blanks and `@`,
  which keeps the command from being echoed (Microsoft Learn, echo).
- A line whose command starts with `:` is a label, and nothing on it runs
  (Microsoft Learn, goto); a `::` label is the usual remark.
- `rem`, in any case (Microsoft Learn, rem), followed by a blank or the end of
  the line makes the whole line a remark (`WCMD_ReadAndParseLine`). Followed
  by one of `+./(;=:`, it is still the `rem` command (`search_command`), but
  the rest of the line is read, and an `&` or `|` outside quotes runs more,
  as the documented `Rem/||(` does. Followed by anything else, it is part of
  a longer command name.
- A caret escapes the next character outside quotes; one before the newline
  joins the next line to the command, which then cannot start a remark.
  Remarks and labels take their line whole, so a caret ends neither.

Reads paths on stdin and prints, per file, the comment ranges in byte
offsets after any byte order mark, as judgecmp's stream expects. Batch has
no lexical errors.
"""

import json
import sys

COMMAND_PUNCTUATION = b"+./(;=:"


def strip_command_start(line):
    """The offset of the command after leading blanks and `@`."""
    i = 0
    while i < len(line) and line[i] in b" \t@":
        i += 1
    return i


def scan(line, start):
    """Whether `line` from `start` holds an `&` or `|` outside quotes and
    carets, and whether it ends in a caret that joins the next line."""
    quoted, i, operator = False, start, False
    while i < len(line):
        byte = line[i]
        if byte == ord('"'):
            quoted = not quoted
        elif not quoted and byte == ord("^"):
            if i + 1 == len(line):
                return operator, True
            i += 1
        elif not quoted and byte in b"&|":
            operator = True
        i += 1
    return operator, False


def is_remark(line):
    """Whether the physical `line`, starting a command, runs nothing."""
    i = strip_command_start(line)
    rest = line[i:]
    if rest.startswith(b"::"):
        return True
    if rest[:3].lower() != b"rem":
        return False
    after = rest[3:4]
    if after in (b"", b" ", b"\t"):
        return True
    if after in [bytes([b]) for b in COMMAND_PUNCTUATION]:
        return not scan(line, i + 4)[0]
    return False


def judge(path):
    with open(path, "rb") as f:
        data = f.read().removeprefix(b"\xef\xbb\xbf")
    comments, offset, continued = [], 0, False
    for raw in data.split(b"\n"):
        line = raw[:-1] if raw.endswith(b"\r") else raw
        if continued:
            continued = scan(line, 0)[1]
        elif is_remark(line):
            comments.append([offset, offset + len(line)])
        elif not line[strip_command_start(line):].startswith(b":"):
            continued = scan(line, 0)[1]
        offset += len(raw) + 1
    return {"path": path, "units": "bytes", "errors": 0, "comments": comments, "literals": []}


for entry in sys.stdin:
    print(json.dumps(judge(entry.rstrip("\n"))))
