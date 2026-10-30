//! Haskell's pragmas that GHC reads as code although they open like a block
//! comment, such as `{-# INLINE f #-}`, while it reads other pragmas, such as
//! `{-# LANGUAGE CPP #-}`, as comments; and its quasi-quotes, such as
//! `[r|a|]`, which close at their first `|]`.

use crate::language::quote_like::QuoteLikeSyntax;
use crate::language::syntax::{starts_line, SyntaxCounter};
use crate::utils::ext::utf8_len;

/// A block comment opener and the pragma words after it that make it code.
#[derive(Clone, Copy, Debug)]
pub(crate) struct CodePragmas {
    pub(crate) open: &'static str,
    pub(crate) words: &'static [&'static str],
}

impl CodePragmas {
    /// Length of the opener that starts `window` when the pragma word after
    /// it, in any case and after any whitespace, line breaks and Unicode
    /// spaces such as U+00A0 included, is one of `words`; a word runs over
    /// letters, digits and underscores, as GHC's `$whitechar` and
    /// `pragmachar` do (GHC 9.14.1, `GHC.Parser.Lexer`). The rest of the
    /// pragma reads as code, so only the opener is skipped, which keeps the
    /// skip within its line.
    pub(crate) fn code_len(self, window: &[u8]) -> Option<usize> {
        let rest = window.strip_prefix(self.open.as_bytes())?;
        let mut space = 0;
        while let Some(len) = rest.get(space).map(|&lead| utf8_len(lead)) {
            match rest
                .get(space..space + len)
                .and_then(|text| std::str::from_utf8(text).ok())
                .and_then(|text| text.chars().next())
            {
                Some(c) if c.is_whitespace() => space += len,
                _ => break,
            }
        }
        let word = &rest[space..];
        let len = word
            .iter()
            .take_while(|b| b.is_ascii_alphanumeric() || **b == b'_')
            .count();
        self.words
            .iter()
            .any(|w| w.as_bytes().eq_ignore_ascii_case(&word[..len]))
            .then_some(self.open.len())
    }
}

/// The bracket quotes that a Haskell file's LANGUAGE pragmas turn on, as
/// GHC reads them only with these extensions: without `QuasiQuotes`,
/// `[x|x<-xs]` is a list comprehension.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub(crate) struct QuasiQuoting {
    quasi_quotes: bool,
    template_quotes: bool,
}

impl QuasiQuoting {
    /// The quotes that `text`'s LANGUAGE pragmas turn on, a later pragma
    /// overriding an earlier one; `TemplateHaskell` turns on the quotes of
    /// `TemplateHaskellQuotes` (GHC User's Guide, "Template Haskell").
    pub(crate) fn of(text: &[u8]) -> Self {
        let mut quoting = Self::default();
        for start in memchr::memmem::find_iter(text, b"{-#") {
            let body = &text[start + 3..];
            let body = &body[..memchr::memmem::find(body, b"#-}").unwrap_or(body.len())];
            let mut words = body
                .split(|b| b.is_ascii_whitespace() || *b == b',')
                .filter(|word| !word.is_empty());
            if !words
                .next()
                .is_some_and(|word| word.eq_ignore_ascii_case(b"language"))
            {
                continue;
            }
            for word in words {
                match word {
                    b"QuasiQuotes" => quoting.quasi_quotes = true,
                    b"NoQuasiQuotes" => quoting.quasi_quotes = false,
                    b"TemplateHaskell" | b"TemplateHaskellQuotes" => quoting.template_quotes = true,
                    b"NoTemplateHaskell" | b"NoTemplateHaskellQuotes" => {
                        quoting.template_quotes = false
                    }
                    _ => {}
                }
            }
        }
        quoting
    }

    /// Whether any `[quoter|` opens a quasi-quote.
    pub(crate) fn any(self) -> bool {
        self.quasi_quotes
    }

    /// Whether `[quoter|` opens a quasi-quote: with Template Haskell's
    /// quotes on, `[e|`, `[d|`, `[t|` and `[p|` open quotes of code.
    fn quotes(self, quoter: &[u8]) -> bool {
        self.quasi_quotes && !(self.template_quotes && matches!(quoter, b"e" | b"d" | b"t" | b"p"))
    }
}

/// Length of the Haskell quasi-quote at `lines[i]`: `[`, a quoter, a
/// variable name that modules may qualify, as in `[r|` or `[Q.str|`, then
/// `|` and text up to the first `|]` (GHC User's Guide, "Template Haskell",
/// quasi-quotation; GHC 9.14.1 `GHC.Parser.Lexer`, `@varid` and `@qvarid`).
/// The file's `quoting` is read only once such an opener is found.
pub(crate) fn quasi_quote_len(
    lines: &[u8],
    i: usize,
    quoting: impl FnOnce() -> QuasiQuoting,
) -> Option<usize> {
    let rest = lines[i..].strip_prefix(b"[")?;
    let mut name = 0;
    loop {
        let first = *rest.get(name)?;
        let len = rest[name..]
            .iter()
            .take_while(|b| b.is_ascii_alphanumeric() || matches!(b, b'_' | b'\''))
            .count();
        if first.is_ascii_uppercase() && rest.get(name + len) == Some(&b'.') {
            name += len + 1;
        } else if first.is_ascii_lowercase() || first == b'_' {
            name += len;
            break;
        } else {
            return None;
        }
    }
    let quoter = &rest[..name];
    if rest.get(name) != Some(&b'|') || !quoting().quotes(quoter) {
        return None;
    }
    let close = memchr::memmem::find(&rest[name + 1..], b"|]")?;
    Some(1 + name + 1 + close + 2)
}

impl SyntaxCounter {
    /// Length of the text of a string gap at `lines[i]`, where a string has
    /// gaps: the escape that closes a gap begun on an earlier line, which
    /// therefore starts its line after whitespace, or a whole gap on one
    /// line, an escape, whitespace and an escape (Haskell 2010 report, §2.6).
    pub(crate) fn string_gap_len(&self, lines: &[u8], i: usize) -> Option<usize> {
        let escape = self.shared.escape;
        if !self.shared.string_gaps || self.quote_is_verbatim || lines[i] != escape {
            return None;
        }
        if starts_line(lines, i) {
            return Some(1);
        }
        let space = lines[i + 1..]
            .iter()
            .take_while(|b| matches!(b, b' ' | b'\t'))
            .count();
        (space > 0 && lines.get(i + 1 + space) == Some(&escape)).then_some(space + 2)
    }

    /// The quasi-quotes that the file `lines` turns on, read once per file.
    pub(crate) fn quasi_quoting(&mut self, lines: &[u8]) -> QuasiQuoting {
        *self
            .quasi_quoting
            .get_or_insert_with(|| QuasiQuoting::of(lines))
    }

    /// Whether `line`, of the file `lines`, may open a Haskell quasi-quote,
    /// whose `|` is too common in Haskell to search every line for: as
    /// important syntax, it sent a sixth of pandoc's lines to the full
    /// analysis, although few files turn quasi-quotes on (2026-10-30).
    pub(crate) fn may_open_quasi_quote(&mut self, lines: &[u8], line: &[u8]) -> bool {
        self.shared.quote_like == Some(QuoteLikeSyntax::HaskellQuasiQuote)
            && memchr::memrchr(b'|', line)
                .is_some_and(|bar| memchr::memchr(b'[', &line[..bar]).is_some())
            && self.quasi_quoting(lines).any()
    }
}
