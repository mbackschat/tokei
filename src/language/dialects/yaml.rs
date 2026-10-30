//! YAML's scalars whose text spans lines. Block scalars, such as `run: |`,
//! hold every following line indented at least as deep as their first line,
//! which is deeper than the node that holds the scalar (YAML 1.2.2 §8.1).
//! Plain scalars continue on the following, more deeply indented lines, with
//! the empty lines between them (§7.3.3). Their text is not syntax, so a `#`,
//! a quote or a blank line in it is text.

use crate::language::syntax::SyntaxCounter;
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
    let indicators = header_indicators(header)?;
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
            // Whitespace past the content's indentation is text, so the line
            // is content, not empty (YAML 1.2.2 §8.1.2, `l-nb-literal-text`).
            let past_indent = content_indent.is_some_and(|content_indent| {
                line.strip_suffix(b"\r").unwrap_or(line).len() > content_indent
            });
            if keeps_trailing_lines || past_indent {
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

/// Whether a block scalar may start on `line`: a `|` or `>` that ends the
/// line, apart from its header, where a node may start. The content that
/// follows decides whether it does; a `>` elsewhere, as in `<strong>`, is
/// text, so such lines keep the per-line fast path.
pub(crate) fn may_start_block_scalar(line: &[u8]) -> bool {
    memchr::memchr2_iter(b'|', b'>', line).any(|i| {
        header_indicators(&line[i + 1..]).is_some() && min_content_indent(&line[..i]).is_some()
    })
}

/// The length of the indentation and chomping indicators that start the
/// `header` after a `|` or `>`, when only a comment follows them
/// (YAML 1.2.2 §8.1.1).
fn header_indicators(header: &[u8]) -> Option<usize> {
    let indicators = header
        .iter()
        .take_while(|b| matches!(b, b'-' | b'+' | b'0'..=b'9'))
        .count();
    let rest = header[indicators..].trim_start();
    (rest.is_empty() || (rest.starts_with(b"#") && header[indicators].is_ascii_whitespace()))
        .then_some(indicators)
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
fn is_document_marker(line: &[u8]) -> bool {
    (line.starts_with(b"---") || line.starts_with(b"..."))
        && line.get(3).map_or(true, u8::is_ascii_whitespace)
}

/// The end of the last continuation line of the plain scalar that the line
/// at `lines[start]` ends in, read in block context, or `None` when the line
/// does not end in a plain scalar or nothing continues it.
pub(crate) fn plain_scalar_end(lines: &[u8], start: usize) -> Option<usize> {
    let end = line_end(lines, start);
    // Most lines are followed by a key or a comment, which ends any plain
    // scalar whatever this line holds, so that is checked before the line is
    // parsed: on the 549 YAML files of Mastodon 0cdf6f8 this cut the
    // instructions from 1,890M to 1,673M (measured 2026-10-29).
    if next_text(lines, end + 1).map_or(true, ends_plain_scalar) {
        return None;
    }
    let line = &lines[start..end];
    let parent_indent = match last_node(line)? {
        Node::Value { parent_column } => Some(parent_column),
        Node::Line => enclosing_indent(lines, start, indent_of(line)),
    };
    let mut content_end = None;
    let mut next = (end + 1).min(lines.len());
    while next < lines.len() {
        let next_end = line_end(lines, next);
        let text = &lines[next..next_end];
        if !text.trim().is_empty() {
            let indent = indent_of(text);
            // A key cannot continue a plain scalar, which holds no `: `
            // (YAML 1.2.2 §7.3.3, ns-plain-char).
            if parent_indent.is_some_and(|parent| indent <= parent) || ends_plain_scalar(text) {
                break;
            }
            content_end = Some((next_end + 1).min(lines.len()));
        }
        next = next_end + 1;
    }
    content_end
}

/// The first line at or after `from` that is not blank.
fn next_text(lines: &[u8], mut from: usize) -> Option<&[u8]> {
    while from < lines.len() {
        let end = line_end(lines, from);
        let text = &lines[from..end];
        if !text.trim().is_empty() {
            return Some(text);
        }
        from = end + 1;
    }
    None
}

/// Whether the non-blank line `text` ends a plain scalar before it, whatever
/// its indentation: a comment, a document marker or a key.
fn ends_plain_scalar(text: &[u8]) -> bool {
    text.trim_start().starts_with(b"#")
        || is_document_marker(text)
        || mapping_indicator(text.trim_start()).is_some()
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
            Some(colon) if colon + 1 >= rest.trim_end().len() => return None,
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
                if column >= line.trim_end().len() {
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
    let text = text.trim_end();
    (0..text.len()).find(|&i| text[i] == b':' && text.get(i + 1).map_or(true, |b| *b == b' '))
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

/// The position of the newline that ends the line at `lines[start]`, or the
/// end of `lines`.
fn line_end(lines: &[u8], start: usize) -> usize {
    memchr::memchr(b'\n', &lines[start..]).map_or(lines.len(), |n| start + n)
}

fn indent_of(line: &[u8]) -> usize {
    line.iter().take_while(|b| **b == b' ').count()
}

impl SyntaxCounter {
    /// Marks the content of the block scalar whose indicator is at
    /// `lines[i]` as literal.
    pub(crate) fn parse_block_scalar(&mut self, lines: &[u8], i: usize) -> bool {
        if !self.shared.block_scalars || !self.is_plain_mode() {
            return false;
        }
        let Some(end) = block_scalar_end(lines, i) else {
            return false;
        };
        trace!("Start block scalar, content ends at {}", end);
        self.literal_end = self.literal_end.max(end);
        true
    }

    /// Whether a quote at `lines[i]` can open a string.
    pub(crate) fn may_open_quote(&self, lines: &[u8], i: usize) -> bool {
        let Some(after) = self.shared.quotes_after else {
            return true;
        };
        let line_start = memchr::memrchr(b'\n', &lines[..i]).map_or(0, |n| n + 1);
        let before = &lines[line_start..i];
        let spaces = before
            .iter()
            .rev()
            .take_while(|b| matches!(b, b' ' | b'\t'))
            .count();
        let code = &before[..before.len() - spaces];
        let spaced = spaces > 0;
        // `[`, `{` and `,` are flow indicators only inside a flow collection;
        // in a plain scalar such as `a ["']` they are text.
        code.is_empty()
            || after.iter().any(|text| match text.strip_suffix(' ') {
                Some(indicator) => spaced && code.ends_with(indicator.as_bytes()),
                None => self.flow_depth > 0 && code.ends_with(text.as_bytes()),
            })
    }

    /// Whether a YAML flow collection opens on `line`. A `[` or `{` elsewhere,
    /// as in a plain scalar such as `Followers of %{name}`, is text, so such
    /// lines keep the per-line fast path.
    pub(crate) fn opens_flow_collection(&self, line: &[u8]) -> bool {
        self.shared.quotes_after.is_some()
            && memchr::memchr2_iter(b'[', b'{', line).any(|i| self.may_open_quote(line, i))
    }

    /// Opens a YAML flow collection at a `[` or `{` where a node may start,
    /// and closes one at `]` or `}` (YAML 1.2.2 §7.4).
    pub(crate) fn track_flow_collection(&mut self, lines: &[u8], i: usize) {
        if self.shared.quotes_after.is_none() {
            return;
        }
        match lines[i] {
            b'[' | b'{' if self.may_open_quote(lines, i) => self.flow_depth += 1,
            b']' | b'}' => self.flow_depth = self.flow_depth.saturating_sub(1),
            _ => {}
        }
    }

    /// Reads the end of a YAML line that did not start inside a literal: when
    /// it ends in a plain scalar that continues on the next lines, those
    /// lines are its text.
    pub(crate) fn end_line(&mut self, lines: &[u8], start: usize) {
        if !self.shared.block_scalars
            || self.flow_depth > 0
            || !self.is_plain_mode()
            || self.literal_end > start
        {
            return;
        }
        if let Some(scalar_end) = plain_scalar_end(lines, start) {
            trace!("Plain scalar continues to {}", scalar_end);
            self.literal_end = scalar_end;
        }
    }
}
