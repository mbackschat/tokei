// Judges F# files with the F# compiler's own tokenizer: reads paths from
// stdin and prints, per file, the comment and literal ranges in UTF-16
// offsets after a byte order mark and the number of syntax errors that the
// compiler's parser reports, as judgecmp's stream expects. Tokens of the
// comment classes are comments and tokens of the string class are literals;
// the parts of a string that spans lines are joined into one range, since a
// blank line inside one is part of the string. Text that an `#if` leaves
// inactive is tokenized on its own, as the tokenizer reads it as inactive
// code without comments or literals, while its author wrote them as such.
// With JUDGE_DUMP set, it prints each token instead.
open System
open System.IO
open System.Text
open FSharp.Compiler.CodeAnalysis
open FSharp.Compiler.Diagnostics
open FSharp.Compiler.Text
open FSharp.Compiler.Tokenization

let checker = FSharpChecker.Create()
let decoder = UTF8Encoding(false, true)
let dump = not (isNull (Environment.GetEnvironmentVariable "JUDGE_DUMP"))

/// Tokens that leave their string open at the end of their line.
let continuesString (name: string) =
    name = "STRING_TEXT" || name.StartsWith "INTERP_STRING_BEGIN" || name = "INTERP_STRING_PART"

type Span = { Start: int; End: int; Comment: bool }

/// Tokenizes the lines `indices` of `lines` from the initial state, adding
/// their comment and literal spans, and returns the lines read as inactive.
let scan (path: string) (lines: string[]) (starts: int[]) (indices: int list) (spans: ResizeArray<Span>) =
    let tokenizer = FSharpSourceTokenizer([], Some path, Some "PREVIEW", None)
    let inactive = ResizeArray<int>()
    let mutable state = FSharpTokenizerLexState.Initial
    let mutable openString = false
    let mutable openComment = false
    for index in indices do
        let line = tokenizer.CreateLineTokenizer(lines[index])
        let mutable go = true
        let mutable lineInactive = false
        while go do
            match line.ScanToken(state) with
            | Some token, next ->
                state <- next
                let first = starts[index] + token.LeftColumn
                let last = starts[index] + token.RightColumn + 1
                if dump then
                    printfn "%d:%d-%d %s %A %A" (index + 1) token.LeftColumn token.RightColumn token.TokenName token.CharClass token.ColorClass
                if token.ColorClass = FSharpTokenColorKind.InactiveCode then
                    lineInactive <- true
                else
                    match token.CharClass with
                    | FSharpTokenCharKind.LineComment
                    | FSharpTokenCharKind.Comment ->
                        if spans.Count > 0 && spans[spans.Count - 1].Comment && (openComment || spans[spans.Count - 1].End = first) then
                            spans[spans.Count - 1] <- { spans[spans.Count - 1] with End = last }
                        else
                            spans.Add { Start = first; End = last; Comment = true }
                    | FSharpTokenCharKind.String ->
                        if openString && spans.Count > 0 && not spans[spans.Count - 1].Comment then
                            spans[spans.Count - 1] <- { spans[spans.Count - 1] with End = last }
                        else
                            spans.Add { Start = first; End = last; Comment = false }
                        openString <- continuesString token.TokenName
                    | _ -> openString <- false
            | None, next ->
                state <- next
                go <- false
            openComment <- false
        // A block comment that the line leaves open goes on over the next
        // lines, blank ones included. The states are matched by case, as
        // FSharp.Core 10's `string` renders an enum as its number.
        openComment <-
            match FSharpLineTokenizer.ColorStateOfLexState state with
            | FSharpTokenizerColorState.Comment
            | FSharpTokenizerColorState.StringInComment
            | FSharpTokenizerColorState.VerbatimStringInComment
            | FSharpTokenizerColorState.TripleQuoteStringInComment
            | FSharpTokenizerColorState.SingleLineComment -> true
            | _ -> false
        if lineInactive then inactive.Add index
    inactive

/// Groups inactive lines into runs, joining runs that only blank lines part.
let runs (lines: string[]) (inactive: ResizeArray<int>) =
    let result = ResizeArray<int list>()
    let mutable current = []
    for index in inactive do
        match current with
        | last :: _ when [ last + 1 .. index - 1 ] |> List.forall (fun i -> String.IsNullOrWhiteSpace lines[i]) ->
            current <- index :: ([ index - 1 .. -1 .. last + 1 ] @ current)
        | [] -> current <- [ index ]
        | _ ->
            result.Add(List.rev current)
            current <- [ index ]
    if not current.IsEmpty then result.Add(List.rev current)
    result

let quote (s: string) = "\"" + s.Replace("\\", "\\\\").Replace("\"", "\\\"") + "\""

let output = new StreamWriter(Console.OpenStandardOutput(), UTF8Encoding(false), AutoFlush = true)
let mutable path = Console.ReadLine()
while not (isNull path) do
    let text =
        try
            let decoded = decoder.GetString(File.ReadAllBytes path)
            // The char overload compares ordinally; the string one would
            // ignore the byte order mark culturally and match every text.
            Some(if decoded.StartsWith '﻿' then decoded.Substring 1 else decoded)
        with :? DecoderFallbackException -> None
    match text with
    | None -> ()
    | Some text ->
        let raw = text.Split '\n'
        let lines = raw |> Array.map (fun l -> l.TrimEnd '\r')
        let starts = Array.scan (fun offset (l: string) -> offset + l.Length + 1) 0 raw
        let spans = ResizeArray<Span>()
        let mutable pending = [ [ 0 .. lines.Length - 1 ] ]
        while not pending.IsEmpty do
            let next = scan path lines starts pending.Head spans
            pending <- pending.Tail @ List.ofSeq (runs lines next)
        // A file is parsed as a whole program, so that the module
        // declaration a library file needs (FS0222) is not required, and
        // without the strict indentation of F# 8, which older code predates;
        // neither is lexical.
        let options =
            { FSharpParsingOptions.Default with
                SourceFiles = [| path |]
                IsExe = true
                IsInteractive = path.EndsWith ".fsx"
                StrictIndentation = Some false }
        let parsed = checker.ParseFile(path, SourceText.ofString text, options) |> Async.RunSynchronously
        let errors =
            parsed.Diagnostics
            |> Array.filter (fun d -> d.Severity = FSharpDiagnosticSeverity.Error)
        if not (isNull (Environment.GetEnvironmentVariable "JUDGE_ERRORS")) then
            for d in errors do
                eprintfn "%s: FS%04d %s" path d.ErrorNumber d.Message
        let errors = errors.Length
        let ranges comment =
            spans
            |> Seq.filter (fun s -> s.Comment = comment)
            |> Seq.map (fun s -> sprintf "[%d,%d]" s.Start s.End)
            |> String.concat ","
        if not dump then
            output.WriteLine(
                sprintf "{\"path\":%s,\"units\":\"utf16\",\"errors\":%d,\"comments\":[%s],\"literals\":[%s]}"
                    (quote path) errors (ranges true) (ranges false)
            )
    path <- Console.ReadLine()
