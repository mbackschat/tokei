# 18 lines 13 code 5 comments 0 blanks
{
  # a comment
  script = ''
    # not a comment

    echo '''quoted''' ''${HOME} ''\''
    # still in the string
    ${pkgs.hello}/bin/hello
  '';
  /* a block comment */
  name = "a $${";
  dollars = "$$${x}";
  # after the strings
  tick = x'';
  # after an identifier with quotes
  x = 1;
}
