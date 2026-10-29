//! Perl's POD blocks, documentation that starts with a line beginning with
//! `=` and a command word, such as `=head1`, and ends after the next line
//! beginning with `=cut`; Perl skips them (perlpod, perlsyn "PODs: Embedded
//! Documentation").

/// The end of the POD block whose first line starts at `lines[start]`, after
/// its `=cut` line or at the end of the text, or `None` when no block starts
/// there.
pub(crate) fn pod_block_end(lines: &[u8], start: usize) -> Option<usize> {
    if !starts_command(&lines[start..]) {
        return None;
    }
    let mut line_start = start;
    loop {
        let line_end = lines[line_start..]
            .iter()
            .position(|b| *b == b'\n')
            .map_or(lines.len(), |n| line_start + n + 1);
        let line = &lines[line_start..line_end];
        if line.starts_with(b"=cut") && !line.get(4).is_some_and(u8::is_ascii_alphabetic) {
            return Some(line_end);
        }
        if line_end >= lines.len() {
            return Some(lines.len());
        }
        line_start = line_end;
    }
}

fn starts_command(line: &[u8]) -> bool {
    line.first() == Some(&b'=') && line.get(1).is_some_and(u8::is_ascii_alphabetic)
}
