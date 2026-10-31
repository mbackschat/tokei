//! PlantUML's comments, as its preprocessor drops them line by line before
//! any diagram reads the text (PlantUML v1.2026.9: `TContext` chains
//! `CodeIteratorLongComment`, `CodeIteratorShortComment` and
//! `CodeIteratorInnerComment`, over line types from `TLineType`). A line
//! whose first text is `'` is a comment, and so is one holding nothing but a
//! `/' … '/`. A line whose first text is `/'` and that holds no `'/` opens a
//! comment of whole lines, which ends with the first line, that one
//! included, whose text ends with `'/`. Any other `/' … '/` shares its line
//! with text, and PlantUML reads no string across lines, so every other line
//! that is not blank is code.

use std::ops::Range;

use grep_searcher::LineStep;

use crate::language::{language_type::LineReport, syntax::LineClass};
use crate::utils::ext::SliceExt;
use crate::{CodeStats, Config};

pub(crate) fn parse(
    _config: &Config,
    text: &[u8],
    mut stats: CodeStats,
    counted: Range<usize>,
    on_line: &mut impl FnMut(LineReport<'_>),
) -> CodeStats {
    let mut in_comment = false;
    let mut stepper = LineStep::new(b'\n', 0, text.len());
    while let Some((start, end)) = stepper.next(text) {
        let line = &text[start..end];
        let class = if in_comment {
            in_comment = !trin_end(line).ends_with(b"'/");
            LineClass::Comment
        } else if opens_long_comment(line) {
            in_comment = true;
            LineClass::Comment
        } else if is_comment_line(line) {
            LineClass::Comment
        } else if line.trim().is_empty() {
            LineClass::Blank
        } else {
            LineClass::Code
        };
        if counted.contains(&start) {
            stats.add_line(class);
            on_line(LineReport::Class(Some(class)));
        }
    }
    stats
}

/// `COMMENT_LONG_START`: `/'` after blanks, with no `'/` on the line.
fn opens_long_comment(line: &[u8]) -> bool {
    skip_blanks(line).starts_with(b"/'") && !line.windows(2).any(|w| w == b"'/")
}

/// `COMMENT_SIMPLE`: `'` after blanks, or a line of nothing but `/' … '/`.
fn is_comment_line(line: &[u8]) -> bool {
    let text = skip_blanks(line);
    text.starts_with(b"'")
        || text
            .strip_prefix(b"/'")
            .is_some_and(|rest| trim_end_blanks(rest).ends_with(b"'/"))
}

/// The `[%s]*` the line patterns skip: Java's `\s` and U+00A0 (`Pattern2`).
fn blank_len(text: &[u8]) -> Option<usize> {
    match text {
        [b' ' | b'\t' | b'\n' | 0x0b | 0x0c | b'\r', ..] => Some(1),
        [0xc2, 0xa0, ..] => Some(2),
        _ => None,
    }
}

fn skip_blanks(mut text: &[u8]) -> &[u8] {
    while let Some(len) = blank_len(text) {
        text = &text[len..];
    }
    text
}

fn trim_end_blanks(mut text: &[u8]) -> &[u8] {
    loop {
        text = match text {
            [rest @ .., b' ' | b'\t' | b'\n' | 0x0b | 0x0c | b'\r'] => rest,
            [rest @ .., 0xc2, 0xa0] => rest,
            _ => return text,
        };
    }
}

/// The end `StringUtils.trin` trims: characters up to U+0020 and U+00A0.
fn trin_end(mut text: &[u8]) -> &[u8] {
    loop {
        text = match text {
            [rest @ .., last] if *last <= b' ' => rest,
            [rest @ .., 0xc2, 0xa0] => rest,
            _ => return text,
        };
    }
}
