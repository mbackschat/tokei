//! Ruby's percent literals, such as `%w[a b]` (Ruby documentation,
//! "Literals", Percent Literals), its `?` character literals, and the
//! arguments of methods called without parentheses, which a `/` or `%` may
//! open.

use crate::language::quote_like::is_word_byte;
use crate::language::regex_literal::{self, RegexContext};
use crate::utils::ext::utf8_len;

/// The position of the opening delimiter of the Ruby percent literal at
/// `lines[i]`, such as `%w[`, `%r{` or `%(`, which is one part. A typed
/// literal starts where an expression may, or after a space as a method's
/// argument; a bare `%(` only where an expression may, since it is the
/// modulo operator after a value.
pub(crate) fn percent_literal(
    lines: &[u8],
    i: usize,
    context: &RegexContext,
) -> Option<(usize, usize)> {
    if lines[i] != b'%' {
        return None;
    }
    let next = *lines.get(i + 1)?;
    let typed = b"qQwWiIrsx".contains(&next);
    let open = if typed { i + 2 } else { i + 1 };
    let delimiter = *lines.get(open)?;
    // After `:` the `%` is a symbol, as in `method?(:%)`, and a closing
    // bracket cannot open a literal.
    if !delimiter.is_ascii_punctuation()
        || matches!(delimiter, b'=' | b')' | b']' | b'}' | b'>')
        || lines[..i].last() == Some(&b':')
    {
        return None;
    }
    let after_space = lines[..i].last().is_some_and(|b| *b == b' ' || *b == b'\t');
    let starts = context.expression_may_start(lines, i)
        || (typed && after_space)
        || starts_command_argument(lines, i);
    starts.then_some((open, 1))
}

/// Length of the Ruby character literal at `lines[i]`, a `?` and one
/// character or escape sequence, as in `?a`, `?/` or `?\n` (Ruby
/// `doc/syntax/literals.rdoc`, "Character Literal"). A `?` that ends a name,
/// such as `include?` or the global `$?`, or follows a closing bracket, or
/// comes before whitespace is an operator or part of a name; so is one before
/// two identifier characters, as in `a ?bc : d` (parse.y, `parser_yylex`).
pub(crate) fn character_len(lines: &[u8], i: usize) -> Option<usize> {
    if lines[i] != b'?'
        || lines[..i]
            .last()
            .is_some_and(|b| is_word_byte(*b) || matches!(b, b'$' | b')' | b']' | b'}'))
    {
        return None;
    }
    let body = &lines[i + 1..];
    let first = *body.first()?;
    if first.is_ascii_whitespace()
        || (is_word_byte(first) && body.get(1).is_some_and(|b| is_word_byte(*b)))
    {
        return None;
    }
    let escape = usize::from(first == b'\\');
    Some(1 + escape + utf8_len(*body.get(escape)?))
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
    if !matches!(before.last(), Some(b' ' | b'\t'))
        || !lines
            .get(i + 1)
            .is_some_and(|b| !b.is_ascii_whitespace() && *b != b'=')
    {
        return false;
    }
    let code = &before[..before
        .iter()
        .rposition(|b| !matches!(b, b' ' | b'\t'))
        .map_or(0, |n| n + 1)];
    let name = code
        .strip_suffix(b"?")
        .or_else(|| code.strip_suffix(b"!"))
        .unwrap_or(code);
    let word = &name[name.len()
        - name
            .iter()
            .rev()
            .take_while(|b| regex_literal::is_word_byte(**b))
            .count()..];
    !word.is_empty() && !NOT_COMMANDS.contains(&word)
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
