"""Judges GNU-style assembly files with LLVM's assembler lexer: reads paths on
stdin and prints, per file, the comment and literal ranges in byte offsets
after a byte order mark and whether a comment or string ran to the end of
the file, as judgecmp's stream expects.

A file's comment syntax depends on the processor it is assembled for, `#`
on x86, `@` on 32-bit Arm and `//` on 64-bit Arm, so each file is lexed for
the target its build assembles it for, named by the nearest directory or
file name word that names a processor, as `arch/arm64/` or
`aesv8-armx-apple.S` do; a word naming Apple's platforms selects Apple's
assembler syntax, in which `;` opens a comment. A file whose path names no
processor LLVM assembles for is left out. A file ending in `.S` is read by
the C preprocessor first, as compiler drivers do, and one ending in `.s` is
not (GCC manual, Overall Options, `-x assembler-with-cpp`). Go's assembly
files, which name symbols as `name(SB)`, are another syntax, for the Go
assembly judge.

    python3 gas/judge.py | judgecmp stream auto
"""

import os
import re
import subprocess
import sys

LEX = os.path.join(os.path.dirname(os.path.abspath(__file__)), "lex")
PROCESSORS = [
    (r"x86_64|amd64|x64|x86-64|em64t", "x86_64"),
    (r"x86|i?[3-6]86|ia32|x86-32", "i686"),
    (r"aarch64|arm64|armv8|armv9", "aarch64"),
    (r"arm|armv[4-7].*|arm32|thumb.*", "armv7"),
    (r"riscv|riscv64|riscv32|rv64|rv32", "riscv64"),
    (r"powerpc|powerpc64|ppc|ppc64|ppc64le|ppc32|power[4-9]", "powerpc64le"),
    (r"mips|mips64|mipsel|mips32", "mips64el"),
    (r"loongarch|loongarch64|loongarch32|lsx|lasx", "loongarch64"),
    (r"s390|s390x|zarch", "s390x"),
    (r"sparc|sparc64|sparcv9", "sparcv9"),
    (r"hexagon", "hexagon"),
    (r"avr", "avr"),
    (r"msp430", "msp430"),
]
APPLE = re.compile(r"apple|darwin|macos|macosx|ios|mach-?o|osx")
WORD = re.compile(r"[-.+/]")
GO_SYMBOL = re.compile(rb"\(SB\)")


def target(path):
    words = []
    for part in reversed(path.split("/")):
        words.extend(reversed([w for w in WORD.split(part.lower()) if w]))
    apple = any(APPLE.fullmatch(w) for w in words)
    for word in words:
        for pattern, processor in PROCESSORS:
            if re.fullmatch(pattern, word):
                if apple and processor in ("x86_64", "aarch64"):
                    return ("arm64" if processor == "aarch64" else processor) + "-apple-darwin"
                return processor + "-unknown-linux-gnu"
    return None


def main():
    requests = []
    for line in sys.stdin:
        path = line.rstrip("\n")
        if not path:
            continue
        with open(path, "rb") as handle:
            if GO_SYMBOL.search(handle.read()):
                continue
        triple = target(path)
        if triple:
            mode = "cpp" if path.endswith(".S") else "plain"
            requests.append(f"{mode} {triple} {path}\n")
    run = subprocess.run([LEX], input="".join(requests), capture_output=True, text=True, check=True)
    sys.stdout.write(run.stdout)


main()
