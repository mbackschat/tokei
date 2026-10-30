//! PostgreSQL's dollar-quoted strings, `$tag$…$tag$` (PostgreSQL 18
//! documentation §4.1.2.4; `src/backend/parser/scan.l`, `dolqdelim`). They
//! mostly hold the bodies of functions and `DO` blocks, which PostgreSQL
//! reads as SQL or PL/pgSQL with the same scanner (`pl_scanner.c` calls
//! `core_yylex`), so a body that lexes as SQL is read as code, and any other
//! body, such as a Perl function's or a fragment of a query, is a literal.

use crate::language::syntax::SyntaxCounter;

/// What a `$` read as code starts.
#[derive(Debug, PartialEq)]
pub(crate) enum DollarQuote {
    /// A delimiter of a body read as SQL, of this length.
    Code(usize),
    /// A dollar-quoted literal, of this length, delimiters included.
    Literal(usize),
}

/// The delimiter `$tag$` that starts `lines[i..]`. A tag starts with a letter,
/// `_` or a non-ASCII byte and continues with digits too, so `$1` is a
/// parameter; a `$` inside an identifier, as in `a$b$c`, starts none, since
/// identifiers may hold `$` (scan.l, `ident_cont`).
pub(crate) fn opener_at(lines: &[u8], i: usize) -> Option<&[u8]> {
    if i > 0 && is_ident_byte(lines[i - 1]) {
        return None;
    }
    delimiter(&lines[i..])
}

/// The delimiter `$tag$` that starts `window`, wherever it stands.
fn delimiter(window: &[u8]) -> Option<&[u8]> {
    let tag = window.strip_prefix(b"$")?;
    if tag.first().is_some_and(u8::is_ascii_digit) {
        return None;
    }
    let len = tag.iter().take_while(|&&b| is_tag_byte(b)).count();
    (tag.get(len) == Some(&b'$')).then(|| &window[..len + 2])
}

/// What the dollar quote opened by `delimiter` at `lines[i..]` is: its body
/// runs to the next occurrence of the delimiter, as scan.l's `xdolq` state
/// reads it. An unterminated one, which PostgreSQL rejects, is `None`.
pub(crate) fn classify(lines: &[u8], i: usize, delimiter: &[u8]) -> Option<DollarQuote> {
    let body_start = i + delimiter.len();
    let body_len = memchr::memmem::find(&lines[body_start..], delimiter)?;
    Some(if lexes_as_sql(&lines[body_start..body_start + body_len]) {
        DollarQuote::Code(delimiter.len())
    } else {
        DollarQuote::Literal(delimiter.len() + body_len + delimiter.len())
    })
}

/// Whether `body` lexes as SQL: every literal, quoted identifier, comment and
/// dollar quote in it closes, and no quoted identifier is empty, which are the
/// scanner errors a function body runs into.
fn lexes_as_sql(body: &[u8]) -> bool {
    let mut i = 0;
    while i < body.len() {
        let rest = &body[i..];
        i += match rest[0] {
            b'-' if rest.starts_with(b"--") => memchr::memchr(b'\n', rest).unwrap_or(rest.len()),
            b'/' if rest.starts_with(b"/*") => match nested_comment_len(rest) {
                Some(len) => len,
                None => return false,
            },
            b'\'' => {
                // An E'…' string escapes with backslashes (scan.l, `xestart`).
                let escapes = i > 0
                    && matches!(body[i - 1], b'e' | b'E')
                    && (i < 2 || !is_ident_byte(body[i - 2]));
                match quoted_len(rest, b'\'', escapes) {
                    Some(len) => len,
                    None => return false,
                }
            }
            b'"' => match quoted_len(rest, b'"', false) {
                Some(2) | None => return false,
                Some(len) => len,
            },
            b'$' => match opener_at(body, i) {
                Some(delimiter) => {
                    let inner = &rest[delimiter.len()..];
                    match memchr::memmem::find(inner, delimiter) {
                        Some(at) => 2 * delimiter.len() + at,
                        None => return false,
                    }
                }
                None => 1,
            },
            _ => 1,
        };
    }
    true
}

/// The length of the literal opened by `quote` at the start of `text`, a
/// doubled quote standing for one, or `None` when it does not close.
fn quoted_len(text: &[u8], quote: u8, escapes: bool) -> Option<usize> {
    let mut i = 1;
    while i < text.len() {
        match text[i] {
            b'\\' if escapes => i += 2,
            b if b == quote && text.get(i + 1) == Some(&quote) => i += 2,
            b if b == quote => return Some(i + 1),
            _ => i += 1,
        }
    }
    None
}

/// The length of the block comment that opens `text`, nested ones included
/// (scan.l, `xc`), or `None` when it does not close.
fn nested_comment_len(text: &[u8]) -> Option<usize> {
    let mut depth = 0;
    let mut i = 0;
    while i + 1 < text.len() {
        match &text[i..i + 2] {
            b"/*" => {
                depth += 1;
                i += 2;
            }
            b"*/" => {
                depth -= 1;
                i += 2;
                if depth == 0 {
                    return Some(i);
                }
            }
            _ => i += 1,
        }
    }
    None
}

fn is_tag_byte(b: u8) -> bool {
    b.is_ascii_alphanumeric() || b == b'_' || b >= 0x80
}

fn is_ident_byte(b: u8) -> bool {
    is_tag_byte(b) || b == b'$'
}

impl SyntaxCounter {
    /// Reads the dollar-quote delimiter at `lines[i..]`, outside literals and
    /// comments: the closer of the innermost body read as SQL, or an opener.
    pub(crate) fn dollar_quote(&mut self, lines: &[u8], i: usize) -> Option<DollarQuote> {
        if !self.shared.dollar_quotes || lines[i] != b'$' || !self.is_plain_mode() {
            return None;
        }
        if let Some(close) = self.dollar_bodies.last() {
            if lines[i..].starts_with(close) {
                let len = close.len();
                self.dollar_bodies.pop();
                return Some(DollarQuote::Code(len));
            }
        }
        let delimiter = opener_at(lines, i)?;
        let quote = classify(lines, i, delimiter)?;
        if matches!(quote, DollarQuote::Code(_)) {
            trace!("Start dollar-quoted body read as SQL");
            self.dollar_bodies.push(delimiter.to_vec());
        }
        Some(quote)
    }
}
