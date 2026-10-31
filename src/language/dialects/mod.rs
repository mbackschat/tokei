//! The lexical rules that only one language, or one family of languages,
//! has, kept apart from the engine's shared mechanisms in `syntax.rs`.

pub(crate) mod batch;
pub(crate) mod commonmark;
pub(crate) mod elixir;
pub(crate) mod erlang;
pub(crate) mod fsharp;
pub(crate) mod haskell;
pub(crate) mod jsx;
pub(crate) mod julia;
pub(crate) mod m4;
pub(crate) mod make;
pub(crate) mod markdown;
pub(crate) mod perl;
pub(crate) mod php;
pub(crate) mod rpm;
pub(crate) mod ruby;
pub(crate) mod rust;
pub(crate) mod sql;
pub(crate) mod svelte;
pub(crate) mod visual_basic;
pub(crate) mod yaml;

/// Whose rules tell the strings and character literals that a block comment
/// reads as text, so that a comment delimiter inside one is text too.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum CommentLiterals {
    FSharp,
}

impl CommentLiterals {
    /// Length of the literal starting `window` inside a block comment.
    pub(crate) fn len(self, window: &[u8]) -> Option<usize> {
        match self {
            CommentLiterals::FSharp => fsharp::comment_literal_len(window),
        }
    }
}
