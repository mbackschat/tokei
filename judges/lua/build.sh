#!/bin/sh
# Builds the Lua judge for Linux from Lua 5.5.1's source, checked against the
# SHA-256 that https://www.lua.org/ftp/ lists, in an image with a C compiler:
#
#     docker run --rm -v "$PWD/judges/lua:/judge" -w /judge golang:1.25 sh build.sh
#
# Sources go to /tmp of the throwaway container, which removes them.
set -eu
src=/tmp/lua-src
mkdir -p "$src"
curl -fsSL https://www.lua.org/ftp/lua-5.5.1.tar.gz -o "$src/lua.tar.gz"
echo "1c4b4068d67061f2a2231ad2b5422e77acea1487ea9890f6320af614f4373dce  $src/lua.tar.gz" | sha256sum -c -
tar -xzf "$src/lua.tar.gz" -C "$src"
lua=$src/lua-5.5.1/src
cc -O2 -std=gnu99 -I "$lua" judge.c $(ls "$lua"/*.c | grep -v -e '/lua\.c$' -e '/luac\.c$') -o judge-linux -lm
