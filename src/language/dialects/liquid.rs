//! Liquid's comments and strings, as Liquid 5.14.0 reads them: its tokenizer
//! (`Liquid::Tokenizer`) splits a template into text, `{% … %}` tags and
//! `{{ … }}` variables, and only tags and variables hold strings, `"…"` or
//! `'…'` without escapes (`Liquid::QuotedString`). A `comment` block, which
//! nests and skips `raw` bodies (`Liquid::Comment#parse_body`), a `doc` block,
//! an inline `{% # … %}` tag, and inside a `{% liquid %}` tag a `#` line or the
//! lines from `comment` to its `endcomment` (`BlockBody#parse_for_liquid_tag`)
//! are comments, whatever whitespace control they use. Liquid reads text as
//! text, but the HTML it usually holds has comments, so a `<!--` in text
//! opens a comment that the next `-->` closes.
//!
//! A line is classed from the bytes it holds, as the step 6 judges class
//! them: a comment when every byte that is not whitespace lies in a comment,
//! code when any other byte does, and a line of whitespace takes the class of
//! the comment or string it lies in.

use std::ops::Range;

use grep_searcher::LineStep;

use crate::language::{language_type::LineReport, syntax::LineClass};
use crate::{CodeStats, Config};

#[derive(Clone, Copy, PartialEq)]
enum Byte {
    Code,
    Literal,
    Comment,
}

pub(crate) fn parse(
    _config: &Config,
    text: &[u8],
    mut stats: CodeStats,
    counted: Range<usize>,
    on_line: &mut impl FnMut(LineReport<'_>),
) -> CodeStats {
    let bytes = classes(text);
    let mut stepper = LineStep::new(b'\n', 0, text.len());
    while let Some((start, end)) = stepper.next(text) {
        let class = line_class(&text[start..end], &bytes[start..end]);
        if counted.contains(&start) {
            stats.add_line(class);
            on_line(LineReport::Class(Some(class)));
        }
    }
    stats
}

fn is_whitespace(byte: u8) -> bool {
    byte.is_ascii_whitespace() || byte == 0x0b
}

fn line_class(line: &[u8], bytes: &[Byte]) -> LineClass {
    let mut text_seen = false;
    for (&byte, &class) in line.iter().zip(bytes) {
        if !is_whitespace(byte) {
            if class != Byte::Comment {
                return LineClass::Code;
            }
            text_seen = true;
        }
    }
    if text_seen || bytes.contains(&Byte::Comment) {
        LineClass::Comment
    } else if bytes.contains(&Byte::Literal) {
        LineClass::Code
    } else {
        LineClass::Blank
    }
}

/// The class of every byte of `text`.
fn classes(text: &[u8]) -> Vec<Byte> {
    let tokens = tokens(text);
    let mut bytes = vec![Byte::Code; text.len()];
    // The end of the last HTML comment; tokens that start before it lie in it.
    let mut covered = 0;
    let mut i = 0;
    while i < tokens.len() {
        let token = tokens[i].clone();
        let source = &text[token.clone()];
        if token.start < covered {
            if !is_tag_or_variable(source) {
                covered = html_comments(text, covered, token.end, &mut bytes);
            }
        } else if source.starts_with(b"{%") {
            match full_token(source) {
                Some((b"comment", _)) => {
                    let last = comment_end(text, &tokens, i);
                    bytes[token.start..tokens[last].end].fill(Byte::Comment);
                    i = last;
                }
                Some((b"doc", _)) => {
                    let last = block_end(text, &tokens, i, b"enddoc");
                    bytes[token.start..tokens[last].end].fill(Byte::Comment);
                    i = last;
                }
                Some((b"raw", _)) => i = block_end(text, &tokens, i, b"endraw"),
                Some((b"#", _)) => bytes[token].fill(Byte::Comment),
                Some((b"liquid", markup)) => {
                    let markup = token.start + markup.start..token.start + markup.end;
                    liquid_tag(text, markup, &mut bytes);
                }
                _ => strings(text, token, &mut bytes),
            }
        } else if source.starts_with(b"{{") {
            strings(text, token, &mut bytes);
        } else {
            covered = html_comments(text, token.start, token.end, &mut bytes);
        }
        i += 1;
    }
    bytes
}

fn is_tag_or_variable(source: &[u8]) -> bool {
    source.starts_with(b"{%") || source.starts_with(b"{{")
}

/// Marks the HTML comments that open in text from `from` to `to`, each running
/// to the next `-->` whatever tokens it crosses, and returns where the last
/// one ends.
fn html_comments(text: &[u8], from: usize, to: usize, bytes: &mut [Byte]) -> usize {
    let mut covered = from;
    while covered < to {
        let Some(open) = memchr::memmem::find(&text[covered..to], b"<!--").map(|at| covered + at)
        else {
            break;
        };
        covered = memchr::memmem::find(&text[open + 4..], b"-->")
            .map_or(text.len(), |at| open + 4 + at + 3);
        bytes[open..covered].fill(Byte::Comment);
    }
    covered
}

/// Marks the `QuotedString`s of a tag or variable as literals.
fn strings(text: &[u8], range: Range<usize>, bytes: &mut [Byte]) {
    let mut i = range.start;
    while i < range.end {
        let quote = text[i];
        if quote == b'"' || quote == b'\'' {
            if let Some(close) = memchr::memchr(quote, &text[i + 1..range.end]) {
                let end = i + 1 + close + 1;
                bytes[i..end].fill(Byte::Literal);
                i = end;
                continue;
            }
        }
        i += 1;
    }
}

/// Marks a `{% liquid %}` tag's markup line by line, as
/// `BlockBody#parse_for_liquid_tag` and `Liquid::Comment#parse_body` read it.
fn liquid_tag(text: &[u8], markup: Range<usize>, bytes: &mut [Byte]) {
    let mut depth = 0;
    let mut comment_start = 0;
    let mut start = markup.start;
    loop {
        let end =
            memchr::memchr(b'\n', &text[start..markup.end]).map_or(markup.end, |at| start + at);
        let leading = text[start..end]
            .iter()
            .take_while(|&&b| is_whitespace(b))
            .count();
        let name_start = start + leading;
        let name = tag_name(&text[name_start..end]);
        if depth > 0 {
            match name {
                b"comment" => depth += 1,
                b"endcomment" => depth -= 1,
                _ => {}
            }
            if depth == 0 {
                bytes[comment_start..end].fill(Byte::Comment);
            }
        } else if name == b"comment" {
            depth = 1;
            comment_start = name_start;
        } else if name == b"#" {
            bytes[name_start..end].fill(Byte::Comment);
        } else {
            strings(text, start..end, bytes);
        }
        if end == markup.end {
            break;
        }
        start = end + 1;
    }
    if depth > 0 {
        bytes[comment_start..markup.end].fill(Byte::Comment);
    }
}

/// `TagName`, `#` or a word, at the start of `text`; empty when there is none.
fn tag_name(text: &[u8]) -> &[u8] {
    if text.first() == Some(&b'#') {
        return &text[..1];
    }
    let len = text
        .iter()
        .take_while(|b| b.is_ascii_alphanumeric() || **b == b'_')
        .count();
    &text[..len]
}

/// `BlockBody::FullToken`: a tag's name and the range of its markup within
/// the token, or `None` for a token Liquid rejects.
fn full_token(token: &[u8]) -> Option<(&[u8], Range<usize>)> {
    let inner_end = token
        .len()
        .checked_sub(2)
        .filter(|_| token.ends_with(b"%}"))?;
    let mut at = 2 + usize::from(token.get(2) == Some(&b'-'));
    at += token[at..inner_end]
        .iter()
        .take_while(|&&b| is_whitespace(b))
        .count();
    let name = tag_name(&token[at..inner_end]);
    if name.is_empty() {
        return None;
    }
    at += name.len();
    at += token[at..inner_end]
        .iter()
        .take_while(|&&b| is_whitespace(b))
        .count();
    let markup_end = if inner_end > at && token[inner_end - 1] == b'-' {
        inner_end - 1
    } else {
        inner_end
    };
    Some((name, at..markup_end.max(at)))
}

/// `BlockBody::FullTokenPossiblyInvalid`'s name: of the token's last `{%`
/// that a name follows, that name.
fn possibly_invalid_name(token: &[u8]) -> Option<&[u8]> {
    if !token.ends_with(b"%}") {
        return None;
    }
    memchr::memmem::rfind_iter(token, b"{%").find_map(|open| {
        let mut at = open + 2 + usize::from(token.get(open + 2) == Some(&b'-'));
        at += token[at..]
            .iter()
            .take_while(|&&b| is_whitespace(b))
            .count();
        let name = tag_name(&token[at..]);
        (!name.is_empty() && name != b"#").then_some(name)
    })
}

/// The index of the first token after `from` whose possibly invalid tag is
/// named `end`, as `Liquid::Raw` and `Liquid::Doc` find their ends, or the last
/// token when none is.
fn block_end(text: &[u8], tokens: &[Range<usize>], from: usize, end: &[u8]) -> usize {
    (from + 1..tokens.len())
        .find(|&i| possibly_invalid_name(&text[tokens[i].clone()]) == Some(end))
        .unwrap_or(tokens.len() - 1)
}

/// The index of the token that closes the comment block opened at `from`, or
/// the last token when none does.
fn comment_end(text: &[u8], tokens: &[Range<usize>], from: usize) -> usize {
    let mut depth = 1;
    let mut i = from + 1;
    while i < tokens.len() {
        match full_token(&text[tokens[i].clone()]).map(|(name, _)| name) {
            Some(b"raw") => i = block_end(text, tokens, i, b"endraw"),
            Some(b"comment") => depth += 1,
            Some(b"endcomment") => depth -= 1,
            _ => {}
        }
        if depth == 0 {
            return i;
        }
        i += 1;
    }
    tokens.len() - 1
}

/// The tokens `Liquid::Tokenizer` splits `text` into.
fn tokens(text: &[u8]) -> Vec<Range<usize>> {
    let mut tokens = Vec::new();
    let mut start = 0;
    while start < text.len() {
        let end = if text[start..].starts_with(b"{%") {
            memchr::memmem::find(&text[start + 2..], b"%}")
                .map_or(start + 2, |at| start + 2 + at + 2)
        } else if text[start..].starts_with(b"{{") {
            variable_end(text, start)
        } else {
            next_start(text, start)
        };
        tokens.push(start..end);
        start = end;
    }
    tokens
}

/// Where the next `{{` or `{%` at or after `from` starts, or the end of `text`.
fn next_start(text: &[u8], from: usize) -> usize {
    let mut at = from;
    while let Some(brace) = memchr::memchr(b'{', &text[at..]).map(|i| at + i) {
        if matches!(text.get(brace + 1), Some(b'{' | b'%')) {
            return brace;
        }
        at = brace + 1;
    }
    text.len()
}

/// The end of the variable token at `start`, as `next_variable_token` finds
/// it: the first `}}`, or a lone `}`, or the `%}` of a `{%` met first.
fn variable_end(text: &[u8], start: usize) -> usize {
    let mut pos = start + 2;
    let mut a = text.get(pos).copied();
    pos += usize::from(a.is_some());
    let mut b = a;
    while b.is_some() {
        while matches!(a, Some(byte) if byte != b'}' && byte != b'{') {
            a = text.get(pos).copied();
            pos += usize::from(a.is_some());
        }
        let Some(byte) = a else { break };
        if pos == text.len() {
            return pos;
        }
        b = text.get(pos).copied();
        pos += 1;
        if byte == b'}' {
            return if b == Some(b'}') { pos } else { pos - 1 };
        }
        if byte == b'{' && b == Some(b'%') {
            return memchr::memmem::find(&text[pos..], b"%}").map_or(pos, |at| pos + at + 2);
        }
        a = b;
    }
    text.len()
}
