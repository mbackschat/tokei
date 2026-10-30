//! Svelte's markup expressions, `{…}` in text and attributes, whose code
//! its script's language reads, and the comments a tag may hold between
//! its attributes.

use std::borrow::Cow;

use crate::language::syntax::SyntaxCounter;

impl SyntaxCounter {
    /// Opens the markup expression at `window`, outside literals and
    /// comments, whose contents the expression's language reads up to its
    /// closer (Svelte, "Basic markup", text expressions and attributes).
    pub(crate) fn parse_markup_expression(&mut self, window: &[u8]) -> Option<usize> {
        let expressions = self.shared.markup_expressions?;
        if !self.is_plain_mode()
            || !window.starts_with(expressions.open.as_bytes())
            || expressions.block_closer.is_some()
                && window.get(expressions.open.len()).copied() == expressions.block_closer
        {
            return None;
        }
        trace!("Start markup expression {:?}", expressions.open);
        self.open_hole(
            expressions.close,
            expressions.nest,
            Some(expressions.language),
        );
        Some(expressions.open.len())
    }

    /// Opens the block comment at `lines[i..]` that a tag holds between its
    /// attributes.
    pub(crate) fn parse_tag_block_comment(&mut self, lines: &[u8], i: usize) -> Option<usize> {
        let &(open, close) = self
            .shared
            .tag_multi_line_comments
            .iter()
            .find(|(open, _)| lines[i..].starts_with(open.as_bytes()))?;
        if !may_start_tag_comment(&lines[..i]) {
            return None;
        }
        trace!("Start {:?} in a tag", open);
        self.stack.push(Cow::Borrowed(close));
        Some(open.len())
    }
}

/// Whether a comment may start after `before` in a tag: where an attribute
/// may, after whitespace or another attribute's quoted value or expression,
/// and not as an unquoted value after `=` (Svelte 5.53.0, `read_attribute`
/// in `src/compiler/phases/1-parse/state/element.js`).
pub(crate) fn may_start_tag_comment(before: &[u8]) -> bool {
    before
        .last()
        .is_some_and(|b| b.is_ascii_whitespace() || b"\"'}".contains(b))
        && before.iter().rev().find(|b| !b.is_ascii_whitespace()) != Some(&b'=')
}
