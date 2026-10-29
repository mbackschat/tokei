//! Literals and comments whose closing delimiter repeats a variable part of
//! the opening one, such as Rust's `r#"…"#`, C++'s `R"x(…)x"` or Lua's
//! `[==[…]==]`. Tokei's quote pairs are static strings, so these need a closer
//! computed from the opener found in the file.

/// The variable part of a raw delimiter.
#[derive(Debug)]
pub(crate) enum RawMiddle {
    /// One byte repeated at least `min` times, such as Rust's `#`.
    Repeat { byte: u8, min: usize },
    /// Up to `max` bytes from `chars`, such as the `x` of C++'s `R"x(`.
    Label { chars: &'static str, max: usize },
}

/// A literal or comment that opens with `start`, a middle and `open`, and
/// closes with `close`, the same middle and `end`.
#[derive(Debug)]
pub(crate) struct RawDelimiter {
    pub(crate) start: &'static str,
    pub(crate) middle: RawMiddle,
    pub(crate) open: &'static str,
    pub(crate) close: &'static str,
    pub(crate) end: &'static str,
    /// Text that, followed by the middle and one character, escapes that
    /// character, as `\#"` does in Swift's `#"…"#`; empty without escapes.
    pub(crate) escape: &'static str,
}

/// An opener found at the start of a window.
pub(crate) struct RawOpening {
    pub(crate) len: usize,
    pub(crate) closer: String,
    pub(crate) escape: Option<String>,
}

impl RawDelimiter {
    /// The opener starting `window`, with the closer and escape it implies.
    pub(crate) fn parse_opener(&self, window: &[u8]) -> Option<RawOpening> {
        let rest = window.strip_prefix(self.start.as_bytes())?;
        let middle_len = match self.middle {
            RawMiddle::Repeat { byte, min } => {
                let count = rest.iter().take_while(|b| **b == byte).count();
                (count >= min).then_some(count)?
            }
            RawMiddle::Label { chars, max } => {
                let count = rest
                    .iter()
                    .take_while(|b| chars.as_bytes().contains(b))
                    .count();
                (count <= max).then_some(count)?
            }
        };
        let (middle, rest) = rest.split_at(middle_len);
        if !rest.starts_with(self.open.as_bytes()) {
            return None;
        }
        let middle = std::str::from_utf8(middle).ok()?;
        Some(RawOpening {
            len: self.start.len() + middle.len() + self.open.len(),
            closer: [self.close, middle, self.end].concat(),
            escape: (!self.escape.is_empty()).then(|| [self.escape, middle].concat()),
        })
    }

    /// Texts of which every opener contains at least one, so that a line
    /// without any of them can skip analysis.
    pub(crate) fn important_syntax(&self) -> Vec<String> {
        match self.middle {
            RawMiddle::Repeat { byte, min } => {
                let mut texts = vec![format!("{}{}", self.start, char::from(byte))];
                if min == 0 {
                    texts.push([self.start, self.open].concat());
                }
                texts
            }
            RawMiddle::Label { .. } if self.start.is_empty() => vec![self.open.to_owned()],
            RawMiddle::Label { .. } => vec![self.start.to_owned()],
        }
    }

    /// The bytes an opener can begin with.
    pub(crate) fn first_bytes(&self) -> Vec<u8> {
        if let Some(&first) = self.start.as_bytes().first() {
            return vec![first];
        }
        let mut bytes = match self.middle {
            RawMiddle::Repeat { byte, .. } => vec![byte],
            RawMiddle::Label { chars, .. } => chars.as_bytes().to_vec(),
        };
        bytes.extend(self.open.as_bytes().first());
        bytes
    }
}
