//! POD blocks, documentation in whole lines that the language skips. Perl's
//! start with a line beginning with `=` and a command word, such as `=head1`,
//! and end after the next line beginning with `=cut` (perlpod, perlsyn "PODs:
//! Embedded Documentation"); NQP's are the `pod_comment` token of its grammar
//! (src/NQP/Grammar.nqp at Raku/nqp `e5e5a90`).

use crate::language::quote_like::is_word_byte;
use crate::utils::ext::SliceExt;

/// Which language's POD rules apply.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum PodSyntax {
    Perl,
    Nqp,
}

/// The end of the POD block whose first line starts at `lines[start]`, after
/// its last line or at the end of the text, or `None` when no block starts
/// there.
pub(crate) fn pod_block_end(lines: &[u8], start: usize, syntax: PodSyntax) -> Option<usize> {
    match syntax {
        PodSyntax::Perl => perl_block_end(lines, start),
        PodSyntax::Nqp => nqp_block_end(lines, start),
    }
}

fn perl_block_end(lines: &[u8], start: usize) -> Option<usize> {
    let starts_command = lines.get(start) == Some(&b'=')
        && lines.get(start + 1).is_some_and(u8::is_ascii_alphabetic);
    starts_command.then(|| {
        end_after_line(lines, start, |line| {
            line.starts_with(b"=cut") && !line.get(4).is_some_and(u8::is_ascii_alphabetic)
        })
    })
}

/// `=begin NAME` runs through the line `=end NAME`, the rest of that line
/// included; a bare `=begin` through the next `=end`; `=NAME` through the
/// last line before an empty line or a line starting with `=`; and `=`
/// followed by whitespace is its own line only.
fn nqp_block_end(lines: &[u8], start: usize) -> Option<usize> {
    let first = &lines[start..line_end(lines, start)];
    let directive = first.trim_start().strip_prefix(b"=")?;
    let Some(rest) = directive
        .strip_prefix(b"begin")
        .filter(|rest| !rest.first().is_some_and(|b| is_word(*b)))
    else {
        return paragraph_end(lines, start, directive);
    };
    let name_text = rest.trim_start();
    let name = &name_text[..name_text.iter().take_while(|b| is_word(**b)).count()];
    let closes = |line: &[u8]| {
        let Some(after) = line.trim_start().strip_prefix(b"=end") else {
            return false;
        };
        if name.is_empty() {
            return !after.first().is_some_and(|b| is_word(*b));
        }
        let named = after.trim_start();
        named.len() < after.len()
            && named.starts_with(name)
            && !named.get(name.len()).is_some_and(|b| is_word(*b))
    };
    Some(end_after_line(lines, line_end(lines, start) + 1, closes))
}

/// The end of an NQP `=NAME` paragraph block, or of an `=` line alone.
fn paragraph_end(lines: &[u8], start: usize, directive: &[u8]) -> Option<usize> {
    let first_end = (line_end(lines, start) + 1).min(lines.len());
    if !directive.first().is_some_and(|b| is_word(*b)) {
        return directive
            .first()
            .is_some_and(u8::is_ascii_whitespace)
            .then_some(first_end);
    }
    let mut next = first_end;
    while next < lines.len() {
        let end = line_end(lines, next);
        let line = &lines[next..end];
        if line.is_empty() || line.trim_start().starts_with(b"=") {
            return Some(next);
        }
        next = (end + 1).min(lines.len());
    }
    Some(lines.len())
}

/// The end of the first line at or after `from` for which `last` holds, its
/// newline included, or the end of the text.
fn end_after_line(lines: &[u8], from: usize, last: impl Fn(&[u8]) -> bool) -> usize {
    let mut line_start = from;
    while line_start < lines.len() {
        let end = line_end(lines, line_start);
        if last(&lines[line_start..end]) {
            return (end + 1).min(lines.len());
        }
        line_start = end + 1;
    }
    lines.len()
}

fn line_end(lines: &[u8], start: usize) -> usize {
    let start = start.min(lines.len());
    lines[start..]
        .iter()
        .position(|b| *b == b'\n')
        .map_or(lines.len(), |n| start + n)
}

/// NQP's identifier characters, `\w` with the `-` and `'` that join words.
fn is_word(byte: u8) -> bool {
    byte.is_ascii_alphanumeric() || matches!(byte, b'_' | b'-' | b'\'')
}

/// The position of the opening delimiter and the number of delimited parts of
/// the quote-like operator at `lines[i]` (perlop, "Quote and Quote-like
/// Operators"): one for `q`, `qq`, `qw`, `qx`, `qr` and `m`, two for `s`,
/// `tr` and `y`. Its delimiter is the next character that is not whitespace,
/// whichever it is, as in `m,/,` or `m;^;`. The word is a name instead where
/// PPI 1.270's tokenizer forces one
/// (`PPI::Tokenizer::__current_token_is_forced_word`): after a sigil, `$#` as
/// in `$#y`, `->` or `sub`, `package`, `use` or `no`, before `=>` or `::`, or
/// alone in braces, as in `$h{s}`. A `#` after whitespace starts a comment,
/// and the delimiter on a later line is not looked for.
pub(crate) fn quote_operator(lines: &[u8], i: usize) -> Option<(usize, usize)> {
    let before = &lines[..i];
    let code_before = before.trim_end();
    if before
        .last()
        .is_some_and(|b| is_word_byte(*b) || matches!(b, b'$' | b'@' | b'%' | b'&' | b'*' | b'-'))
        || code_before.ends_with(b"->")
        || before.ends_with(b"::")
        || before.ends_with(b"$#")
        || previous_word(code_before)
            .is_some_and(|word| matches!(word, b"sub" | b"package" | b"use" | b"no"))
    {
        return None;
    }
    let word_len = lines[i..].iter().take_while(|b| is_word_byte(**b)).count();
    let parts = match &lines[i..i + word_len] {
        b"q" | b"qq" | b"qw" | b"qx" | b"qr" | b"m" => 1,
        b"s" | b"tr" | b"y" => 2,
        _ => return None,
    };
    let after = &lines[i + word_len..];
    let code_after = after.trim_start();
    if code_after.starts_with(b"=>")
        || after.starts_with(b"::")
        || (code_before.ends_with(b"{") && code_after.starts_with(b"}"))
    {
        return None;
    }
    let spaces = after
        .iter()
        .take_while(|b| matches!(b, b' ' | b'\t'))
        .count();
    let open = i + word_len + spaces;
    let delimiter = *lines.get(open)?;
    let opens = delimiter.is_ascii_graphic() && !(delimiter == b'#' && spaces > 0);
    opens.then_some((open, parts))
}

/// The word that `code` ends in, if it ends in one that no sigil or `->`
/// makes a variable or method name.
fn previous_word(code: &[u8]) -> Option<&[u8]> {
    let len = code.iter().rev().take_while(|b| is_word_byte(**b)).count();
    let start = code.len() - len;
    let word = &code[start..];
    let named = code[..start]
        .last()
        .is_some_and(|b| matches!(b, b'$' | b'@' | b'%' | b'&' | b'*'))
        || code[..start].ends_with(b"->");
    (len > 0 && !named).then_some(word)
}
