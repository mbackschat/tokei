//! GNU make's references, line splices and `define` bodies, as GNU make
//! 4.4.1's `src/read.c` reads them.

/// The length of the reference at `lines[i]`, whose `#` starts no comment
/// (`find_map_unquote` with `MAP_VARIABLE`): `$(...)` or `${...}` up to the
/// closer that balances its opener, counting that opener alone, or `$` and
/// the byte after it, as in `$$` or `$@`. None where the logical line ends
/// first.
pub(crate) fn reference_len(lines: &[u8], i: usize) -> Option<usize> {
    let open = *lines.get(i + 1)?;
    let close = match open {
        b'(' => b')',
        b'{' => b'}',
        b'\n' | b'\r' => return None,
        _ => return Some(2),
    };
    let mut depth = 0usize;
    for (j, &byte) in lines.iter().enumerate().skip(i + 2) {
        if byte == open {
            depth += 1;
        } else if byte == close {
            if depth == 0 {
                return Some(j + 1 - i);
            }
            depth -= 1;
        } else if byte == b'\n' && !ends_in_splice(&lines[..j]) {
            return None;
        }
    }
    None
}

/// Whether `line`, up to its newline, joins the next: `readline` joins a line
/// ending in an odd number of backslashes, with nothing after them but a
/// carriage return.
pub(crate) fn ends_in_splice(line: &[u8]) -> bool {
    let line = line.strip_suffix(b"\r").unwrap_or(line);
    line.iter().rev().take_while(|b| **b == b'\\').count() % 2 == 1
}

/// Whether the line starting at `lines[start]` lies in a `define` body,
/// which `do_define` keeps unread until `$(eval)` or a recipe reads it. Only
/// a line whose first word is `define` or `endef`, and that does not start
/// with a tab, opens or closes one; a `define` after modifiers such as
/// `override` opens one too.
pub(crate) fn in_define_body(lines: &[u8], start: usize) -> bool {
    if directive(line_at(lines, start)) == Some(Directive::Endef) {
        return false;
    }
    let mut depth = 0usize;
    let mut end = start;
    while end > 0 {
        let line_start = memchr::memrchr(b'\n', &lines[..end - 1]).map_or(0, |n| n + 1);
        match directive(&lines[line_start..end - 1]) {
            Some(Directive::Endef) => depth += 1,
            Some(Directive::Define) if depth == 0 => return true,
            Some(Directive::Define) => depth -= 1,
            None => {}
        }
        end = line_start;
    }
    false
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum Directive {
    Define,
    Endef,
}

fn line_at(lines: &[u8], start: usize) -> &[u8] {
    let end = memchr::memchr(b'\n', &lines[start..]).map_or(lines.len(), |n| start + n);
    &lines[start..end]
}

fn directive(line: &[u8]) -> Option<Directive> {
    if line.first() == Some(&b'\t') {
        return None;
    }
    let mut words = line
        .split(|b| b.is_ascii_whitespace())
        .filter(|word| !word.is_empty())
        .skip_while(|word| matches!(*word, b"override" | b"export" | b"private"));
    match words.next()? {
        b"define" => Some(Directive::Define),
        b"endef" => Some(Directive::Endef),
        _ => None,
    }
}
