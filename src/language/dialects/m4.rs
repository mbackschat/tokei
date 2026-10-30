//! m4's quoted strings, as GNU m4 1.4.19's `next_token` (`src/input.c`)
//! reads them before any expansion: from the left quote to the right quote
//! that balances it, nesting, across lines, with neither comments nor `dnl`
//! recognized inside (m4 manual, "Quoted strings").

/// The length of the quoted string at `lines[i]`, quoted by `open` and
/// `close`, up to the closer that balances its opener. None where no `open`
/// is at `lines[i]` or the text ends first, which m4 reports as an error.
pub(crate) fn quote_len(lines: &[u8], i: usize, open: &str, close: &str) -> Option<usize> {
    let (open, close) = (open.as_bytes(), close.as_bytes());
    if !lines[i..].starts_with(open) {
        return None;
    }
    let mut depth = 0usize;
    let mut j = i + open.len();
    while j < lines.len() {
        if lines[j..].starts_with(open) {
            depth += 1;
            j += open.len();
        } else if lines[j..].starts_with(close) {
            j += close.len();
            if depth == 0 {
                return Some(j - i);
            }
            depth -= 1;
        } else {
            j += 1;
        }
    }
    None
}
