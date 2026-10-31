//! Erlang's character literals, `$` and one character or escape sequence,
//! as in `$%` or `$\"` (Erlang Reference Manual, "Data Types", Escape
//! Sequences), and its sigils, as `erl_scan` reads them (OTP 29.1.2,
//! stdlib's erl_scan.erl: `scan_sigil_prefix`, `string_right_delimiter`,
//! `scan_qstring` and `scan_vstring`).

use crate::language::quote_like::closing_bracket;
use crate::utils::ext::utf8_len;

/// Length of the character literal at `lines[i]`. Of an escape, only the
/// characters that could start a comment or literal matter: the one after
/// the backslash, and after `\^` the control character's.
pub(crate) fn character_len(lines: &[u8], i: usize) -> Option<usize> {
    if lines[i] != b'$' {
        return None;
    }
    let escape = match (lines.get(i + 1), lines.get(i + 2)) {
        (Some(b'\\'), Some(b'^')) => 2,
        (Some(b'\\'), _) => 1,
        _ => 0,
    };
    Some(1 + escape + utf8_len(*lines.get(i + 1 + escape)?))
}

/// Length of the sigil at `lines[i]`: `~`, a name, the text between a
/// delimiter and its partner, or between `"""` and the same quotes starting a
/// later line, and a suffix name, as in `~B"\\"` or `~s(a % b)`. Brackets
/// pair and do not nest; only the vanilla `~`, `~b` and `~s` sigils take
/// backslash escapes, the others being verbatim.
pub(crate) fn sigil_len(lines: &[u8], i: usize) -> Option<usize> {
    let name = &lines[i..].strip_prefix(b"~")?;
    let name = &name[..name.iter().take_while(|b| is_name_byte(**b)).count()];
    let open = i + 1 + name.len();
    let end = if lines[open..].starts_with(b"\"\"\"") {
        triple_quoted_end(lines, open)?
    } else {
        let close = match *lines.get(open)? {
            delimiter @ (b'/' | b'|' | b'#' | b'`' | b'\'' | b'"') => delimiter,
            delimiter => closing_bracket(delimiter)?,
        };
        let escapes = matches!(name, b"" | b"b" | b"s");
        text_end(lines, open + 1, close, escapes)?
    };
    let suffix = lines[end..]
        .iter()
        .take_while(|b| is_name_byte(**b))
        .count();
    Some(end + suffix - i)
}

/// Whether `byte` may be part of a sigil's name or suffix: a letter, digit,
/// `_` or `@`, or a byte of a non-ASCII letter (erl_scan.erl, `scan_name`).
fn is_name_byte(byte: u8) -> bool {
    byte.is_ascii_alphanumeric() || matches!(byte, b'_' | b'@') || byte >= 0x80
}

/// The end of a sigil's text from `lines[from]`, after its `close`.
fn text_end(lines: &[u8], from: usize, close: u8, escapes: bool) -> Option<usize> {
    let mut j = from;
    while let Some(&byte) = lines.get(j) {
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

/// The end of the triple-quoted text whose quotes start at `lines[open]`:
/// after the same run of quotes starting a later line, past its indentation
/// (EEP 64).
fn triple_quoted_end(lines: &[u8], open: usize) -> Option<usize> {
    let quotes = lines[open..].iter().take_while(|b| **b == b'"').count();
    let quote = &lines[open..open + quotes];
    let mut line = open + memchr::memchr(b'\n', &lines[open..])? + 1;
    loop {
        let text = &lines[line..];
        let indent = text
            .iter()
            .take_while(|b| matches!(b, b' ' | b'\t'))
            .count();
        if text[indent..].starts_with(quote) {
            return Some(line + indent + quotes);
        }
        line += memchr::memchr(b'\n', text)? + 1;
    }
}
