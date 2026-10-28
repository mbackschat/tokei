"""Bucket lines where the fork and tree-sitter disagree. Usage: adjudicate_fork.py HARNESS RESULTS CORPORA LANG"""
import collections, glob, json, os, re, subprocess, sys
harness, results, corpora, lang = sys.argv[1:5]
files = []
for f in glob.glob(results + "/*.jsonl"):
    repo = os.path.basename(f)[:-6]
    for l in open(f):
        r = json.loads(l)
        if r["lang"] == lang and r["fork"] != r["ts"]:
            files.append((repo, r))
buckets = collections.Counter(); samples = collections.defaultdict(list)
for repo, r in files:
    out = subprocess.run([harness, "lines", f"{corpora}/{repo}/{r['path']}", "--diff"], capture_output=True, text=True).stdout
    for line in out.splitlines():
        m = re.match(r"\s*(\d+) tok:(.) ts:(.) lex:(.) fork:(.) . ?(.*)", line)
        if not m: continue
        n, t, s, x, f, src = m.groups()
        if f == s: continue
        key = "doc-comment" if re.match(r"\s*//[/!]", src) else f"fork:{f} ts:{s}"
        buckets[key] += 1
        if len(samples[key]) < 20: samples[key].append(f"{repo}/{r['path']}:{n}: {src[:110]}")
print(f"{lang}: {len(files)} files where fork ≠ tree-sitter")
for k, v in buckets.most_common():
    print(f"  {v:6} lines  {k}")
    if k != "doc-comment":
        for x in samples[k][:6]: print("        ", x)
