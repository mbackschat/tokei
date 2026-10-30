//! Tags that delimit a language's code inside a text, as PHP's `<?php … ?>`:
//! the text outside them is output, where no comment or literal starts
//! (PHP manual, "PHP tags").

/// Whose tags delimit the code.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum CodeTags {
    Php,
}

impl CodeTags {
    /// Length of the opening tag at the start of `window`: `<?php` in any
    /// case before whitespace, `<?=`, or the short `<?` that PHP's default
    /// `short_open_tag` allows.
    pub(crate) fn open_len(self, window: &[u8]) -> Option<usize> {
        match self {
            CodeTags::Php => {
                if !window.starts_with(self.open().as_bytes()) {
                    None
                } else if window
                    .get(2..5)
                    .is_some_and(|name| name.eq_ignore_ascii_case(b"php"))
                    && window.get(5).map_or(true, u8::is_ascii_whitespace)
                {
                    Some(5)
                } else if window.get(2) == Some(&b'=') {
                    Some(3)
                } else {
                    Some(2)
                }
            }
        }
    }

    /// The start that every opening tag shares.
    pub(crate) fn open(self) -> &'static str {
        match self {
            CodeTags::Php => "<?",
        }
    }

    /// The closing tag, which also ends a line comment (PHP manual,
    /// "Comments").
    pub(crate) fn close(self) -> &'static str {
        match self {
            CodeTags::Php => "?>",
        }
    }
}
