// 6 lines 2 code 4 comments 0 blanks
// Before Rust 2024, `#"` is `#` and a string, as rustc_parse splits it.
m!(#"a"
/* a comment */
"b"#);
// a comment
