//! CommonMark's block structure, as far as it decides where fenced code
//! blocks open and close (CommonMark 0.31.2, §4.5 and Appendix A "A parsing
//! strategy"). The reference implementation, commonmark.js 0.31.2, finds the
//! blocks in a first pass over the lines and parses inlines in a second; this
//! module ports that first pass from its `lib/blocks.js` and skips the second,
//! which never changes the blocks. A full parser, pulldown-cmark 0.13.5, also
//! scans inline syntax in its first pass and took 1.37 times upstream Tokei's
//! time to count github/docs's `content` (judges/speed/check.py, 2026-10-30).
//!
//! Link reference definitions are not read, so a paragraph holding only
//! them still makes a `===` or `---` line under it a setext heading.

use super::markdown::FencedBlock;

/// Columns of indentation that make a line indented code (§4.4).
const CODE_INDENT: usize = 4;

/// The tag names that open an HTML block of the sixth kind (§4.6).
const BLOCK_TAGS: &[&str] = &[
    "address",
    "article",
    "aside",
    "base",
    "basefont",
    "blockquote",
    "body",
    "caption",
    "center",
    "col",
    "colgroup",
    "dd",
    "details",
    "dialog",
    "dir",
    "div",
    "dl",
    "dt",
    "fieldset",
    "figcaption",
    "figure",
    "footer",
    "form",
    "frame",
    "frameset",
    "h1",
    "h2",
    "h3",
    "h4",
    "h5",
    "h6",
    "head",
    "header",
    "hr",
    "html",
    "iframe",
    "legend",
    "li",
    "link",
    "main",
    "menu",
    "menuitem",
    "nav",
    "noframes",
    "ol",
    "optgroup",
    "option",
    "p",
    "param",
    "search",
    "section",
    "summary",
    "table",
    "tbody",
    "td",
    "tfoot",
    "th",
    "thead",
    "title",
    "tr",
    "track",
    "ul",
];

/// The raw-text tags that open an HTML block of the first kind, which ends
/// at their end tag.
const RAW_TAGS: &[&str] = &["pre", "script", "style", "textarea"];

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
struct ListData {
    marker: Marker,
    marker_offset: usize,
    padding: usize,
}

/// A list item's marker: an item continues a list only with the same bullet
/// character or ordered delimiter (§5.3).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum Marker {
    Bullet(u8),
    Ordered(u8),
}

impl ListData {
    /// Whether an item with `other`'s marker continues this list.
    fn matches(&self, other: &ListData) -> bool {
        self.marker == other.marker
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum Kind {
    Document,
    List(ListData),
    Item(ListData),
    BlockQuote,
    Heading,
    ThematicBreak,
    Fenced,
    Indented,
    Html(u8),
    Paragraph,
}

impl Kind {
    fn can_contain(self, child: Kind) -> bool {
        match self {
            Kind::Document | Kind::BlockQuote | Kind::Item(_) => !matches!(child, Kind::Item(_)),
            Kind::List(_) => matches!(child, Kind::Item(_)),
            _ => false,
        }
    }

    fn accepts_lines(self) -> bool {
        matches!(
            self,
            Kind::Fenced | Kind::Indented | Kind::Html(_) | Kind::Paragraph
        )
    }
}

#[derive(Debug)]
struct Open {
    kind: Kind,
    has_child: bool,
}

/// The fenced code block open at the tip; no other block is open inside it.
#[derive(Debug)]
struct Fence {
    byte: u8,
    len: usize,
    indent: usize,
    line_start: usize,
    info: Option<String>,
    content: String,
    end: usize,
}

/// What a block's continuation check made of the line.
enum Continued {
    Matched,
    Failed,
    /// The line closed the block and holds nothing else.
    Done,
}

/// What a block start made of the line.
enum Started {
    None,
    Container,
    Leaf,
}

struct BlockParser<'a> {
    line: &'a str,
    line_start: usize,
    line_end: usize,
    offset: usize,
    column: usize,
    partially_consumed_tab: bool,
    next_nonspace: usize,
    next_nonspace_column: usize,
    indent: usize,
    indented: bool,
    blank: bool,
    all_closed: bool,
    last_matched: usize,
    stack: Vec<Open>,
    fence: Option<Fence>,
    blocks: Vec<FencedBlock>,
}

/// The fenced code blocks of `text`, in order.
pub(crate) fn fenced_blocks(text: &str) -> Vec<FencedBlock> {
    let mut parser = BlockParser {
        line: "",
        line_start: 0,
        line_end: 0,
        offset: 0,
        column: 0,
        partially_consumed_tab: false,
        next_nonspace: 0,
        next_nonspace_column: 0,
        indent: 0,
        indented: false,
        blank: false,
        all_closed: true,
        last_matched: 0,
        stack: vec![Open {
            kind: Kind::Document,
            has_child: false,
        }],
        fence: None,
        blocks: Vec::new(),
    };
    let mut start = 0;
    while start < text.len() {
        // The `memchr` crate's search, unlike `str::find`'s, is vectorized
        // (a third of the pass's time, measured 2026-10-30).
        let end =
            memchr::memchr(b'\n', &text.as_bytes()[start..]).map_or(text.len(), |n| start + n + 1);
        let line = text[start..end].trim_end_matches('\n');
        parser.line = line.strip_suffix('\r').unwrap_or(line);
        parser.line_start = start;
        parser.line_end = end;
        parser.incorporate_line();
        start = end;
    }
    while parser.stack.len() > 1 {
        parser.close_tip(false);
    }
    parser.blocks
}

impl BlockParser<'_> {
    fn byte(&self, at: usize) -> Option<u8> {
        self.line.as_bytes().get(at).copied()
    }

    fn rest(&self) -> &str {
        &self.line[self.next_nonspace..]
    }

    fn tip(&self) -> Kind {
        self.stack.last().expect("the document stays open").kind
    }

    /// commonmark.js `incorporateLine`.
    fn incorporate_line(&mut self) {
        self.offset = 0;
        self.column = 0;
        self.blank = false;
        self.partially_consumed_tab = false;

        let mut container = 0;
        for k in 1..self.stack.len() {
            self.find_next_nonspace();
            match self.continue_block(k) {
                Continued::Matched => container = k,
                Continued::Failed => break,
                Continued::Done => return,
            }
        }
        self.all_closed = container + 1 == self.stack.len();
        self.last_matched = container;

        let mut kind = self.stack[container].kind;
        let mut matched_leaf = kind != Kind::Paragraph && kind.accepts_lines();
        while !matched_leaf {
            self.find_next_nonspace();
            if !self.indented
                && !self
                    .rest()
                    .bytes()
                    .next()
                    .is_some_and(|b| b"#`~*+_=<>-".contains(&b) || b.is_ascii_digit())
            {
                self.advance_next_nonspace();
                break;
            }
            match self.start_block(kind) {
                Started::None => {
                    self.advance_next_nonspace();
                    break;
                }
                Started::Container => kind = self.tip(),
                Started::Leaf => {
                    kind = self.tip();
                    matched_leaf = true;
                }
            }
        }

        if !self.all_closed && !self.blank && self.tip() == Kind::Paragraph {
            // A lazy continuation line of the paragraph.
            return;
        }
        self.close_unmatched();
        if kind.accepts_lines() {
            self.add_line();
            if let Kind::Html(html) = kind {
                if html <= 5 && html_block_closes(html, &self.line[self.offset..]) {
                    self.close_tip(false);
                }
            }
        } else if self.offset < self.line.len() && !self.blank {
            self.add_child(Kind::Paragraph);
            self.advance_next_nonspace();
        }
    }

    /// commonmark.js `blocks[type].continue` for the open block `stack[k]`.
    fn continue_block(&mut self, k: usize) -> Continued {
        match self.stack[k].kind {
            Kind::Document | Kind::List(_) => Continued::Matched,
            Kind::BlockQuote => {
                if self.indented || self.byte(self.next_nonspace) != Some(b'>') {
                    return Continued::Failed;
                }
                self.advance_next_nonspace();
                self.advance_offset(1, false);
                if self.byte(self.offset).is_some_and(is_space_or_tab) {
                    self.advance_offset(1, true);
                }
                Continued::Matched
            }
            Kind::Item(data) => {
                if self.blank {
                    if !self.stack[k].has_child {
                        return Continued::Failed;
                    }
                    self.advance_next_nonspace();
                } else if self.indent >= data.marker_offset + data.padding {
                    self.advance_offset(data.marker_offset + data.padding, true);
                } else {
                    return Continued::Failed;
                }
                Continued::Matched
            }
            Kind::Heading | Kind::ThematicBreak => Continued::Failed,
            Kind::Fenced => {
                let fence = self.fence.as_ref().expect("an open fenced block");
                let (byte, len, mut skip) = (fence.byte, fence.len, fence.indent);
                if self.indent <= 3 && self.byte(self.next_nonspace) == Some(byte) {
                    let rest = self.rest().as_bytes();
                    let run = rest.iter().take_while(|b| **b == byte).count();
                    if run >= 3 && run >= len && rest[run..].iter().all(|b| is_space_or_tab(*b)) {
                        self.fence.as_mut().expect("an open fenced block").end = self.line_end;
                        self.close_tip(true);
                        return Continued::Done;
                    }
                }
                while skip > 0 && self.byte(self.offset).is_some_and(is_space_or_tab) {
                    self.advance_offset(1, true);
                    skip -= 1;
                }
                Continued::Matched
            }
            Kind::Indented => {
                if self.indent >= CODE_INDENT {
                    self.advance_offset(CODE_INDENT, true);
                } else if self.blank {
                    self.advance_next_nonspace();
                } else {
                    return Continued::Failed;
                }
                Continued::Matched
            }
            Kind::Html(html) => {
                if self.blank && html >= 6 {
                    Continued::Failed
                } else {
                    Continued::Matched
                }
            }
            Kind::Paragraph => {
                if self.blank {
                    Continued::Failed
                } else {
                    Continued::Matched
                }
            }
        }
    }

    /// commonmark.js `blockStarts`, tried in order inside `container`.
    fn start_block(&mut self, container: Kind) -> Started {
        let rest = self.rest().as_bytes();
        if !self.indented {
            if rest.first() == Some(&b'>') {
                self.advance_next_nonspace();
                self.advance_offset(1, false);
                if self.byte(self.offset).is_some_and(is_space_or_tab) {
                    self.advance_offset(1, true);
                }
                self.close_unmatched();
                self.add_child(Kind::BlockQuote);
                return Started::Container;
            }
            if is_atx_heading(rest) {
                self.close_unmatched();
                self.add_child(Kind::Heading);
                self.offset = self.line.len();
                return Started::Leaf;
            }
            if let Some((byte, len)) = code_fence(rest) {
                self.close_unmatched();
                self.add_child(Kind::Fenced);
                self.fence = Some(Fence {
                    byte,
                    len,
                    indent: self.indent,
                    line_start: self.line_start,
                    info: None,
                    content: String::new(),
                    end: self.line_end,
                });
                self.advance_next_nonspace();
                self.advance_offset(len, false);
                return Started::Leaf;
            }
            if rest.first() == Some(&b'<') {
                let lazy = !self.all_closed && !self.blank && self.tip() == Kind::Paragraph;
                let interrupts = container != Kind::Paragraph && !lazy;
                if let Some(html) = (1..=7)
                    .find(|&html| (html < 7 || interrupts) && html_block_opens(html, self.rest()))
                {
                    self.close_unmatched();
                    self.add_child(Kind::Html(html));
                    return Started::Leaf;
                }
            }
            if container == Kind::Paragraph && is_setext_underline(rest) {
                self.close_unmatched();
                self.stack.last_mut().expect("the paragraph").kind = Kind::Heading;
                self.offset = self.line.len();
                return Started::Leaf;
            }
            if is_thematic_break(rest) {
                self.close_unmatched();
                self.add_child(Kind::ThematicBreak);
                self.offset = self.line.len();
                return Started::Leaf;
            }
        }
        if !self.indented || matches!(container, Kind::List(_)) {
            if let Some(data) = self.parse_list_marker(container) {
                self.close_unmatched();
                let continues = matches!(self.tip(), Kind::List(list) if list.matches(&data));
                if !continues {
                    self.add_child(Kind::List(data));
                }
                self.add_child(Kind::Item(data));
                return Started::Container;
            }
        }
        if self.indented && self.tip() != Kind::Paragraph && !self.blank {
            self.advance_offset(CODE_INDENT, true);
            self.close_unmatched();
            self.add_child(Kind::Indented);
            return Started::Leaf;
        }
        Started::None
    }

    /// commonmark.js `parseListMarker`: the list item marker at the next
    /// non-space position, which it moves past (§5.2).
    fn parse_list_marker(&mut self, container: Kind) -> Option<ListData> {
        if self.indent >= 4 {
            return None;
        }
        let rest = self.rest().as_bytes();
        let (marker, marker_len) = match *rest.first()? {
            bullet @ (b'*' | b'+' | b'-') => (Marker::Bullet(bullet), 1),
            _ => {
                let digits = rest.iter().take_while(|b| b.is_ascii_digit()).count();
                let delimiter = *rest.get(digits)?;
                if !(1..=9).contains(&digits) || !matches!(delimiter, b'.' | b')') {
                    return None;
                }
                // Only a list starting at 1 interrupts a paragraph, however
                // the number is written, as `01.` is (§5.3).
                let (last, leading) = rest[..digits].split_last()?;
                if container == Kind::Paragraph
                    && !(*last == b'1' && leading.iter().all(|b| *b == b'0'))
                {
                    return None;
                }
                (Marker::Ordered(delimiter), digits + 1)
            }
        };
        if !rest.get(marker_len).map_or(true, |b| is_space_or_tab(*b)) {
            return None;
        }
        if container == Kind::Paragraph && rest[marker_len..].iter().all(|b| is_blank_byte(*b)) {
            return None;
        }
        let marker_offset = self.indent;
        self.advance_next_nonspace();
        self.advance_offset(marker_len, true);
        let (spaces_column, spaces_offset) = (self.column, self.offset);
        loop {
            self.advance_offset(1, true);
            if !(self.column - spaces_column < 5
                && self.byte(self.offset).is_some_and(is_space_or_tab))
            {
                break;
            }
        }
        let blank_item = self.offset >= self.line.len();
        let spaces = self.column - spaces_column;
        let padding = if !(1..5).contains(&spaces) || blank_item {
            self.column = spaces_column;
            self.offset = spaces_offset;
            if self.byte(self.offset).is_some_and(is_space_or_tab) {
                self.advance_offset(1, true);
            }
            marker_len + 1
        } else {
            marker_len + spaces
        };
        Some(ListData {
            marker,
            marker_offset,
            padding,
        })
    }

    fn find_next_nonspace(&mut self) {
        let bytes = self.line.as_bytes();
        let (mut i, mut column) = (self.offset, self.column);
        while let Some(&byte) = bytes.get(i) {
            match byte {
                b' ' => column += 1,
                b'\t' => column += 4 - column % 4,
                _ => break,
            }
            i += 1;
        }
        self.blank = i >= bytes.len();
        self.next_nonspace = i;
        self.next_nonspace_column = column;
        self.indent = column - self.column;
        self.indented = self.indent >= CODE_INDENT;
    }

    fn advance_next_nonspace(&mut self) {
        self.offset = self.next_nonspace;
        self.column = self.next_nonspace_column;
        self.partially_consumed_tab = false;
    }

    /// Moves `count` bytes on, or `count` columns when `columns` is set, of
    /// which a tab may cover only part.
    fn advance_offset(&mut self, mut count: usize, columns: bool) {
        while count > 0 {
            let Some(byte) = self.byte(self.offset) else {
                break;
            };
            if byte == b'\t' {
                let to_tab = 4 - self.column % 4;
                if columns {
                    self.partially_consumed_tab = to_tab > count;
                    let advance = to_tab.min(count);
                    self.column += advance;
                    self.offset += usize::from(!self.partially_consumed_tab);
                    count -= advance;
                } else {
                    self.partially_consumed_tab = false;
                    self.column += to_tab;
                    self.offset += 1;
                    count -= 1;
                }
            } else {
                self.partially_consumed_tab = false;
                self.offset += 1;
                self.column += 1;
                count -= 1;
            }
        }
    }

    /// Adds the rest of the line to the tip, which takes lines; only a fenced
    /// block keeps them, its first as the info string.
    fn add_line(&mut self) {
        if self.tip() != Kind::Fenced {
            return;
        }
        let tab_spaces = if self.partially_consumed_tab {
            self.offset += 1;
            4 - self.column % 4
        } else {
            0
        };
        let rest = &self.line[self.offset..];
        let fence = self.fence.as_mut().expect("an open fenced block");
        fence.end = self.line_end;
        match fence.info {
            // Spaces before the info string are trimmed with it.
            None => fence.info = Some(unescape(rest.trim())),
            Some(_) => {
                fence
                    .content
                    .extend(std::iter::repeat(' ').take(tab_spaces));
                fence.content.push_str(rest);
                fence.content.push('\n');
            }
        }
    }

    fn add_child(&mut self, kind: Kind) {
        while !self.tip().can_contain(kind) {
            self.close_tip(false);
        }
        self.stack
            .last_mut()
            .expect("the document stays open")
            .has_child = true;
        self.stack.push(Open {
            kind,
            has_child: false,
        });
    }

    fn close_unmatched(&mut self) {
        if !self.all_closed {
            while self.stack.len() > self.last_matched + 1 {
                self.close_tip(false);
            }
            self.all_closed = true;
        }
    }

    /// Closes the tip; a fenced block is `closed` by a closing fence.
    fn close_tip(&mut self, closed: bool) {
        let open = self.stack.pop().expect("an open block");
        if open.kind == Kind::Fenced {
            let fence = self.fence.take().expect("an open fenced block");
            self.blocks.push(FencedBlock {
                line_start: fence.line_start,
                end: fence.end,
                fence_lines: 1 + usize::from(closed),
                info: fence.info.unwrap_or_default(),
                content: fence.content,
            });
        }
    }
}

fn is_space_or_tab(byte: u8) -> bool {
    byte == b' ' || byte == b'\t'
}

/// Whether `byte` is one that commonmark.js's `reNonSpace` does not match.
fn is_blank_byte(byte: u8) -> bool {
    matches!(byte, b' ' | b'\t' | b'\n' | b'\r' | 0x0b | 0x0c)
}

/// An opening code fence: three or more backticks, with none in the rest of
/// the line, or three or more tildes (§4.5); its byte and length.
fn code_fence(rest: &[u8]) -> Option<(u8, usize)> {
    let byte = *rest.first().filter(|b| matches!(b, b'`' | b'~'))?;
    let len = rest.iter().take_while(|b| **b == byte).count();
    (len >= 3 && (byte == b'~' || !rest[len..].contains(&b'`'))).then_some((byte, len))
}

/// `#{1,6}` followed by a space, a tab or the end of the line (§4.2).
fn is_atx_heading(rest: &[u8]) -> bool {
    let hashes = rest.iter().take_while(|b| **b == b'#').count();
    (1..=6).contains(&hashes) && rest.get(hashes).map_or(true, |b| is_space_or_tab(*b))
}

/// Three or more `*`, `-` or `_`, with only spaces and tabs between (§4.1).
fn is_thematic_break(rest: &[u8]) -> bool {
    let Some(&byte) = rest.first().filter(|b| matches!(b, b'*' | b'-' | b'_')) else {
        return false;
    };
    let mut count = 0;
    for &b in rest {
        if b == byte {
            count += 1;
        } else if !is_space_or_tab(b) {
            return false;
        }
    }
    count >= 3
}

/// A run of `=` or of `-`, then only spaces and tabs (§4.3).
fn is_setext_underline(rest: &[u8]) -> bool {
    let Some(&byte) = rest.first().filter(|b| matches!(b, b'=' | b'-')) else {
        return false;
    };
    let run = rest.iter().take_while(|b| **b == byte).count();
    rest[run..].iter().all(|b| is_space_or_tab(*b))
}

/// Whether the HTML block of kind `html` opens at `rest` (§4.6, start
/// conditions).
fn html_block_opens(html: u8, rest: &str) -> bool {
    let bytes = rest.as_bytes();
    match html {
        1 => RAW_TAGS.iter().any(|tag| {
            tag_name_at(rest, 1, tag).is_some_and(|end| ends_tag_name(rest, end, false))
        }),
        2 => bytes.starts_with(b"<!--"),
        3 => bytes.starts_with(b"<?"),
        4 => bytes.starts_with(b"<!") && bytes.get(2).is_some_and(u8::is_ascii_alphabetic),
        5 => bytes.starts_with(b"<![CDATA["),
        6 => {
            let from = if bytes.get(1) == Some(&b'/') { 2 } else { 1 };
            BLOCK_TAGS.iter().any(|tag| {
                tag_name_at(rest, from, tag).is_some_and(|end| ends_tag_name(rest, end, true))
            })
        }
        _ => complete_tag_len(rest).is_some_and(|len| rest[len..].chars().all(char::is_whitespace)),
    }
}

/// Whether the HTML block of kind `html`, from 1 to 5, ends on the line
/// `text` (§4.6, end conditions).
fn html_block_closes(html: u8, text: &str) -> bool {
    let bytes = text.as_bytes();
    match html {
        1 => memchr::memmem::find_iter(bytes, b"</").any(|at| {
            RAW_TAGS.iter().any(|tag| {
                tag_name_at(text, at + 2, tag).is_some_and(|end| bytes.get(end) == Some(&b'>'))
            })
        }),
        2 => memchr::memmem::find(bytes, b"-->").is_some(),
        3 => memchr::memmem::find(bytes, b"?>").is_some(),
        4 => bytes.contains(&b'>'),
        _ => memchr::memmem::find(bytes, b"]]>").is_some(),
    }
}

/// The end of `tag` at `text[from..]`, matched without regard to case.
fn tag_name_at(text: &str, from: usize, tag: &str) -> Option<usize> {
    let end = from + tag.len();
    text.get(from..end)
        .filter(|name| name.eq_ignore_ascii_case(tag))
        .map(|_| end)
}

/// Whether a tag name ends at `text[end]`: before whitespace, `>`, the end of
/// the line or, where `slash` allows, `/>`.
fn ends_tag_name(text: &str, end: usize, slash: bool) -> bool {
    let rest = &text[end..];
    rest.is_empty()
        || rest.starts_with('>')
        || (slash && rest.starts_with("/>"))
        || rest.chars().next().is_some_and(char::is_whitespace)
}

/// The length of the complete open or closing tag at the start of `text`
/// (§6.6, commonmark.js `OPENTAG` and `CLOSETAG`).
fn complete_tag_len(text: &str) -> Option<usize> {
    let bytes = text.as_bytes();
    let closing = bytes.get(1) == Some(&b'/');
    let mut i = if closing { 2 } else { 1 };
    if !bytes.get(i)?.is_ascii_alphabetic() {
        return None;
    }
    i += bytes[i..]
        .iter()
        .take_while(|b| b.is_ascii_alphanumeric() || **b == b'-')
        .count();
    if closing {
        i = skip_whitespace(text, i);
        return (bytes.get(i) == Some(&b'>')).then_some(i + 1);
    }
    loop {
        let name = skip_whitespace(text, i);
        if name == i
            || !bytes
                .get(name)
                .is_some_and(|b| b.is_ascii_alphabetic() || matches!(b, b'_' | b':'))
        {
            break;
        }
        i = name
            + bytes[name..]
                .iter()
                .take_while(|b| b.is_ascii_alphanumeric() || matches!(b, b'_' | b'.' | b':' | b'-'))
                .count();
        let equals = skip_whitespace(text, i);
        if bytes.get(equals) == Some(&b'=') {
            let value = skip_whitespace(text, equals + 1);
            if let Some(len) = attribute_value_len(&bytes[value..]) {
                i = value + len;
            }
        }
    }
    i = skip_whitespace(text, i);
    if bytes.get(i) == Some(&b'/') {
        i += 1;
    }
    (bytes.get(i) == Some(&b'>')).then_some(i + 1)
}

fn attribute_value_len(value: &[u8]) -> Option<usize> {
    match *value.first()? {
        quote @ (b'"' | b'\'') => memchr::memchr(quote, &value[1..]).map(|n| n + 2),
        _ => {
            let len = value
                .iter()
                .take_while(|b| {
                    !matches!(b, b'"' | b'\'' | b'=' | b'<' | b'>' | b'`') && **b > b' '
                })
                .count();
            (len > 0).then_some(len)
        }
    }
}

/// The position after the whitespace, JavaScript's `\s`, at `text[from..]`.
fn skip_whitespace(text: &str, from: usize) -> usize {
    from + text[from..]
        .char_indices()
        .find(|(_, c)| !c.is_whitespace())
        .map_or(text.len() - from, |(at, _)| at)
}

/// The info string without its backslash escapes of ASCII punctuation
/// (§2.4); entity references are kept.
fn unescape(text: &str) -> String {
    let mut out = String::with_capacity(text.len());
    let mut chars = text.chars().peekable();
    while let Some(c) = chars.next() {
        if c == '\\' && chars.peek().is_some_and(char::is_ascii_punctuation) {
            continue;
        }
        out.push(c);
    }
    out
}

#[cfg(test)]
mod tests {
    use super::fenced_blocks;

    /// Each fenced block of `text` as commonmark.js 0.31.2 reports it: its
    /// first and last line, counted from 1, its info string and its content.
    fn blocks(text: &str) -> Vec<(usize, usize, String, String)> {
        let line = |at: usize| text[..at].matches('\n').count() + 1;
        fenced_blocks(text)
            .into_iter()
            .map(|block| {
                let last = line(block.end) - usize::from(text[..block.end].ends_with('\n'));
                (line(block.line_start), last, block.info, block.content)
            })
            .collect()
    }

    // The expectations are commonmark.js 0.31.2's `sourcepos`, `info` and
    // `literal` for the same texts.

    #[test]
    fn a_fence_inside_an_html_block_is_html() {
        assert_eq!(blocks("<div>\n```\nx\n```\n</div>\n"), []);
    }

    #[test]
    fn a_fence_indented_four_columns_is_indented_code() {
        assert_eq!(blocks("    ```\n    x\n    ```\n"), []);
    }

    #[test]
    fn a_fence_after_paragraph_text_interrupts_the_paragraph() {
        assert_eq!(
            blocks("text\n```\nx\n```\n"),
            [(2, 4, "".into(), "x\n".into())]
        );
    }

    #[test]
    fn a_fence_indented_after_paragraph_text_continues_the_paragraph() {
        assert_eq!(blocks("a\n    ```\n"), []);
    }

    #[test]
    fn a_block_in_a_block_quote_ends_with_the_quote() {
        assert_eq!(blocks("> ```\n> a\nb\n"), [(1, 2, "".into(), "a\n".into())]);
    }

    #[test]
    fn a_block_in_a_list_item_loses_the_item_indentation() {
        assert_eq!(
            blocks("- a\n\n  ```py\n  x\n  ```\n"),
            [(3, 5, "py".into(), "x\n".into())]
        );
    }

    #[test]
    fn a_tab_after_a_list_marker_is_partly_the_item_indentation() {
        assert_eq!(
            blocks("-\t```\n\t  x\n"),
            [(1, 2, "".into(), "  x\n".into())]
        );
    }

    #[test]
    fn a_backtick_in_the_info_string_makes_inline_code() {
        assert_eq!(blocks("``` a`b\nx\n"), []);
    }

    #[test]
    fn only_a_fence_at_least_as_long_closes_the_block() {
        assert_eq!(
            blocks("````\nx\n```\n`````\n"),
            [(1, 4, "".into(), "x\n```\n".into())]
        );
    }
}
