"""For each file where fork != tree-sitter, print the first differing line and its preceding context."""
import glob, json, os, re, subprocess, sys
harness, results, corpora, lang = sys.argv[1:5]
ctx = int(sys.argv[5]) if len(sys.argv) > 5 else 2
for f in sorted(glob.glob(results + "/*.jsonl")):
    repo = os.path.basename(f)[:-6]
    for l in open(f):
        r = json.loads(l)
        if r["lang"] != lang or r["fork"] == r["ts"]: continue
        path = f"{corpora}/{repo}/{r['path']}"
        out = subprocess.run([harness, "lines", path], capture_output=True, text=True).stdout.splitlines()
        rows = [re.match(r"\s*(\d+) tok:(.) ts:(.) lex:(.) fork:(.) . ?(.*)", x) for x in out]
        rows = [m.groups() for m in rows if m]
        idx = next((i for i, g in enumerate(rows) if g[4] != g[2]), None)
        if idx is None: continue
        print(f"== {repo}/{r['path']}")
        for g in rows[max(0, idx - ctx):idx + 1]:
            print(f"   {g[0]:>5} ts:{g[2]} fork:{g[4]} {'≠' if g[2]!=g[4] else ' '} {g[5][:120]}")
