# 18 lines 13 code 5 comments 0 blanks
a = <<~EOS.strip # a comment after the opener
  Don't panic
  # not a comment
EOS
# comment
b = foo(<<-ONE, <<'TWO')
  it's
  ONE
not interpolated: #{x}
TWO
# comment
c = x << y # an append, not a heredoc
# comment
d = <<~"EOS"
  "#{1}"
  EOS
# comment
