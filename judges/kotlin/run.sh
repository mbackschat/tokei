#!/bin/bash
# Builds the judge when needed and judges the Kotlin paths on stdin. The
# classpath is KOTLIN_JUDGE_CLASSPATH, or kotlin-compiler-embeddable and
# kotlin-stdlib 2.4.0 from the Gradle cache.
dir=$(cd "$(dirname "$0")" && pwd)
if [ -z "$KOTLIN_JUDGE_CLASSPATH" ]; then
  cache="$HOME/.gradle/caches/modules-2/files-2.1/org.jetbrains.kotlin"
  KOTLIN_JUDGE_CLASSPATH="$(ls "$cache"/kotlin-compiler-embeddable/2.4.0/*/kotlin-compiler-embeddable-2.4.0.jar):$(ls "$cache"/kotlin-stdlib/2.4.0/*/kotlin-stdlib-2.4.0.jar)"
fi
[ "$dir/out/Judge.class" -nt "$dir/Judge.java" ] || javac -cp "$KOTLIN_JUDGE_CLASSPATH" -d "$dir/out" "$dir/Judge.java" >&2 || exit 1
exec java -cp "$KOTLIN_JUDGE_CLASSPATH:$dir/out" Judge
