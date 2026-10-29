//! Regular expression literals delimited by `/`, which a lexer can only tell
//! from division by the token before them: ECMAScript selects its lexical goal
//! symbol from the syntactic context (ECMA-262 §12), and Ruby's parser from its
//! lexer state. Like most tools without a parser, this module approximates the
//! context by the previous significant byte or keyword.

/// Length of the regular expression literal starting at `lines[i]`, or `None`
/// when the `/` there is division or starts a comment, or when no closing `/`
/// follows on the same line.
pub(crate) fn regex_literal_len(lines: &[u8], i: usize, keywords: &[&str]) -> Option<usize> {
    let window = &lines[i..];
    if !window.starts_with(b"/") || matches!(window.get(1), Some(b'/' | b'*') | None) {
        return None;
    }
    if !regex_allowed_after(&lines[..i], keywords) {
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

/// Whether a regular expression may start after `before`: at the start of the
/// text, after an operator or opening punctuation, or after a keyword such as
/// `return`, but not after a value such as an identifier, a number, a closing
/// bracket or a string. A regex after `}` or `<` is rare, while JSX writes
/// `</div>` and `<Foo {...p} />`, so both count as division.
fn regex_allowed_after(before: &[u8], keywords: &[&str]) -> bool {
    let end = before.len()
        - before
            .iter()
            .rev()
            .take_while(|b| b.is_ascii_whitespace())
            .count();
    let Some(&previous) = before[..end].last() else {
        return true;
    };
    if is_word_byte(previous) {
        let start = end
            - before[..end]
                .iter()
                .rev()
                .take_while(|b| is_word_byte(**b))
                .count();
        return keywords.iter().any(|k| k.as_bytes() == &before[start..end]);
    }
    !matches!(previous, b')' | b']' | b'}' | b'<' | b'"' | b'\'' | b'`')
}

fn is_word_byte(byte: u8) -> bool {
    byte.is_ascii_alphanumeric() || matches!(byte, b'_' | b'$') || byte >= 0x80
}
