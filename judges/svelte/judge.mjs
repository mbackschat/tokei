// Judges Svelte components with Svelte's own parser, `parse` from
// svelte/compiler, and their script and style blocks with Babel, PostCSS and
// postcss-scss.
//
// Reads paths on stdin and prints, per file, the comment and literal ranges
// in UTF-16 offsets and the parse errors, as judgecmp's stream expects.
// Markup comments are comments, quoted attribute values literals, and text
// and `{…}` expressions code, except for the JavaScript comments in them.
// Script elements in the markup are read as scripts. A script is read by Babel, as TypeScript for
// `lang="ts"`; a style by postcss-scss for `lang="scss"`, `"sass"` or
// `"less"`, and by PostCSS otherwise. Svelte's parser reads style blocks
// only as CSS, so their text is blanked, keeping its offsets, before the
// component is parsed. A component the parser rejects is an error.
//
//     pnpm install && node judge.mjs < paths
import { parse as babel } from "@babel/parser";
import { readFileSync } from "node:fs";
import { createRequire } from "node:module";
import { createInterface } from "node:readline";
import { parse } from "svelte/compiler";

const require = createRequire(import.meta.url);
const tokenizeCss = require("postcss/lib/tokenize");
const tokenizeScss = require("postcss-scss/lib/scss-tokenize");
const { Input } = require("postcss");

const STYLE = /(<style\b[^>]*>)([\s\S]*?)(<\/style\s*>)/gi;

function lang(node) {
  const value = node.attributes?.find((a) => a.name === "lang")?.value;
  return Array.isArray(value) ? value[0]?.data : undefined;
}

function script(node, text, judged) {
  const open = text.indexOf(">", node.start) + 1;
  const close = text.lastIndexOf("<", node.end - 1);
  const typescript = ["ts", "typescript"].includes(lang(node));
  let ast;
  try {
    ast = babel(text.slice(open, close), {
      sourceType: "module", errorRecovery: true, tokens: true, plugins: typescript ? ["typescript"] : [],
    });
  } catch {
    judged.errors += 1;
    return;
  }
  judged.errors += ast.errors?.length ?? 0;
  for (const c of ast.comments) judged.comments.push([open + c.start, open + c.end]);
  for (const t of ast.tokens) {
    const label = t.type && (t.type.label ?? t.type);
    if (label === "string" || label === "template" || label === "regexp" || label === "`") {
      judged.literals.push([open + t.start, open + t.end]);
    }
  }
}

function style(start, styles, lang, judged) {
  const tokenize = ["scss", "sass", "less"].includes(lang) ? tokenizeScss : tokenizeCss;
  try {
    const tokens = tokenize(new Input(styles));
    while (!tokens.endOfFile()) {
      const [type, , from, to] = tokens.nextToken();
      if (type === "comment") judged.comments.push([start + from, start + to + 1]);
      else if (type === "string") judged.literals.push([start + from, start + to + 1]);
    }
  } catch {
    judged.errors += 1;
  }
}

// Markup nodes nest in fragments under several keys, such as an element's
// `fragment` or a block's `consequent`, so every object below a node is read.
function markup(node, text, judged) {
  if (Array.isArray(node)) return node.forEach((child) => markup(child, text, judged));
  if (!node || typeof node !== "object") return;
  if (node.type === "Comment") return judged.comments.push([node.start, node.end]);
  // A script element in the markup, as in `<svelte:head>`, is one the
  // browser runs (HTML Standard, "The script element").
  if (node.type === "RegularElement" && node.name === "script") return script(node, text, judged);
  if (node.type === "Attribute" && Array.isArray(node.value) && node.value.length > 0) {
    const start = node.value[0].start;
    const end = node.value[node.value.length - 1].end;
    const quote = text[start - 1];
    if ((quote === '"' || quote === "'") && text[end] === quote) judged.literals.push([start - 1, end + 1]);
  }
  for (const [key, value] of Object.entries(node)) {
    if (key !== "metadata" && value && typeof value === "object") markup(value, text, judged);
  }
}

function judge(path, text) {
  const judged = { path, units: "utf16", errors: 0, comments: [], literals: [] };
  const styles = [];
  const blanked = text.replace(STYLE, (match, open, body, close, offset) => {
    styles.push({ start: offset + open.length, body, lang: open.match(/\blang\s*=\s*["']?([\w-]+)/i)?.[1] });
    return open + body.replace(/[^\n]/g, " ") + close;
  });
  let ast;
  try {
    ast = parse(blanked, { modern: true });
  } catch {
    judged.errors += 1;
    return judged;
  }
  markup(ast.fragment, text, judged);
  // The JavaScript comments of `{…}` expressions, which Svelte's parser
  // lists with those of the scripts.
  const seen = new Set(judged.comments.map(([start]) => start));
  for (const c of ast.comments ?? []) if (!seen.has(c.start)) judged.comments.push([c.start, c.end]);
  for (const node of [ast.instance, ast.module]) if (node) script(node, text, judged);
  for (const { start, body, lang } of styles) style(start, body, lang, judged);
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
