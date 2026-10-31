//! COBOL's reference format, as GnuCOBOL's preprocessor reads it (`ppinput`
//! in `cobc/pplex.l` at gnucobol `eda8905e44`). In fixed form, columns 1 to
//! 6 hold a sequence number and columns 73 on an identification, both
//! ignored; column 7 is the indicator, `*` or `/` for a comment line, `-` for
//! a continuation line, `D` for a debugging line, which is a comment until
//! `WITH DEBUGGING MODE` turns debugging lines on. In both forms a `*>`
//! opens a floating comment, a line whose first text is `>>` or `$` is a
//! compiler directive, and in the identification division the comment
//! paragraphs, such as `AUTHOR.`, are comments, in fixed form with the
//! following lines whose area A, columns 8 to 11, is blank. A literal can
//! only follow code on its line, so it never changes a line's class.

use std::{borrow::Cow, ops::Range};

use grep_searcher::LineStep;

use crate::language::{language_type::LineReport, syntax::LineClass};
use crate::{CodeStats, Config};

/// The indicators that let GnuCOBOL's default `auto` format keep a file in
/// fixed form (`is_fixed_indicator`).
const FIXED_INDICATORS: &[u8] = b" -dD*/\\$";
/// The paragraphs whose text is a comment (`next_word_is_comment_paragraph_name`).
const COMMENT_PARAGRAPHS: &[&[u8]] = &[
    b"AUTHOR",
    b"REMARKS",
    b"SECURITY",
    b"DATE-WRITTEN",
    b"INSTALLATION",
    b"DATE-MODIFIED",
    b"DATE-COMPILED",
];
const TAB_WIDTH: usize = 8;
const INDICATOR: usize = 6;
const TEXT_END: usize = 72;

pub(crate) fn parse(
    _config: &Config,
    text: &[u8],
    mut stats: CodeStats,
    counted: Range<usize>,
    on_line: &mut impl FnMut(LineReport<'_>),
) -> CodeStats {
    let mut reader = Reader::new(text);
    let mut stepper = LineStep::new(b'\n', 0, text.len());
    while let Some((start, end)) = stepper.next(text) {
        let class = reader.class(&text[start..end]);
        if counted.contains(&start) {
            stats.add_line(class);
            on_line(LineReport::Class(Some(class)));
        }
    }
    stats
}

/// The state that carries from line to line.
struct Reader {
    free: bool,
    debugging: bool,
    comment_paragraphs_allowed: bool,
    in_comment_paragraph: bool,
}

impl Reader {
    fn new(text: &[u8]) -> Self {
        Reader {
            free: starts_free(text),
            debugging: false,
            comment_paragraphs_allowed: true,
            in_comment_paragraph: false,
        }
    }

    fn class(&mut self, line: &[u8]) -> LineClass {
        let line = expand_tabs(line.strip_suffix(b"\n").unwrap_or(line));
        let line = line.strip_suffix(b"\r").unwrap_or(&line);
        if line.iter().all(u8::is_ascii_whitespace) {
            return LineClass::Blank;
        }
        if self.free {
            self.free_class(line)
        } else {
            self.fixed_class(line)
        }
    }

    fn free_class(&mut self, line: &[u8]) -> LineClass {
        let text = trim_start(line);
        // A comment paragraph continues on later lines only in fixed form.
        self.in_comment_paragraph = false;
        if text.starts_with(b"*>") {
            return LineClass::Comment;
        }
        if let Some(class) = self.directive_class(text) {
            return class;
        }
        if self.comment_paragraphs_allowed && opens_comment_paragraph(text) {
            return LineClass::Comment;
        }
        self.read_code(text);
        LineClass::Code
    }

    fn fixed_class(&mut self, line: &[u8]) -> LineClass {
        let Some(&indicator) = line.get(INDICATOR) else {
            // Only a sequence number.
            return LineClass::Comment;
        };
        let program = &line[INDICATOR..line.len().min(TEXT_END)];
        let first = trim_start(program);
        if first.starts_with(b"*>") {
            return LineClass::Comment;
        }
        if let Some(class) = self.directive_class(first) {
            return class;
        }
        if self.comment_paragraphs_allowed && opens_comment_paragraph(program) {
            self.in_comment_paragraph = true;
            return LineClass::Comment;
        }
        match indicator {
            b'-' => return LineClass::Code,
            b'D' | b'd' if self.debugging => {}
            b' ' => {}
            // Comment lines, debugging lines outside debugging mode and, as
            // cobc reads them after an error, invalid indicators.
            _ => return LineClass::Comment,
        }
        let text = &program[1..];
        if self.in_comment_paragraph {
            if text.iter().take(4).all(|b| *b == b' ') {
                return LineClass::Comment;
            }
            self.in_comment_paragraph = false;
        }
        if text.iter().all(|b| *b == b' ') {
            // Text only in the sequence or identification area.
            return LineClass::Comment;
        }
        self.read_code(text);
        LineClass::Code
    }

    /// The class of a line whose first text is `text` when it is a compiler
    /// directive, switching the form after a `>>SOURCE FORMAT` or a
    /// `$SET SOURCEFORMAT` one.
    fn directive_class(&mut self, text: &[u8]) -> Option<LineClass> {
        let directive = match text {
            [b'>', b'>', rest @ ..] => rest,
            [b'$', next, ..] if *next != b' ' => &text[1..],
            _ => return None,
        };
        let words = words(directive);
        let debugging_line = text.starts_with(b">>")
            && directive
                .first()
                .is_some_and(|b| b.eq_ignore_ascii_case(&b'D'))
            && !words.first().is_some_and(|w| {
                w.eq_ignore_ascii_case(b"DEFINE") || w.eq_ignore_ascii_case(b"DISPLAY")
            });
        if debugging_line {
            // `>>D` and the line's text, unless a longer directive starts so
            // (pplex's `">>D"` rule).
            return Some(if self.debugging {
                LineClass::Code
            } else {
                LineClass::Comment
            });
        }
        if let Some(form) = source_form(&words) {
            if form.eq_ignore_ascii_case(b"FREE") {
                self.free = true;
            } else if form.eq_ignore_ascii_case(b"FIXED") {
                self.free = false;
            }
        }
        Some(LineClass::Code)
    }

    /// Follows the words of a code line that open and close the comment
    /// paragraphs and turn on debugging lines.
    fn read_code(&mut self, text: &[u8]) {
        let code = code_part(text);
        let words = words(&code);
        for (index, word) in words.iter().enumerate() {
            let previous = index.checked_sub(1).map(|i| words[i]);
            if word.eq_ignore_ascii_case(b"PROGRAM-ID") {
                self.comment_paragraphs_allowed = true;
            } else if word.eq_ignore_ascii_case(b"DIVISION") {
                self.comment_paragraphs_allowed = previous.is_some_and(|p| {
                    p.eq_ignore_ascii_case(b"IDENTIFICATION") || p.eq_ignore_ascii_case(b"ID")
                });
            } else if word.eq_ignore_ascii_case(b"SECTION") {
                self.comment_paragraphs_allowed = false;
            } else if word.eq_ignore_ascii_case(b"MODE")
                && previous.is_some_and(|p| p.eq_ignore_ascii_case(b"DEBUGGING"))
            {
                self.debugging = true;
            }
        }
    }
}

/// Whether a file starts in free form: GnuCOBOL's default `auto` format
/// reads it so when the first line that reaches column 7 has no fixed-form
/// indicator there (`ppopen_get_file`); a `*>` starting in columns 1 to 6
/// also marks free form, as fixed form leaves those columns to the sequence
/// number and every measured file holding one is built with `cobc -free`.
fn starts_free(text: &[u8]) -> bool {
    let lines = || text.split(|b| *b == b'\n').map(expand_tabs);
    let early_comment = |line: &[u8]| {
        let spaces = line.iter().take_while(|b| **b == b' ').count();
        spaces < INDICATOR && line[spaces..].starts_with(b"*>")
    };
    if lines().any(|line| early_comment(&line)) {
        return true;
    }
    lines()
        .find_map(|line| line.get(INDICATOR).copied())
        .is_some_and(|indicator| !FIXED_INDICATORS.contains(&indicator))
}

/// The form a directive's words name, after `>>SOURCE [FORMAT] [IS]` or a
/// `$SET` option `SOURCEFORMAT` or `SOURCE-FORMAT` (pplex's `SOURCE` and
/// `SOURCEFORMAT` rules).
fn source_form<'a>(words: &[&'a [u8]]) -> Option<&'a [u8]> {
    let (first, rest) = words.split_first()?;
    if first.eq_ignore_ascii_case(b"SOURCE") {
        rest.iter()
            .find(|w| !w.eq_ignore_ascii_case(b"FORMAT") && !w.eq_ignore_ascii_case(b"IS"))
            .copied()
    } else if first.eq_ignore_ascii_case(b"SET") {
        let option = rest.iter().position(|w| {
            w.eq_ignore_ascii_case(b"SOURCEFORMAT") || w.eq_ignore_ascii_case(b"SOURCE-FORMAT")
        })?;
        rest.get(option + 1).copied()
    } else {
        None
    }
}

/// Whether `text`, from the indicator on in fixed form, opens with the name
/// of a comment paragraph.
fn opens_comment_paragraph(text: &[u8]) -> bool {
    let text = &text[text.iter().take_while(|b| b" ,;".contains(b)).count()..];
    let len = text.iter().take_while(|b| is_word_byte(**b)).count();
    COMMENT_PARAGRAPHS
        .iter()
        .any(|name| text[..len].eq_ignore_ascii_case(name))
}

/// `text` up to a floating comment, with its literals blanked out.
fn code_part(text: &[u8]) -> Vec<u8> {
    let mut code = Vec::with_capacity(text.len());
    let mut quote = None;
    let mut i = 0;
    while i < text.len() {
        let byte = text[i];
        match quote {
            Some(q) if byte == q && text.get(i + 1) == Some(&q) => i += 1,
            Some(q) if byte == q => quote = None,
            Some(_) => {}
            None if byte == b'"' || byte == b'\'' => quote = Some(byte),
            None if text[i..].starts_with(b"*>") => break,
            None => code.push(byte),
        }
        if quote.is_some() {
            code.push(b' ');
        }
        i += 1;
    }
    code
}

fn words(text: &[u8]) -> Vec<&[u8]> {
    text.split(|b| !is_word_byte(*b))
        .filter(|w| !w.is_empty())
        .collect()
}

fn is_word_byte(byte: u8) -> bool {
    byte.is_ascii_alphanumeric() || byte == b'-' || byte == b'_'
}

fn trim_start(text: &[u8]) -> &[u8] {
    &text[text.iter().take_while(|b| **b == b' ').count()..]
}

/// `line` with its tabs expanded to the columns GnuCOBOL gives them.
fn expand_tabs(line: &[u8]) -> Cow<'_, [u8]> {
    if !line.contains(&b'\t') {
        return Cow::Borrowed(line);
    }
    let mut expanded = Vec::with_capacity(line.len() + TAB_WIDTH);
    for &byte in line {
        if byte == b'\t' {
            expanded.push(b' ');
            while expanded.len() % TAB_WIDTH != 0 {
                expanded.push(b' ');
            }
        } else {
            expanded.push(byte);
        }
    }
    Cow::Owned(expanded)
}
