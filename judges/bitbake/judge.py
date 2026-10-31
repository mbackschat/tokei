"""Judges BitBake recipes, classes, appends and includes by BitBake's reading
rules.

BitBake has no tokenizer to call, so this ports how BitBake 2.16's
`lib/bb/parse/parse_py/BBHandler.py` and `ConfHandler.py` read a file, which
`lib/bb/parse/ast.py` also applies to `.bbappend` files. Each line is read
with its trailing whitespace stripped. A line whose first character is `#`
is a comment, and a trailing backslash continues it to the next line, which
must be a comment too. Elsewhere a trailing backslash joins the next line
to the statement. A line that ends in `() {` opens a function whose body
runs to a line that is exactly `}`: a Python function when `python` comes
first or it has no name, as `ast.py` runs it, and a shell function
otherwise. A line that starts with `def` opens a Python function whose body
runs while lines are indented, empty or start with `#`. An assignment's
quoted value must end its statement, and is a literal; any other statement
BitBake does not know, an indented comment line among them, makes the file
invalid. An appending function such as `do_task:append() {` runs in the
interpreter of the function it appends to, which another file may define,
so its body is read as shell; a Python one makes the file invalid.

The shell judge reads shell bodies and the Python judge Python bodies,
through views that keep each body byte at its offset and blank every other
byte but newlines. BitBake expands `${@…}` and `${VAR}` before the shell runs
a function (`lib/bb/data_smart.py`), so the shell view replaces each such
expansion whose name the shell would reject by a shell variable of the same
length.

Two passes around those judges: `views DIR` reads paths on stdin, writes the
views to DIR and prints, per file, the comments and literals it found and its
views; `merge FILES SHELL PYTHON` joins those lines with the judges' output
for the views and prints, per file, the comment and literal ranges in byte
offsets, as judgecmp's stream expects.
"""

import json
import os
import re
import sys

# BBHandler.py's and ConfHandler.py's statement patterns, verbatim.
FUNC_START = re.compile(r"(((?P<py>python(?=(\s|\()))|(?P<fr>fakeroot(?=\s)))\s*)*(?P<func>[\w\.\-\+\{\}\$:]+)?\s*\(\s*\)\s*{$")
DEF = re.compile(r"def\s+(\w+).*:")
PYTHON_BODY = re.compile(r"(\s+.*)|(^$)|(^#)")
CONFIG = re.compile(r"""
    ^
    (?P<exp>export\s+)?
    (?P<var>[a-zA-Z0-9\-_+.${}/~:]*?)
    (\[(?P<flag>[a-zA-Z0-9\-_+.][a-zA-Z0-9\-_+.@/]*)\])?

    (?P<whitespace>\s*) (
        (?P<colon>:=) |
        (?P<lazyques>\?\?=) |
        (?P<ques>\?=) |
        (?P<append>\+=) |
        (?P<prepend>=\+) |
        (?P<predot>=\.) |
        (?P<postdot>\.=) |
        =
    ) (?P<whitespace2>\s*)

    (?!'[^']*'[^']*'$)
    (?!\"[^\"]*\"[^\"]*\"$)
    (?P<apo>['\"])
    (?P<value>.*)
    (?P=apo)
    $
    """, re.X)
OTHER_STATEMENTS = [re.compile(p) for p in (
    r"EXPORT_FUNCTIONS\s+(.+)", r"addtask\s+([^#\n]+)(?P<comment>#.*|.*?)",
    r"deltask\s+([^#\n]+)(?P<comment>#.*|.*?)", r"addhandler\s+(.+)", r"inherit\s+(.+)",
    r"inherit_defer\s+(.+)", r"include\s+(.+)", r"require\s+(.+)", r"include_all\s+(.+)",
    r"export\s+([a-zA-Z0-9\-_+.${}/~]+)$", r"unset\s+([a-zA-Z0-9\-_+.${}/~]+)$",
    r"unset\s+([a-zA-Z0-9\-_+.${}/~]+)\[([a-zA-Z0-9\-_+.][a-zA-Z0-9\-_+.@]+)\]$",
    r"addfragments\s+(.+)\s+(.+)\s+(.+)\s+(.+)")]
TASK_COMMENT = re.compile(r"(?:addtask|deltask)\s+[^#\n]+(#.*)")
# data_smart.py's expansion patterns, verbatim.
EXPAND_VAR = re.compile(r"\${[a-zA-Z0-9\-_+./~:]+}")
EXPAND_PYTHON = re.compile(r"\${@(?:{.*?}|.)+?}")
SHELL_NAME = re.compile(r"\${[A-Za-z_][A-Za-z0-9_]*}")


def shell_expanded(text):
    """`text` with each BitBake expansion the shell would reject replaced by
    a shell variable of the same length."""
    def variable(m):
        return m[0] if SHELL_NAME.fullmatch(m[0]) else "${" + "_" * (len(m[0].encode()) - 3) + "}"
    return EXPAND_VAR.sub(variable, EXPAND_PYTHON.sub(variable, text))


def read(text):
    """The comments, literals, function bodies and error count of a file's
    text, as BBHandler reads it."""
    comments, literals, errors = [], [], 0
    shell, python = [], []  # bodies as (start, end) offsets
    lines, offset = [], 0
    for line in text.splitlines(keepends=True):
        lines.append((offset, line.rstrip()))
        offset += len(line)
    infunc, inpython, residue = None, None, []
    for i, (start, s) in enumerate(lines):
        if infunc is not None:
            if s == "}":
                (python if infunc else shell).append((body_start, start))
                infunc = None
            continue
        if inpython is not None:
            if PYTHON_BODY.match(s):
                continue
            python.append((inpython, start))
            inpython = None
        if s and s[0] == "#" and residue and residue[0][1][0] != "#":
            errors += 1
        if residue and residue[0][1][0] == "#" and (not s or s[0] != "#"):
            errors += 1
        if s.endswith("\\"):
            residue.append((start, s))
            continue
        parts, residue = residue + [(start, s)], []
        joined, at = "", []
        for part_start, part in parts:
            piece = part[:-1] if part_start != start else part
            joined += piece
            at.extend(range(part_start, part_start + len(piece)))
        if joined == "":
            continue
        if joined[0] == "#":
            comments.extend([part_start, part_start + len(part)] for part_start, part in parts)
            continue
        m = FUNC_START.match(joined)
        if m:
            # MethodNode.eval in ast.py runs a function without a name as Python.
            infunc = m.group("py") is not None or m.group("func") is None
            body_start = lines[i + 1][0] if i + 1 < len(lines) else len(text)
            continue
        if DEF.match(joined):
            inpython = parts[0][0]
            continue
        m = CONFIG.match(joined)
        if m:
            literals.append([at[m.start("apo")], at[len(joined) - 1] + 1])
            continue
        task = TASK_COMMENT.match(joined)
        if task:
            comments.append([at[task.start(1)], at[len(joined) - 1] + 1])
        if not task and not any(p.match(joined) for p in OTHER_STATEMENTS):
            errors += 1
    if inpython is not None:
        python.append((inpython, len(text)))
    errors += int(infunc is not None) + int(bool(residue))
    return comments, literals, shell, python, errors


def blank(text):
    """`text` with each character but a newline replaced by spaces, one per
    byte of its UTF-8 encoding."""
    return re.sub(r"[^\n]", lambda m: " " * len(m[0].encode()), text)


def view(text, ranges, transform=lambda t: t):
    """`text` with the bytes outside `ranges` blanked, newlines kept."""
    out, previous = [], 0
    for start, end in sorted(ranges):
        out += [blank(text[previous:start]), transform(text[start:end])]
        previous = end
    return "".join(out + [blank(text[previous:])])


def views(directory):
    os.makedirs(directory, exist_ok=True)
    for number, entry in enumerate(sys.stdin):
        path = entry.rstrip("\n")
        with open(path, "rb") as f:
            data = f.read().removeprefix(b"\xef\xbb\xbf")
        try:
            text = data.decode("utf-8")
        except UnicodeDecodeError:
            continue
        comments, literals, shell, python, errors = read(text)
        byte = [0]
        for c in text:
            byte.append(byte[-1] + len(c.encode()))
        comments = [[byte[a], byte[b]] for a, b in comments]
        literals = [[byte[a], byte[b]] for a, b in literals]
        shell_views, python_views = [], []
        if shell:
            shell_views.append(os.path.join(directory, f"{number}.sh"))
            with open(shell_views[-1], "w") as f:
                f.write(view(text, shell, shell_expanded))
        for k, body in enumerate(python):
            python_views.append(os.path.join(directory, f"{number}-{k}.py"))
            with open(python_views[-1], "w") as f:
                f.write(view(text, [body]))
        print(json.dumps({"path": path, "comments": comments, "literals": literals, "errors": errors,
                          "shell": shell_views, "python": python_views}))


def merge(files, *judged):
    by_view = {}
    for name in judged:
        with open(name) as f:
            for line in f:
                result = json.loads(line)
                by_view[result["path"]] = result
    with open(files) as f:
        for line in f:
            file = json.loads(line)
            comments, literals, errors = file["comments"], file["literals"], file["errors"]
            for name in file["shell"] + file["python"]:
                result = by_view.get(name)
                errors += 1 if result is None else result["errors"]
                comments += result["comments"] if result else []
                literals += result["literals"] if result else []
            print(json.dumps({"path": file["path"], "units": "bytes", "errors": errors,
                              "comments": sorted(comments), "literals": sorted(literals)}))


if sys.argv[1] == "views":
    views(sys.argv[2])
else:
    merge(*sys.argv[2:])
