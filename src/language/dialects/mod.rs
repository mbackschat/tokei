//! The lexical rules that only one language, or one family of languages,
//! has, kept apart from the engine's shared mechanisms in `syntax.rs`.

pub(crate) mod batch;
pub(crate) mod bitbake;
pub(crate) mod cobol;
pub(crate) mod commonmark;
pub(crate) mod elixir;
pub(crate) mod erlang;
pub(crate) mod fortran;
pub(crate) mod fsharp;
pub(crate) mod haml;
pub(crate) mod haskell;
pub(crate) mod jsx;
pub(crate) mod julia;
pub(crate) mod just;
pub(crate) mod liquid;
pub(crate) mod m4;
pub(crate) mod make;
pub(crate) mod markdown;
pub(crate) mod masm;
pub(crate) mod ocaml;
pub(crate) mod pascal;
pub(crate) mod perl;
pub(crate) mod php;
pub(crate) mod plantuml;
pub(crate) mod rpm;
pub(crate) mod ruby;
pub(crate) mod rust;
pub(crate) mod sql;
pub(crate) mod svelte;
pub(crate) mod visual_basic;
pub(crate) mod yaml;

use std::ops::Range;

use grep_searcher::LineStep;

use crate::language::{language_type::LineReport, syntax::LineClass};
use crate::{CodeStats, Config, LanguageType};

/// The byte ranges of `text`'s lines, each with its line ending.
pub(crate) fn lines(text: &[u8]) -> impl Iterator<Item = Range<usize>> + '_ {
    let mut stepper = LineStep::new(b'\n', 0, text.len());
    std::iter::from_fn(move || stepper.next(text).map(|(start, end)| start..end))
}

/// The end of a string whose text starts at `text[from]`, just past the
/// `close` that ends it, a backslash escaping the byte after it when
/// `escapes`.
pub(crate) fn text_end(text: &[u8], from: usize, close: u8, escapes: bool) -> Option<usize> {
    let mut j = from;
    while let Some(&byte) = text.get(j) {
        if escapes && byte == b'\\' {
            j += 2;
            continue;
        }
        if byte == close {
            return Some(j + 1);
        }
        j += 1;
    }
    None
}

/// The end of a string whose text starts at `text[from]`, just past the `"`
/// that ends it, in which `""` stands for a quote.
pub(crate) fn doubled_quote_end(text: &[u8], from: usize) -> Option<usize> {
    let mut k = from;
    loop {
        k += memchr::memchr(b'"', &text[k..])? + 1;
        if text.get(k) != Some(&b'"') {
            return Some(k);
        }
        k += 1;
    }
}

/// Adds each line's class to `stats` and reports it, for the lines whose
/// start lies within `counted`.
pub(crate) fn report(
    classes: impl IntoIterator<Item = (Range<usize>, LineClass)>,
    mut stats: CodeStats,
    counted: Range<usize>,
    on_line: &mut impl FnMut(LineReport<'_>),
) -> CodeStats {
    for (line, class) in classes {
        if counted.contains(&line.start) {
            stats.add_line(class);
            on_line(LineReport::Class(Some(class)));
        }
    }
    stats
}

/// The classes `language` gives the `count` lines of an embedded `text`, or
/// code for each when it cannot class them line by line.
pub(crate) fn classes_or_code(
    config: &Config,
    language: LanguageType,
    text: &[u8],
    count: usize,
) -> Vec<LineClass> {
    language
        .line_classes(text, config)
        .filter(|classes| classes.len() == count)
        .unwrap_or_else(|| vec![LineClass::Code; count])
}

/// Whose rules tell the strings and character literals that a block comment
/// reads as text, so that a comment delimiter inside one is text too.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum CommentLiterals {
    FSharp,
    OCaml,
}

impl CommentLiterals {
    /// Length of the literal starting `window` inside a block comment.
    pub(crate) fn len(self, window: &[u8]) -> Option<usize> {
        match self {
            CommentLiterals::FSharp => fsharp::comment_literal_len(window),
            CommentLiterals::OCaml => ocaml::comment_literal_len(window),
        }
    }
}
