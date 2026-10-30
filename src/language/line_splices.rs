use super::dialects::make;
use crate::utils::ext::SliceExt;

/// How a backslash before a newline splices two lines into one, so that a
/// line comment ending in one continues on the next line.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum LineSplices {
    /// C's translation phase 2 deletes every backslash before a newline
    /// (C23 §5.1.1.2); like GCC and Clang, whitespace may follow the
    /// backslash.
    C,
    /// GNU make joins a line ending in an odd number of backslashes to the
    /// next (`readline` in GNU make 4.4.1's `src/read.c`), with nothing after
    /// them but a carriage return. A comment in a recipe, whose lines start
    /// with a tab or continue one that does, is the shell's, which ends at the
    /// newline (POSIX.1-2024, Shell Command Language, 2.3 Token Recognition);
    /// so is a comment in a `define` body, which may become a recipe; only
    /// lines starting with `#` are comments in both readings. Unlike C's, its backslash is no blank symbol: elsewhere on a line it
    /// quotes the next character, as in `\#`.
    Make,
}

impl LineSplices {
    /// The end of the line comment at `text[from]`: the end of its line and
    /// of each following line that a splice joins to it.
    pub(crate) fn comment_end(self, text: &[u8], mut from: usize) -> usize {
        if self == LineSplices::Make {
            let line_end = memchr::memchr(b'\n', &text[from..]).map_or(text.len(), |n| from + n);
            if !self.splices(&text[from..line_end]) {
                return line_end;
            }
            // A `define` body may be read as a recipe; the scan for its
            // `define` runs only for the rare comment that a splice continues.
            let start = self.logical_line_start(text, from);
            if text.get(start) == Some(&b'\t') || make::in_define_body(text, start) {
                return line_end;
            }
        }
        loop {
            let Some(newline) = memchr::memchr(b'\n', &text[from..]).map(|n| from + n) else {
                return text.len();
            };
            if !self.splices(&text[from..newline]) {
                return newline;
            }
            from = newline + 1;
        }
    }

    /// The start of the first line of those that splices join into the one
    /// holding `text[at]`.
    fn logical_line_start(self, text: &[u8], at: usize) -> usize {
        let mut start = memchr::memrchr(b'\n', &text[..at]).map_or(0, |n| n + 1);
        while start > 0 {
            let previous = memchr::memrchr(b'\n', &text[..start - 1]).map_or(0, |n| n + 1);
            if !self.splices(&text[previous..start - 1]) {
                break;
            }
            start = previous;
        }
        start
    }

    /// Whether `line`, without its newline, ends in a splice.
    fn splices(self, line: &[u8]) -> bool {
        match self {
            LineSplices::C => line.trim_end().ends_with(b"\\"),
            LineSplices::Make => make::ends_in_splice(line),
        }
    }
}
