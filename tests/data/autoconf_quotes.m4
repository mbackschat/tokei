dnl 12 lines 8 code 1 blanks 3 comments
AC_DEFUN([AX_CHECK], [
dnl m4 passes this line on as text
#include <stdio.h>

AC_LANG_SOURCE([[int main(void) { return 0; }]])
])

# a comment outside quotes
dnl a [ in a remark opens no quote
AC_INIT([pkg], [1.0]) dnl a trailing remark
AC_OUTPUT
