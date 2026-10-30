# 23 lines 18 code 4 comments 1 blanks
chars = [?", ?#, ?\", ?']
# one
regex = ~r/a#b/i
upper = ~S(a" # b)
hole = ~s[#{")"}] # c
html = ~HTML"<p>'</p>"

doc = """
a """ b # not a comment

  #{if true do
    """
    ## inner
    """
  end}
"""
# two
sigil = ~S"""
# text
"""
# three
x = valid?(1) # four
