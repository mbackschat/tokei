#!/bin/sh
# Builds the judge when needed and judges the Scala paths on stdin, with the
# jars that fetch.sh downloads.
set -eu
dir=$(cd "$(dirname "$0")" && pwd)
[ -f "$dir/lib/scala3-compiler_3-3.10.0.jar" ] || "$dir/fetch.sh"
cp=$(ls "$dir"/lib/*.jar | tr '\n' ':')
[ "$dir/out/main.class" -nt "$dir/Judge.scala" ] ||
  java -cp "$cp" dotty.tools.dotc.Main -classpath "$cp" -d "$dir/out" "$dir/Judge.scala" >&2
exec java -Xss64m -cp "$cp$dir/out" main
