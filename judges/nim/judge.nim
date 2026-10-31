# Judges Nim files with the compiler's own lexer, compiler/lexer.nim: reads
# paths on stdin and prints, per file, the comment and literal ranges in byte
# offsets after a byte order mark and whether the lexer reported an error, as
# judgecmp's stream expects. Build it with -d:nimpretty, which makes the lexer
# record each token's byte range.
#
# Strings, raw, triple-quoted and generalized strings and character literals
# are the literal tokens. The lexer merges comments into one token with the
# whitespace between them, so the comments are found again between the
# literals by its own rules (skip, scanComment and skipMultiLineComment): a
# `#` runs to the end of its line, `#[ ]#` nests on `#[`, and the
# documentation block `##[ ]##` nests on `##[` only. No other token holds a
# `#`.
import std/[json, unicode]
import compiler/[idents, lexer, llstream, options, pathutils]

const literalTokens = {tkStrLit, tkRStrLit, tkTripleStrLit, tkGStrLit,
                       tkGTripleStrLit, tkCharLit}

proc at(text: string, i: int, s: string): bool =
  i + s.len <= text.len and text[i ..< i + s.len] == s

proc blockCommentEnd(text: string, start: int, opener, closer: string): int =
  ## The end of the block comment opened at `start`, nested ones included.
  var depth = 0
  var i = start
  while i < text.len:
    if text.at(i, opener):
      inc depth
      i += opener.len
    elif text.at(i, closer):
      dec depth
      i += closer.len
      if depth == 0: return i
    else: inc i
  text.len

proc comments(text: string, literals: seq[(int, int)]): seq[(int, int)] =
  var i = 0
  var next = 0
  while i < text.len:
    if next < literals.len and i >= literals[next][0]:
      i = max(i, literals[next][1])
      inc next
    elif text[i] == '#':
      var last: int
      if text.at(i, "##["):
        last = blockCommentEnd(text, i, "##[", "]##")
      elif text.at(i, "#["):
        last = blockCommentEnd(text, i, "#[", "]#")
      else:
        last = i
        while last < text.len and text[last] notin {'\r', '\n'}: inc last
      result.add (i, last)
      i = last
    else: inc i

proc judge(path: string) =
  var text: string
  try: text = readFile(path)
  except IOError: return
  if text.at(0, "\xEF\xBB\xBF"): text = text[3 .. ^1]
  if validateUtf8(text) >= 0: return
  let conf = newConfigRef()
  conf.errorMax = high(int)
  conf.writelnHook = proc (s: string) = discard
  var lex: Lexer
  openLexer(lex, AbsoluteFile(path), llStreamOpen(text), newIdentCache(), conf)
  var literals: seq[(int, int)]
  var tok: Token
  while true:
    rawGetTok(lex, tok)
    if tok.tokType == tkEof: break
    if tok.tokType in literalTokens: literals.add (tok.offsetA, tok.offsetB + 1)
  closeLexer(lex)
  var commentRanges = newJArray()
  for (a, b) in comments(text, literals): commentRanges.add %*[a, b]
  var literalRanges = newJArray()
  for (a, b) in literals: literalRanges.add %*[a, b]
  echo $(%*{"path": path, "units": "bytes", "errors": int(conf.errorCounter > 0),
            "comments": commentRanges, "literals": literalRanges})

for path in stdin.lines:
  if path.len > 0: judge(path)
