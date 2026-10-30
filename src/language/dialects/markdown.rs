//! Markdown's fenced code blocks, as CommonMark's block structure finds them
//! (CommonMark 0.31.2 §4.5, read by `commonmark.rs`): a fence opens a block only where
//! CommonMark reads one, not in a paragraph or inside another block, `~~~`
//! fences included, and the block's content is the text it holds, without
//! the prefixes of the list items and block quotes around it.

use super::commonmark;
use crate::language::syntax::{FileContext, LanguageContext, SyntaxCounter};
use crate::{Config, LanguageType};

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

/// The fenced code blocks of `text`, in order; none when it is not UTF-8.
pub(crate) fn fenced_blocks(text: &[u8]) -> Vec<FencedBlock> {
    std::str::from_utf8(text).map_or_else(|_| Vec::new(), commonmark::fenced_blocks)
}

impl SyntaxCounter {
    /// The Markdown code block whose opening fence is on the line at `start`,
    /// when the fork names its language: the first word of its info string,
    /// or the first of that word's comma-separated parts, that names one.
    /// Blocks in other languages are text.
    pub(crate) fn parse_fenced_block(
        &mut self,
        lines: &[u8],
        start: usize,
        config: &Config,
    ) -> Option<FileContext> {
        if !matches!(
            self.shared.language,
            LanguageType::Markdown | LanguageType::UnrealDeveloperMarkdown
        ) {
            return None;
        }
        use std::str::FromStr;

        let blocks = self
            .fenced_blocks
            .get_or_insert_with(|| fenced_blocks(lines));
        // `start` follows the line's indentation, which the opening fence of
        // a block in a list item has.
        let at = blocks.partition_point(|block| block.line_start <= start);
        let block = blocks[..at].last().filter(|block| {
            memchr::memchr(b'\n', &lines[block.line_start..])
                .map_or(true, |n| start < block.line_start + n)
        })?;
        let language = block
            .info
            .split_whitespace()
            .next()?
            .split(',')
            .find_map(|part| LanguageType::from_str(part).ok())?;
        trace!("{} block: {:?}", language, block.content);
        Some(FileContext::new(
            LanguageContext::Markdown {
                balanced: block.fence_lines == 2,
                language,
            },
            block.end,
            language.parse_from_str(&block.content, config),
        ))
    }
}
