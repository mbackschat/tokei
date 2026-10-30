# 13 lines 5 code 7 comments 1 blanks
#[[ A bracket comment
over lines,

with a blank line ]]
set(A "quoted # not a comment
second line") # trailing comment
set(B [=[ bracket argument
# not a comment ]=])
#[==[ a comment holding ]=] and
closing only here ]==]
message(\# escaped)

