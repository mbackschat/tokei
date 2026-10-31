; 9 lines 6 code 2 comments 1 blanks
section .data
path    db "C:\", 0          ; a backslash ends the string
quote   db 'it''s', 0
newline db `line\n`, 0      ; a backquote string escapes
semi    db `a;b`, 0

; done
        ret
