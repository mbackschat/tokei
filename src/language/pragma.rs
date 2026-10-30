//! Pragmas that a compiler reads as code although they open like a block
//! comment, as GHC reads `{-# INLINE f #-}`, while it reads other pragmas,
//! such as `{-# LANGUAGE CPP #-}`, as comments.

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
