#!/bin/bash
# Builds the judge when needed and judges the Velocity paths on stdin in a
# JDK image, with the Velocity 2.4.1 jars from Maven Central in lib/. The
# paths must be readable inside the container, so the directory holding
# them, JUDGE_ROOT, is mounted at its own path.
dir=$(cd "$(dirname "$0")" && pwd)
exec docker run --rm -i --network none -v "$dir:/judge" -v "$JUDGE_ROOT:$JUDGE_ROOT:ro" eclipse-temurin:21-jdk sh -c '
  jar=$(echo /judge/lib/*.jar | tr " " :)
  [ /judge/out/Judge.class -nt /judge/Judge.java ] || javac -cp "$jar" -d /judge/out /judge/Judge.java >&2 || exit 1
  exec java -cp "$jar:/judge/out" Judge'
