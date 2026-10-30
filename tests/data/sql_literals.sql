-- 13 lines 8 code 4 comments 1 blanks
SELECT 'it''s -- not a comment' AS a,
       'C:\' AS path, -- a comment after a backslash
       "col--name" AS b,
       E'it\'s' AS c
/* outer /* nested */
   still a comment */
FROM t;
CREATE FUNCTION f() RETURNS int AS $$
-- a comment in the body
SELECT 1;

$$ LANGUAGE sql;
