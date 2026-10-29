//! Regular expression literals delimited by `/`, which a lexer can only tell
//! from division by the token before them: ECMAScript selects its lexical goal
//! symbol from the syntactic context (ECMA-262 §12), and Ruby's parser from its
//! lexer state. Like most tools without a parser, this module approximates the
//! context by the previous significant byte or keyword.

use crate::utils::ext::SliceExt;

/// The syntax that decides whether a `/` starts a regular expression.
pub(crate) struct RegexContext<'a> {
    /// Keywords after which an expression, and so a regex, may follow.
    pub(crate) keywords: &'a [&'a str],
    pub(crate) line_comments: &'a [&'a str],
    pub(crate) multi_line_comments: &'a [(&'a str, &'a str)],
}

/// Length of the regular expression literal starting at `lines[i]`, or `None`
/// when the `/` there is division or starts a comment, or when no closing `/`
/// follows on the same line.
pub(crate) fn regex_literal_len(lines: &[u8], i: usize, context: &RegexContext) -> Option<usize> {
    let window = &lines[i..];
    if !window.starts_with(b"/") || matches!(window.get(1), Some(b'/' | b'*') | None) {
        return None;
    }
    if !regex_allowed_after(&lines[..context.code_end(lines, i)], context.keywords) {
        return None;
    }
    let mut in_class = false;
    let mut j = 1;
    while let Some(&byte) = window.get(j) {
        match byte {
            b'\n' | b'\r' => return None,
            b'\\' => j += 1,
            b'[' => in_class = true,
            b']' => in_class = false,
            b'/' if !in_class => {
                let flags = window[j + 1..]
                    .iter()
                    .take_while(|b| b.is_ascii_alphabetic())
                    .count();
                return Some(j + 1 + flags);
            }
            _ => {}
        }
        j += 1;
    }
    None
}

impl RegexContext<'_> {
    /// The end of the code before `i`, skipping whitespace, comments that
    /// close just before it and lines that are wholly line comments. A comment
    /// that follows code on its line is not skipped, since telling its marker
    /// from one inside a string would take the line's lexing state.
    ///
    /// Only an earlier line can be a line comment, so the look-back seeks a
    /// line start only after crossing a newline; seeking it for every `/`
    /// would be quadratic in the length of a minified line.
    fn code_end(&self, lines: &[u8], i: usize) -> usize {
        let mut end = i;
        loop {
            let code_end = end
                - lines[..end]
                    .iter()
                    .rev()
                    .take_while(|b| b.is_ascii_whitespace())
                    .count();
            let crossed_line = lines[code_end..end].contains(&b'\n');
            end = code_end;
            let before = &lines[..end];
            if let Some(start) = self.multi_line_comments.iter().find_map(|(open, close)| {
                let inner = before.strip_suffix(close.as_bytes())?;
                inner
                    .windows(open.len())
                    .rposition(|w| w == open.as_bytes())
            }) {
                end = start;
                continue;
            }
            if !crossed_line {
                return end;
            }
            let line_start = before
                .iter()
                .rposition(|b| *b == b'\n')
                .map_or(0, |n| n + 1);
            let line = before[line_start..].trim_start();
            if !line.is_empty()
                && self
                    .line_comments
                    .iter()
                    .any(|c| line.starts_with(c.as_bytes()))
            {
                end = line_start;
                continue;
            }
            return end;
        }
    }
}

/// Whether a regular expression may start after the code `before`, which ends
/// in its last token: at the start of the
/// text, after an operator or opening punctuation, or after a keyword such as
/// `return`, but not after a value such as an identifier, a number, a closing
/// bracket or a string. A regex after `}` or `<` is rare, while JSX writes
/// `</div>` and `<Foo {...p} />`, so both count as division.
fn regex_allowed_after(before: &[u8], keywords: &[&str]) -> bool {
    let Some(&previous) = before.last() else {
        return true;
    };
    if is_word_byte(previous) {
        let start = before.len()
            - before
                .iter()
                .rev()
                .take_while(|b| is_word_byte(**b))
                .count();
        return keywords.iter().any(|k| k.as_bytes() == &before[start..]);
    }
    !matches!(previous, b')' | b']' | b'}' | b'<' | b'"' | b'\'' | b'`')
}

fn is_word_byte(byte: u8) -> bool {
    byte.is_ascii_alphanumeric() || matches!(byte, b'_' | b'$') || byte >= 0x80
}
