import Lean
open Lean Parser

/-! Per-line classes of Lean files from Lean's own lexer functions: `whitespace`
and `finishCommentBlock` for comments, `tokenFn` for literals, and
`interpolatedStr` after the syntax that takes interpolated strings. Lines are
classified as the owner decided on 2026-10-29: a line is a comment when it
holds nothing but comments. Prints `path<TAB>classes<TAB>fallbacks` per file, one
class per line: `C` code, `m` comment, `.` blank. -/

def CODE : UInt8 := 0
def COMMENT : UInt8 := 1
def LITERAL : UInt8 := 2

structure St where
  cls : ByteArray
  /-- Interpolated strings whose terms Lean's term parser rejected, read as plain. -/
  fallbacks : Nat := 0
  /-- Positions where Lean's lexer reports an error, so the file is not valid Lean. -/
  lexErrors : Nat := 0

abbrev M := StateM St

def mark (a b : Nat) (k : UInt8) : M Unit :=
  modify fun st => Id.run do
    let mut c := st.cls
    for i in [a:b] do
      if i < c.size then c := c.set! i k
    pure { st with cls := c }

/-- Runs a parser function from a byte position. -/
abbrev Runner := ParserFn → Nat → ParserState

/-- Marks the comments in a range that `whitespace` consumed. -/
partial def markWs (run : Runner) (src : String) (a b : Nat) : M Unit := do
  let rec go (p : Nat) : M Unit := do
    if p >= b then return
    let ch := String.Pos.Raw.get src ⟨p⟩
    if ch.isWhitespace then
      go (p + ch.utf8Size)
    else if (String.Pos.Raw.extract src ⟨p⟩ ⟨p + 2⟩) == "--" then
      let mut e := p
      while e < b && String.Pos.Raw.get src ⟨e⟩ != '\n' do
        e := e + (String.Pos.Raw.get src ⟨e⟩).utf8Size
      mark p e COMMENT
      go e
    else if (String.Pos.Raw.extract src ⟨p⟩ ⟨p + 2⟩) == "/-" then
      let s := run (finishCommentBlock (pushMissingOnError := false) 1) (p + 2)
      if s.hasError then modify fun st => { st with lexErrors := st.lexErrors + 1 }
      let e := if s.hasError then b else s.pos.byteIdx
      mark p e COMMENT
      go e
    else
      go (p + ch.utf8Size)
  go a

def literalKinds : List SyntaxNodeKind := [strLitKind, charLitKind, interpolatedStrLitKind]

/-- Marks the tokens of a parsed syntax tree and the comments after them. -/
partial def markStx (run : Runner) (src : String) : Syntax → M Unit
  | stx@(.node _ k args) => do
    if literalKinds.contains k then
      if let some r := stx.getRange? then mark r.start.byteIdx r.stop.byteIdx LITERAL
      markTrailing stx
    else
      for a in args do markStx run src a
  | stx@(.atom ..) | stx@(.ident ..) => markTrailing stx
  | _ => pure ()
where
  markTrailing (stx : Syntax) : M Unit := do
    if let .original _ _ trailing _ := stx.getTailInfo then
      markWs run src trailing.startPos.byteIdx trailing.stopPos.byteIdx

def interpolating : List String :=
  ["s!", "f!", "m!", "throwError", "throwErrorAt", "println!", "reportIssue!", "reportDbgIssue!",
   "reportEMatchIssue!"]
def traceOpeners : List String := ["trace[", "trace_goal[", "Macro.trace["]

partial def classify (env : Environment) (input : String) : St := Id.run do
  let ictx := mkInputContext input "<input>"
  let src := ictx.inputString
  let s0 := mkParserState src
  let tokens := getTokenTable env
  let run : Runner := fun f pos => f.run ictx { env, options := {} } tokens (s0.setPos ⟨pos⟩)
  let size := src.utf8ByteSize
  let termFn := (categoryParser `term 0).fn
  let rec loop (pos : Nat) (prev : String) (inTrace : Bool) (pendingAt : Bool) (fuel : Nat) : M Unit := do
    if fuel == 0 || pos >= size then return
    let s := run whitespace pos
    let wsEnd := s.pos.byteIdx
    markWs run src pos wsEnd
    if s.hasError then
      -- A tab or an isolated carriage return: whitespace for counting.
      let ch := String.Pos.Raw.get src ⟨wsEnd⟩
      loop (wsEnd + max ch.utf8Size 1) prev inTrace pendingAt (fuel - 1)
    else if wsEnd >= size then return
    else
      let p := wsEnd
      let rest := String.Pos.Raw.extract src ⟨p⟩ ⟨p + 3⟩
      if rest == "/--" || rest == "/-!" then
        let s' := run (finishCommentBlock (pushMissingOnError := false) 1) (p + 3)
        if s'.hasError then modify fun st => { st with lexErrors := st.lexErrors + 1 }
        let e := if s'.hasError then size else s'.pos.byteIdx
        mark p e COMMENT
        loop e "" false pendingAt (fuel - 1)
      else
        let ch := String.Pos.Raw.get src ⟨p⟩
        let interp := ch == '"' && (interpolating.contains prev || (prev == "]" && inTrace) || pendingAt)
        let tried : Option ParserState :=
          if interp then
            let s' := run (interpolatedStrFn termFn) p
            if s'.hasError then none else some s'
          else none
        if interp && tried.isNone then
          modify fun st => { st with fallbacks := st.fallbacks + 1 }
        match tried with
        | some s' =>
          let stx := s'.stxStack.back
          markStx run src stx
          let e := (stx.getTailPos?.map (·.byteIdx)).getD s'.pos.byteIdx
          loop (max e (p + 1)) "\"" false false (fuel - 1)
        | none =>
          let s' := run (tokenFn []) p
          -- `\{` is valid only in an interpolated string, so a string that
          -- fails as plain is retried as one, as after `aesop_trace[…]`.
          let retried : Option ParserState :=
            if s'.hasError && ch == '"' then
              let s'' := run (interpolatedStrFn termFn) p
              if s''.hasError then none else some s''
            else none
          if let some s'' := retried then
            let stx := s''.stxStack.back
            markStx run src stx
            let e := (stx.getTailPos?.map (·.byteIdx)).getD s''.pos.byteIdx
            loop (max e (p + 1)) "\"" false false (fuel - 1)
          else if s'.hasError then
            -- Other failures are notation tokens from imports outside Lean core.
            if ch == '"' then modify fun st => { st with lexErrors := st.lexErrors + 1 }
            loop (p + max ch.utf8Size 1) "" inTrace pendingAt (fuel - 1)
          else
            let stx := s'.stxStack.back
            let stop := (stx.getTailPos?.map (·.byteIdx)).getD s'.pos.byteIdx
            match stx with
            | .node _ k _ =>
              if literalKinds.contains k then mark p stop LITERAL
            | _ => pure ()
            let val := match stx with
              | .atom _ v => v
              | .ident _ raw _ _ => raw.toString
              | _ => ""
            let inTrace' := if traceOpeners.contains val then true else if val == "]" then inTrace else inTrace && val != "\""
            let pendingAt' := val == "throwErrorAt" || (pendingAt && val != "\"")
            loop (max stop (p + 1)) val inTrace' pendingAt' (fuel - 1)
  let ((), st) := (loop 0 "" false false (size + 1)).run { cls := ByteArray.mk (Array.replicate size CODE) }
  -- A line is a comment when it holds nothing but comments, blank lines
  -- take the class they start in, and any other line is code.
  let mut out := ""
  let mut started := CODE
  let mut lineStart := 0
  let bytes := src.toUTF8
  for i in [0:size + 1] do
    if i == size || bytes.get! i == 10 then
      if i == size && lineStart == size then break
      let mut sawText := false
      let mut allComment := true
      for j in [lineStart:i] do
        let b := bytes.get! j
        if !(b == 32 || b == 9 || b == 13 || b == 10 || b == 11 || b == 12) then
          sawText := true
          if st.cls.get! j != COMMENT then allComment := false
      let cls :=
        if !sawText then (if started == COMMENT then 'm' else if started == LITERAL then 'C' else '.')
        else if allComment then 'm' else 'C'
      out := out.push cls
      started := if i < size then st.cls.get! i else CODE
      lineStart := i + 1
  { st with cls := out.toUTF8 }

unsafe def main (args : List String) : IO Unit := do
  enableInitializersExecution
  initSearchPath (← findSysroot)
  let env ← importModules #[{ module := `Lean }] {} (loadExts := true)
  let stdin ← IO.getStdin
  let stdout ← IO.getStdout
  let mut line ← stdin.getLine
  while line != "" do
    let path := line.trimRight
    let bytes ← IO.FS.readBinFile path
    match String.fromUTF8? bytes with
    | none => stdout.putStrLn s!"{path}\t!\t0"
    | some input =>
      let input := if input.startsWith "\uFEFF" then (input.drop 1).toString else input
      let st := classify env (input.replace "\r\n" "\n")
      stdout.putStrLn s!"{path}\t{String.fromUTF8! st.cls}\t{st.fallbacks}\t{st.lexErrors}"
    stdout.flush
    line ← stdin.getLine
  let _ := args
