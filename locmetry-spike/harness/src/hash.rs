//! Lexers for `#`-comment languages: Python and POSIX/Bash shell.

use crate::lines::{Class, Region};

fn line_end(b: &[u8], i: usize) -> usize {
    b[i..].iter().position(|c| *c == b'\n').map_or(b.len(), |p| i + p)
}

fn is_word(c: u8) -> bool {
    c == b'_' || c.is_ascii_alphanumeric() || c >= 0x80
}

pub fn python(text: &str) -> Vec<Region> {
    let b = text.as_bytes();
    let mut out = Vec::new();
    let mut i = 0;
    while i < b.len() {
        let c = b[i];
        if c == b'#' {
            let end = line_end(b, i);
            out.push(Region { start: i, end, class: Class::Comment });
            i = end;
            continue;
        }
        if is_word(c) {
            let mut end = i;
            while end < b.len() && is_word(b[end]) {
                end += 1;
            }
            let prefix = text[i..end].to_ascii_lowercase();
            let is_prefix = matches!(prefix.as_str(), "r" | "u" | "b" | "f" | "t" | "rb" | "br" | "fr" | "rf" | "tr" | "rt");
            if is_prefix && matches!(b.get(end), Some(b'"' | b'\'')) {
                let close = python_string(b, end);
                out.push(Region { start: i, end: close, class: Class::Literal });
                i = close;
            } else {
                i = end;
            }
            continue;
        }
        if c == b'"' || c == b'\'' {
            let close = python_string(b, i);
            out.push(Region { start: i, end: close, class: Class::Literal });
            i = close;
            continue;
        }
        i += 1;
    }
    out
}

/// End of the Python string whose opening quote is at `i`. A backslash always
/// protects the next character, also in raw strings, where it stays literal.
fn python_string(b: &[u8], i: usize) -> usize {
    let quote = b[i];
    let triple = b.get(i + 1) == Some(&quote) && b.get(i + 2) == Some(&quote);
    let mut j = i + if triple { 3 } else { 1 };
    while j < b.len() {
        match b[j] {
            b'\\' => j += 2,
            c if c == quote && (!triple || (b.get(j + 1) == Some(&quote) && b.get(j + 2) == Some(&quote))) => {
                return j + if triple { 3 } else { 1 };
            }
            b'\n' if !triple => return j,
            _ => j += 1,
        }
    }
    b.len()
}

pub fn shell(text: &str) -> Vec<Region> {
    let b = text.as_bytes();
    let mut out = Vec::new();
    let mut heredocs: Vec<(Vec<u8>, bool)> = Vec::new();
    let mut i = 0;
    while i < b.len() {
        let c = b[i];
        match c {
            b'\n' => {
                i += 1;
                // Heredoc bodies begin on the line after their redirection.
                for (delimiter, strip_tabs) in std::mem::take(&mut heredocs) {
                    let start = i;
                    loop {
                        if i >= b.len() {
                            break;
                        }
                        let end = line_end(b, i);
                        let mut line = &b[i..end];
                        if strip_tabs {
                            while line.first() == Some(&b'\t') {
                                line = &line[1..];
                            }
                        }
                        if line.strip_suffix(b"\r").unwrap_or(line) == delimiter.as_slice() {
                            if start < i {
                                out.push(Region { start, end: i - 1, class: Class::Literal });
                            }
                            i = (end + 1).min(b.len());
                            break;
                        }
                        i = end + 1;
                    }
                    if i >= b.len() && start < b.len() {
                        out.push(Region { start, end: b.len(), class: Class::Literal });
                    }
                }
            }
            b'#' if i == 0 || matches!(b[i - 1], b' ' | b'\t' | b'\n' | b';' | b'&' | b'|' | b'(' | b')') => {
                let end = line_end(b, i);
                out.push(Region { start: i, end, class: Class::Comment });
                i = end;
            }
            b'\\' => i += 2,
            b'\'' => {
                let end = b[i + 1..].iter().position(|c| *c == b'\'').map_or(b.len(), |p| i + 2 + p);
                out.push(Region { start: i, end, class: Class::Literal });
                i = end;
            }
            b'$' if b.get(i + 1) == Some(&b'\'') => {
                let mut j = i + 2;
                while j < b.len() && b[j] != b'\'' {
                    j += if b[j] == b'\\' { 2 } else { 1 };
                }
                let end = (j + 1).min(b.len());
                out.push(Region { start: i, end, class: Class::Literal });
                i = end;
            }
            b'"' => {
                let end = double_quoted(b, i + 1);
                out.push(Region { start: i, end, class: Class::Literal });
                i = end;
            }
            b'<' if b[i..].starts_with(b"<<<") => i += 3,
            b'<' if b[i..].starts_with(b"<<") => {
                let mut j = i + 2;
                let strip_tabs = b.get(j) == Some(&b'-');
                if strip_tabs {
                    j += 1;
                }
                while matches!(b.get(j), Some(b' ' | b'\t')) {
                    j += 1;
                }
                let mut delimiter = Vec::new();
                while j < b.len() && !matches!(b[j], b' ' | b'\t' | b'\n' | b';' | b'&' | b'|' | b'<' | b'>' | b'(' | b')') {
                    if !matches!(b[j], b'\'' | b'"' | b'\\') {
                        delimiter.push(b[j]);
                    }
                    j += 1;
                }
                if !delimiter.is_empty() {
                    heredocs.push((delimiter, strip_tabs));
                }
                i = j;
            }
            _ => i += 1,
        }
    }
    out
}

/// End of a double-quoted shell string whose body starts at `i`, skipping
/// command and parameter substitutions that may hold their own quotes.
fn double_quoted(b: &[u8], mut i: usize) -> usize {
    while i < b.len() {
        match b[i] {
            b'\\' => i += 2,
            b'"' => return i + 1,
            b'$' if matches!(b.get(i + 1), Some(b'(' | b'{')) => i = substitution(b, i + 1),
            b'`' => i = b[i + 1..].iter().position(|c| *c == b'`').map_or(b.len(), |p| i + 2 + p),
            _ => i += 1,
        }
    }
    b.len()
}

/// End of the `(…)` or `{…}` substitution opening at `i`.
fn substitution(b: &[u8], mut i: usize) -> usize {
    let (open, close) = if b[i] == b'(' { (b'(', b')') } else { (b'{', b'}') };
    let mut depth = 0;
    while i < b.len() {
        match b[i] {
            b'\\' => {
                i += 2;
                continue;
            }
            b'"' => {
                i = double_quoted(b, i + 1);
                continue;
            }
            b'\'' if open == b'(' => {
                i = b[i + 1..].iter().position(|c| *c == b'\'').map_or(b.len(), |p| i + 2 + p);
                continue;
            }
            c if c == open => depth += 1,
            c if c == close => {
                depth -= 1;
                if depth == 0 {
                    return i + 1;
                }
            }
            _ => {}
        }
        i += 1;
    }
    b.len()
}
