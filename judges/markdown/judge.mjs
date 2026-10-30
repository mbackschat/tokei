// Judges Markdown files with commonmark.js, the CommonMark reference
// implementation, by the fenced code blocks it finds.
//
// Reads paths on stdin and prints, per file, each fenced code block: `open`,
// the line of its opening fence, `close`, the line of its closing fence or
// null when the document or its container ends first, `end`, the line after
// the block, all counted from 0, its `info` string and its content,
// `literal`, without the prefixes of the lists or block quotes holding it.
// `judgecmp markdown` counts the file from them.
//
//     pnpm install && node judge.mjs < paths
import { readFileSync } from "node:fs";
import { createInterface } from "node:readline";
import { Parser } from "commonmark";

function judge(path, text) {
  const lines = text.split("\n");
  const blocks = [];
  const walker = new Parser().parse(text).walker();
  let event;
  while ((event = walker.next())) {
    const node = event.node;
    if (!event.entering || node.type !== "code_block" || !node._isFenced) continue;
    const [[startLine, startColumn], [endLine]] = node.sourcepos;
    const fence = lines[startLine - 1].slice(startColumn - 1).match(/^(`{3,}|~{3,})/)[1];
    // A closing fence repeats the opening fence's character at least as
    // often, after the indentation or block quote markers of its container
    // (CommonMark 0.31.2, §4.5).
    const closing = new RegExp(`^${fence[0]}{${fence.length},}[ \\t]*\\r?$`);
    const closed = endLine > startLine && closing.test(lines[endLine - 1].replace(/^[ \t>]*/, ""));
    blocks.push({
      open: startLine - 1,
      close: closed ? endLine - 1 : null,
      end: endLine,
      info: node.info,
      literal: node.literal,
    });
  }
  return { path, blocks };
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
