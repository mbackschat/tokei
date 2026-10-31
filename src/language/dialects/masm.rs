//! MASM's `COMMENT` directive, which makes a comment of the text from the
//! directive to the end of the line holding the next occurrence of its
//! delimiter, the first character after it (MASM 6.1 Programmer's Guide,
//! chapter 1, "Comments"). NASM, which shares the `.asm` extension, has no
//! such directive.

use crate::language::syntax::line_end;

/// The end of the `COMMENT` block that the line at `lines[start..]` opens,
/// the end of the line holding its closing delimiter or of the text when
/// none follows.
pub(crate) fn comment_block_end(lines: &[u8], start: usize) -> Option<usize> {
    let line = &lines[start..line_end(lines, start)];
    let text = &line[line
        .iter()
        .take_while(|b| matches!(b, b' ' | b'\t'))
        .count()..];
    let word = text.get(..b"COMMENT".len())?;
    if !word.eq_ignore_ascii_case(b"COMMENT") {
        return None;
    }
    let rest = &text[word.len()..];
    let space = rest
        .iter()
        .take_while(|b| matches!(b, b' ' | b'\t'))
        .count();
    let &delimiter = rest.get(space).filter(|_| space > 0)?;
    if delimiter.is_ascii_whitespace() {
        return None;
    }
    let after = start + (line.len() - rest.len()) + space + 1;
    Some(
        memchr::memchr(delimiter, &lines[after..])
            .map_or(lines.len(), |at| line_end(lines, after + at)),
    )
}
