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
    /// Whether a line break before the `/` ends the statement, so that the
    /// `/` starts an expression.
    pub(crate) after_line_break: bool,
    /// Whether a regular expression may span lines, as Ruby's may; an
    /// ECMAScript one may not (ECMA-262 §12.9.6, RegularExpressionLiteral).
    pub(crate) spans_lines: bool,
    /// Whether a `:` touching the `/` makes the symbol `:/`, Ruby's name for
    /// the division method, as in `calculate(:/, other)`.
    pub(crate) operator_symbols: bool,
    /// Whether a `/` after a method name and a space, but before none, opens
    /// a regular expression as the method's argument, as in Ruby.
    pub(crate) command_arguments: bool,
    pub(crate) line_comments: &'a [&'a str],
    pub(crate) multi_line_comments: &'a [(&'a str, &'a str)],
}

/// Length of the regular expression literal starting at `lines[i]`, or `None`
/// when the `/` there is division or starts a comment, or when no closing `/`
/// follows on the same line or, where a regex may span lines, in the text.
pub(crate) fn regex_literal_len(lines: &[u8], i: usize, context: &RegexContext) -> Option<usize> {
    let window = &lines[i..];
    // The second `/` of `//`, Perl's defined-or operator, starts nothing.
    if !window.starts_with(b"/")
        || matches!(window.get(1), Some(b'/' | b'*') | None)
        || lines[..i].last() == Some(&b'/')
    {
        return None;
    }
    if (context.operator_symbols && lines[..i].last() == Some(&b':'))
        || !(context.expression_may_start(lines, i)
            || (context.command_arguments && starts_command_argument(lines, i)))
    {
        return None;
    }
    let mut in_class = false;
    let mut j = 1;
    while let Some(&byte) = window.get(j) {
        match byte {
            b'\n' | b'\r' if !context.spans_lines => return None,
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
    /// Whether an expression, such as a regular expression or a JSX element,
    /// may start at `lines[i]`, judged by the code before it.
    pub(crate) fn expression_may_start(&self, lines: &[u8], i: usize) -> bool {
        let code_end = self.code_end(lines, i);
        (self.after_line_break && lines[code_end..i].contains(&b'\n'))
            || regex_allowed_after(&lines[..code_end], self.keywords)
    }

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
    // A `<` opens a JSX tag or type arguments, but `<<`, a shift or Ruby's
    // append, is an operator before an operand.
    if previous == b'<' {
        return before.ends_with(b"<<");
    }
    !matches!(previous, b')' | b']' | b'}' | b'"' | b'\'' | b'`')
}

/// Whether the operator-like byte at `lines[i]`, after a space but before no
/// space or `=`, starts the argument of a method called without parentheses,
/// as the `%` of `assert_equal %{a}` or the `/` of `match /re/` do in Ruby
/// (parse.y, `IS_SPCARG`). Before it stands a name, which may end in `?` or
/// `!`; were the name a local variable, the byte would be an operator, which
/// only a parser can tell. A keyword that ends a value or names a method,
/// such as `end` or `def`, takes no argument.
pub(crate) fn starts_command_argument(lines: &[u8], i: usize) -> bool {
    let before = &lines[..i];
    let code = &before[..before
        .iter()
        .rposition(|b| !matches!(b, b' ' | b'\t'))
        .map_or(0, |n| n + 1)];
    let name = code
        .strip_suffix(b"?")
        .or_else(|| code.strip_suffix(b"!"))
        .unwrap_or(code);
    let word = &name[name.len() - name.iter().rev().take_while(|b| is_word_byte(**b)).count()..];
    code.len() < before.len()
        && !word.is_empty()
        && !NOT_COMMANDS.contains(&word)
        && lines
            .get(i + 1)
            .is_some_and(|b| !b.is_ascii_whitespace() && *b != b'=')
}

/// Ruby keywords after which the lexer expects an operator or a method name
/// (parse.y lex states `EXPR_END` and `EXPR_FNAME`).
const NOT_COMMANDS: &[&[u8]] = &[
    b"def",
    b"end",
    b"self",
    b"nil",
    b"true",
    b"false",
    b"alias",
    b"undef",
    b"__FILE__",
    b"__LINE__",
    b"__ENCODING__",
];

pub(crate) fn is_word_byte(byte: u8) -> bool {
    byte.is_ascii_alphanumeric() || matches!(byte, b'_' | b'$') || byte >= 0x80
}
