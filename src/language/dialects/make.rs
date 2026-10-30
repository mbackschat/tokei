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

/// Whether the class of a line a splice joins to `line` may depend on what
/// `line` holds: a `#` that may start a comment the splice continues, or a
/// reference the splice carries on. Every other line's class is that of its
/// first bytes, which the per-line fast path reads; taking all spliced lines,
/// or all lines holding a `#`, through the full parse cost Linux's makefiles
/// at `d89e1d1` about 1.1 to 1.2 times upstream's time (measured 2026-10-30).
#[inline]
pub(crate) fn needs_full_parse(line: &[u8]) -> bool {
    if !ends_in_splice(line) {
        return false;
    }
    if memchr::memchr(b'#', line).is_some() {
        return true;
    }
    let mut i = 0;
    while let Some(at) = memchr::memchr(b'$', &line[i..]).map(|n| i + n) {
        match reference_len(line, at) {
            Some(len) => i = at + len,
            None => return matches!(line.get(at + 1), Some(b'(' | b'{')),
        }
    }
    false
}

/// Whether `line`, up to its newline, joins the next: `readline` joins a line
/// ending in an odd number of backslashes, with nothing after them but a
/// carriage return.
pub(crate) fn ends_in_splice(line: &[u8]) -> bool {
    let line = line.strip_suffix(b"\r").unwrap_or(line);
    line.iter().rev().take_while(|b| **b == b'\\').count() % 2 == 1
}

/// How far a file's `define` bodies have been read, so that asking whether
/// a line lies in one reads the file once. Scanning back to the start of the
/// file for each continued comment instead took Linux's makefiles,
/// concatenated into one file of 713,600 lines, 1.9 times upstream's time,
/// and reading each line forward still 1.15 times (measured 2026-10-30).
#[derive(Clone, Debug, Default)]
pub(crate) struct Defines {
    read: usize,
    depth: usize,
}

impl Defines {
    /// Whether the line starting at `lines[start]` lies in a `define` body,
    /// which `do_define` keeps unread until `$(eval)` or a recipe reads it.
    /// Outside one, a line whose first word after modifiers such as
    /// `override` is `define` opens one; inside, a line whose first word is
    /// `define` nests and one whose first word is `endef` closes. Lines
    /// starting with a tab do neither. Lines are asked in order.
    pub(crate) fn contain(&mut self, lines: &[u8], start: usize) -> bool {
        if start < self.read {
            *self = Self::default();
        }
        // Only a line holding `def`, as `define` and `endef` do, can change
        // the depth, so the scan jumps from one to the next.
        while let Some(at) =
            memchr::memmem::find(&lines[self.read..start], b"def").map(|n| self.read + n)
        {
            let line_start = memchr::memrchr(b'\n', &lines[..at]).map_or(0, |n| n + 1);
            let end = memchr::memchr(b'\n', &lines[at..]).map_or(lines.len(), |n| at + n);
            self.depth = self.depth_after(&lines[line_start..end]);
            self.read = (end + 1).min(start);
        }
        self.read = start;
        self.depth > 0 && self.depth_after(line_at(lines, start)) > 0
    }

    fn depth_after(&self, line: &[u8]) -> usize {
        if line.first() == Some(&b'\t') {
            return self.depth;
        }
        let mut words = line
            .split(|b| b.is_ascii_whitespace())
            .filter(|word| !word.is_empty());
        if self.depth == 0 {
            let opens = words
                .find(|word| !matches!(*word, b"override" | b"export" | b"private"))
                .is_some_and(|word| word == b"define");
            return usize::from(opens);
        }
        match words.next() {
            Some(b"define") => self.depth + 1,
            Some(b"endef") => self.depth - 1,
            _ => self.depth,
        }
    }
}

fn line_at(lines: &[u8], start: usize) -> &[u8] {
    let end = memchr::memchr(b'\n', &lines[start..]).map_or(lines.len(), |n| start + n);
    &lines[start..end]
}
