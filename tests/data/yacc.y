// 19 lines 11 code 5 comments 3 blanks
%{
/* A Bison grammar's C prologue */
#include <stdio.h>
%}

%token NUM   /* a token, then a comment */

%%
/* The rules: a C comment between them
   spans lines. */
exp: NUM
   | exp '+' exp      { $$ = $1 + $3; }  // trailing comment
   | '"' exp '"'      { printf("/* not a comment */"); }
   ;
%%

// A Bison line comment.
int main(void) { return yyparse(); }
