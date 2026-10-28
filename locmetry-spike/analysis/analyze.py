"""Summarize spike results per language: code totals, files that differ from tree-sitter, timings."""
import glob, json, sys, collections
rows = [json.loads(l) for f in glob.glob(sys.argv[1] + "/*.jsonl") for l in open(f)]
by = collections.defaultdict(list)
for r in rows: by[r["lang"]].append(r)
keys = ("tokei", "ts", "lex", "fork")
print(f"{'lang':11} {'files':>6} {'MB':>6} | code: {'tokei':>8} {'ts':>8} {'lex':>8} {'fork':>8} | files ≠ ts: {'tokei':>5} {'lex':>4} {'fork':>4} | MB/s: {'tokei':>5} {'ts':>4} {'lex':>4} {'fork':>4}")
for lang, rs in sorted(by.items()):
    mb = sum(r["bytes"] for r in rs) / 1e6
    tot = {k: sum(r[k][0] for r in rs if r[k] is not None) if any(r[k] is not None for r in rs) else "-" for k in keys}
    diff = lambda a: sum(r[a] != r["ts"] for r in rs if r[a] is not None) if any(r[a] is not None for r in rs) else "-"
    us = [max(sum(r["us"][i] for r in rs) / 1e6, 1e-9) for i in range(4)]
    print(f"{lang:11} {len(rs):6} {mb:6.1f} |       {tot['tokei']:8} {tot['ts']:8} {tot['lex']:8} {tot['fork']:8} |             {diff('tokei'):5} {diff('lex'):4} {diff('fork'):4} |       {mb/us[0]:5.0f} {mb/us[1]:4.1f} {mb/us[2]:4.0f} {mb/us[3]:4.0f}")
