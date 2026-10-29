# 7 lines 5 code 2 comments 0 blanks
x = f'''
{# a comment in a replacement field
86 # constant
# nothing more
}'''
y = f"{x!r:{'>10'}}"  # a comment
