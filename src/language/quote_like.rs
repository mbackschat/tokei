//! Literals whose delimiters the program chooses, which may span lines:
//! Perl's quote-like operators, Ruby's percent literals, Elixir's sigils and
//! Haskell's quasi-quotes, each read by its dialect's rules. For Perl's and
//! Ruby's, a bracket delimiter closes at its partner and nests; any other
//! delimiter closes at its next occurrence; a backslash escapes either.

use super::dialects::{elixir, ruby};

/// Which family of chosen-delimiter literals a language has.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum QuoteLikeSyntax {
    Perl,
    RubyPercent,
    ElixirSigil,
    HaskellQuasiQuote,
}

/// Whose character literals a `?` starts.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum QuestionMarkSyntax {
    Ruby,
    Elixir,
}

impl QuestionMarkSyntax {
    /// Length of the character literal that the `?` at `lines[i]` starts.
    pub(crate) fn character_len(self, lines: &[u8], i: usize) -> Option<usize> {
        match self {
            QuestionMarkSyntax::Ruby => ruby::character_len(lines, i),
            QuestionMarkSyntax::Elixir => elixir::character_len(lines, i),
        }
    }
}

/// The length of the Perl or Ruby literal that starts at `lines[i]` and
/// whose first delimiter is at `lines[open]`, with `parts` delimited parts,
/// up to its closing delimiter and modifiers.
pub(crate) fn delimited_len(lines: &[u8], i: usize, open: usize, parts: usize) -> Option<usize> {
    let mut end = part_end(lines, open)?;
    if parts == 2 {
        end = if is_bracket(lines[open]) {
            let next = open_after_space(lines, end)?;
            part_end(lines, next)?
        } else {
            part_end(lines, end - 1)?
        };
    }
    let modifiers = lines[end..]
        .iter()
        .take_while(|b| b.is_ascii_alphabetic())
        .count();
    Some(end + modifiers - i)
}

/// The end of the part whose delimiter is at `lines[open]`, after its closer.
fn part_end(lines: &[u8], open: usize) -> Option<usize> {
    let delimiter = lines[open];
    let close = closing_bracket(delimiter).unwrap_or(delimiter);
    let mut depth = 0usize;
    let mut j = open + 1;
    while let Some(&byte) = lines.get(j) {
        match byte {
            b'\\' => j += 1,
            b if b == close && depth == 0 => return Some(j + 1),
            b if b == close => depth -= 1,
            b if b == delimiter && close != delimiter => depth += 1,
            _ => {}
        }
        j += 1;
    }
    None
}

/// The position of the delimiter that opens a second part after
/// whitespace, as in `s{a} {b}`.
fn open_after_space(lines: &[u8], from: usize) -> Option<usize> {
    let open = from
        + lines[from..]
            .iter()
            .take_while(|b| b.is_ascii_whitespace())
            .count();
    lines.get(open).filter(|b| b.is_ascii_punctuation())?;
    Some(open)
}

pub(crate) fn closing_bracket(open: u8) -> Option<u8> {
    match open {
        b'(' => Some(b')'),
        b'[' => Some(b']'),
        b'{' => Some(b'}'),
        b'<' => Some(b'>'),
        _ => None,
    }
}

fn is_bracket(byte: u8) -> bool {
    closing_bracket(byte).is_some()
}

pub(crate) fn is_word_byte(byte: u8) -> bool {
    byte.is_ascii_alphanumeric() || byte == b'_'
}
