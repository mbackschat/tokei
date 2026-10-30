// Judges YAML files with the `yaml` package's lexer, from the YAML 1.2
// implementation that passes the yaml-test-suite, and counts its parser's
// errors.
//
// Reads paths on stdin and prints, per file, the comment and literal ranges
// in UTF-16 offsets and the parse errors, as judgecmp's stream expects.
// Comments are comments; scalars are literals: plain, quoted and the bodies of
// block scalars. After a block scalar without content and without keep
// chomping, `+`, the lexer reads the empty lines that follow as its body,
// although chomping drops them from its value as it drops those after any
// other such scalar (YAML 1.2.2 §8.1.1.2), so they are not literal. The
// lexer's output is lossless, so the offsets follow from the lengths of its
// lexemes, less the one-character control lexemes that mark where documents
// and scalars start.
//
//     pnpm install && node judge.mjs < paths
import { readFileSync } from "node:fs";
import { createInterface } from "node:readline";
import { CST, Lexer, parseAllDocuments } from "yaml";

const QUOTED = new Set(["single-quoted-scalar", "double-quoted-scalar"]);

function isControl(lexeme) {
  return lexeme.length === 1 && lexeme.charCodeAt(0) < 0x20 && !"\t\n\r".includes(lexeme);
}

function judge(path, text) {
  const judged = { path, units: "utf16", errors: 0, comments: [], literals: [] };
  for (const doc of parseAllDocuments(text)) judged.errors += doc.errors?.length ?? 0;
  let offset = 0;
  let scalar = false;
  let keeps = false;
  for (const lexeme of new Lexer().lex(text)) {
    if (isControl(lexeme)) {
      // The lexeme after this marker is a plain scalar or a block scalar's
      // body (yaml's `src/parse/cst.ts`, `SCALAR`).
      scalar = lexeme === CST.SCALAR;
      continue;
    }
    const end = offset + lexeme.length;
    const type = CST.tokenType(lexeme);
    const emptyBody = scalar && !keeps && /^(\r?\n)+$/.test(lexeme);
    if (type === "block-scalar-header") keeps = lexeme.includes("+");
    if (type === "comment") judged.comments.push([offset, end]);
    else if ((scalar && !emptyBody) || QUOTED.has(type)) judged.literals.push([offset, end]);
    scalar = false;
    offset = end;
  }
  return judged;
}

const out = [];
for await (const path of createInterface({ input: process.stdin })) {
  let text;
  try {
    text = new TextDecoder("utf-8", { fatal: true }).decode(readFileSync(path));
  } catch {
    continue;
  }
  if (text.charCodeAt(0) === 0xfeff) text = text.slice(1);
  out.push(JSON.stringify(judge(path, text)));
  if (out.length >= 200) process.stdout.write(out.splice(0).join("\n") + "\n");
}
if (out.length) process.stdout.write(out.join("\n") + "\n");
