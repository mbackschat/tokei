/* 9 lines 5 code 3 comments 1 blanks */
#include <board.h>
SECTIONS
{
  //*libarch.a:patch.o(.text.fast)
  .text : { *(.text*) }
  // a line comment

}
