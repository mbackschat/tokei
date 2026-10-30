//! cmd.exe's remarks.

/// Whether `text` starts with a `rem` command, whose line cmd.exe runs
/// nothing of. cmd.exe matches the word in any case ([Microsoft Learn,
/// rem](https://learn.microsoft.com/en-us/windows-server/administration/windows-commands/rem))
/// and takes it as a command name where the line ends after it or one of
/// ` \t+./(;=:` follows it (`search_command` in Wine's
/// `programs/cmd/wcmdmain.c`); any other byte makes a longer name, as in
/// `remark` or `rem-x`. Only a blank after the word makes the rest of the
/// line the remark (`WCMD_ReadAndParseLine`); after other punctuation, cmd.exe
/// reads on, and an `&` or `|` runs more, as the documented `Rem/||(` does.
pub(crate) fn starts_remark(text: &[u8]) -> bool {
    if !text
        .get(..3)
        .is_some_and(|word| word.eq_ignore_ascii_case(b"rem"))
    {
        return false;
    }
    match text.get(3) {
        None | Some(b' ' | b'\t' | b'\r' | b'\n') => true,
        Some(byte) if b"+./(;=:".contains(byte) => {
            let line = &text[4..];
            let line = &line[..memchr::memchr(b'\n', line).unwrap_or(line.len())];
            memchr::memchr2(b'&', b'|', line).is_none()
        }
        Some(_) => false,
    }
}
