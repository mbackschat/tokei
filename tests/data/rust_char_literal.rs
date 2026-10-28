// 11 lines 8 code 2 comments 1 blanks
fn first<'a>(x: &'a str) -> &'a str {
    let q = '"';
    let b = b'x';
    let e = '\'';
    let s = "/*";

    // a comment
    'outer: loop { break 'outer; }
    x
}
