//! Here-documents, whose body runs from the line after the opener to a line
//! holding the word the opener names, such as shell's `<<EOF`. Tokei's quote
//! pairs are static strings that close anywhere in a line, so these need the
//! word from the file and a closer matched against whole lines.

/// Where whitespace may separate the opener from its word.
#[derive(Debug)]
pub(crate) enum HeredocSpacing {
    /// Never, as in Ruby, where `x << "a"` appends a string.
    Adjacent,
    /// Only before a quoted word, as in Perl.
    QuotedOnly,
    Any,
}

/// The characters of an unquoted word.
#[derive(Debug)]
pub(crate) enum HeredocWord {
    /// A letter or underscore followed by letters, digits or underscores.
    Identifier,
    /// Any characters up to whitespace or a shell operator character, after
    /// an optional backslash that quotes the word.
    Shell,
}

/// How a line closes the body.
#[derive(Debug)]
pub(crate) enum HeredocCloser {
    /// The line holds only the word, indented only after an indent flag.
    Line,
    /// The word, optionally indented and followed by anything but an
    /// identifier character, as PHP 7.3 allows.
    Flexible,
}

/// A here-document opened by `start`, an optional indent flag and a word.
#[derive(Debug)]
pub(crate) struct Heredoc {
    pub(crate) start: &'static str,
    /// Flags that may follow `start` and let the closing word be indented,
    /// such as Ruby's `-` and `~`.
    pub(crate) indent_flags: &'static str,
    pub(crate) spacing: HeredocSpacing,
    pub(crate) word: HeredocWord,
    pub(crate) closer: HeredocCloser,
}

/// An opener found at the start of a window.
pub(crate) struct HeredocOpening<'a> {
    pub(crate) len: usize,
    word: &'a [u8],
    indented: bool,
}

impl Heredoc {
    /// The opener at `lines[i]`, with the word its closing line holds.
    pub(crate) fn parse_opener<'a>(&self, lines: &'a [u8], i: usize) -> Option<HeredocOpening<'a>> {
        let rest = lines[i..].strip_prefix(self.start.as_bytes())?;
        // A longer run of `<`, such as a shell here-string `<<<`, is another
        // operator, read from its first `<` (Bash Reference Manual, 3.6.7
        // "Here Strings").
        if rest.first() == self.start.as_bytes().last()
            || lines[..i].last() == self.start.as_bytes().first()
        {
            return None;
        }
        let indented = rest
            .first()
            .is_some_and(|b| self.indent_flags.as_bytes().contains(b));
        let rest = &rest[usize::from(indented)..];
        let spaces = rest
            .iter()
            .take_while(|b| matches!(b, b' ' | b'\t'))
            .count();
        let rest = &rest[spaces..];
        let quoted = matches!(rest.first(), Some(b'\'' | b'"' | b'`'));
        let allowed_spacing = match self.spacing {
            HeredocSpacing::Adjacent => spaces == 0,
            HeredocSpacing::QuotedOnly => spaces == 0 || quoted,
            HeredocSpacing::Any => true,
        };
        if !allowed_spacing {
            return None;
        }
        let (word, word_len) = if quoted {
            let quote = rest[0];
            let inner = &rest[1..];
            let close = inner.iter().position(|b| *b == quote || *b == b'\n')?;
            (inner[close] == quote).then_some((&inner[..close], close + 2))?
        } else {
            self.bare_word(rest)?
        };
        Some(HeredocOpening {
            len: self.start.len() + usize::from(indented) + spaces + word_len,
            word,
            indented,
        })
    }

    /// An unquoted word starting `text`, with the length of its source text.
    fn bare_word<'a>(&self, text: &'a [u8]) -> Option<(&'a [u8], usize)> {
        let (prefix, text) = match (&self.word, text.first()) {
            (HeredocWord::Shell, Some(b'\\')) => (1, &text[1..]),
            _ => (0, text),
        };
        let len = match self.word {
            HeredocWord::Identifier => {
                if !text
                    .first()
                    .is_some_and(|b| b.is_ascii_alphabetic() || *b == b'_')
                {
                    return None;
                }
                text.iter().take_while(|b| is_identifier_byte(**b)).count()
            }
            HeredocWord::Shell => text
                .iter()
                .take_while(|b| !b.is_ascii_whitespace() && !b";&|()<>'\"`".contains(b))
                .count(),
        };
        (len > 0).then(|| (&text[..len], prefix + len))
    }

    /// The end of the line that closes the body, searching the lines from
    /// `from` on, or `None` when no line closes it, in which case the opener
    /// is taken for another operator, such as a shift.
    pub(crate) fn closing_line_end(
        &self,
        opening: &HeredocOpening,
        lines: &[u8],
        from: usize,
    ) -> Option<usize> {
        let mut start = from;
        while start < lines.len() {
            let end = lines[start..]
                .iter()
                .position(|b| *b == b'\n')
                .map_or(lines.len(), |n| start + n);
            let mut line = &lines[start..end];
            line = line.strip_suffix(b"\r").unwrap_or(line);
            if opening.indented || matches!(self.closer, HeredocCloser::Flexible) {
                let indent = line
                    .iter()
                    .take_while(|b| matches!(b, b' ' | b'\t'))
                    .count();
                line = &line[indent..];
            }
            let closes = match self.closer {
                HeredocCloser::Line => line == opening.word,
                HeredocCloser::Flexible => line
                    .strip_prefix(opening.word)
                    .is_some_and(|rest| !rest.first().is_some_and(|b| is_identifier_byte(*b))),
            };
            if closes {
                return Some((end + 1).min(lines.len()));
            }
            start = end + 1;
        }
        None
    }
}

fn is_identifier_byte(byte: u8) -> bool {
    byte.is_ascii_alphanumeric() || byte == b'_' || byte >= 0x80
}
