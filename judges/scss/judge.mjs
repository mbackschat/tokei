// Judges SCSS files with postcss-scss's tokenizer, the one stylelint parses
// SCSS with.
//
// Reads paths on stdin and prints, per file, the comment and literal ranges
// in UTF-16 offsets and the number of tokenizer errors, as judgecmp's stream
// expects. `/* */` and `//` comments are comments and quoted strings
// literals. The tokenizer stops at an unclosed string or comment, which
// counts as an error, keeping the tokens read before it.
//
//     pnpm install && node judge.mjs < paths
import { createRequire } from "node:module";
import { readFileSync } from "node:fs";
import { createInterface } from "node:readline";

const require = createRequire(import.meta.url);
const tokenize = require("postcss-scss/lib/scss-tokenize");
const { Input } = require("postcss");

function judge(path) {
  let text;
  try {
    text = new TextDecoder("utf-8", { fatal: true }).decode(readFileSync(path));
  } catch {
    return null;
  }
  if (text.startsWith("﻿")) text = text.slice(1);
  const comments = [];
  const literals = [];
  let errors = 0;
  try {
    const tokens = tokenize(new Input(text));
    while (!tokens.endOfFile()) {
      const [type, , start, end] = tokens.nextToken();
      if (type === "comment") comments.push([start, end + 1]);
      else if (type === "string") literals.push([start, end + 1]);
    }
  } catch {
    errors = 1;
  }
  return { path, units: "utf16", errors, comments, literals };
}

for await (const line of createInterface({ input: process.stdin })) {
  const judged = judge(line);
  if (judged) console.log(JSON.stringify(judged));
}
