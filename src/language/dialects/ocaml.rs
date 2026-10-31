//! OCaml's lexer reads strings, quoted strings and character literals inside
//! comments, so a `*)` or `(*` within one neither closes nor opens a comment
//! (ocaml/ocaml `a76fb1de94`, parsing/lexer.mll, rule `comment`).

/// Length of the literal starting `window` inside a comment, when it closes
/// on its line: a `"` string with backslash escapes, a quoted string such as
/// `{id|…|id}` or `{%ext id|…|id}`, `''`, or a character literal of one
/// byte or one escape.
///
/// A literal that goes on past its line is left to the comment's own
/// reading, as it is too rare to carry over lines.
pub(crate) fn comment_literal_len(window: &[u8]) -> Option<usize> {
    let line = &window[..memchr::memchr(b'\n', window).unwrap_or(window.len())];
    match line {
        [b'"', rest @ ..] => string_end(rest).map(|end| 1 + end),
        [b'{', rest @ ..] => quoted_string_len(rest).map(|len| 1 + len),
        [b'\'', b'\'', ..] => Some(2),
        [b'\'', rest @ ..] => character_len(rest).map(|len| 1 + len),
        _ => None,
    }
}

/// Length up to and including the closing quote of a `"` string's text.
fn string_end(text: &[u8]) -> Option<usize> {
    let mut i = 0;
    while i < text.len() {
        match text[i] {
            b'\\' => i += 2,
            b'"' => return Some(i + 1),
            _ => i += 1,
        }
    }
    None
}

/// Length of a quoted string after its `{`: an optional `%` or `%%`
/// extension name and blanks, a delimiter of lowercase letters and `_`, `|`,
/// the text, and `|`, the delimiter and `}`.
fn quoted_string_len(text: &[u8]) -> Option<usize> {
    let mut i = 0;
    if text.first() == Some(&b'%') {
        i += 1 + usize::from(text.get(1) == Some(&b'%'));
        let name = text[i..]
            .iter()
            .take_while(|b| b.is_ascii_alphanumeric() || matches!(b, b'_' | b'\'' | b'.'))
            .count();
        if name == 0 {
            return None;
        }
        i += name;
        i += text[i..]
            .iter()
            .take_while(|b| matches!(b, b' ' | b'\t' | 0x0C))
            .count();
    }
    let delimiter = text[i..]
        .iter()
        .take_while(|b| b.is_ascii_lowercase() || **b == b'_')
        .count();
    let delimiter = &text[i..i + delimiter];
    i += delimiter.len();
    if text.get(i) != Some(&b'|') {
        return None;
    }
    let closer = [b"|", delimiter, b"}"].concat();
    let body = &text[i + 1..];
    memchr::memmem::find(body, &closer).map(|at| i + 1 + at + closer.len())
}

/// Length up to and including the closing `'` of a character literal's
/// text: one byte other than a backslash, a quote or a line break, or an
/// escape of a listed character, three decimal digits, `o` and three octal
/// digits or `x` and two hexadecimal digits.
fn character_len(text: &[u8]) -> Option<usize> {
    let len = match text {
        [b'\\', b'\\' | b'"' | b'\'' | b'n' | b't' | b'b' | b'r' | b' ', ..] => 2,
        [b'\\', b'0'..=b'9', b'0'..=b'9', b'0'..=b'9', ..] => 4,
        [b'\\', b'o', b'0'..=b'3', b'0'..=b'7', b'0'..=b'7', ..] => 5,
        [b'\\', b'x', a, b, ..] if a.is_ascii_hexdigit() && b.is_ascii_hexdigit() => 4,
        [b'\\' | b'\'' | b'\n' | b'\r', ..] | [] => return None,
        _ => 1,
    };
    (text.get(len) == Some(&b'\'')).then_some(len + 1)
}
