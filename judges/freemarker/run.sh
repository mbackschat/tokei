#!/bin/bash
# Builds the judge when needed and judges the FreeMarker paths on stdin in a
# JDK image, with the FreeMarker 2.3.34 jar from Maven Central in lib/. The
# paths must be readable inside the container, so the directory holding
# them, JUDGE_ROOT, is mounted at its own path.
dir=$(cd "$(dirname "$0")" && pwd)
exec docker run --rm -i --network none -v "$dir:/judge" -v "$JUDGE_ROOT:$JUDGE_ROOT:ro" eclipse-temurin:21-jdk sh -c '
  jar=/judge/lib/freemarker-2.3.34.jar
  [ /judge/out/Judge.class -nt /judge/Judge.java ] || javac -cp "$jar" -d /judge/out /judge/Judge.java >&2 || exit 1
  exec java -cp "$jar:/judge/out" Judge'
