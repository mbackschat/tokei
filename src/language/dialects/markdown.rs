//! Markdown's fenced code blocks, as CommonMark's block structure finds them
//! (CommonMark 0.31.2 §4.5, read by `commonmark.rs`): a fence opens a block only where
//! CommonMark reads one, not in a paragraph or inside another block, `~~~`
//! fences included, and the block's content is the text it holds, without
//! the prefixes of the list items and block quotes around it.

use std::ops::Range;
use std::str::FromStr;

use grep_searcher::LineStep;

use super::commonmark;
use crate::language::{language_type::LineReport, syntax::LineClass};
use crate::{utils::ext::SliceExt, CodeStats, Config, LanguageType};

#[derive(Clone, Debug)]
pub(crate) struct FencedBlock {
    /// The start of the line of the opening fence.
    pub(crate) line_start: usize,
    /// The end of the block's last line, after its line break.
    pub(crate) end: usize,
    /// The lines of its fences: 2, or 1 when the document or the block's
    /// container ends before a closing fence.
    pub(crate) fence_lines: usize,
    pub(crate) info: String,
    pub(crate) content: String,
}

impl FencedBlock {
    /// The language the fork names for the block: the first word of its info
    /// string, or the first of that word's comma-separated parts, that names
    /// one. Blocks in other languages are text.
    fn language(&self) -> Option<LanguageType> {
        self.info
            .split_whitespace()
            .next()?
            .split(',')
            .find_map(|part| LanguageType::from_str(part).ok())
    }
}

/// The fenced code blocks of `text`, in order; none when it is not UTF-8.
/// A text without three backticks or tildes in a row holds no fence
/// (CommonMark 0.31.2 §4.5), so the block pass, most of Markdown's cost, is
/// skipped for it.
pub(crate) fn fenced_blocks(text: &[u8]) -> Vec<FencedBlock> {
    if memchr::memmem::find(text, b"```").is_none() && memchr::memmem::find(text, b"~~~").is_none()
    {
        return Vec::new();
    }
    std::str::from_utf8(text).map_or_else(|_| Vec::new(), commonmark::fenced_blocks)
}

/// Counts the lines of the Markdown `text` that start within `counted`, as
/// `LanguageType::parse_lines` does for other languages: the fences of a
/// block in a language the fork names are comments and its content counts as
/// that language; every other line is text, a comment, or blank where only
/// whitespace. Markdown holds no comments or literals of its own, so its
/// lines are classified straight from the block list rather than through the
/// generic line loop, whose per-line analysis made Markdown slower than
/// upstream Tokei (fork speed gate on docs/content, 1.06 to above 1.10,
/// measured 2026-10-30).
pub(crate) fn parse(
    config: &Config,
    text: &[u8],
    mut stats: CodeStats,
    counted: Range<usize>,
    on_line: &mut impl FnMut(LineReport<'_>),
) -> CodeStats {
    let blocks = fenced_blocks(text);
    let mut next_block = 0;
    let mut stepper = LineStep::new(b'\n', 0, text.len());
    while let Some((start, end)) = stepper.next(text) {
        while blocks
            .get(next_block)
            .is_some_and(|block| block.line_start < start)
        {
            next_block += 1;
        }
        // A block's opening fence lies on this line; with `\r` alone ending
        // lines, which CommonMark reads as line endings, it may lie within it.
        let block = blocks
            .get(next_block)
            .filter(|block| block.line_start < end)
            .and_then(|block| Some((block, block.language()?)));
        let is_counted = counted.contains(&start);
        if let Some((block, language)) = block {
            if is_counted {
                stats.comments += block.fence_lines;
                *stats.blobs.entry(language).or_default() +=
                    language.parse_from_str(&block.content, config);
                on_line(LineReport::Class(None));
            }
            stepper = LineStep::new(b'\n', block.end, text.len());
            continue;
        }
        if !is_counted {
            continue;
        }
        let class = if text[start..end].trim().is_empty() {
            stats.blanks += 1;
            LineClass::Blank
        } else {
            stats.comments += 1;
            LineClass::Comment
        };
        on_line(LineReport::Class(Some(class)));
    }
    stats
}
