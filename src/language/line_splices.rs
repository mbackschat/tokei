use super::dialects::make;
use super::syntax::{line_end, line_start};
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
    /// lines starting with `#` are comments in both readings. Unlike C's, its
    /// backslash is no blank symbol: elsewhere on a line it quotes the next
    /// character, as in `\#`.
    Make,
}

impl LineSplices {
    /// The text that sends a line to the full parse, as a comment it ends
    /// may continue: C's backslash. Make's lines go by
    /// `make::needs_full_parse` instead.
    pub(crate) fn important_syntax(self) -> Option<&'static str> {
        match self {
            LineSplices::C => Some("\\"),
            LineSplices::Make => None,
        }
    }

    /// The end of the line comment at `text[from]`: the end of its line and
    /// of each following line that a splice joins to it.
    pub(crate) fn comment_end(
        self,
        text: &[u8],
        from: usize,
        defines: &mut make::Defines,
    ) -> usize {
        let mut end = line_end(text, from);
        if !self.splices(&text[from..end]) {
            return end;
        }
        match self {
            LineSplices::C => {}
            // The scan for a `define` runs only for the rare comment that a
            // splice continues.
            LineSplices::Make => {
                if make::shell_may_read(text, self.logical_line_start(text, from), defines) {
                    return end;
                }
            }
        }
        while end < text.len() {
            let next = end + 1;
            end = line_end(text, next);
            if !self.splices(&text[next..end]) {
                break;
            }
        }
        end
    }

    /// The start of the first line of those that splices join into the one
    /// holding `text[at]`.
    fn logical_line_start(self, text: &[u8], at: usize) -> usize {
        let mut start = line_start(text, at);
        while start > 0 {
            let previous = line_start(text, start - 1);
            if !self.splices(&text[previous..start - 1]) {
                break;
            }
            start = previous;
        }
        start
    }

    /// Whether `line`, without its newline, ends in a splice.
    pub(crate) fn splices(self, line: &[u8]) -> bool {
        match self {
            LineSplices::C => line.trim_end().ends_with(b"\\"),
            LineSplices::Make => make::ends_in_splice(line),
        }
    }
}
