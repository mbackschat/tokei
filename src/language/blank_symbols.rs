use crate::utils::ext::SliceExt;

/// A language's blank symbols, texts beyond whitespace that its lexer skips
/// as whitespace, such as WenYan's statement-separating punctuation, Rust's
/// Unicode line separators or the backslash of a C line splice, with a table
/// of the bytes that begin them. Searching for Rust's five at every byte and
/// at the start of every line cost Tokio `6470b5e` about 50M of 550M and
/// 29M of 503M instructions without the table (measured 2026-10-29).
#[derive(Clone, Copy, Debug)]
pub(crate) struct BlankSymbols {
    symbols: &'static [&'static str],
    first_bytes: [bool; 256],
}

impl BlankSymbols {
    pub(crate) fn new(symbols: &'static [&'static str]) -> Self {
        let mut first_bytes = [false; 256];
        for symbol in symbols {
            first_bytes[usize::from(symbol.as_bytes()[0])] = true;
        }
        Self {
            symbols,
            first_bytes,
        }
    }

    /// The blank symbol `window` starts with.
    pub(crate) fn at(&self, window: &[u8]) -> Option<&'static str> {
        if !self.first_bytes[usize::from(*window.first()?)] {
            return None;
        }
        self.symbols
            .iter()
            .copied()
            .find(|symbol| window.starts_with(symbol.as_bytes()))
    }

    /// `text` after its leading whitespace and blank symbols.
    pub(crate) fn trim_start<'a>(&self, mut text: &'a [u8]) -> &'a [u8] {
        loop {
            text = text.trim_start();
            match self.at(text) {
                Some(symbol) => text = &text[symbol.len()..],
                None => return text,
            }
        }
    }
}
