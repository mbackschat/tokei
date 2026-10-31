#!/bin/bash
# Builds the judge when needed and judges the Zig paths on stdin in the
# locmetry-zig-judge image, built from zig/Dockerfile. The paths must be
# readable inside the container, so the directory holding them, JUDGE_ROOT,
# is mounted at its own path.
dir=$(cd "$(dirname "$0")" && pwd)
exec docker run --rm -i --network none -v "$dir:/judge" -v "$JUDGE_ROOT:$JUDGE_ROOT:ro" \
  -e ZIG_GLOBAL_CACHE_DIR=/judge/out/cache -e ZIG_LOCAL_CACHE_DIR=/judge/out/cache locmetry-zig-judge sh -c '
  [ /judge/out/judge -nt /judge/judge.zig ] || zig build-exe -O ReleaseFast -femit-bin=/judge/out/judge /judge/judge.zig >&2 || exit 1
  exec /judge/out/judge'
