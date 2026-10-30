# 30 lines 18 code 10 comments 2 blanks
# a comment \
  continued by a backslash
X := a # trailing \
  still the comment
Y := b # two backslashes \\
Z := c

all:
	# a shell comment \
	echo the shell runs this
	@# a silent comment
	-# an ignored comment
	@-# a comment after two flags
	+ # a comment after a flag and a blank
W := d # a backslash and a space \ 
V := e
U := a\#b \
  continued code
install:
	cp a b; \
# a shell comment in the recipe \
	cp c d
R := $(subst a,#,b) \
  still code
define BODY
  x=$${y#z} \
  continued shell code
endef

