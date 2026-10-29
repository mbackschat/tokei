#!/usr/bin/env -S cargo +nightly -Zscript
----cargo
# 14 lines 10 code 3 comments 1 blanks
[dependencies]
// a TOML line, not a Rust comment
---

 ---
----

// a comment
fn main() {} // code
/* a comment */
----
