# 18 lines 12 code 6 comments 0 blanks
{
  // a comment
  script: |||
    #!/usr/bin/env bash
    # not a comment

    echo done
  |||,
  # a hash comment
  quoted: @'it''s /* not a comment',
  doubled: @"say ""hi"" # still a string",
  /* a block
     comment */
  plain: "a \" # b",
  path: @'C:\',
  // after a verbatim backslash
}
