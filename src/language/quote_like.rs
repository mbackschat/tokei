//! Literals whose delimiters the program chooses: Perl's quote-like
//! operators, such as `qw(a b)` and `s{a}{b}g` (perlop, "Quote and Quote-like
//! Operators"), and Ruby's percent literals, such as `%w[a b]` (Ruby
//! documentation, "Literals", Percent Literals). A bracket delimiter closes at
//! its partner and nests; any other delimiter closes at its next occurrence;
//! a backslash escapes either. The literal may span lines.

use super::regex_literal::RegexContext;
use crate::utils::ext::SliceExt;

/// Which family of chosen-delimiter literals a language has.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum QuoteLikeSyntax {
    Perl,
    RubyPercent,
}

/// The length of the literal that starts at `lines[i]`, up to its closing
/// delimiter and modifiers, or `None` when none starts there.
pub(crate) fn quote_like_len(
    lines: &[u8],
    i: usize,
    syntax: QuoteLikeSyntax,
    context: Option<&RegexContext>,
) -> Option<usize> {
    let (open, parts) = match syntax {
        QuoteLikeSyntax::Perl => perl_operator(lines, i)?,
        QuoteLikeSyntax::RubyPercent => ruby_percent(lines, i, context?)?,
    };
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

/// The position of the opening delimiter and the number of delimited parts
/// of the Perl operator at `lines[i]`: one for `q`, `qq`, `qw`, `qx`, `qr`
/// and `m`, two for `s`, `tr` and `y`. A word that is a hash key or method
/// name, as in `$h{s}`, `->q(` or `y => 1`, is not an operator.
fn perl_operator(lines: &[u8], i: usize) -> Option<(usize, usize)> {
    let before = &lines[..i];
    if before
        .last()
        .is_some_and(|b| is_word_byte(*b) || matches!(b, b'$' | b'@' | b'%' | b'&' | b'*' | b'-'))
        || before.ends_with(b"->")
        || before.ends_with(b"::")
        || before.trim_end().ends_with(b"sub")
    {
        return None;
    }
    let word_len = lines[i..].iter().take_while(|b| is_word_byte(**b)).count();
    let parts = match &lines[i..i + word_len] {
        b"q" | b"qq" | b"qw" | b"qx" | b"qr" | b"m" => 1,
        b"s" | b"tr" | b"y" => 2,
        _ => return None,
    };
    let spaces = lines[i + word_len..]
        .iter()
        .take_while(|b| matches!(b, b' ' | b'\t'))
        .count();
    let open = i + word_len + spaces;
    let delimiter = *lines.get(open)?;
    let opens = delimiter.is_ascii_punctuation()
        && !matches!(
            delimiter,
            b'=' | b',' | b';' | b')' | b']' | b'}' | b'>' | b'$' | b'@'
        )
        && !(delimiter == b'#' && spaces > 0);
    opens.then_some((open, parts))
}

/// The position of the opening delimiter of the Ruby percent literal at
/// `lines[i]`, such as `%w[`, `%r{` or `%(`, which is one part. A typed
/// literal starts where an expression may, or after a space as a method's
/// argument; a bare `%(` only where an expression may, since it is the
/// modulo operator after a value.
fn ruby_percent(lines: &[u8], i: usize, context: &RegexContext) -> Option<(usize, usize)> {
    if lines[i] != b'%' {
        return None;
    }
    let next = *lines.get(i + 1)?;
    let typed = b"qQwWiIrsx".contains(&next);
    let open = if typed { i + 2 } else { i + 1 };
    let delimiter = *lines.get(open)?;
    // After `:` the `%` is a symbol, as in `method?(:%)`, and a closing
    // bracket cannot open a literal.
    if !delimiter.is_ascii_punctuation()
        || matches!(delimiter, b'=' | b')' | b']' | b'}' | b'>')
        || lines[..i].last() == Some(&b':')
    {
        return None;
    }
    let after_space = lines[..i].last().is_some_and(|b| *b == b' ' || *b == b'\t');
    let starts = context.expression_may_start(lines, i) || (typed && after_space);
    starts.then_some((open, 1))
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

fn closing_bracket(open: u8) -> Option<u8> {
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

fn is_word_byte(byte: u8) -> bool {
    byte.is_ascii_alphanumeric() || byte == b'_'
}
