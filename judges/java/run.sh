#!/bin/bash
# Builds the judge when needed and judges the Java paths on stdin.
dir=$(cd "$(dirname "$0")" && pwd)
X="--add-opens jdk.compiler/com.sun.tools.javac.parser=ALL-UNNAMED --add-exports jdk.compiler/com.sun.tools.javac.parser=ALL-UNNAMED --add-exports jdk.compiler/com.sun.tools.javac.util=ALL-UNNAMED --add-exports jdk.compiler/com.sun.tools.javac.api=ALL-UNNAMED"
[ "$dir/out/Judge.class" -nt "$dir/Judge.java" ] || javac $X -d "$dir/out" "$dir/Judge.java" >&2 || exit 1
exec java $X -cp "$dir/out" Judge
