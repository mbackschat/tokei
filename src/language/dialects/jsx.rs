//! JSX elements, whose text between tags is not code, so a quote or comment
//! marker in it opens nothing (JSX specification, facebook.github.io/jsx).
//! Attribute strings have no escapes, and `{…}` holds code at any depth.

use std::borrow::Cow;

use crate::language::syntax::SyntaxCounter;

/// A level of JSX nesting, innermost last.
#[derive(Clone, Debug)]
pub(crate) enum JsxFrame {
    /// Inside an opening tag, such as `<p className='a'`, with the angle
    /// brackets of TypeScript type arguments opened in it and not yet closed,
    /// as in `<Item<Exclude<T, "a">>`.
    Tag { angles: usize },
    /// Between an element's opening and closing tags.
    Children,
    /// Inside `{…}`, with the braces opened in it and not yet closed, and the
    /// number of interpolation holes open when it began, since braces inside
    /// a later hole belong to that hole.
    Expression { depth: usize, holes: usize },
}

/// Whether the `<` at `lines[i]` starts an element: it is followed by a name
/// or `>`, and either closes itself or has a closing tag later in the text.
/// The closing tag tells an element from a TypeScript type parameter such as
/// `<T,>(x: T) => x`, which also follows `=`.
pub(crate) fn element_starts(lines: &[u8], i: usize) -> bool {
    let name_start = i + 1;
    let name_len = lines[name_start..]
        .iter()
        .take_while(|b| is_name_byte(**b))
        .count();
    let name = &lines[name_start..name_start + name_len];
    if name
        .first()
        .is_some_and(|b| b.is_ascii_digit() || *b == b'-')
    {
        return false;
    }
    if name.is_empty() {
        return lines.get(name_start) == Some(&b'>')
            && memchr::memmem::find(&lines[name_start..], b"</>").is_some();
    }
    let Some(tag_end) = opening_tag_end(lines, name_start + name_len) else {
        return false;
    };
    if lines[tag_end - 1] == b'/' {
        return true;
    }
    let mut from = tag_end;
    while let Some(at) = memchr::memmem::find(&lines[from..], b"</") {
        let candidate = from + at + 2;
        let rest = &lines[candidate..];
        if rest.starts_with(name) && !rest.get(name.len()).is_some_and(|b| is_name_byte(*b)) {
            return true;
        }
        from = candidate;
    }
    false
}

/// The position of the `>` that ends the opening tag whose attributes start
/// at `lines[start]`, skipping quoted values, `{…}` expressions and `<…>`
/// type arguments.
fn opening_tag_end(lines: &[u8], start: usize) -> Option<usize> {
    let mut depth = 0usize;
    let mut angles = 0usize;
    let mut j = start;
    while let Some(&byte) = lines.get(j) {
        match byte {
            b'"' | b'\'' if depth == 0 => {
                j += 1 + lines[j + 1..].iter().position(|b| *b == byte)?;
            }
            b'{' => depth += 1,
            b'}' => depth = depth.checked_sub(1)?,
            b'<' if depth == 0 => angles += 1,
            b'>' if depth == 0 && angles > 0 => angles -= 1,
            b'>' if depth == 0 => return Some(j),
            _ => {}
        }
        j += 1;
    }
    None
}

/// Whether a tag name continues with `byte`, as in `Foo.Bar`, `my-tag` or
/// `svg:rect`.
pub(crate) fn is_name_byte(byte: u8) -> bool {
    byte.is_ascii_alphanumeric() || matches!(byte, b'_' | b'$' | b'.' | b'-' | b':') || byte >= 0x80
}

impl SyntaxCounter {
    /// Reads JSX at `lines[i]`: the text and attributes of open elements,
    /// the braces of their expressions, and the start of a new element.
    /// Returns the bytes consumed, or `None` to lex the position as code.
    pub(crate) fn parse_jsx(&mut self, lines: &[u8], i: usize) -> Option<usize> {
        if !self.shared.jsx || !self.stack.is_empty() {
            return None;
        }
        let window = &lines[i..];
        let holes = self.holes.len();
        match self.jsx.last_mut() {
            Some(&mut JsxFrame::Tag { angles }) => self.parse_jsx_tag(window, angles),
            Some(JsxFrame::Children) => Some(self.parse_jsx_children(window)),
            Some(JsxFrame::Expression { depth, holes: open }) if *open == holes => {
                match window[0] {
                    b'{' => *depth += 1,
                    b'}' if *depth == 0 => {
                        self.jsx.pop();
                        return Some(1);
                    }
                    b'}' => *depth -= 1,
                    b'<' => return self.open_jsx_element(lines, i),
                    _ => {}
                }
                None
            }
            Some(JsxFrame::Expression { .. }) => None,
            None if window[0] == b'<' => self.open_jsx_element(lines, i),
            None => None,
        }
    }

    pub(crate) fn open_jsx_element(&mut self, lines: &[u8], i: usize) -> Option<usize> {
        let context = self.expression_context()?;
        if !context.expression_may_start(lines, i) || !element_starts(lines, i) {
            return None;
        }
        trace!("Start JSX element");
        self.jsx.push(JsxFrame::Tag { angles: 0 });
        Some(1)
    }

    /// Reads an opening tag, where quotes open attribute strings without
    /// escapes and comments may stand between attributes; `angles` counts the
    /// type-argument brackets still open.
    pub(crate) fn parse_jsx_tag(&mut self, window: &[u8], angles: usize) -> Option<usize> {
        if window.starts_with(b"//") || window.starts_with(b"/*") {
            return None;
        }
        Some(match window[0] {
            b'"' | b'\'' => {
                let quote = if window[0] == b'"' { "\"" } else { "'" };
                self.quote = Some(Cow::Borrowed(quote));
                self.quote_opener = quote;
                self.quote_is_verbatim = true;
                1
            }
            b'{' => {
                self.push_jsx_expression();
                1
            }
            b'/' if window.get(1) == Some(&b'>') => {
                self.jsx.pop();
                2
            }
            b'<' => {
                *self.jsx.last_mut().expect("open tag") = JsxFrame::Tag { angles: angles + 1 };
                1
            }
            b'>' => {
                *self.jsx.last_mut().expect("open tag") = match angles.checked_sub(1) {
                    Some(angles) => JsxFrame::Tag { angles },
                    None => JsxFrame::Children,
                };
                1
            }
            _ => 1,
        })
    }

    /// Reads the text between tags, which holds only expressions, elements
    /// and the closing tag.
    pub(crate) fn parse_jsx_children(&mut self, window: &[u8]) -> usize {
        match window {
            [b'{', ..] => {
                self.push_jsx_expression();
                1
            }
            [b'<', b'/', ..] => {
                self.jsx.pop();
                window
                    .iter()
                    .position(|b| *b == b'>')
                    .map_or(window.len(), |n| n + 1)
            }
            [b'<', next, ..] if *next == b'>' || is_name_byte(*next) => {
                self.jsx.push(JsxFrame::Tag { angles: 0 });
                1
            }
            _ => 1,
        }
    }

    pub(crate) fn push_jsx_expression(&mut self) {
        self.jsx.push(JsxFrame::Expression {
            depth: 0,
            holes: self.holes.len(),
        });
    }

    /// Whether the line starting here starts inside JSX text, which is not a
    /// comment even when it starts with a comment marker.
    pub(crate) fn in_jsx_text(&self) -> bool {
        matches!(self.jsx.last(), Some(JsxFrame::Children))
    }
}
