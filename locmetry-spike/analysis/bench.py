"""Single-thread throughput per counter from harness timings."""
import glob, json
tot = [0] * 4; mb = 0
for f in sorted(glob.glob("bench/*.jsonl")):
    rows = [json.loads(l) for l in open(f)]
    b = sum(r["bytes"] for r in rows) / 1e6
    us = [sum(r["us"][i] for r in rows) / 1e6 for i in range(4)]
    mb += b; tot = [a + c for a, c in zip(tot, us)]
    print(f"{f[6:-6]:9} {b:6.1f} MB  MB/s  tokei {b/us[0]:5.0f}  tree-sitter {b/us[1]:5.1f}  lexer {b/us[2]:5.0f}  fork {b/us[3]:5.0f}")
print(f"{'all':9} {mb:6.1f} MB  MB/s  tokei {mb/tot[0]:5.0f}  tree-sitter {mb/tot[1]:5.1f}  lexer {mb/tot[2]:5.0f}  fork {mb/tot[3]:5.0f}")
print(f"single-thread seconds: tokei {tot[0]:.1f}, tree-sitter {tot[1]:.1f}, lexer {tot[2]:.1f}, fork {tot[3]:.1f}")
