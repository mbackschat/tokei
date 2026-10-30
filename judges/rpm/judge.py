"""Judges RPM spec files by rpmbuild's reading rules.

rpm has no tokenizer to call, so this ports how rpm 6's `build/parseSpec.cc`
reads a spec. A section starts at a line whose start matches a name of
`partList`, case-insensitively, followed by a blank or the line's end. The
preamble, `%package`, `%description`, `%files`, `%changelog` and the list
sections are read with `STRIP_COMMENTS`: `handleComments` drops each line
whose first non-blank character is `#`, line by line after macro expansion,
so a trailing backslash does not continue a comment. The build scripts and
scriptlets are read with `STRIP_NOTHING` and run by the shell, so the shell
judge reads them, with the `%if`, `%else`, `%endif` and `%include` lines of
`lineTypes`, which rpm consumes, left out. A scriptlet run by another
interpreter, as `-p <lua>` asks, is not judged and makes the file invalid.

Two passes around the shell judge: `views DIR` reads spec paths on stdin,
writes to DIR one shell view per spec, which keeps each script byte at its
offset and blanks every other byte but newlines, and prints the comments it
found per spec; `merge SPECS VIEWS` joins those lines with the shell judge's
output for the views and prints, per spec, the comment and literal ranges in
byte offsets after any byte order mark, as judgecmp's stream expects.
"""

import json
import os
import re
import sys

PARTS = ["%package", "%prep", "%generate_buildrequires", "%conf", "%build", "%install", "%check",
         "%clean", "%preun", "%postun", "%pretrans", "%posttrans", "%preuntrans", "%postuntrans",
         "%pre", "%post", "%files", "%changelog", "%description", "%triggerpostun", "%triggerprein",
         "%triggerun", "%triggerin", "%trigger", "%verifyscript", "%sepolicy", "%filetriggerin",
         "%filetrigger", "%filetriggerun", "%filetriggerpostun", "%transfiletriggerin",
         "%transfiletrigger", "%transfiletriggerun", "%transfiletriggerpostun", "%end",
         "%patchlist", "%sourcelist"]
NOT_SCRIPTS = {"%package", "%files", "%changelog", "%description", "%sepolicy", "%end",
               "%patchlist", "%sourcelist"}
SHELLS = {"/bin/sh", "/bin/bash", "/usr/bin/sh", "/usr/bin/bash"}
# `ISMACRO` for the directives without arguments, `ISMACROWITHARG` for the others.
DIRECTIVE = re.compile(rb"[ \t]*(?:%(?:endif|else)(?![A-Za-z])"
                       rb"|%(?:if|ifarch|ifnarch|ifos|ifnos|include|elifarch|elifos|elif)(?:[ \t]|$))")


def part(line):
    """The section a line starts, as `isPart` finds it, or None."""
    lowered = line.lower()
    for name in PARTS:
        name_bytes = name.encode()
        if lowered.startswith(name_bytes) and (len(line) == len(name_bytes) or line[len(name_bytes)] in b" \t\r\n\v\f"):
            return name
    return None


def views(directory):
    os.makedirs(directory, exist_ok=True)
    for number, entry in enumerate(sys.stdin):
        path = entry.rstrip("\n")
        with open(path, "rb") as f:
            data = f.read().removeprefix(b"\xef\xbb\xbf")
        view, comments, scripts, other_interpreter = bytearray(), [], False, False
        offset = 0
        for line in data.splitlines(keepends=True):
            body = line.rstrip(b"\r\n")
            section = part(body)
            if section is not None:
                scripts = section not in NOT_SCRIPTS
                interpreter = re.search(rb"\s-p\s+(\S+)", body)
                if scripts and interpreter and interpreter.group(1).decode(errors="replace") not in SHELLS:
                    other_interpreter = scripts = True
                view += re.sub(rb"[^\n]", b" ", line)
            elif scripts and not DIRECTIVE.match(body):
                view += line
            else:
                if not scripts and body.lstrip(b" \t\v\f").startswith(b"#"):
                    comments.append([offset + len(body) - len(body.lstrip(b" \t\v\f")), offset + len(body)])
                view += re.sub(rb"[^\n]", b" ", line)
            offset += len(line)
        view_path = os.path.join(directory, f"{number}.sh")
        with open(view_path, "wb") as f:
            f.write(view)
        print(json.dumps({"path": path, "view": view_path, "comments": comments, "invalid": other_interpreter}))


def merge(specs, judged_views):
    by_view = {}
    with open(judged_views) as f:
        for line in f:
            judged = json.loads(line)
            by_view[judged["path"]] = judged
    with open(specs) as f:
        for line in f:
            spec = json.loads(line)
            shell = by_view.get(spec["view"])
            errors = int(spec["invalid"]) + (1 if shell is None else shell["errors"])
            comments = spec["comments"] + (shell["comments"] if shell else [])
            literals = shell["literals"] if shell else []
            print(json.dumps({"path": spec["path"], "units": "bytes", "errors": errors,
                              "comments": sorted(comments), "literals": literals}))


if sys.argv[1] == "views":
    views(sys.argv[2])
else:
    merge(sys.argv[2], sys.argv[3])
