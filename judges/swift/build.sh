#!/bin/sh
# Builds the judge against the swift-syntax libraries of the active Xcode toolchain.
set -eu
host="$(dirname "$(xcrun --find swiftc)")/../lib/swift/host"
cd "$(dirname "$0")"
xcrun swiftc -O -I "$host" -L "$host" -lSwiftParser -lSwiftSyntax -Xlinker -rpath -Xlinker "$host" judge.swift -o judge
