//! Markdown's fenced code blocks, as a CommonMark parser finds them
//! (pulldown-cmark, CommonMark 0.31.2 §4.5): a fence opens a block only where
//! CommonMark reads one, not in a paragraph or inside another block, `~~~`
//! fences included, and the block's content is the text it holds, without
//! the prefixes of the list items and block quotes around it.

use pulldown_cmark::{CodeBlockKind, Event, Parser, Tag, TagEnd};

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
    let Ok(text) = std::str::from_utf8(text) else {
        return Vec::new();
    };
    let mut blocks = Vec::new();
    let mut open: Option<(usize, String, String)> = None;
    for (event, range) in Parser::new(text).into_offset_iter() {
        match event {
            Event::Start(Tag::CodeBlock(CodeBlockKind::Fenced(info))) => {
                open = Some((range.start, info.into_string(), String::new()));
            }
            Event::Text(content) => {
                if let Some((_, _, text)) = &mut open {
                    text.push_str(&content);
                }
            }
            Event::End(TagEnd::CodeBlock) => {
                let Some((start, info, content)) = open.take() else {
                    continue;
                };
                let line_start = text[..start].rfind('\n').map_or(0, |n| n + 1);
                let last = range.end.saturating_sub(1).max(start);
                let end = text[last..].find('\n').map_or(text.len(), |n| last + n + 1);
                let lines = text[line_start..end].lines().count();
                blocks.push(FencedBlock {
                    line_start,
                    end,
                    fence_lines: lines.saturating_sub(content.lines().count()),
                    info,
                    content,
                });
            }
            _ => {}
        }
    }
    blocks
}
