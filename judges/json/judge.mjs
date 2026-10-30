// Judges JSON files with `jsonc-parser`'s scanner, the one VS Code reads JSON
// and JSON with Comments with. Strict JSON (RFC 8259) has no comments, while
// files such as `tsconfig.json` and VS Code's settings use the `.json`
// extension for JSON with Comments, which adds `//` and `/* */` comments;
// the scanner reads both, and strict JSON is the subset without comments.
//
// Reads paths on stdin and prints, per file, the comment and literal ranges
// in UTF-16 offsets and the scanner's errors, as judgecmp's stream expects.
// Strings are literals.
//
//     pnpm install && node judge.mjs < paths
import { readFileSync } from "node:fs";
import { createInterface } from "node:readline";
import { createScanner, ScanError, SyntaxKind } from "jsonc-parser";

function judge(path, text) {
  const judged = { path, units: "utf16", errors: 0, comments: [], literals: [] };
  const scanner = createScanner(text, false);
  for (let kind = scanner.scan(); kind !== SyntaxKind.EOF; kind = scanner.scan()) {
    if (scanner.getTokenError() !== ScanError.None || kind === SyntaxKind.Unknown) judged.errors++;
    const range = [scanner.getTokenOffset(), scanner.getTokenOffset() + scanner.getTokenLength()];
    if (kind === SyntaxKind.LineCommentTrivia || kind === SyntaxKind.BlockCommentTrivia) judged.comments.push(range);
    else if (kind === SyntaxKind.StringLiteral) judged.literals.push(range);
  }
  return judged;
}

const decoder = new TextDecoder("utf-8", { fatal: true });
for await (const path of createInterface({ input: process.stdin })) {
  let text;
  try {
    text = decoder.decode(readFileSync(path));
  } catch {
    continue;
  }
  if (text.startsWith("﻿")) text = text.slice(1);
  console.log(JSON.stringify(judge(path, text)));
}
