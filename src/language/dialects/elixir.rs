//! Elixir's sigils, such as `~r/a/i` or `~S"""`, which close at their first
//! unescaped delimiter, without nesting, and its `?` character literals.

use crate::language::quote_like::{closing_bracket, is_word_byte};
use crate::utils::ext::utf8_len;

/// Length of the Elixir sigil at `lines[i]`: a `~`, one lowercase letter or
/// an uppercase letter followed by uppercase letters and digits, a delimiter
/// of `/`, `|`, `"`, `'`, `(`, `[`, `{` or `<`, or a heredoc's `"""` or `'''`,
/// the text, and modifiers of letters and digits, as in `~r/a/iu8` (Elixir
/// guide, "Sigils"). A backslash escapes the next byte, brackets do not nest,
/// and a lowercase sigil skips the braces of its `#{…}` holes, as Elixir
/// 1.20.4's tokenizer reads them.
pub(crate) fn sigil_len(lines: &[u8], i: usize) -> Option<usize> {
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

/// Length of the Elixir character literal at `lines[i]`, a `?` and one
/// character, or a backslash and one character, as in `?a`, `?#` or `?\"`
/// (Elixir guide, "Binaries, strings, and charlists"). A `?` after an
/// identifier's character ends the identifier, as in `valid?`.
pub(crate) fn character_len(lines: &[u8], i: usize) -> Option<usize> {
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
