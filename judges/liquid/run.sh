#!/bin/bash
# Judges the Liquid paths on stdin in the locmetry-liquid-judge image, built
# from liquid/Dockerfile. The paths must be readable inside the container, so
# the directory holding them, JUDGE_ROOT, is mounted at its own path.
dir=$(cd "$(dirname "$0")" && pwd)
exec docker run --rm -i --network none -v "$dir:/judge:ro" -v "$JUDGE_ROOT:$JUDGE_ROOT:ro" locmetry-liquid-judge ruby /judge/judge.rb
