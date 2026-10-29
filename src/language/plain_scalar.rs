//! YAML plain scalars that continue on the following lines, such as
//! `key: some text` followed by more deeply indented lines. Those lines,
//! with the empty lines between them, are part of the scalar's text
//! (YAML 1.2.2 §7.3.3), so a quote or a blank line in them is text.

use super::block_scalar::is_document_marker;
use crate::utils::ext::SliceExt;

/// The end of the last continuation line of the plain scalar that the line
/// at `lines[start]` ends in, read in block context, or `None` when the line
/// does not end in a plain scalar or nothing continues it.
pub(crate) fn plain_scalar_end(lines: &[u8], start: usize) -> Option<usize> {
    let end = lines[start..]
        .iter()
        .position(|b| *b == b'\n')
        .map_or(lines.len(), |n| start + n);
    let line = &lines[start..end];
    let parent_indent = match last_node(line)? {
        Node::Value { parent_column } => Some(parent_column),
        Node::Line => enclosing_indent(lines, start, indent_of(line)),
    };
    let mut content_end = None;
    let mut next = (end + 1).min(lines.len());
    while next < lines.len() {
        let next_end = lines[next..]
            .iter()
            .position(|b| *b == b'\n')
            .map_or(lines.len(), |n| next + n);
        let text = &lines[next..next_end];
        if !text.trim().is_empty() {
            let indent = indent_of(text);
            // A key cannot continue a plain scalar, which holds no `: `
            // (YAML 1.2.2 §7.3.3, ns-plain-char).
            if parent_indent.is_some_and(|parent| indent <= parent)
                || text.trim_start().starts_with(b"#")
                || is_document_marker(text)
                || mapping_indicator(text.trim_start()).is_some()
            {
                break;
            }
            content_end = Some((next_end + 1).min(lines.len()));
        }
        next = next_end + 1;
    }
    content_end
}

/// Where the plain scalar that ends a line begins.
enum Node {
    /// After `key: ` or `- `, as the value of a node at `parent_column`.
    Value { parent_column: usize },
    /// At the start of the line, as a node of its own.
    Line,
}

/// The node that ends `line` when it is a plain scalar: the text after the
/// last `key: `, `- ` or `? ` indicator, or the whole line, when that text
/// starts like a plain scalar and holds neither a mapping indicator nor a
/// comment (YAML 1.2.2 §7.3.3, ns-plain-first).
fn last_node(line: &[u8]) -> Option<Node> {
    if is_document_marker(line) {
        return None;
    }
    let indent = indent_of(line);
    let mut column = indent;
    let mut node = Node::Line;
    loop {
        let rest = &line[column..];
        if rest.starts_with(b"- ") || rest.starts_with(b"? ") {
            node = Node::Value {
                parent_column: column,
            };
            column += 2 + rest[2..].iter().take_while(|b| **b == b' ').count();
            continue;
        }
        let rest = skip_properties(rest);
        let first = *rest.first()?;
        let starts_plain = !b"-?:,[]{}#&*!|>'\"%@`".contains(&first)
            || (matches!(first, b'-' | b'?' | b':')
                && rest.get(1).is_some_and(|b| !b.is_ascii_whitespace()));
        if !starts_plain {
            return None;
        }
        if rest.windows(2).any(|w| w == b" #") {
            return None;
        }
        match mapping_indicator(rest) {
            Some(colon) if colon + 1 >= rest.trim_ascii_end().len() => return None,
            Some(colon) => {
                let key_column = line.len() - rest.len();
                node = Node::Value {
                    parent_column: if matches!(node, Node::Line) {
                        indent
                    } else {
                        key_column
                    },
                };
                let value = colon + 1;
                column = line.len() - rest.len()
                    + value
                    + rest[value..].iter().take_while(|b| **b == b' ').count();
                if column >= line.trim_ascii_end().len() {
                    return None;
                }
            }
            None => return Some(node),
        }
    }
}

/// The position of the `:` that ends an implicit key in `text`, one followed
/// by a space or the end of the line.
fn mapping_indicator(text: &[u8]) -> Option<usize> {
    let text = text.trim_ascii_end();
    (0..text.len()).find(|&i| text[i] == b':' && text.get(i + 1).is_none_or(|b| *b == b' '))
}

/// `text` after its anchor and tag properties, such as `&a !tag `.
fn skip_properties(mut text: &[u8]) -> &[u8] {
    while matches!(text.first(), Some(b'&' | b'!')) {
        let token = text.iter().take_while(|b| !b.is_ascii_whitespace()).count();
        text = text[token..].trim_start();
    }
    text
}

/// The indentation of the closest earlier line that is indented less than
/// `indent` and is not blank or a comment, which holds the node starting a
/// line at `indent`; `None` at the top level of the document.
fn enclosing_indent(lines: &[u8], start: usize, indent: usize) -> Option<usize> {
    lines[..start.saturating_sub(1)]
        .split(|b| *b == b'\n')
        .rev()
        .filter(|line| !line.trim().is_empty() && !line.trim_start().starts_with(b"#"))
        .map(indent_of)
        .find(|&i| i < indent)
}

fn indent_of(line: &[u8]) -> usize {
    line.iter().take_while(|b| **b == b' ').count()
}
