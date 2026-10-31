//! Haml's comments, as Haml 7.5.1's parser (`Haml::Parser`) reads them: by
//! line and indentation, with no strings outside the Ruby code of scripts and
//! attributes, which hold no comment lines. A `-#` comment and a `/` HTML
//! comment, but not a revealed `/![…]` conditional comment, take in every
//! line indented beneath them, blank lines between them included; a filter
//! such as `:javascript` takes in its body the same way, as text, so a `-#`
//! there is no comment. Blank lines after a block's last line belong to no
//! block.

use std::ops::Range;

use grep_searcher::LineStep;

use crate::language::{language_type::LineReport, syntax::LineClass};
use crate::utils::ext::SliceExt;
use crate::{CodeStats, Config};

#[derive(Clone, Copy, PartialEq)]
enum Block {
    Comment,
    Filter,
}

pub(crate) fn parse(
    _config: &Config,
    text: &[u8],
    mut stats: CodeStats,
    counted: Range<usize>,
    on_line: &mut impl FnMut(LineReport<'_>),
) -> CodeStats {
    let mut lines = Vec::new();
    let mut stepper = LineStep::new(b'\n', 0, text.len());
    while let Some((start, end)) = stepper.next(text) {
        lines.push(start..end);
    }
    for (line, class) in lines.iter().zip(classes(text, &lines)) {
        if counted.contains(&line.start) {
            stats.add_line(class);
            on_line(LineReport::Class(Some(class)));
        }
    }
    stats
}

fn classes(text: &[u8], lines: &[Range<usize>]) -> Vec<LineClass> {
    let mut classes = Vec::with_capacity(lines.len());
    // The open block with its marker's indentation, and the blank lines since
    // its last line, which are in it only if a line beneath it follows.
    let mut block: Option<(usize, Block)> = None;
    let mut blanks = 0;
    for line in lines {
        let line = &text[line.clone()];
        let indent = line
            .iter()
            .take_while(|&&b| b == b' ' || b == b'\t')
            .count();
        let content = line[indent..].trim_end();
        if content.is_empty() {
            classes.push(LineClass::Blank);
            blanks += 1;
            continue;
        }
        if let Some((marker, kind)) = block {
            if indent > marker {
                let class = match kind {
                    Block::Comment => LineClass::Comment,
                    Block::Filter => LineClass::Code,
                };
                if kind == Block::Comment {
                    let len = classes.len();
                    classes[len - blanks..].fill(LineClass::Comment);
                }
                classes.push(class);
                blanks = 0;
                continue;
            }
            block = None;
        }
        blanks = 0;
        classes.push(
            if content.starts_with(b"-#")
                || content.starts_with(b"/") && !content.starts_with(b"/![")
            {
                block = Some((indent, Block::Comment));
                LineClass::Comment
            } else {
                if is_filter(content) {
                    block = Some((indent, Block::Filter));
                }
                LineClass::Code
            },
        );
    }
    classes
}

/// A filter's opening line: `:` and a name of word characters (`Parser#filter`).
fn is_filter(content: &[u8]) -> bool {
    content.strip_prefix(b":").is_some_and(|name| {
        !name.is_empty() && name.iter().all(|b| b.is_ascii_alphanumeric() || *b == b'_')
    })
}
