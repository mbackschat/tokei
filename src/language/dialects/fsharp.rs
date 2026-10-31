//! F#'s lexer reads strings and character literals inside block comments,
//! so a `*)` or `(*` within one neither closes nor opens a comment
//! (dotnet/fsharp `fbc9be3aa8`, src/Compiler/lex.fsl, rule `comment` and the
//! `stringInComment`, `verbatimStringInComment` and
//! `tripleQuoteStringInComment` rules).

/// Length of the literal starting `window` inside a block comment, when it
/// closes on its line: a `"""` string, an `@"` string with doubled quotes, a
/// `"` string with backslash escapes, or a character literal.
///
/// A string that goes on past its line is left to the comment's own reading,
/// as it is too rare to carry over lines.
pub(crate) fn comment_literal_len(window: &[u8]) -> Option<usize> {
    let line = &window[..memchr::memchr(b'\n', window).unwrap_or(window.len())];
    match line {
        [b'"', b'"', b'"', rest @ ..] => memchr::memmem::find(rest, b"\"\"\"").map(|at| 3 + at + 3),
        [b'@', b'"', rest @ ..] => verbatim_end(rest).map(|end| 2 + end),
        [b'"', rest @ ..] => string_end(rest).map(|end| 1 + end),
        [b'\'', rest @ ..] => character_len(rest).map(|len| 1 + len),
        _ => None,
    }
}

/// Length up to and including the closing quote of an `@"` string's text.
fn verbatim_end(text: &[u8]) -> Option<usize> {
    let mut i = 0;
    while i < text.len() {
        match text[i..] {
            [b'"', b'"', ..] => i += 2,
            [b'"', ..] => return Some(i + 1),
            _ => i += 1,
        }
    }
    None
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

/// Length up to and including the closing `'` of a character literal's
/// text: an escape, or one character other than a backslash, a tab or a
/// backspace (lex.fsl, `char` and `escape_char`). A character outside the
/// Basic Multilingual Plane, four bytes in UTF-8, is two UTF-16 units to the
/// lexer and so no character literal.
fn character_len(text: &[u8]) -> Option<usize> {
    let len = match *text.first()? {
        b'\\' => match *text.get(1)? {
            b'\\' | b'"' | b'\'' | b'a' | b'f' | b'v' | b'n' | b't' | b'b' | b'r' => 2,
            _ => return None,
        },
        b'\t' | 0x08 | b'\r' => return None,
        b if b < 0x80 => 1,
        b if b >= 0xF0 => return None,
        b if b >= 0xE0 => 3,
        _ => 2,
    };
    (text.get(len) == Some(&b'\'')).then_some(len + 1)
}
