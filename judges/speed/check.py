"""Fails when the fork parses slower than upstream Tokei beyond a threshold.

Builds the same benchmark twice, against this working tree and against the
upstream release tag, then times both on each root given, interleaved, and
compares their minimum user CPU time. The minimum of interleaved runs is the
statistic because a loaded machine only ever adds time; instruction counts
varied by more than the differences measured (2026-10-29). The owner decided
on 2026-10-30 that the fork's speed must stay close to upstream's, so any root
over the threshold fails the check.

Upstream is given the string quotes of `baseline-quotes.json`, for languages
whose upstream definition has none: without them it analyses none of the
lines holding strings, a quarter of cabal's Haskell lines, a cost no correct
count can avoid. With them, the check measures the fork's own overhead, as
the owner decided on 2026-10-30.

    python3 judges/speed/check.py ROOT...
"""

import argparse
import json
from pathlib import Path
import resource
import subprocess
import sys

HERE = Path(__file__).resolve().parent
FORK = HERE.parent.parent


def build(variant, target):
    result = subprocess.run(
        ["cargo", "build", "--release", "--quiet", "--manifest-path", str(HERE / variant / "Cargo.toml"),
         "--target-dir", str(target)],
        capture_output=True, text=True,
    )
    if result.returncode != 0:
        sys.exit(f"building the {variant} benchmark failed:\n{result.stderr}")
    return target / "release" / f"speed-{variant}"


def add_quotes(path, quotes):
    definitions = json.loads(path.read_text())
    for language, pairs in quotes.items():
        definition = definitions["languages"][language]
        if "quotes" in definition:
            sys.exit(f"upstream's {language} already has quotes; remove it from baseline-quotes.json")
        definition["quotes"] = pairs
    path.write_text(json.dumps(definitions, indent=2, ensure_ascii=False) + "\n")


def user_time(binary, root):
    before = resource.getrusage(resource.RUSAGE_CHILDREN).ru_utime
    subprocess.run([str(binary), root], check=True, stdout=subprocess.DEVNULL)
    return resource.getrusage(resource.RUSAGE_CHILDREN).ru_utime - before


def main():
    parser = argparse.ArgumentParser(description=__doc__.split("\n\n")[0])
    parser.add_argument("roots", nargs="+")
    parser.add_argument("--base", default="v15.0.0", help="the upstream tag to compare with")
    parser.add_argument("--threshold", type=float, default=1.10,
                        help="the largest allowed ratio of fork to upstream time")
    parser.add_argument("--runs", type=int, default=15)
    args = parser.parse_args()

    upstream = HERE / "upstream"
    stamp = upstream / ".tag"
    quotes = (HERE / "baseline-quotes.json").read_text()
    version = f"{args.base}\n{quotes}"
    if not stamp.exists() or stamp.read_text() != version:
        upstream.mkdir(exist_ok=True)
        archive = subprocess.run(["git", "-C", str(FORK), "archive", args.base], check=True, capture_output=True)
        subprocess.run(["tar", "-x", "-C", str(upstream)], input=archive.stdout, check=True)
        add_quotes(upstream / "languages.json", json.loads(quotes))
        stamp.write_text(version)
    target = HERE / "target"
    binaries = {variant: build(variant, target) for variant in ("base", "fork")}

    failed = False
    print(f"{'root':<40} {'upstream':>9} {'fork':>9} {'ratio':>6}")
    for root in args.roots:
        best = {}
        for _ in range(args.runs):
            for variant, binary in binaries.items():
                best[variant] = min(best.get(variant, float("inf")), user_time(binary, root))
        ratio = best["fork"] / best["base"]
        over = ratio > args.threshold
        failed |= over
        name = "/".join(Path(root).resolve().parts[-2:])
        print(f"{name:<40} {best['base']:>8.3f}s {best['fork']:>8.3f}s {ratio:>6.3f}{'  OVER' if over else ''}")
    if failed:
        sys.exit(f"a root above takes more than {args.threshold:.2f} times {args.base}'s time")


if __name__ == "__main__":
    main()
