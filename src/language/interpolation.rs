//! Holes in strings whose contents are code, such as `${…}` in a JavaScript
//! template literal. A hole can hold strings, comments and further holes, so
//! the string is suspended while the hole is read.

use std::borrow::Cow;

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
}
