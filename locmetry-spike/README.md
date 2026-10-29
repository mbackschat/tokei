# Locmetry Counting Spike

Material from the 2026-10-28/29 spike that compared Tokei 15.0.0, tree-sitter, hand-written lexers and this fork, preserved unchanged for step 1 of Locmetry's [Tokei fork plan](https://github.com/mbackschat/locmetry/blob/main/docs/plans/tokei-fork.md). The results and method are in Locmetry's [counting backends record](https://github.com/mbackschat/locmetry/blob/main/docs/research/counting-backends.md).

- `harness/`: counts a directory with Tokei, tree-sitter, the spike lexers and the fork, applying Tokei's line rules to comment and literal ranges. `spike count ROOT` prints one JSON line per file; `spike lines FILE [--diff]` prints each line's class per counter. Its `Cargo.toml` builds this repository as the package `tokei-fork`, so it measures the fork's current state; it adds tree-sitter grammars for the step-1 languages.
- `fork-prototype/`: the prototype's engine changes as `engine.patch` against `v15.0.0`, and `declare-data.py`, which applies its `languages.json` declarations but reformats the file.
- `probes/`: `probe.py` measures 32 constructs across 25 languages with the Locmetry binary; `spike_probes.py` scores the harness's counters on 25 probes with hand-derived counts.
- `analysis/`: per-language summaries, disagreement bucketing, the first disagreement per file with its preceding lines, and single-thread timing over the harness output.
- `corpora.txt`: the 14 public repositories and commits the spike measured; `corpora-step1.txt`: the 17 measured for step 1 of the plan.

The spike lexers and declarations carry no cited sources; step 1 grounds each declaration before it ships.
