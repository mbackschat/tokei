//! Visual Basic's `REM` comments and XML literals.
//!
//! `REM` is read as Roslyn's scanner reads it (`PeekStartComment` in
//! `src/Compilers/VisualBasic/Portable/Scanner/Scanner.vb`): in any case,
//! where a token starts, followed by the end of the line or a character that
//! cannot continue an identifier.
//!
//! An XML literal is an expression whose text is XML, so a quote or comment
//! marker in it opens nothing, while its `<%= … %>` holes hold Visual Basic
//! code (Visual Basic Language Specification, "XML Literal Expressions").

use crate::utils::ext::SliceExt;

/// Whether a `REM` comment starts at `lines[i]`. A `REM` after an identifier
/// character continues that identifier, and one after `[` is the bracketed
/// identifier `[REM]`.
pub(crate) fn starts_rem(lines: &[u8], i: usize) -> bool {
    let token_start = i == 0 || !(is_identifier_byte(lines[i - 1]) || lines[i - 1] == b'[');
    token_start
        && lines
            .get(i..i + 3)
            .is_some_and(|word| word.eq_ignore_ascii_case(b"rem"))
        && lines.get(i + 3).map_or(true, |&b| !is_identifier_byte(b))
}

/// Bytes that may continue an identifier; every byte of a non-ASCII
/// character counts, as most of them are letters.
fn is_identifier_byte(b: u8) -> bool {
    b.is_ascii_alphanumeric() || b == b'_' || !b.is_ascii()
}

/// The keywords after which an expression, and so an XML literal, may start.
const EXPRESSION_KEYWORDS: &[&[u8]] = &[
    b"And", b"AndAlso", b"Await", b"Case", b"Equals", b"In", b"Is", b"IsNot", b"Like", b"Mod",
    b"Not", b"Or", b"OrElse", b"Return", b"Select", b"Step", b"Throw", b"To", b"Where", b"Xor",
    b"Yield",
];

/// Whether an expression may start at `lines[i]`: after an operator, an
/// opening bracket, a comma or a keyword such as `Return`, also across the
/// line break that such a token continues over. Elsewhere a `<` opens an
/// attribute, as at the start of a statement, or compares.
pub(crate) fn expression_may_start(lines: &[u8], i: usize) -> bool {
    let mut before = lines[..i].trim_end();
    if let Some(continued) = before.strip_suffix(b" _") {
        before = continued.trim_end();
    }
    match before.last() {
        Some(b) if b"=(,&+-*/\\^{".contains(b) => true,
        Some(b) if is_identifier_byte(*b) => {
            let word_start = before
                .iter()
                .rposition(|b| !is_identifier_byte(*b))
                .map_or(0, |at| at + 1);
            let word = &before[word_start..];
            EXPRESSION_KEYWORDS
                .iter()
                .any(|keyword| word.eq_ignore_ascii_case(keyword))
        }
        _ => false,
    }
}

/// The length of the XML literal that the `<` at `lines[i]` starts: an
/// element, a document opened by its `<?xml …?>` declaration, a comment, a
/// CDATA section or a processing instruction. None where the text is none of
/// them, as for an attribute such as `<Fact>`, which no end tag closes.
pub(crate) fn xml_literal_len(lines: &[u8], i: usize) -> Option<usize> {
    let end = if lines[i..].starts_with(b"<?xml") {
        let mut root = markup_end(lines, i)?;
        loop {
            root = skip_whitespace(lines, root);
            match markup_end(lines, root) {
                Some(end) => root = end,
                None => break,
            }
        }
        element_end(lines, root)?
    } else {
        markup_end(lines, i).or_else(|| element_end(lines, i))?
    };
    Some(end - i)
}

/// The end of the comment, CDATA section or processing instruction at
/// `lines[i]`, none of which nests.
fn markup_end(lines: &[u8], i: usize) -> Option<usize> {
    let (open, close): (&[u8], &[u8]) = [
        (&b"<!--"[..], &b"-->"[..]),
        (b"<![CDATA[", b"]]>"),
        (b"<?", b"?>"),
    ]
    .into_iter()
    .find(|(open, _)| lines[i..].starts_with(open))?;
    let from = i + open.len();
    memchr::memmem::find(&lines[from..], close).map(|at| from + at + close.len())
}

/// The end of the element whose start tag opens at `lines[start]`, after its
/// end tag. An end tag closes the element it names, or any with `</>`.
fn element_end(lines: &[u8], start: usize) -> Option<usize> {
    let mut open: Vec<&[u8]> = Vec::new();
    let mut j = start;
    loop {
        let (name, attributes) = tag_name(lines, j + 1)?;
        let (tag_end, empty) = start_tag_end(lines, attributes)?;
        j = tag_end;
        if !empty {
            open.push(name);
        }
        loop {
            let Some(&current) = open.last() else {
                return Some(j);
            };
            j += memchr::memchr(b'<', &lines[j..])?;
            if lines[j..].starts_with(b"</") {
                let name_end = j
                    + 2
                    + lines[j + 2..]
                        .iter()
                        .take_while(|b| is_name_byte(**b))
                        .count();
                let name = &lines[j + 2..name_end];
                if !name.is_empty() && !current.is_empty() && name != current {
                    return None;
                }
                let close = skip_whitespace(lines, name_end);
                if lines.get(close) != Some(&b'>') {
                    return None;
                }
                open.pop();
                j = close + 1;
            } else if lines[j..].starts_with(b"<%=") {
                j = embedded_end(lines, j)?;
            } else if let Some(end) = markup_end(lines, j) {
                j = end;
            } else {
                break;
            }
        }
    }
}

/// The name that starts at `lines[k]` and the position after it; a name
/// given by an embedded expression is empty, as it matches any end tag.
fn tag_name(lines: &[u8], k: usize) -> Option<(&[u8], usize)> {
    if lines[k..].starts_with(b"<%=") {
        return Some((b"", embedded_end(lines, k)?));
    }
    let first = *lines.get(k)?;
    if !(first.is_ascii_alphabetic() || first == b'_' || !first.is_ascii()) {
        return None;
    }
    let end = k + lines[k..].iter().take_while(|b| is_name_byte(**b)).count();
    Some((&lines[k..end], end))
}

/// The end of the start tag whose attributes start at `lines[k]`, and
/// whether it closes its element, as `<a/>` does. Attribute values are
/// quoted without escapes, or are embedded expressions.
fn start_tag_end(lines: &[u8], mut k: usize) -> Option<(usize, bool)> {
    loop {
        k = skip_whitespace(lines, k);
        let rest = &lines[k..];
        if rest.starts_with(b"/>") {
            return Some((k + 2, true));
        } else if rest.starts_with(b">") {
            return Some((k + 1, false));
        } else if rest.starts_with(b"<%=") {
            k = embedded_end(lines, k)?;
            continue;
        }
        let (_, after) = tag_name(lines, k)?;
        k = skip_whitespace(lines, after);
        if lines.get(k) != Some(&b'=') {
            return None;
        }
        k = skip_whitespace(lines, k + 1);
        k = match *lines.get(k)? {
            quote @ (b'"' | b'\'') => k + 2 + memchr::memchr(quote, &lines[k + 1..])?,
            b'<' if lines[k..].starts_with(b"<%=") => embedded_end(lines, k)?,
            _ => return None,
        };
    }
}

/// The end of the `<%= … %>` expression at `lines[k]`, whose strings and
/// XML literals may hold `%>`.
fn embedded_end(lines: &[u8], k: usize) -> Option<usize> {
    let mut j = k + 3;
    while let Some(&byte) = lines.get(j) {
        j = match byte {
            b'%' if lines.get(j + 1) == Some(&b'>') => return Some(j + 2),
            b'"' => string_end(lines, j)?,
            b'<' => xml_literal_len(lines, j).map_or(j + 1, |len| j + len),
            _ => j + 1,
        };
    }
    None
}

/// The end of the string at `lines[j]`, in which `""` is a quote.
fn string_end(lines: &[u8], j: usize) -> Option<usize> {
    let mut k = j + 1;
    loop {
        k += memchr::memchr(b'"', &lines[k..])? + 1;
        if lines.get(k) != Some(&b'"') {
            return Some(k);
        }
        k += 1;
    }
}

fn skip_whitespace(lines: &[u8], k: usize) -> usize {
    k + lines[k..]
        .iter()
        .take_while(|b| b.is_ascii_whitespace())
        .count()
}

/// Whether an XML name continues with `byte`, as in `xs:element` or
/// `my-tag.name`.
fn is_name_byte(byte: u8) -> bool {
    byte.is_ascii_alphanumeric() || matches!(byte, b'_' | b'-' | b'.' | b':') || !byte.is_ascii()
}
