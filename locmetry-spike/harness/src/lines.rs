//! Tokei 15.0.0's line rules (`line_is_comment`) applied to byte ranges, so
//! every lexer is judged by the same classification.

#[derive(Clone, Copy, PartialEq, Debug)]
pub enum Class {
    Code,
    Comment,
    Literal,
}

#[derive(Clone, Copy, Debug)]
pub struct Region {
    pub start: usize,
    pub end: usize,
    pub class: Class,
}

#[derive(Clone, Copy, Default, PartialEq, Debug)]
pub struct Counts {
    pub code: u64,
    pub comments: u64,
    pub blanks: u64,
}

#[derive(Clone, Copy, PartialEq, Debug)]
pub enum Line {
    Code,
    Comment,
    Blank,
}

/// Classes per byte, painting regions in order so that a region emitted after
/// an enclosing one wins: the code of an interpolation hole inside a literal,
/// and the strings and comments inside that hole.
pub fn paint(len: usize, regions: &[Region]) -> Vec<Class> {
    let mut class = vec![Class::Code; len];
    for region in regions {
        let end = region.end.min(len);
        if region.start < end {
            class[region.start..end].fill(region.class);
        }
    }
    class
}

pub fn lines(text: &str, regions: &[Region]) -> Vec<Line> {
    let bytes = text.as_bytes();
    let class = paint(bytes.len(), regions);
    let mut out = Vec::new();
    let mut start = 0;
    while start < bytes.len() {
        let end = bytes[start..]
            .iter()
            .position(|b| *b == b'\n')
            .map_or(bytes.len(), |p| start + p);
        let started_in = if start > 0 { class[start - 1] } else { Class::Code };
        let ends_in = if end < bytes.len() { class[end] } else { Class::Code };
        let mut first = None;
        let mut last = Class::Code;
        for i in start..end {
            if !bytes[i].is_ascii_whitespace() {
                first.get_or_insert(class[i]);
                last = class[i];
            }
        }
        out.push(match first {
            None if started_in == Class::Comment => Line::Comment,
            None if started_in == Class::Literal => Line::Code,
            None => Line::Blank,
            Some(_) if ends_in == Class::Literal => Line::Code,
            Some(_) if started_in == Class::Comment => Line::Comment,
            Some(Class::Comment) if last == Class::Comment => Line::Comment,
            Some(_) => Line::Code,
        });
        start = end + 1;
    }
    out
}

pub fn count(lines: &[Line]) -> Counts {
    let mut counts = Counts::default();
    for line in lines {
        match line {
            Line::Code => counts.code += 1,
            Line::Comment => counts.comments += 1,
            Line::Blank => counts.blanks += 1,
        }
    }
    counts
}
