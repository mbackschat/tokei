//! just 1.59.0's lexer, `src/lexer.rs` with its tests left out, vendored
//! verbatim from https://github.com/casey/just (CC0-1.0) with the token and
//! string kinds it reads, since just exports no lexer; the error, keyword
//! and path types of the rest of just are stand-ins of the same shape.

#![allow(dead_code)]

pub(crate) use regex::Regex;
pub(crate) use std::{
    fmt::{self, Display, Formatter},
    str::Chars,
    sync::LazyLock,
};

mod delimiter;
mod lexer;
mod position;
mod string_delimiter;
mod string_kind;
mod token_kind;

pub(crate) use {
    delimiter::Delimiter, lexer::Lexer, position::Position, string_delimiter::StringDelimiter,
    string_kind::StringKind, token_kind::TokenKind,
};

pub(crate) type Utf8Path = std::path::Path;

pub(crate) type CompileResult<'a, T = ()> = Result<T, CompileError<'a>>;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Ord, PartialOrd)]
pub(crate) struct Token<'src> {
    pub(crate) column: usize,
    pub(crate) kind: TokenKind,
    pub(crate) length: usize,
    pub(crate) line: usize,
    pub(crate) offset: usize,
    pub(crate) path: &'src Utf8Path,
    pub(crate) src: &'src str,
}

impl<'src> Token<'src> {
    pub(crate) fn lexeme(&self) -> &'src str {
        &self.src[self.offset..self.offset + self.length]
    }

    pub(crate) fn error(&self, kind: CompileErrorKind<'src>) -> CompileError<'src> {
        CompileError::new(*self, kind)
    }
}

#[derive(Debug)]
pub(crate) struct CompileError<'src> {
    pub(crate) token: Token<'src>,
    pub(crate) kind: CompileErrorKind<'src>,
}

impl<'src> CompileError<'src> {
    pub(crate) fn new(token: Token<'src>, kind: CompileErrorKind<'src>) -> Self {
        Self { token, kind }
    }
}

/// The variants of just's `CompileErrorKind` that the lexer raises.
#[derive(Debug)]
pub(crate) enum CompileErrorKind<'src> {
    EscapeEndOfFile,
    InconsistentLeadingWhitespace { expected: &'src str, found: &'src str },
    Internal { message: String },
    InvalidEscapeSequence { character: char },
    MismatchedClosingDelimiter { close: Delimiter, open: Delimiter, open_line: usize },
    MixedLeadingWhitespace { whitespace: &'src str },
    UnexpectedCharacter { expected: Vec<char> },
    UnexpectedClosingDelimiter { close: Delimiter },
    UnexpectedEndOfToken { expected: Vec<char> },
    UnknownStartOfToken { start: char },
    UnpairedCarriageReturn,
    UnterminatedBacktick,
    UnterminatedInterpolation,
    UnterminatedString,
}

/// The one keyword the lexer reads, the prefix of format strings.
pub(crate) enum Keyword {
    F,
}

impl Keyword {
    pub(crate) fn lexeme(self) -> &'static str {
        match self {
            Keyword::F => "f",
        }
    }
}

pub(crate) fn lex<'src>(path: &'src Utf8Path, src: &'src str) -> CompileResult<'src, Vec<Token<'src>>> {
    Lexer::lex(path, src)
}
