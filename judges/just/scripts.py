"""Hands the bodies of just's script recipes to their interpreter's judge.

just writes a script recipe's body to a file and runs it with the command
a `[script]` attribute or the `script-interpreter` setting names, `sh -eu`
by default, or else with the interpreter its `#!` line names
(`Recipe::run_script` in just 1.59.0's `src/recipe.rs`), after
evaluating its `{{…}}` interpolations and taking the recipe's indentation off
each line. A view holds that script, with each interpolation's characters but
newlines replaced by `x`, so its offsets map back to the justfile's.

Two passes around the Shell and Python judges: `views DIR` reads the just
judge's output on stdin, writes the shell and Python views to DIR and prints
each file's judgement with its views; a script for another interpreter makes
the file invalid. `merge FILES SHELL PYTHON` joins those lines with the
judges' output for the views and prints, per file, the comment and literal
ranges in byte offsets, as judgecmp's stream expects.
"""

import bisect
import json
import os
import sys

SHELLS = {"sh", "bash", "dash", "zsh", "ksh"}
PYTHONS = {"python", "python3"}


def interpreter(line):
    """The program a `#!` line, or the just judge's `#!` form of a script
    command, runs."""
    words = line[2:].split()
    # `env` and `pkexec` run the program that follows them.
    if words and os.path.basename(words[0]) in ("env", "pkexec"):
        words = [w for w in words[1:] if not w.startswith("-")]
    return os.path.basename(words[0]) if words else ""


def script_view(data, script):
    """The script just runs, and the justfile offset of each of its lines."""
    body = bytearray(data[script["start"]:script["end"]])
    for start, end in script["interpolations"]:
        span = slice(start - script["start"], end - script["start"])
        body[span] = bytes(b if b == 0x0A else 0x78 for b in body[span])
    line_start = data.rfind(b"\n", 0, script["start"]) + 1
    indentation = data[line_start:script["start"]]
    view, starts, offset = bytearray(), [], line_start
    for line in (indentation + bytes(body)).split(b"\n"):
        removed = len(indentation) if line.startswith(indentation) else len(line) - len(line.lstrip(b" \t"))
        starts.append((len(view), offset + removed))
        view += line[removed:] + b"\n"
        offset += len(line) + 1
    return bytes(view[:-1]), starts


def views(directory):
    os.makedirs(directory, exist_ok=True)
    for number, line in enumerate(sys.stdin):
        judgement = json.loads(line)
        with open(judgement["path"], "rb") as f:
            data = f.read().removeprefix(b"\xef\xbb\xbf")
        judgement["views"] = []
        for k, script in enumerate(judgement.pop("scripts")):
            program = interpreter(script["interpreter"])
            extension = "sh" if program in SHELLS else "py" if program in PYTHONS else None
            if extension is None:
                judgement["errors"] += 1
                continue
            view, starts = script_view(data, script)
            path = os.path.join(directory, f"{number}-{k}.{extension}")
            with open(path, "wb") as f:
                f.write(view)
            judgement["views"].append({"path": path, "starts": starts})
        print(json.dumps(judgement))


def merge(files, *judged):
    by_view = {}
    for name in judged:
        with open(name) as f:
            for line in f:
                result = json.loads(line)
                by_view[result["path"]] = result
    with open(files) as f:
        for line in f:
            judgement = json.loads(line)
            for view in judgement.pop("views"):
                result = by_view.get(view["path"])
                if result is None:
                    judgement["errors"] += 1
                    continue
                judgement["errors"] += result["errors"]
                view_starts = [s for s, _ in view["starts"]]

                def original(offset):
                    i = bisect.bisect_right(view_starts, offset) - 1
                    return view["starts"][i][1] + offset - view_starts[i]

                for key in ("comments", "literals"):
                    judgement[key] += [[original(a), original(b)] for a, b in result[key]]
            judgement["comments"].sort()
            judgement["literals"].sort()
            print(json.dumps(judgement))


if sys.argv[1] == "views":
    views(sys.argv[2])
else:
    merge(*sys.argv[2:])
