// Judges JavaScript and TypeScript files: reads paths from stdin and prints,
// per file, comment and literal ranges in UTF-16 offsets and the parse errors,
// as judgecmp's stream expects. JavaScript takes Babel with JSX and Flow, or
// Meta's Hermes parser for Flow that only Hermes accepts; TypeScript and TSX
// take the TypeScript compiler's parser. Strings, template text and regular
// expressions are literals. JSX text is not: JSX drops lines of whitespace
// (JSX specification, JSXText; React "Writing Markup with JSX").
// A hashbang is a comment (ECMA-262 2025 §12.5, HashbangComment).
import { parse } from "@babel/parser";
import { readFileSync } from "node:fs";
import { createRequire } from "node:module";
import { createInterface } from "node:readline";
const require = createRequire(import.meta.url);
const hermes = require("hermes-parser");
const ts = require("typescript");

// Proposals that Babel parses only with a plugin, as Prettier's tests use
// them; pipeline variants exclude each other, so each attempt takes one.
const PROPOSALS = [
  ["decorators", { decoratorsBeforeExport: false }], "doExpressions", "asyncDoExpressions", "exportDefaultFrom",
  "functionBind", "functionSent", "throwExpressions", "partialApplication", "v8intrinsic",
  "destructuringPrivate", "decoratorAutoAccessors", "importReflection", "moduleBlocks", "sourcePhaseImports",
  "deferredImportEvaluation", ["optionalChainingAssign", { version: "2023-07" }], ["discardBinding", { syntaxType: "void" }],
  "explicitResourceManagement",
];
const ATTEMPTS = [
  ["jsx", "flow"],
  ["jsx", "flow", ...PROPOSALS, ["pipelineOperator", { proposal: "minimal" }]],
  ["jsx", "flow", ...PROPOSALS.filter((p) => p !== "v8intrinsic"), ["pipelineOperator", { proposal: "hack", topicToken: "%" }]],
  ["jsx", "flow", ...PROPOSALS, ["pipelineOperator", { proposal: "fsharp" }]],
  ["jsx", "typescript", ...PROPOSALS, ["pipelineOperator", { proposal: "minimal" }]],
];

function babel(text) {
  let last;
  for (const plugins of ATTEMPTS) {
    try {
      last = babelWith(text, plugins);
    } catch {
      last = { errors: 1 };
    }
    if (last.errors === 0) return last;
  }
  return last;
}

function babelWith(text, plugins) {
  const ast = parse(text, {
    sourceType: "unambiguous",
    errorRecovery: true,
    tokens: true,
    allowReturnOutsideFunction: true,
    allowImportExportEverywhere: true,
    allowAwaitOutsideFunction: true,
    plugins,
  });
  const literals = [];
  for (const t of ast.tokens) {
    const label = t.type && (t.type.label ?? t.type);
    if (label === "string" || label === "template" || label === "regexp" || label === "`") literals.push([t.start, t.end]);
  }
  return { comments: ast.comments.map((c) => [c.start, c.end]), literals, errors: ast.errors?.length ?? 0 };
}

function hermesJudge(text) {
  const tree = hermes.parse(text, { flow: "all", tokens: true, sourceType: "module" });
  const literals = tree.tokens
    .filter((t) => t.type === "String" || t.type === "Template" || t.type === "RegularExpression")
    .map((t) => t.range);
  return { comments: tree.comments.map((c) => c.range), literals, errors: 0 };
}

const LITERAL_KINDS = new Set([
  ts.SyntaxKind.StringLiteral, ts.SyntaxKind.NoSubstitutionTemplateLiteral, ts.SyntaxKind.TemplateHead,
  ts.SyntaxKind.TemplateMiddle, ts.SyntaxKind.TemplateTail, ts.SyntaxKind.RegularExpressionLiteral,
]);

function typescript(text, path) {
  const kind = path.endsWith(".jsx") ? ts.ScriptKind.JSX : path.endsWith("x") ? ts.ScriptKind.TSX : ts.ScriptKind.TS;
  const file = ts.createSourceFile(path, text, ts.ScriptTarget.Latest, true, kind);
  const comments = [], literals = [], seen = new Set();
  const isJsDoc = (node) => node.kind >= ts.SyntaxKind.FirstJSDocNode && node.kind <= ts.SyntaxKind.LastJSDocNode;
  const visit = (node) => {
    // A JSDoc comment is a child of the node it documents, even of a token
    // such as the end of the file; its text is read as trivia below.
    const children = node.getChildren(file).filter((child) => !isJsDoc(child));
    if (children.length > 0) return children.forEach(visit);
    if (LITERAL_KINDS.has(node.kind)) literals.push([node.getStart(file), node.end]);
    // JSX text holds no trivia, and a `//` in it is text.
    if (node.kind === ts.SyntaxKind.JsxText || seen.has(node.pos)) return;
    seen.add(node.pos);
    // Comments on the previous token's line are its trailing ones, and those
    // after a line break this token's leading ones.
    const ranges = [...(ts.getTrailingCommentRanges(text, node.pos) ?? []), ...(ts.getLeadingCommentRanges(text, node.pos) ?? [])];
    for (const c of ranges) {
      if (!seen.has(-1 - c.pos)) comments.push([c.pos, c.end]);
      seen.add(-1 - c.pos);
    }
  };
  visit(file);
  return { comments, literals, errors: file.parseDiagnostics.length };
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
  let judged;
  if (/\.[cm]?tsx?$/.test(path)) {
    judged = typescript(text, path);
  } else {
    try {
      judged = babel(text);
    } catch {
      judged = { errors: 1 };
    }
    if (judged.errors > 0) {
      try {
        judged = hermesJudge(text);
      } catch {
        // TypeScript's parser recovers from any error, so the file still gets
        // line verdicts; it stays counted as invalid.
        judged = { ...typescript(text, path.replace(/\.[cm]?jsx?$/, ".jsx")), errors: Math.max(judged.errors, 1) };
      }
    }
    judged.comments ??= [];
    judged.literals ??= [];
  }
  const hashbang = text.startsWith("#!") ? [[0, text.search(/\r?\n|$/)]] : [];
  out.push(JSON.stringify({ path, units: "utf16", errors: judged.errors, comments: [...hashbang, ...judged.comments], literals: judged.literals }));
  if (out.length >= 200) process.stdout.write(out.splice(0).join("\n") + "\n");
}
if (out.length) process.stdout.write(out.join("\n") + "\n");
