//! Fortran's fixed source form, the one `.f` files and their kin are read in
//! unless their content is free form (Fortran 2018, J3/18-007r1, §6.3.3): a
//! `C`, `c` or `*` in the first column opens a comment line (§6.3.3.2), and a
//! character other than a blank or a zero in the sixth column marks a line
//! as continuing the one before, even an `!`, which opens a comment in any
//! other column (§6.3.3.3).

use crate::{utils::ext::SliceExt, LineClass};

/// Whether `text` is in free source form, by fparser's rule (fparser 0.2.5,
/// `sourceinfo.get_source_info_str`): one of its first 10,000 lines that are
/// not blank and open with no comment marker either holds a character other
/// than a blank or a digit among its first five columns after one that is
/// not `C`, `c` or `*`, or ends with `&`. Preprocessor directives are no
/// such lines, as the preprocessor removes them before the compiler reads
/// the source, though fparser counts a `#if` line as free form; neither are
/// the `*` comment lines it reads as free form when they end with `&`.
pub(crate) fn is_free_form(text: &[u8]) -> bool {
    text.split(|&b| b == b'\n')
        .map(SliceExt::trim_end)
        .filter(|line| !matches!(line.first(), None | Some(b'!' | b'#' | b'*')))
        .take(10_000)
        .any(|line| (line[0] != b'\t' && opens_free_form(line)) || line.ends_with(b"&"))
}

/// Whether the first five columns of `line` hold a character other than a
/// blank or a digit after a first one that is not `C` or `c`.
fn opens_free_form(line: &[u8]) -> bool {
    let columns = &line[..line.len().min(5)];
    !matches!(columns[0], b'c' | b'C')
        && columns[1..]
            .iter()
            .find(|b| !b.is_ascii_whitespace())
            .is_some_and(|b| !b.is_ascii_digit())
}

/// The class of `line`, untrimmed, in fixed source form, when it follows
/// from the line's first columns alone: a comment line by its first column,
/// or a line continued with an `!` mark and holding no string, whose `!`
/// later on can only open a comment after code.
pub(crate) fn line_class(line: &[u8]) -> Option<LineClass> {
    if matches!(line.first(), Some(b'c' | b'C' | b'*')) {
        Some(LineClass::Comment)
    } else if has_continuation_mark(line) && !line.iter().any(|b| matches!(b, b'"' | b'\'')) {
        Some(LineClass::Code)
    } else {
        None
    }
}

/// Whether `line`, untrimmed, continues the line before it with an `!` in
/// its sixth column, which is no comment there.
pub(crate) fn has_continuation_mark(line: &[u8]) -> bool {
    line.get(..6) == Some(b"     !")
}
