//! One lexer for the C-like family: `//` and `/* */` comments, double-quoted
//! strings and character literals, plus per-language literal forms.

use crate::lines::{Class, Region};

#[derive(Clone, Copy, PartialEq, Debug)]
pub enum Lang {
    C,
    Java,
    Go,
    Rust,
    Js,
}

pub fn regions(text: &str, lang: Lang) -> Vec<Region> {
    let b = text.as_bytes();
    let mut out = Vec::new();
    // Open `${` substitutions of JS template literals, with their brace depth.
    let mut templates: Vec<usize> = Vec::new();
    // Whether a `/` here starts a JS regular expression.
    let mut regex_ok = true;
    let mut i = 0;
    let push = |out: &mut Vec<Region>, start, end, class| out.push(Region { start, end, class });
    while i < b.len() {
        let c = b[i];
        if c == b'/' && b.get(i + 1) == Some(&b'/') {
            let end = line_end(b, i);
            push(&mut out, i, end, Class::Comment);
            i = end;
            continue;
        }
        if c == b'/' && b.get(i + 1) == Some(&b'*') {
            let end = block_comment(b, i, lang == Lang::Rust);
            push(&mut out, i, end, Class::Comment);
            i = end;
            continue;
        }
        if c.is_ascii_whitespace() {
            i += 1;
            continue;
        }
        let start = i;
        let mut literal = true;
        match c {
            b'"' if lang == Lang::Java && b[i..].starts_with(b"\"\"\"") => i = quoted(b, i + 3, b"\"\"\"", true, true),
            b'"' => i = quoted(b, i + 1, b"\"", true, lang == Lang::Rust),
            b'\'' if lang == Lang::Js => i = quoted(b, i + 1, b"'", true, false),
            b'\'' if lang == Lang::Rust => match rust_char(b, i) {
                Some(end) => i = end,
                None => {
                    literal = false;
                    i += 1;
                }
            },
            b'\'' if lang == Lang::C && i > 0 && b[i - 1].is_ascii_hexdigit() && b.get(i + 1).is_some_and(u8::is_ascii_hexdigit) => {
                // C23 and C++14 digit separator.
                literal = false;
                i += 1;
            }
            b'\'' => {
                let end = quoted(b, i + 1, b"'", true, false);
                if end > i + 1 && b[end - 1] == b'\'' {
                    i = end;
                } else {
                    literal = false;
                    i += 1;
                }
            }
            b'`' if lang == Lang::Go => i = quoted(b, i + 1, b"`", false, true),
            b'`' if lang == Lang::Js => {
                let (end, open) = template(b, i + 1);
                i = end;
                if open {
                    templates.push(0);
                }
            }
            b'/' if lang == Lang::Js && regex_ok => i = regex(b, i),
            b'}' if lang == Lang::Js && templates.last() == Some(&0) => {
                templates.pop();
                let (end, open) = template(b, i + 1);
                i = end;
                if open {
                    templates.push(0);
                }
            }
            _ if c == b'_' || c == b'$' || c.is_ascii_alphanumeric() || c >= 0x80 => {
                let end = word_end(b, i);
                let word = &text[i..end];
                if lang == Lang::Rust && matches!(word, "r" | "br" | "cr") && raw_start(b, end).is_some() {
                    i = rust_raw(b, end);
                } else if lang == Lang::Rust && matches!(word, "b" | "c") && b.get(end) == Some(&b'"') {
                    i = quoted(b, end + 1, b"\"", true, true);
                } else if lang == Lang::Rust && word == "b" && b.get(end) == Some(&b'\'') {
                    i = rust_char(b, end).unwrap_or(end + 1);
                } else {
                    literal = false;
                    i = end;
                    regex_ok = matches!(
                        word,
                        "return" | "typeof" | "instanceof" | "in" | "of" | "new" | "delete" | "void" | "throw" | "case" | "do" | "else" | "yield" | "await"
                    );
                    continue;
                }
            }
            _ => {
                literal = false;
                if lang == Lang::Js {
                    if c == b'{' {
                        if let Some(depth) = templates.last_mut() {
                            *depth += 1;
                        }
                    } else if c == b'}' {
                        if let Some(depth) = templates.last_mut() {
                            *depth -= 1;
                        }
                    }
                }
                regex_ok = !matches!(c, b')' | b']' | b'}');
                i += 1;
                continue;
            }
        }
        if literal {
            push(&mut out, start, i, Class::Literal);
        }
        regex_ok = false;
    }
    out
}

fn line_end(b: &[u8], i: usize) -> usize {
    b[i..].iter().position(|c| *c == b'\n').map_or(b.len(), |p| i + p)
}

fn block_comment(b: &[u8], start: usize, nested: bool) -> usize {
    let mut depth = 0;
    let mut i = start;
    while i < b.len() {
        if b[i..].starts_with(b"/*") && (nested || depth == 0) {
            depth += 1;
            i += 2;
        } else if b[i..].starts_with(b"*/") {
            depth -= 1;
            i += 2;
            if depth == 0 {
                return i;
            }
        } else {
            i += 1;
        }
    }
    b.len()
}

/// End of a literal whose body starts at `i` and closes with `close`; an
/// unescaped newline ends a single-line literal so a stray quote stays local.
fn quoted(b: &[u8], mut i: usize, close: &[u8], escapes: bool, multiline: bool) -> usize {
    while i < b.len() {
        if escapes && b[i] == b'\\' {
            i += 2;
        } else if b[i..].starts_with(close) {
            return i + close.len();
        } else if b[i] == b'\n' && !multiline {
            return i;
        } else {
            i += 1;
        }
    }
    b.len()
}

/// A Rust character literal at `i`, or `None` for a lifetime or label.
fn rust_char(b: &[u8], i: usize) -> Option<usize> {
    let body = i + 1;
    if b.get(body) == Some(&b'\\') {
        let end = quoted(b, body, b"'", true, false);
        return (b.get(end - 1) == Some(&b'\'')).then_some(end);
    }
    let width = match *b.get(body)? {
        lead if lead < 0x80 => 1,
        lead if lead >= 0xf0 => 4,
        lead if lead >= 0xe0 => 3,
        _ => 2,
    };
    (b.get(body + width) == Some(&b'\'')).then_some(body + width + 1)
}

fn raw_start(b: &[u8], i: usize) -> Option<usize> {
    let hashes = b[i..].iter().take_while(|c| **c == b'#').count();
    (b.get(i + hashes) == Some(&b'"')).then_some(hashes)
}

fn rust_raw(b: &[u8], i: usize) -> usize {
    let hashes = raw_start(b, i).unwrap();
    let mut close = vec![b'"'];
    close.extend(std::iter::repeat_n(b'#', hashes));
    quoted(b, i + hashes + 1, &close, false, true)
}

/// Scans template text from `i`; returns its end and whether it stopped at `${`.
fn template(b: &[u8], mut i: usize) -> (usize, bool) {
    while i < b.len() {
        match b[i] {
            b'\\' => i += 2,
            b'`' => return (i + 1, false),
            b'$' if b.get(i + 1) == Some(&b'{') => return (i + 2, true),
            _ => i += 1,
        }
    }
    (b.len(), false)
}

fn regex(b: &[u8], mut i: usize) -> usize {
    i += 1;
    let mut class = false;
    while i < b.len() {
        match b[i] {
            b'\\' => i += 1,
            b'[' => class = true,
            b']' => class = false,
            b'/' if !class => {
                i += 1;
                while i < b.len() && b[i].is_ascii_alphabetic() {
                    i += 1;
                }
                return i;
            }
            b'\n' => return i,
            _ => {}
        }
        i += 1;
    }
    b.len()
}

fn word_end(b: &[u8], mut i: usize) -> usize {
    while i < b.len() && (b[i] == b'_' || b[i] == b'$' || b[i].is_ascii_alphanumeric() || b[i] >= 0x80) {
        i += 1;
    }
    i
}
