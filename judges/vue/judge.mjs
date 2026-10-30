// Judges Vue single-file components with Vue's own parser, @vue/compiler-dom
// in its SFC mode, and their script and style blocks with Babel, PostCSS and
// postcss-scss.
//
// Reads paths on stdin and prints, per file, the comment and literal ranges
// in UTF-16 offsets and the parse errors, as judgecmp's stream expects.
// Comments at the top level and in the template are comments, attribute
// values, directives' included, literals, and template text and
// interpolations code. A script block is read by Babel, as TypeScript for
// `lang="ts"` or `"tsx"`; a style block by postcss-scss for `lang="scss"`,
// `"sass"` or `"less"`, whose `//` comments it knows, and by PostCSS
// otherwise. Other blocks, such as `<i18n>`, and templates in other
// languages, such as Pug, are code.
//
//     pnpm install && node judge.mjs < paths
import { parse as babel } from "@babel/parser";
import { readFileSync } from "node:fs";
import { createRequire } from "node:module";
import { createInterface } from "node:readline";

const require = createRequire(import.meta.url);
const { parse, NodeTypes } = require("@vue/compiler-dom");
const tokenizeCss = require("postcss/lib/tokenize");
const tokenizeScss = require("postcss-scss/lib/scss-tokenize");
const { Input } = require("postcss");

const SCRIPT_PLUGINS = { ts: ["typescript"], tsx: ["typescript", "jsx"] };

function script(text, offset, lang, judged) {
  let ast;
  try {
    ast = babel(text, {
      sourceType: "module", errorRecovery: true, tokens: true, plugins: SCRIPT_PLUGINS[lang] ?? ["jsx"],
    });
  } catch {
    judged.errors += 1;
    return;
  }
  judged.errors += ast.errors?.length ?? 0;
  for (const c of ast.comments) judged.comments.push([offset + c.start, offset + c.end]);
  for (const t of ast.tokens) {
    const label = t.type && (t.type.label ?? t.type);
    if (label === "string" || label === "template" || label === "regexp" || label === "`") {
      judged.literals.push([offset + t.start, offset + t.end]);
    }
  }
}

function style(text, offset, lang, judged) {
  const tokenize = ["scss", "sass", "less"].includes(lang) ? tokenizeScss : tokenizeCss;
  try {
    const tokens = tokenize(new Input(text));
    while (!tokens.endOfFile()) {
      const [type, , start, end] = tokens.nextToken();
      if (type === "comment") judged.comments.push([offset + start, offset + end + 1]);
      else if (type === "string") judged.literals.push([offset + start, offset + end + 1]);
    }
  } catch {
    judged.errors += 1;
  }
}

function attributes(node, text, judged) {
  for (const prop of node.props ?? []) {
    if (prop.type === NodeTypes.ATTRIBUTE && prop.value) {
      judged.literals.push([prop.value.loc.start.offset, prop.value.loc.end.offset]);
    } else if (prop.type === NodeTypes.DIRECTIVE && prop.exp) {
      const start = prop.exp.loc.start.offset;
      const end = prop.exp.loc.end.offset;
      const quote = text[start - 1];
      if ((quote === '"' || quote === "'") && text[end] === quote) judged.literals.push([start - 1, end + 1]);
    }
  }
}

function template(nodes, text, judged) {
  for (const node of nodes) {
    if (node.type === NodeTypes.COMMENT) judged.comments.push([node.loc.start.offset, node.loc.end.offset]);
    if (node.type === NodeTypes.ELEMENT) {
      attributes(node, text, judged);
      template(node.children, text, judged);
    }
  }
}

function judge(path, text) {
  const judged = { path, units: "utf16", errors: 0, comments: [], literals: [] };
  const root = parse(text, { parseMode: "sfc", comments: true, onError: () => (judged.errors += 1) });
  for (const node of root.children) {
    if (node.type === NodeTypes.COMMENT) judged.comments.push([node.loc.start.offset, node.loc.end.offset]);
    if (node.type !== NodeTypes.ELEMENT) continue;
    attributes(node, text, judged);
    const lang = node.props.find((p) => p.type === NodeTypes.ATTRIBUTE && p.name === "lang")?.value?.content;
    const inner = node.children[0];
    if (node.tag === "template" && (!lang || lang === "html")) template(node.children, text, judged);
    else if (!inner) continue;
    else if (node.tag === "script") script(inner.content ?? "", inner.loc.start.offset, lang, judged);
    else if (node.tag === "style") style(inner.content ?? "", inner.loc.start.offset, lang, judged);
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
