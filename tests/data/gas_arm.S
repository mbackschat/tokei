@ 7 lines 3 code 4 comments 0 blanks
@ void ff_add(int *dst)
	mov	r0, #1		@ the count
	@@ aligned
# a comment line
	add	r0, r0, #-1
	bx	lr
