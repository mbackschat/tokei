"""Compares Locmetry's Lean lines with the Lean-lexer judge, file by file.
Usage: leancmp.py OUT_JSON ROOT... ; files are the Lean files Git tracks."""
import json, os, subprocess, sys
S = os.path.dirname(os.path.abspath(__file__))
out, roots = sys.argv[1], sys.argv[2:]
files = []
for root in roots:
    ls = subprocess.run(["git", "-C", root, "ls-files", "-z", "*.lean", "*.hlean"], capture_output=True, check=True).stdout
    files += [f"{root}/{p}" for p in ls.decode().split("\0") if p]
def run(tool):
    r = subprocess.run([tool], input="\n".join(files) + "\n", capture_output=True, text=True)
    res = {}
    for line in r.stdout.splitlines():
        parts = line.split("\t")
        res[parts[0]] = parts[1:]
    return res, r.stderr
loc, e1 = run(f"{S}/leanlines/target/release/leanlines")
ref, e2 = run(f"{S}/leanref/.lake/build/bin/leanref")
diffs = []; fallbacks = 0; lines = 0; invalid = []
for f in files:
    a = loc.get(f, [None])[0]; b, fb, le = (ref.get(f, [None]) + ["0", "0"])[:3]
    fallbacks += int(fb); lines += len(b or "")
    if int(le): invalid.append(f)
    if a != b:
        idx = [i + 1 for i in range(max(len(a or ""), len(b or ""))) if (a or "")[i:i+1] != (b or "")[i:i+1]]
        diffs.append({"invalid": int(le) > 0, "file": f, "lines": idx[:20], "n": len(idx), "loc": a and a.count("C"), "ref": b and b.count("C")})
json.dump({"files": len(files), "lines": lines, "diffs": diffs, "fallbacks": fallbacks, "invalid": invalid}, open(out, "w"), indent=1)
valid = [d for d in diffs if not d["invalid"]]
print(f"files {len(files)} lines {lines} lexically-invalid files {len(invalid)}; differing valid files {len(valid)} ({sum(d['n'] for d in valid)} lines), differing invalid files {len(diffs) - len(valid)}; interp-fallbacks {fallbacks}")
if e1.strip() or e2.strip(): print("stderr:", (e1 + e2)[:500])
