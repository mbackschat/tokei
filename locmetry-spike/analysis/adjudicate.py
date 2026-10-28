"""Collect lines where Tokei, tree-sitter and the lexer disagree, per language.

Usage: adjudicate.py HARNESS RESULTS CORPORA LANG [MAX_FILES]
Prints a bucket summary (Rust doc-comment lines separated) and sample lines."""
import collections, glob, json, os, re, subprocess, sys
harness, results, corpora, lang = sys.argv[1:5]
cap = int(sys.argv[5]) if len(sys.argv) > 5 else 60
files = []
for f in glob.glob(results + "/*.jsonl"):
    repo = os.path.basename(f)[:-6]
    for l in open(f):
        r = json.loads(l)
        if r["lang"] == lang and (r["tokei"] != r["ts"] or r["ts"] != r["lex"]):
            files.append((abs(r["tokei"][0] - r["ts"][0]), repo, r))
files.sort(key=lambda x: -x[0])
buckets = collections.Counter(); samples = collections.defaultdict(list)
for _, repo, r in files[:cap]:
    out = subprocess.run([harness, "lines", f"{corpora}/{repo}/{r['path']}", "--diff"], capture_output=True, text=True).stdout
    for line in out.splitlines():
        m = re.match(r"\s*(\d+) tok:(.) ts:(.) lex:(.) . ?(.*)", line)
        if not m: continue
        n, t, s, x, src = m.groups()
        key = "doc-comment" if re.match(r"\s*//[/!]", src) else f"tok:{t} ts:{s} lex:{x}"
        buckets[key] += 1
        if len(samples[key]) < 400: samples[key].append(f"{repo}/{r['path']}:{n}: {src[:110]}")
print(f"{lang}: {len(files)} files differ; inspected {min(cap, len(files))}")
for k, v in buckets.most_common(): print(f"  {v:6} lines  {k}")
json.dump(samples, open(f"/private/tmp/claude-501/-Users-mbackschat-Projects-locmetry/85a7e0be-58fe-4309-b128-e0a0a7d08a6e/scratchpad/samples-{lang}.json", "w"), indent=0)
