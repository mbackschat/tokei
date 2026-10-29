//! JSX elements, whose text between tags is not code, so a quote or comment
//! marker in it opens nothing (JSX specification, facebook.github.io/jsx).
//! Attribute strings have no escapes, and `{…}` holds code at any depth.

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
        return lines.get(name_start) == Some(&b'>') && contains(&lines[name_start..], b"</>");
    }
    let Some(tag_end) = opening_tag_end(lines, name_start + name_len) else {
        return false;
    };
    if lines[tag_end - 1] == b'/' {
        return true;
    }
    let mut from = tag_end;
    while let Some(at) = find(&lines[from..], b"</") {
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

fn find(haystack: &[u8], needle: &[u8]) -> Option<usize> {
    haystack.windows(needle.len()).position(|w| w == needle)
}

fn contains(haystack: &[u8], needle: &[u8]) -> bool {
    find(haystack, needle).is_some()
}
