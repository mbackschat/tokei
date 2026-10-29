# Generates src/main.rs from Locmetry's src/backend/lean.rs: the counter's
# logic verbatim, with each count replaced by a per-line class.
import re, sys
src = open(sys.argv[1]).read()
body = src[src.index("#[derive(Default)]\nenum State"):]
body = body.replace("fn count(text: &str) -> CodeStats {", "fn count(text: &str) -> String {")
body = body.replace("let mut stats = CodeStats::new();", "let mut stats = String::new();")
body = body.replace("stats.comments += 1", "stats.push('m')").replace("stats.code += 1", "stats.push('C')").replace("stats.blanks += 1", "stats.push('.')")
assert "CodeStats" not in body
main = '''
use std::io::BufRead;
fn main() {
    for path in std::io::stdin().lock().lines() {
        let path = path.unwrap();
        let bytes = std::fs::read(&path).unwrap();
        match std::str::from_utf8(&bytes) {
            Ok(text) => println!("{}\\t{}", path, count(text.strip_prefix('\\u{feff}').unwrap_or(text))),
            Err(_) => println!("{}\\t!", path),
        }
    }
}
'''
open(sys.argv[2], "w").write("#![allow(dead_code)]\n" + body + main)
