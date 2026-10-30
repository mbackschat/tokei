-- | Judges Haskell files with GHC's own lexer, 'lexTokenStream' from the
-- @ghc@ library: reads paths on stdin and prints, per file, the comment and
-- literal ranges in byte offsets after a byte order mark and whether the
-- lexer rejected the file, as judgecmp's stream expects.
--
-- Comment tokens are comments; character, string and multiline string
-- literals, their unboxed forms and quasi-quotes are literals. Pragmas the
-- lexer knows, such as @{-# INLINE f #-}@, are tokens of code; it reads
-- others, @{-# LANGUAGE … #-}@ included, as block comments. The lexer runs
-- with GHC2021's extensions and those the file's own LANGUAGE pragmas turn on
-- or off, as GHC's driver would; a project's default extensions from its
-- Cabal file are not read, and files that use the C preprocessor, which GHC
-- lexes only after preprocessing, mostly fail. Token spans count characters,
-- converted here to bytes.
module Main (main) where

import Data.Array.Unboxed (UArray, listArray, (!))
import qualified Data.ByteString as B
import Data.Char (toLower)
import Data.List (foldl', intercalate)
import qualified Data.Text as T
import qualified Data.Text.Encoding as T
import qualified GHC.Data.EnumSet as EnumSet
import GHC.Data.FastString (mkFastString)
import qualified GHC.Data.Strict as Strict
import GHC.Data.StringBuffer (stringToStringBuffer)
import GHC.Driver.Flags (Language (GHC2021), OnOff (..), impliedXFlags)
import GHC.Driver.Session (FlagSpec (..), languageExtensions, xFlags)
import GHC.LanguageExtensions (Extension)
import GHC.Parser.Lexer
import GHC.Types.SrcLoc
import GHC.Unit.Module.Warnings (emptyWarningCategorySet)
import GHC.Utils.Error (DiagOpts (..))
import GHC.Utils.Outputable (defaultSDocContext)
import Numeric (showHex)
import System.IO

main :: IO ()
main = do
  hSetEncoding stdin utf8
  hSetEncoding stdout utf8
  paths <- lines <$> getContents
  mapM_ (\path -> judge path >>= maybe (pure ()) putStrLn) paths

judge :: FilePath -> IO (Maybe String)
judge path = do
  raw <- B.readFile path
  let bytes = maybe raw id (B.stripPrefix (B.pack [0xEF, 0xBB, 0xBF]) raw)
  pure $ case T.decodeUtf8' bytes of
    Left _ -> Nothing
    Right text -> Just (report path (T.unpack text))

report :: FilePath -> String -> String
report path source =
  case lexTokenStream opts (stringToStringBuffer source) start of
    POk _ tokens ->
      let ranges kind = [r | L span token <- tokens, classify token == Just kind, Just r <- [bytes span]]
       in json 0 (ranges True) (ranges False)
    PFailed _ -> json 1 [] []
  where
    opts = mkParserOpts (extensions source) diagnostics False False True False
    start = mkRealSrcLoc (mkFastString path) 1 1
    offsets = listArray (0, length source) (scanl (\n c -> n + utf8Length c) 0 source) :: UArray Int Int
    byteAt = (offsets !)
    bytes span = case getBufSpan span of
      Strict.Just (BufSpan (BufPos s) (BufPos e)) -> Just (byteAt s, byteAt e)
      Strict.Nothing -> Nothing
    json :: Int -> [(Int, Int)] -> [(Int, Int)] -> String
    json errors comments literals =
      concat
        [ "{\"path\":", jsonString path, ",\"units\":\"bytes\",\"errors\":", show errors
        , ",\"comments\":", pairs comments, ",\"literals\":", pairs literals, "}" ]
    pairs rs = "[" ++ intercalate "," ["[" ++ show a ++ "," ++ show b ++ "]" | (a, b) <- rs] ++ "]"

jsonString :: String -> String
jsonString text = "\"" ++ concatMap escape text ++ "\""
  where
    escape '"' = "\\\""
    escape '\\' = "\\\\"
    escape c
      | c < ' ' = "\\u" ++ pad (showHex (fromEnum c) "")
      | otherwise = [c]
    pad digits = replicate (4 - length digits) '0' ++ digits

-- | 'Just True' for a comment, 'Just False' for a literal.
classify :: Token -> Maybe Bool
classify token = case token of
  ITlineComment {} -> Just True
  ITblockComment {} -> Just True
  ITchar {} -> Just False
  ITstring {} -> Just False
  ITstringMulti {} -> Just False
  ITprimchar {} -> Just False
  ITprimstring {} -> Just False
  ITquasiQuote {} -> Just False
  ITqQuasiQuote {} -> Just False
  _ -> Nothing

utf8Length :: Char -> Int
utf8Length c
  | n < 0x80 = 1
  | n < 0x800 = 2
  | n < 0x10000 = 3
  | otherwise = 4
  where
    n = fromEnum c

diagnostics :: DiagOpts
diagnostics =
  DiagOpts
    { diag_warning_flags = EnumSet.empty
    , diag_fatal_warning_flags = EnumSet.empty
    , diag_custom_warning_categories = emptyWarningCategorySet
    , diag_fatal_custom_warning_categories = emptyWarningCategorySet
    , diag_warn_is_error = False
    , diag_reverse_errors = False
    , diag_max_errors = Nothing
    , diag_ppr_ctx = defaultSDocContext
    }

-- | GHC2021's extensions with those of the file's LANGUAGE pragmas.
extensions :: String -> EnumSet.EnumSet Extension
extensions source = foldl' apply (EnumSet.fromList (languageExtensions (Just GHC2021))) (pragmaWords source)
  where
    apply set name
      | Just e <- lookup name known = foldl' imply (EnumSet.insert e set) [o | (from, o) <- impliedXFlags, from == e]
      | 'N' : 'o' : rest <- name, Just e <- lookup rest known = EnumSet.delete e set
      | otherwise = set
    imply set (On e) = EnumSet.insert e set
    imply set (Off e) = EnumSet.delete e set
    known = [(flagSpecName f, flagSpecFlag f) | f <- xFlags]

-- | The extension names of every LANGUAGE pragma.
pragmaWords :: String -> [String]
pragmaWords ('{' : '-' : '#' : rest) =
  let (body, after) = untilClose rest
   in case words (map (\c -> if c == ',' then ' ' else c) body) of
        w : names | map toLower w == "language" -> names ++ pragmaWords after
        _ -> pragmaWords after
pragmaWords (_ : rest) = pragmaWords rest
pragmaWords [] = []

untilClose :: String -> (String, String)
untilClose ('#' : '-' : '}' : rest) = ([], rest)
untilClose (c : rest) = let (body, after) = untilClose rest in (c : body, after)
untilClose [] = ([], [])
