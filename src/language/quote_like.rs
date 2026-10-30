//! Literals whose delimiters the program chooses: Perl's quote-like
//! operators, such as `qw(a b)` and `s{a}{b}g` (perlop, "Quote and Quote-like
//! Operators"), and Ruby's percent literals, such as `%w[a b]` (Ruby
//! documentation, "Literals", Percent Literals). A bracket delimiter closes at
//! its partner and nests; any other delimiter closes at its next occurrence;
//! a backslash escapes either. The literal may span lines. Elixir's sigils,
//! such as `~r/a/i` or `~S"""`, close at their first unescaped delimiter,
//! without nesting.

use super::regex_literal::{starts_command_argument, RegexContext};
use crate::utils::ext::{utf8_len, SliceExt};

/// Which family of chosen-delimiter literals a language has.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum QuoteLikeSyntax {
    Perl,
    RubyPercent,
    ElixirSigil,
}

/// Whose character literals a `?` starts.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum QuestionMarkSyntax {
    Ruby,
    Elixir,
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
        QuoteLikeSyntax::ElixirSigil => return elixir_sigil_len(lines, i),
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
    let starts = context.expression_may_start(lines, i)
        || (typed && after_space)
        || starts_command_argument(lines, i);
    starts.then_some((open, 1))
}

/// Length of the Elixir sigil at `lines[i]`: a `~`, one lowercase letter or
/// an uppercase letter followed by uppercase letters and digits, a delimiter
/// of `/`, `|`, `"`, `'`, `(`, `[`, `{` or `<`, or a heredoc's `"""` or `'''`,
/// the text, and modifiers of letters and digits, as in `~r/a/iu8` (Elixir
/// guide, "Sigils"). A backslash escapes the next byte, brackets do not nest,
/// and a lowercase sigil skips the braces of its `#{…}` holes, as Elixir
/// 1.20.4's tokenizer reads them.
fn elixir_sigil_len(lines: &[u8], i: usize) -> Option<usize> {
    let rest = lines[i..].strip_prefix(b"~")?;
    let name = match *rest.first()? {
        b'a'..=b'z' => 1,
        b'A'..=b'Z' => rest
            .iter()
            .take_while(|b| b.is_ascii_uppercase() || b.is_ascii_digit())
            .count(),
        _ => return None,
    };
    let interpolates = rest[0].is_ascii_lowercase();
    let open = i + 1 + name;
    let body = &lines[open..];
    let end = if body.starts_with(br#"""""#) || body.starts_with(b"'''") {
        heredoc_end(lines, open)?
    } else {
        let close = match *body.first()? {
            delimiter @ (b'/' | b'|' | b'"' | b'\'') => delimiter,
            delimiter => closing_bracket(delimiter)?,
        };
        sigil_text_end(lines, open + 1, close, interpolates)?
    };
    let modifiers = lines[end..]
        .iter()
        .take_while(|b| b.is_ascii_alphanumeric())
        .count();
    Some(end + modifiers - i)
}

/// The end of a sigil's text from `lines[from]`, after its `close`.
fn sigil_text_end(lines: &[u8], from: usize, close: u8, interpolates: bool) -> Option<usize> {
    let mut j = from;
    while let Some(&byte) = lines.get(j) {
        match byte {
            b'\\' => j += 1,
            b'#' if interpolates && lines.get(j + 1) == Some(&b'{') => {
                j = hole_end(lines, j + 2)?;
                continue;
            }
            byte if byte == close => return Some(j + 1),
            _ => {}
        }
        j += 1;
    }
    None
}

/// The end of the `#{…}` hole whose code starts at `lines[from]`, after the
/// brace that closes it.
fn hole_end(lines: &[u8], from: usize) -> Option<usize> {
    let mut depth = 0usize;
    for (j, &byte) in lines.iter().enumerate().skip(from) {
        match byte {
            b'{' => depth += 1,
            b'}' if depth == 0 => return Some(j + 1),
            b'}' => depth -= 1,
            _ => {}
        }
    }
    None
}

/// The end of the heredoc whose three quotes are at `lines[open]`: after the
/// same three quotes starting a later line, past its indentation.
fn heredoc_end(lines: &[u8], open: usize) -> Option<usize> {
    let quote = &lines[open..open + 3];
    let mut line = open + memchr::memchr(b'\n', &lines[open..])? + 1;
    loop {
        let text = &lines[line..];
        let indent = text
            .iter()
            .take_while(|b| matches!(b, b' ' | b'\t'))
            .count();
        if text[indent..].starts_with(quote) {
            return Some(line + indent + quote.len());
        }
        line += memchr::memchr(b'\n', text)? + 1;
    }
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

/// Length of the character literal that the `?` at `lines[i]` starts.
pub(crate) fn question_mark_character_len(
    lines: &[u8],
    i: usize,
    syntax: QuestionMarkSyntax,
) -> Option<usize> {
    match syntax {
        QuestionMarkSyntax::Ruby => ruby_character_len(lines, i),
        QuestionMarkSyntax::Elixir => elixir_character_len(lines, i),
    }
}

/// Length of the Elixir character literal at `lines[i]`, a `?` and one
/// character, or a backslash and one character, as in `?a`, `?#` or `?\"`
/// (Elixir guide, "Binaries, strings, and charlists"). A `?` after an
/// identifier's character ends the identifier, as in `valid?`.
fn elixir_character_len(lines: &[u8], i: usize) -> Option<usize> {
    if lines[i] != b'?'
        || lines[..i]
            .last()
            .is_some_and(|b| is_word_byte(*b) || *b >= 0x80)
    {
        return None;
    }
    let escape = usize::from(*lines.get(i + 1)? == b'\\');
    Some(1 + escape + utf8_len(*lines.get(i + 1 + escape)?))
}

/// Length of the Ruby character literal at `lines[i]`, a `?` and one
/// character or escape sequence, as in `?a`, `?/` or `?\n` (Ruby
/// `doc/syntax/literals.rdoc`, "Character Literal"). A `?` that ends a name,
/// such as `include?` or the global `$?`, or follows a closing bracket, or
/// comes before whitespace is an operator or part of a name; so is one before
/// two identifier characters, as in `a ?bc : d` (parse.y, `parser_yylex`).
fn ruby_character_len(lines: &[u8], i: usize) -> Option<usize> {
    if lines[i] != b'?'
        || lines[..i]
            .last()
            .is_some_and(|b| is_word_byte(*b) || matches!(b, b'$' | b')' | b']' | b'}'))
    {
        return None;
    }
    let body = &lines[i + 1..];
    let first = *body.first()?;
    if first.is_ascii_whitespace()
        || (is_word_byte(first) && body.get(1).is_some_and(|b| is_word_byte(*b)))
    {
        return None;
    }
    let escape = usize::from(first == b'\\');
    Some(1 + escape + utf8_len(*body.get(escape)?))
}
