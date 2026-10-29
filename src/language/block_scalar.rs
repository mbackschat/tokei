//! YAML block scalars, such as `run: |`, whose content is every following
//! line indented at least as deep as its first line, which is deeper than the
//! node that holds the scalar (YAML 1.2.2 §8.1). The content is text, so a
//! `#` or a quote in it is not syntax.

use crate::utils::ext::SliceExt;

/// The end of the content of the block scalar whose `|` or `>` indicator is
/// at `lines[i]`, or `None` when the indicator does not start a block scalar
/// or the scalar is empty. Trailing blank lines are left out of the content,
/// as YAML's default chomping strips them from the value.
pub(crate) fn block_scalar_end(lines: &[u8], i: usize) -> Option<usize> {
    if !matches!(lines.get(i), Some(b'|' | b'>')) {
        return None;
    }
    let line_start = lines[..i]
        .iter()
        .rposition(|b| *b == b'\n')
        .map_or(0, |n| n + 1);
    let parent_indent = parent_indent(&lines[line_start..i])?;
    let line_end = lines[i..].iter().position(|b| *b == b'\n').map(|n| i + n)?;
    let header = &lines[i + 1..line_end];
    let indicators = header
        .iter()
        .take_while(|b| matches!(b, b'-' | b'+' | b'0'..=b'9'))
        .count();
    let rest = header[indicators..].trim_start();
    if !(rest.is_empty() || (rest.starts_with(b"#") && header[indicators].is_ascii_whitespace())) {
        return None;
    }
    let mut content_indent = None;
    let mut content_end = None;
    let mut start = line_end + 1;
    while start < lines.len() {
        let end = lines[start..]
            .iter()
            .position(|b| *b == b'\n')
            .map_or(lines.len(), |n| start + n);
        let line = &lines[start..end];
        let indent = line.iter().take_while(|b| **b == b' ').count();
        if !line.trim().is_empty() {
            // The first content line sets the indentation of the rest
            // (YAML 1.2.2 §8.1.1.1), so a less indented line, such as a
            // comment, ends the scalar.
            let content_indent = *content_indent.get_or_insert(indent);
            if indent <= parent_indent || indent < content_indent {
                break;
            }
            content_end = Some((end + 1).min(lines.len()));
        }
        start = end + 1;
    }
    content_end
}

/// The indentation of the node holding a block scalar whose indicator
/// follows `before` on its line: the column of the key for `key: |`, or of
/// the `-` for a sequence entry `- |`.
fn parent_indent(before: &[u8]) -> Option<usize> {
    if !before.last().is_some_and(|b| *b == b' ' || *b == b'\t') {
        return None;
    }
    let text = before.trim();
    match text.last() {
        Some(b':') => {
            let mut column = before.iter().take_while(|b| **b == b' ').count();
            while before[column..].starts_with(b"- ") {
                column += 2 + before[column + 2..]
                    .iter()
                    .take_while(|b| **b == b' ')
                    .count();
            }
            Some(column)
        }
        Some(b'-') => Some(before.len() - before.trim_start().len() + text.len() - 1),
        _ => None,
    }
}
