; 9 lines 5 code 4 comments 0 blanks
#!/usr/bin/env bb
(def quote-char \") ; a quote character
(def semicolon \;) ; a semicolon character
(def re #"a\"b;c") ; a regular expression
#! a comment, as the reader reads it
(defn f [x]
  ;; doc
  (str x \\))
