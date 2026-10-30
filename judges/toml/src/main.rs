//! Judges TOML files with `toml_parser`'s lexer, the one under the `toml`
//! crate that Cargo reads manifests with: reads paths on stdin and prints, per
//! file, the comment and literal ranges in byte offsets after a byte order
//! mark, as judgecmp's stream expects. Comments are comments; basic, literal
//! and multi-line strings are literals. A file the `toml` crate does not
//! parse as a document counts as one error.

use std::io::{self, BufRead, Write};

use toml_parser::lexer::TokenKind;
use toml_parser::Source;

fn main() {
    let stdout = io::stdout();
    let mut out = io::BufWriter::new(stdout.lock());
    for path in io::stdin().lock().lines().map_while(Result::ok) {
        let Ok(bytes) = std::fs::read(&path) else { continue };
        let Ok(text) = String::from_utf8(bytes) else { continue };
        let text = text.strip_prefix('\u{feff}').unwrap_or(&text);
        let mut comments = Vec::new();
        let mut literals = Vec::new();
        for token in Source::new(text).lex() {
            let span = token.span();
            let range = [span.start(), span.end()];
            match token.kind() {
                TokenKind::Comment => comments.push(range),
                TokenKind::BasicString
                | TokenKind::LiteralString
                | TokenKind::MlBasicString
                | TokenKind::MlLiteralString => literals.push(range),
                _ => {}
            }
        }
        let errors = usize::from(text.parse::<toml::Table>().is_err());
        let judged = serde_json::json!({
            "path": path, "units": "bytes", "errors": errors,
            "comments": comments, "literals": literals,
        });
        writeln!(out, "{judged}").unwrap();
    }
}
