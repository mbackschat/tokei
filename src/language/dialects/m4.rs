//! m4's quoted strings, as GNU m4 1.4.19's `next_token` (`src/input.c`)
//! reads them before any expansion: from the left quote to the right quote
//! that balances it, nesting, across lines, with neither comments nor `dnl`
//! recognized inside (m4 manual, "Quoted strings").

use crate::language::syntax::block_comment_len;

/// The length of the quoted string at `lines[i]`, quoted by `open` and
/// `close`, up to the closer that balances its opener. None where no `open`
/// is at `lines[i]` or the text ends first, which m4 reports as an error.
pub(crate) fn quote_len(lines: &[u8], i: usize, open: &str, close: &str) -> Option<usize> {
    if !lines[i..].starts_with(open.as_bytes()) {
        return None;
    }
    block_comment_len(&lines[i..], open, close, true)
}
