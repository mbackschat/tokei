"""Judges Makefiles by GNU make 4.4.1's reading rules.

GNU make has no tokenizer to call, so this ports what `src/read.c` does with
each line, citing it by function, and reads recipes as the shell does
(POSIX.1-2024, Shell Command Language, 2.3 Token Recognition):

- `readline` joins a line ending in an odd number of backslashes to the next;
  a backslash followed by a space does not continue it.
- Outside recipes, `remove_comments` cuts the joined line at the first `#`
  not quoted by an odd number of backslashes and not inside a `$(...)` or
  `${...}` reference, so a comment continues over the lines it joins.
- `eval` reads a line starting with the recipe prefix, a tab unless
  `.RECIPEPREFIX` says otherwise, as a recipe line once a rule line has been
  read, until a variable assignment, `export`, `unexport`, `vpath`,
  `include` or `load` line or another rule line. Comments and blank lines do
  not end a rule, nor do conditional directives. A line of references
  without a colon, such as `$(eval ...)`, leaves the rule context as it was,
  as it may expand to a rule or sit in a branch make ignores.
- A recipe goes to the shell, where `#` starting a word comments out the rest
  of its line, and only that line; quotes are literals. The text after a
  rule line's `;` that precedes any `#` is a recipe too.
- A recipe line's leading `@`, `-` and `+` are make's, not the shell's
  (`start_job_command` in `src/job.c`); before a comment, they are part of
  it, as the line runs nothing.
- `do_define` keeps the lines of a `define` body unread, to be read later as
  makefile text by `$(eval)` or as a recipe. A body line is judged a comment
  where both would read it as one: when it starts with `#`, after blanks and,
  on a line starting with the recipe prefix, recipe flags.

Conditional directives are not evaluated: every branch is read. In an
Automake `.am` file, `if COND` at the start of a line opens Automake's own
conditional, which `automake` turns into make text (`$IF_PATTERN` in
automake 1.18's `bin/automake.in`), and a line holding only an `@NAME@`
substitution is text `configure` replaces before make reads it; the rest
reads as make reads it. Reads paths
on stdin and prints, per file, the comment and literal ranges in byte offsets
and the number of errors, as judgecmp's stream expects: the fatal errors
`eval` reports for an unterminated `define`, a recipe line before the first
rule and a line that is neither a rule nor anything else and has no
reference that could expand to one ("missing separator"), as in nmake files;
and an unterminated shell quote.
"""

import json
import re
import sys

ASSIGNMENT_WORDS = {b"export", b"unexport", b"override", b"private", b"define", b"undefine"}
RULE_ENDING_WORDS = {b"export", b"unexport", b"vpath", b"include", b"-include", b"sinclude", b"load", b"-load"}
SUBSTITUTION = re.compile(rb"@[A-Za-z_][A-Za-z0-9_]*@")
CONDITIONAL_WORDS = {b"ifeq", b"ifneq", b"ifdef", b"ifndef", b"else", b"endif"}
SHELL_OPERATORS = b";&|()<>"


def backslashes_before(text, i, start):
    """The length of the run of backslashes that ends before `text[i]` and
    begins at `start` or later."""
    run = 0
    while i - run - 1 >= start and text[i - run - 1] == 0x5C:
        run += 1
    return run


def logical_lines(text):
    """`readline`: the logical lines as (start, end) byte ranges, each end
    before its final newline."""
    start, size = 0, len(text)
    while start < size:
        end = start
        while True:
            newline = text.find(b"\n", end)
            if newline < 0:
                end = size
                break
            body_end = newline - 1 if newline > start and text[newline - 1] == 0x0D else newline
            if backslashes_before(text, body_end, start) % 2 == 0:
                end = newline
                break
            end = newline + 1
        yield start, end
        start = end + 1


def skip_reference(text, i, end):
    """The end of the `$(...)` or `${...}` reference whose `$` is at `i`, as
    `find_map_unquote` skips it, or `i + 2` for any other `$` sequence."""
    if i + 1 >= end:
        return end
    opener = text[i + 1]
    j = i + 2
    if opener in b"({":
        closer = ord(")") if opener == ord("(") else ord("}")
        depth = 1
        while j < end:
            if text[j] == opener:
                depth += 1
            elif text[j] == closer:
                depth -= 1
                if depth == 0:
                    return j + 1
            j += 1
    return j


def find_unquoted(text, start, end, stops):
    """`find_map_unquote` with MAP_VARIABLE: the first byte of `stops` from
    `start` that no odd run of backslashes quotes, outside references."""
    i = start
    while i < end:
        byte = text[i]
        if byte == 0x24:
            i = skip_reference(text, i, end)
            continue
        if byte in stops and backslashes_before(text, i, start) % 2 == 0:
            return i
        i += 1
    return None


def collapsed_code(text, start, end):
    """The code before a makefile line's comment, with continuations joined
    as `collapse_continuations` joins them, and where the comment starts."""
    cut = find_unquoted(text, start, end, b"#")
    code = text[start:end if cut is None else cut]
    return code.replace(b"\\\r\n", b" ").replace(b"\\\n", b" "), cut


def is_assignment(code):
    """`parse_var_assignment`: whether `code` assigns a variable, after any
    modifier words, and those words."""
    words = code.split()
    modifiers = []
    while words and words[0] in ASSIGNMENT_WORDS:
        modifiers.append(words.pop(0))
    if b"define" in modifiers or b"undefine" in modifiers:
        return True, modifiers
    rest = b" ".join(words)
    i = 0
    while i < len(rest):
        byte = rest[i]
        if byte == 0x24:
            i = skip_reference(rest, i, len(rest))
            continue
        if byte == ord("="):
            return True, modifiers
        if byte == ord(":"):
            tail = rest[i:]
            return tail.startswith((b":=", b"::=", b":::=")), modifiers
        if byte in b"+?!" and rest[i + 1:i + 2] == b"=":
            return True, modifiers
        if byte in b" \t":
            after = rest[i:].lstrip()
            return after.startswith((b"=", b":=", b"::=", b":::=", b"+=", b"?=", b"!=")), modifiers
        i += 1
    return False, modifiers


class Judge:
    def __init__(self, text, automake=False):
        self.text = text
        self.automake = automake
        self.comments = []
        self.literals = []
        self.errors = 0
        self.prefix = 0x09
        self.in_rule = False

    def comment_from(self, cut, end):
        """The comment from `cut` to `end`, if a comment was cut."""
        if cut is not None:
            self.comments.append([cut, end])

    def shell(self, start, end):
        """A recipe from `start` to `end` read by the shell, with make's
        backslash-newlines and the recipe prefix that starts each continued
        line, which make removes (GNU make manual, 5.1.1 Splitting Recipe
        Lines). Make expands its references first; `$$` is the shell's `$`,
        whose `$(...)` and backquotes nest commands, in double quotes too."""
        text = self.text
        flags = start
        while start < end and text[start] in b" \t@-+":
            start += 1
        # Each frame is [kind, paren depth or literal start]: the command
        # context, or the quote, substitution or backquote it is in.
        stack = [["cmd", 0]]
        i, word_start = start, True
        if start < end and text[start] == ord("#"):
            newline = text.find(b"\n", start, end)
            i = end if newline < 0 else newline
            self.comments.append([flags, i])
        while i < end:
            byte = text[i]
            kind = stack[-1][0]
            if kind == "'":
                if byte == ord("'"):
                    self.literals.append([stack.pop()[1], i + 1])
                i += 1
                continue
            if byte == 0x5C and i + 1 < end:
                i += 2
                if text[i - 1] != 0x0A:
                    word_start = False
                elif i < end and text[i] == self.prefix:
                    i += 1
                continue
            if byte == 0x24:
                if text[i + 1:i + 3] == b"$(":
                    if kind == '"':
                        self.literals.append([stack[-1][1], i])
                    stack.append(["sub", 0])
                    i, word_start = i + 3, True
                    continue
                i = skip_reference(text, i, end)
                word_start = False
                continue
            if kind == '"':
                if byte == ord('"'):
                    self.literals.append([stack.pop()[1], i + 1])
                elif byte == ord("`"):
                    self.literals.append([stack[-1][1], i])
                    stack.append(["`", 0])
                i += 1
                continue
            if byte in b"'\"":
                stack.append([chr(byte), i])
            elif byte == ord("`"):
                if kind == "`":
                    stack.pop()
                    if stack[-1][0] == '"':
                        stack[-1][1] = i + 1
                else:
                    stack.append(["`", 0])
            elif byte == ord("#") and word_start:
                newline = text.find(b"\n", i, end)
                comment_end = end if newline < 0 else newline
                self.comments.append([i, comment_end])
                i = comment_end
                continue
            elif kind == "sub" and byte == ord("("):
                stack[-1][1] += 1
            elif kind == "sub" and byte == ord(")"):
                if stack[-1][1] == 0:
                    stack.pop()
                    if stack[-1][0] == '"':
                        stack[-1][1] = i + 1
                else:
                    stack[-1][1] -= 1
            word_start = byte in b" \t\n" or byte in SHELL_OPERATORS
            i += 1
        if len(stack) > 1:
            if stack[-1][0] in "'\"":
                self.literals.append([stack[-1][1], end])
            self.errors += 1

    def makefile_line(self, start, end):
        """A makefile line: its comment, and the rule context it leaves.
        Returns whether it opens a `define` body."""
        code, cut = collapsed_code(self.text, start, end)
        words = code.split()
        if not words:
            self.comment_from(cut, end)
            return False
        assignment, modifiers = is_assignment(code)
        if assignment:
            self.in_rule = False
            self.comment_from(cut, end)
            name_and_value = code.split(b"=", 1)
            if name_and_value[0].strip().rstrip(b":+?!").strip() == b".RECIPEPREFIX" and len(name_and_value) > 1:
                value = name_and_value[1].lstrip()
                self.prefix = value[0] if value else 0x09
            return b"define" in modifiers
        if words[0] in CONDITIONAL_WORDS or self.automake and words[0] == b"if" and self.text[start:start + 2] == b"if":
            self.comment_from(cut, end)
            return False
        if words[0] in RULE_ENDING_WORDS:
            self.in_rule = False
            self.comment_from(cut, end)
            return False
        colon = find_unquoted(self.text, start, end, b":") is not None
        if self.automake and len(words) == 1 and SUBSTITUTION.fullmatch(words[0]):
            return False
        if self.text[start] == self.prefix:
            self.errors += 1
        elif not colon and b"$" not in code:
            self.errors += 1
        # A rule line: `eval` looks for a `;` before any `#`.
        semicolon = find_unquoted(self.text, start, end, b";#")
        if semicolon is not None and self.text[semicolon] == ord(";"):
            self.shell(semicolon + 1, end)
        else:
            self.comment_from(cut, end)
        # A line whose references may expand to a rule, or that sits in a
        # branch make ignores, such as `$(error ...)` under `ifndef`, leaves
        # the rule context as it was: no valid makefile tells the two apart.
        if colon:
            self.in_rule = True
        elif b"$" not in code:
            self.in_rule = False
        return False

    def body_comments(self, start, end):
        """The comment lines of a `define` body's logical line."""
        text = self.text
        while start < end:
            newline = text.find(b"\n", start, end)
            line_end = end if newline < 0 else newline
            i = start
            while i < line_end and text[i] in b" \t":
                i += 1
            lead = i
            if text[start] == self.prefix:
                while i < line_end and text[i] in b" \t@-+":
                    i += 1
            if i < line_end and text[i] == ord("#"):
                self.comments.append([lead, line_end])
            start = line_end + 1

    def run(self):
        text = self.text
        depth = 0
        for start, end in logical_lines(text):
            if start == end:
                continue
            if depth:
                if text[start] != self.prefix:
                    words = text[start:end].split(None, 1)
                    word = words[0] if words else b""
                    if word == b"define":
                        depth += 1
                    elif word == b"endef":
                        depth -= 1
                        if depth == 0:
                            cut = find_unquoted(text, start, end, b"#")
                            self.comment_from(cut, end)
                            continue
                self.body_comments(start, end)
                continue
            if text[start] == self.prefix and self.in_rule:
                self.shell(start + 1, end)
                continue
            if self.makefile_line(start, end):
                depth = 1
        if depth:
            self.errors += 1


def judge(path):
    with open(path, "rb") as f:
        data = f.read()
    if data.startswith(b"\xef\xbb\xbf"):
        data = data[3:]
    try:
        data.decode("utf-8")
    except UnicodeDecodeError:
        return None
    judged = Judge(data, automake=path.endswith(".am"))
    judged.run()
    return {"path": path, "units": "bytes", "errors": judged.errors, "comments": judged.comments, "literals": judged.literals}


for line in sys.stdin:
    judged = judge(line.rstrip("\n"))
    if judged is not None:
        print(json.dumps(judged))
