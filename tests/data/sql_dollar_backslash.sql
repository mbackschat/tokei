-- 5 lines 4 code 1 comments 0 blanks
DO $$ BEGIN x := 1 \' a
  -- not a comment, but text in the string
  '; END $$;
SELECT 1;
