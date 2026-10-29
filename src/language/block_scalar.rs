//! YAML block scalars, such as `run: |`, whose content is every following
//! line indented at least as deep as its first line, which is deeper than the
//! node that holds the scalar (YAML 1.2.2 §8.1). The content is text, so a
//! `#` or a quote in it is not syntax.

use crate::utils::ext::SliceExt;

/// The end of the content of the block scalar whose `|` or `>` indicator is
/// at `lines[i]`, or `None` when the indicator does not start a block scalar
/// or the scalar is empty. Trailing blank lines belong to the content only
/// under keep chomping, `+`, as YAML's chomping indicators define
/// (YAML 1.2.2 §8.1.1.2).
pub(crate) fn block_scalar_end(lines: &[u8], i: usize) -> Option<usize> {
    if !matches!(lines.get(i), Some(b'|' | b'>')) {
        return None;
    }
    let line_start = lines[..i]
        .iter()
        .rposition(|b| *b == b'\n')
        .map_or(0, |n| n + 1);
    let min_indent = min_content_indent(&lines[line_start..i])?;
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
    let keeps_trailing_lines = header[..indicators].contains(&b'+');
    // libyaml takes the deepest of the leading empty lines into the content
    // indentation, which YAML 1.2.2 §8.1.1.1 makes an error, so that a less
    // indented line after them ends the scalar.
    let mut leading_empty_indent = 0;
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
        if line.trim().is_empty() {
            if content_indent.is_none() {
                leading_empty_indent = leading_empty_indent.max(indent);
            }
            if keeps_trailing_lines {
                content_end = Some((end + 1).min(lines.len()));
            }
        } else {
            // The first content line sets the indentation of the rest
            // (YAML 1.2.2 §8.1.1.1), so a less indented line, such as a
            // comment, ends the scalar.
            let content_indent = *content_indent.get_or_insert(indent.max(leading_empty_indent));
            if indent < min_indent || indent < content_indent || is_document_marker(line) {
                break;
            }
            content_end = Some((end + 1).min(lines.len()));
        }
        start = end + 1;
    }
    content_end
}

/// The least indentation of a block scalar's content, from the text
/// `before` its indicator: one deeper than the key for `key: |` or `? |`, or
/// than the `-` for `- |`, and any column for a scalar that is the document,
/// whose indicator starts the line or follows `---` (YAML 1.2.2 §9.1.3).
fn min_content_indent(before: &[u8]) -> Option<usize> {
    let text = before.trim();
    if text.is_empty() || text == b"---" {
        return Some(0);
    }
    if !before.last().is_some_and(|b| *b == b' ' || *b == b'\t') {
        return None;
    }
    match text.last() {
        Some(b':') => {
            let mut column = before.iter().take_while(|b| **b == b' ').count();
            while before[column..].starts_with(b"- ") {
                column += 2 + before[column + 2..]
                    .iter()
                    .take_while(|b| **b == b' ')
                    .count();
            }
            Some(column + 1)
        }
        Some(b'-' | b'?') => Some(before.len() - before.trim_start().len() + text.len()),
        _ => None,
    }
}

/// Whether `line` is a `---` or `...` marker, which ends a document and the
/// scalars in it (YAML 1.2.2 §9.1.4).
pub(crate) fn is_document_marker(line: &[u8]) -> bool {
    (line.starts_with(b"---") || line.starts_with(b"..."))
        && line.get(3).map_or(true, u8::is_ascii_whitespace)
}
