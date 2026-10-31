# 22 lines 17 code 4 comments 1 blanks
quote = '"' # a quote character
apostrophe = '\''
text = <<-EOS
  # not a comment
  #{quote}
  EOS
# after the heredoc
raw = <<-'RAW'
  #{not interpolated} # still text
  RAW
list = %w(# not a comment)
str = %(nested (# parens) here)
re = /#[a-z]+/

# a comment
puts "#{text} # inside" # trailing
x = 4 / 2 # division
def `(command)
  # runs a command
  command
end
