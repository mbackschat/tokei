// Judges HTML files with parse5, the HTML Standard's tokenizer as jsdom and
// Angular use it, and their scripts and style sheets with Babel and PostCSS.
//
// Reads paths on stdin and prints, per file, the comment and literal ranges
// in UTF-16 offsets and the errors, as judgecmp's stream expects. HTML
// comments are comments, quoted attribute values literals, and text code.
// A script whose type is a JavaScript MIME type, `module` or absent, or one
// Babel standalone compiles, is read by Babel, and a style element by
// PostCSS's tokenizer (HTML Standard,
// "The script element" and "The style element"); their comments and
// literals keep their offsets in the file. Other scripts, such as JSON or
// templates, are data blocks the browser does not run, so their text is
// code. A comment that the end of the file cuts off, or a script Babel
// rejects, is an error.
//
//     pnpm install && node judge.mjs < paths
import { parse } from "@babel/parser";
import { readFileSync } from "node:fs";
import { createRequire } from "node:module";
import { createInterface } from "node:readline";

const require = createRequire(import.meta.url);
const { SAXParser } = require("parse5-sax-parser");
const tokenizeCss = require("postcss/lib/tokenize");
const { Input } = require("postcss");

// HTML Standard, "JavaScript MIME type".
const JAVASCRIPT_TYPES = new Set([
  "", "module", "application/ecmascript", "application/javascript", "application/x-ecmascript",
  "application/x-javascript", "text/ecmascript", "text/javascript", "text/javascript1.0", "text/javascript1.1",
  "text/javascript1.2", "text/javascript1.3", "text/javascript1.4", "text/javascript1.5", "text/jscript",
  "text/livescript", "text/x-ecmascript", "text/x-javascript",
]);
// Babel standalone compiles these in the browser, JSX included
// (https://babeljs.io/docs/babel-standalone).
const BABEL_TYPES = new Set(["text/babel", "text/jsx"]);

function script(text, offset, judged, plugins = []) {
  let ast;
  try {
    ast = parse(text, { sourceType: "unambiguous", errorRecovery: true, tokens: true, plugins });
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

function style(text, offset, judged) {
  try {
    const tokens = tokenizeCss(new Input(text));
    while (!tokens.endOfFile()) {
      const [type, , start, end] = tokens.nextToken();
      if (type === "comment") judged.comments.push([offset + start, offset + end + 1]);
      else if (type === "string") judged.literals.push([offset + start, offset + end + 1]);
    }
  } catch {
    judged.errors += 1;
  }
}

function judge(path, text) {
  const judged = { path, units: "utf16", errors: 0, comments: [], literals: [] };
  const parser = new SAXParser({ sourceCodeLocationInfo: true });
  let element = null;
  parser.on("comment", ({ sourceCodeLocation: at }) => {
    if (!text.slice(at.startOffset, at.endOffset).endsWith(">")) judged.errors += 1;
    judged.comments.push([at.startOffset, at.endOffset]);
  });
  parser.on("startTag", ({ tagName, attrs, sourceCodeLocation: at }) => {
    for (const span of Object.values(at.attrs ?? {})) {
      const quote = text.slice(span.startOffset, span.endOffset).search(/=\s*["']/);
      if (quote >= 0) {
        const open = text.indexOf(text.slice(span.startOffset + quote).match(/["']/)[0], span.startOffset + quote);
        judged.literals.push([open, span.endOffset]);
      }
    }
    const type = (attrs.find((a) => a.name === "type")?.value ?? "").trim().toLowerCase();
    if (tagName === "script" && JAVASCRIPT_TYPES.has(type)) element = { read: script, start: at.endOffset };
    else if (tagName === "script" && BABEL_TYPES.has(type)) {
      element = { read: (t, o, j) => script(t, o, j, ["jsx"]), start: at.endOffset };
    }
    else if (tagName === "style") element = { read: style, start: at.endOffset };
  });
  parser.on("endTag", ({ tagName, sourceCodeLocation: at }) => {
    if (element && (tagName === "script" || tagName === "style")) {
      element.read(text.slice(element.start, at.startOffset), element.start, judged);
    }
    element = null;
  });
  parser.end(text);
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
