#!/bin/bash
# Builds the judge when needed and judges the Groovy paths on stdin, in the
# official Groovy image (Groovy 5.1.3 when measured), whose jar holds the
# lexer. The paths must be readable inside the container, so the directory
# holding them, JUDGE_ROOT, is mounted at its own path.
dir=$(cd "$(dirname "$0")" && pwd)
exec docker run --rm -i -v "$dir:/judge" -v "$JUDGE_ROOT:$JUDGE_ROOT:ro" groovy:latest sh -c '
  jar=$(ls /opt/groovy/lib/groovy-[0-9]*.jar)
  [ /judge/out/Judge.class -nt /judge/Judge.java ] || javac -cp "$jar" -d /judge/out /judge/Judge.java >&2 || exit 1
  exec java -cp "$jar:/judge/out" Judge'
