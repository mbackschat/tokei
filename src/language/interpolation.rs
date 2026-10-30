//! Holes in strings whose contents are code, such as `${…}` in a JavaScript
//! template literal. A hole can hold strings, comments and further holes, so
//! the string is suspended while the hole is read.

use std::{borrow::Cow, sync::Arc};

use super::syntax::SharedMatchers;
use crate::LanguageType;

/// A kind of hole in the strings opened by `quote`.
#[derive(Debug)]
pub(crate) struct Interpolation {
    /// The opener of the strings that have these holes, such as "`".
    pub(crate) quote: &'static str,
    pub(crate) open: &'static str,
    /// The closer of a hole, recognized only outside nested brackets.
    pub(crate) close: &'static str,
    /// The bracket that nests inside a hole and is closed by the first byte of
    /// `close`, such as `{` for `${…}` or `(` for Swift's `\(…)`.
    pub(crate) nest: u8,
    /// Texts of the string that would otherwise open a hole, such as C#'s
    /// `{{`, which stands for a brace.
    pub(crate) literal: &'static [&'static str],
    /// Whether an `open` of one repeated byte opens at the end of a longer
    /// run, the bytes before it being text, as C#'s `{{` does in `{{{x}}`
    /// inside `$$"""…"""`, whose innermost braces form the interpolation
    /// (csharplang proposal `raw-string-literal.md`).
    pub(crate) run_opens: bool,
    /// The language whose syntax the hole holds, such as TypeScript in a
    /// Svelte attribute's `{…}`, or `None` for the string's own language.
    pub(crate) language: Option<LanguageType>,
}

/// Expressions in markup, such as Svelte's `{…}`, whose contents another
/// language's syntax reads, in text and tags alike.
#[derive(Clone, Copy, Debug)]
pub(crate) struct MarkupExpressions {
    pub(crate) open: &'static str,
    pub(crate) close: &'static str,
    pub(crate) nest: u8,
    pub(crate) language: LanguageType,
    /// A byte after `open` that makes it the end tag of a block, such as
    /// Svelte's `{/if}`, which holds no expression.
    pub(crate) block_closer: Option<u8>,
}

/// A string suspended while the code of one of its holes is read.
#[derive(Clone, Debug)]
pub(crate) struct OpenHole {
    pub(crate) quote: Option<Cow<'static, str>>,
    pub(crate) quote_opener: &'static str,
    pub(crate) quote_escape: Option<String>,
    pub(crate) quote_is_verbatim: bool,
    pub(crate) close: &'static str,
    pub(crate) nest: u8,
    /// Brackets opened inside the hole and not yet closed.
    pub(crate) depth: usize,
    /// The syntax read around a hole in another language, restored at its
    /// closer, with whether a tag was open.
    pub(crate) host: Option<Arc<SharedMatchers>>,
    pub(crate) in_tag: bool,
}
