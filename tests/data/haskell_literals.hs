-- 10 lines 7 code 3 comments 0 blanks
{-# LANGUAGE QuasiQuotes #-}
s = "a \
    \\"b\" c -- not a comment \
    \d"
-- one
q = [r|
-- not a comment

|]
