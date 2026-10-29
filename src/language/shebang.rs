//! Shebang lines, `#!` opening a file, which the judged lexers read as
//! comments: Kotlin's `SHEBANG_COMMENT` (KotlinLexer.flex), JavaScript's
//! HashbangComment (ECMA-262 2025 §12.5), and rustc, which strips a shebang
//! as it skips comments (rustc_lexer `strip_shebang`).

use super::syntax::block_comment_len;
use crate::utils::ext::SliceExt;

/// Whose rules decide whether a file opens with a shebang.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum ShebangSyntax {
    /// Any first line starting with `#!`.
    Plain,
    /// Rust's: `#!` not followed by `[`, which opens an inner attribute such
    /// as `#![allow(dead_code)]`, past whitespace and comments other than
    /// doc comments.
    Rust,
}

/// The length of the shebang line that opens `lines`, without its line
/// terminator, or `None` when the file opens with none.
pub(crate) fn shebang_len(lines: &[u8], syntax: ShebangSyntax) -> Option<usize> {
    let tail = lines.strip_prefix(b"#!")?;
    if syntax == ShebangSyntax::Rust && opens_inner_attribute(tail) {
        return None;
    }
    Some(
        lines
            .iter()
            .position(|b| *b == b'\n')
            .unwrap_or(lines.len()),
    )
}

/// Whether `tail`, the text after `#!`, holds `[` after whitespace and
/// comments that are not doc comments, which rustc_lexer counts as tokens:
/// `//!`, `///` but not `////`, `/*!`, and `/**` but not `/***` or `/**/`.
fn opens_inner_attribute(mut tail: &[u8]) -> bool {
    loop {
        tail = tail.trim_start();
        if let Some(rest) = tail.strip_prefix(b"//") {
            if rest.first() == Some(&b'!')
                || (rest.first() == Some(&b'/') && rest.get(1) != Some(&b'/'))
            {
                return false;
            }
            tail = &rest[rest.iter().position(|b| *b == b'\n').unwrap_or(rest.len())..];
        } else if tail.starts_with(b"/*") {
            let doc = match tail.get(2) {
                Some(b'!') => true,
                Some(b'*') => !matches!(tail.get(3), Some(b'*' | b'/')),
                _ => false,
            };
            match block_comment_len(tail, "/*", "*/", true) {
                Some(len) if !doc => tail = &tail[len..],
                _ => return false,
            }
        } else {
            return tail.first() == Some(&b'[');
        }
    }
}
