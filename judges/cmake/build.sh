#!/bin/sh
# Builds the CMake judge for Linux from CMake 4.4.5's list-file lexer, the
# flex output that CMake checks in, in an image with a C compiler and curl:
#
#     docker run --rm -v "$PWD/judges/cmake:/judge" -w /judge golang:1.26 sh build.sh
#
# The lexer's headers want CMake's configured platform header and its
# integer types; on Linux the first adds nothing the lexer uses, and the
# second maps to <stdint.h>. Sources go to /tmp of the throwaway container.
set -eu
src=/tmp/cmake-lexer
mkdir -p "$src/cmsys" "$src/cm3p/kwiml"
base=https://raw.githubusercontent.com/Kitware/CMake/v4.4.5/Source
curl -fsSL "$base/LexerParser/cmListFileLexer.c" -o "$src/cmListFileLexer.c"
curl -fsSL "$base/cmListFileLexer.h" -o "$src/cmListFileLexer.h"
curl -fsSL "$base/cmStandardLexer.h" -o "$src/cmStandardLexer.h"
(cd "$src" && sha256sum -c -) < sha256sums
: > "$src/cmsys/Configure.h"
cat > "$src/cm3p/kwiml/int.h" <<'EOF'
#include <stdint.h>
#define KWIML_INT_int8_t int8_t
#define KWIML_INT_uint8_t uint8_t
#define KWIML_INT_int16_t int16_t
#define KWIML_INT_uint16_t uint16_t
#define KWIML_INT_int32_t int32_t
#define KWIML_INT_uint32_t uint32_t
EOF
cc -O2 -I "$src" judge.c "$src/cmListFileLexer.c" -o judge-linux
