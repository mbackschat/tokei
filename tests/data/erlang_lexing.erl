%% 27 lines 17 code 10 comments 0 blanks
f() -> "50% done".
% c
g() -> $%.
% c
h() -> $".
% c
i() -> $\".
% c
j() -> 'it\'s % atom'.
% c
k() -> "line one
% not a comment".
-doc """
Say "hi.
""".
% c
-doc """"
Use """ here, and \ is no escape.
"""".
% c
b() -> ~B"\".
% c
s() -> ~s"\"" ++ ~(a % b) ++ ~"""
  % in sigil
  """.
% c
