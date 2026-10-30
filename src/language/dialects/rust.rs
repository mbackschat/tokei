//! Rust frontmatter: a manifest at the start of a file between fences of
//! three or more dashes, read as rustc_lexer `Cursor::frontmatter` reads it
//! (rust-lang/rust `compiler/rustc_lexer/src/lib.rs`, published as
//! `ra-ap-rustc_lexer` 0.179.0; RFC 3503).

use crate::language::shebang::{shebang_len, ShebangSyntax};

/// The end of the frontmatter whose opening fence starts at `lines[start]`,
/// at the end of the line of its closing fence, or `None` when none opens
/// there. The closing fence is the next line that starts with as many dashes
/// as the opening one. An unclosed frontmatter is an error that rustc
/// recovers from by guessing, so it is left unread.
pub(crate) fn frontmatter_end(lines: &[u8], start: usize) -> Option<usize> {
    let rest = &lines[start..];
    let fence = rest.iter().take_while(|b| **b == b'-').count();
    if fence < 3 || !only_whitespace_before(lines, start) {
        return None;
    }
    let opening_end = start + rest.iter().position(|b| *b == b'\n')?;
    let mut closing = vec![b'\n'];
    closing.resize(fence + 1, b'-');
    let close = opening_end
        + 1
        + lines[opening_end..]
            .windows(closing.len())
            .position(|window| window == closing.as_slice())?;
    Some(
        lines[close..]
            .iter()
            .position(|b| *b == b'\n')
            .map_or(lines.len(), |n| close + n),
    )
}

/// Whether only whitespace comes before `start`, after a shebang line.
fn only_whitespace_before(lines: &[u8], start: usize) -> bool {
    let from = shebang_len(lines, ShebangSyntax::Rust).map_or(0, |len| len + 1);
    from <= start
        && std::str::from_utf8(&lines[from..start])
            .is_ok_and(|prefix| prefix.chars().all(is_whitespace))
}

/// Rust's whitespace, Unicode Pattern_White_Space (Rust reference,
/// Whitespace).
fn is_whitespace(c: char) -> bool {
    matches!(
        c,
        '\t' | '\n'
            | '\u{000B}'
            | '\u{000C}'
            | '\r'
            | ' '
            | '\u{0085}'
            | '\u{200E}'
            | '\u{200F}'
            | '\u{2028}'
            | '\u{2029}'
    )
}
