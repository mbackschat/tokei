#!/bin/sh
# Downloads the Scala 3.10.0 compiler and its runtime dependencies from Maven
# Central into lib/.
set -eu
cd "$(dirname "$0")"
mkdir -p lib
m=https://repo1.maven.org/maven2
for jar in \
  org/scala-lang/scala3-compiler_3/3.10.0 \
  org/scala-lang/scala3-interfaces/3.10.0 \
  org/scala-lang/scala3-library_3/3.10.0 \
  org/scala-lang/scala-library/3.10.0 \
  org/scala-lang/tasty-core_3/3.10.0 \
  org/ow2/asm/asm/9.10.1 \
  org/ow2/asm/asm-tree/9.10.1 \
  org/ow2/asm/asm-analysis/9.10.1 \
  org/ow2/asm/asm-commons/9.10.1 \
  org/ow2/asm/asm-util/9.10.1 \
  org/scala-sbt/compiler-interface/1.12.0 \
  org/scala-sbt/util-interface/1.12.0
do
  name=$(basename "$(dirname "$jar")")-$(basename "$jar").jar
  [ -f "lib/$name" ] || curl -sfo "lib/$name" "$m/$jar/$name"
done
