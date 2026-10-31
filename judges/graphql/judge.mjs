// Judges GraphQL files with graphql-js, the reference implementation: reads
// paths on stdin and prints, per file, the comment and literal ranges in
// UTF-16 offsets after a byte order mark and whether its lexer rejected the
// text, as judgecmp's stream expects.
//
// The lexer keeps `#` comments in its token chain, though the parser skips
// them; strings and block strings are literals. A document the parser
// rejects but the lexer reads, such as a schema fragment or a file holding
// only comments, is judged all the same, as its tokens are what they are.
import { readFileSync } from "node:fs";
import { createInterface } from "node:readline";
import { Lexer, Source, TokenKind } from "graphql";

const decoder = new TextDecoder("utf-8", { fatal: true });

function judge(path) {
  let text;
  try {
    text = decoder.decode(readFileSync(path));
  } catch {
    return;
  }
  if (text.startsWith("﻿")) text = text.slice(1);
  const source = new Source(text);
  const comments = [];
  const literals = [];
  let errors = 0;
  const lexer = new Lexer(source);
  const first = lexer.token;
  try {
    while (lexer.advance().kind !== TokenKind.EOF);
  } catch {
    errors = 1;
  }
  for (let token = first; token; token = token.next) {
    if (token.kind === TokenKind.COMMENT) comments.push([token.start, token.end]);
    else if (token.kind === TokenKind.STRING || token.kind === TokenKind.BLOCK_STRING)
      literals.push([token.start, token.end]);
  }
  process.stdout.write(JSON.stringify({ path, units: "utf16", errors, comments, literals }) + "\n");
}

for await (const line of createInterface({ input: process.stdin })) if (line) judge(line);
