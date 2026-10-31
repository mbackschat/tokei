//! Free Pascal nests a `{ }` comment in another and an `(* *)` comment in
//! another in its FPC and ObjFPC modes, but not in its Delphi and TP modes
//! or in Delphi itself (Free Pascal Reference Guide, §1.5 Comments;
//! fcl-passrc `pscanner.pp` at `fb4143a1ef`, `HandleMultilineComment`). A
//! file's mode may come from an include file or the project, which a counter
//! does not read, so a file is read in the one that leaves it valid.

use crate::language::syntax::block_comment_len;

/// Whether every comment of `text` closes when same-kind comments nest; when
/// one stays open to the end, the file is written for a mode where they do
/// not, as one with `{ a { b }` is.
///
/// Strings and `//` comments are skipped, and inside a comment nothing but
/// its own kind's delimiters counts, as the scanner reads them.
pub(crate) fn nested_comments_close(text: &[u8]) -> bool {
    let mut i = 0;
    while i < text.len() {
        let rest = &text[i..];
        i += match rest {
            [b'\'', ..] => string_len(rest),
            [b'/', b'/', ..] => memchr::memchr(b'\n', rest).unwrap_or(rest.len()),
            [b'{', ..] => match block_comment_len(rest, "{", "}", true) {
                Some(len) => len,
                None => return false,
            },
            [b'(', b'*', ..] => match block_comment_len(rest, "(*", "*)", true) {
                Some(len) => len,
                None => return false,
            },
            _ => 1,
        };
    }
    true
}

/// Length of the string opening `text`, up to its closing quote, or to the
/// end of its line when it has none; a doubled quote reads as two strings.
fn string_len(text: &[u8]) -> usize {
    memchr::memchr2(b'\'', b'\n', &text[1..]).map_or(text.len(), |at| at + 2)
}
