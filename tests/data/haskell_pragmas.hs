-- 15 lines 11 code 3 comments 1 blanks
{-# LANGUAGE CPP #-}
module M where

f = 1
{-# INLINE f #-}
{-#inline g #-}
{-# OPTIONS_GHC -Wall #-}
{-# RULES
"f/g" forall x. f (g x) = x
  #-}
{-# INLINE h #-}
{-#
  RULES "x" forall a. f a = a
 #-}
