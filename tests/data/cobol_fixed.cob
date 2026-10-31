      * 17 lines 8 code 8 comments 1 blanks
000100 IDENTIFICATION DIVISION.                                         SEQ00001
000200*A COMMENT LINE WITH A SEQUENCE NUMBER
       PROGRAM-ID. HELLO.
       AUTHOR. A PERSON.
           MORE ABOUT THE AUTHOR.
       ENVIRONMENT DIVISION.
      /    PAGE EJECT COMMENT
      D    DISPLAY "DEBUG".
000900
       PROCEDURE DIVISION. *> trailing comment
          *> a floating comment
           DISPLAY "A *> NOT A COMMENT"
           DISPLAY "CONTIN
      -    "UED".

           STOP RUN.
