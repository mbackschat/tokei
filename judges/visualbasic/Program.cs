// Judges Visual Basic files with Roslyn's own parser: reads paths from stdin
// and prints, per file, the comment and literal ranges in UTF-16 offsets
// after a byte order mark and the number of syntax errors, as judgecmp's
// stream expects. Comments are `'` and `REM` comments and `'''`
// documentation comments; strings, characters, dates and the text parts of
// interpolated strings are literals; interpolation holes are code with their
// own strings and comments. XML literals are code, as the parser reads them
// as expressions. Text that an `#If` leaves inactive is parsed on its own, as
// Roslyn reads it as disabled text without comments or literals, while its
// author wrote them as such.
using System.Text;
using Microsoft.CodeAnalysis;
using Microsoft.CodeAnalysis.VisualBasic;

var options = new VisualBasicParseOptions(LanguageVersion.Latest);
var decoder = new UTF8Encoding(false, true);
// Flushed per file, so that the file after the last line printed is the one
// a stalled parse is stuck on.
var output = new StreamWriter(Console.OpenStandardOutput(), new UTF8Encoding(false)) { AutoFlush = true };
for (string? path; (path = Console.ReadLine()) != null;)
{
    string text;
    try
    {
        text = decoder.GetString(File.ReadAllBytes(path));
    }
    catch (DecoderFallbackException)
    {
        continue;
    }
    if (text.StartsWith('﻿')) text = text[1..];
    var tree = VisualBasicSyntaxTree.ParseText(text, options);
    var errors = tree.GetDiagnostics().Count(d => d.Severity == DiagnosticSeverity.Error);
    var comments = new List<string>();
    var literals = new List<string>();
    Collect(tree.GetRoot(), 0, comments, literals, options);
    output.WriteLine($"{{\"path\":{Quote(path)},\"units\":\"utf16\",\"errors\":{errors},\"comments\":[{string.Join(",", comments)}],\"literals\":[{string.Join(",", literals)}]}}");
}
output.Flush();

static void Collect(SyntaxNode root, int offset, List<string> comments, List<string> literals, VisualBasicParseOptions options)
{
    var disabled = new List<(int Start, int End)>();
    foreach (var trivia in root.DescendantTrivia())
    {
        var continues = disabled.Count > 0 && disabled[^1].End == trivia.FullSpan.Start;
        var inactive = trivia.IsKind(SyntaxKind.DisabledTextTrivia)
            || (continues && trivia.Kind() is SyntaxKind.WhitespaceTrivia or SyntaxKind.EndOfLineTrivia);
        if (!inactive) continue;
        if (continues)
            disabled[^1] = (disabled[^1].Start, trivia.FullSpan.End);
        else
            disabled.Add((trivia.FullSpan.Start, trivia.FullSpan.End));
    }
    var text = root.ToFullString();
    foreach (var (start, end) in disabled)
    {
        var tree = VisualBasicSyntaxTree.ParseText(text[start..end], options);
        Collect(tree.GetRoot(), offset + start, comments, literals, options);
    }
    bool InDisabled(int position) => disabled.Any(d => d.Start <= position && position < d.End);
    foreach (var trivia in root.DescendantTrivia(descendIntoTrivia: true))
    {
        if (InDisabled(trivia.FullSpan.Start)) continue;
        switch (trivia.Kind())
        {
            case SyntaxKind.CommentTrivia:
                comments.Add($"[{offset + trivia.Span.Start},{offset + trivia.Span.End}]");
                break;
            // A documentation comment is structured trivia whose full span
            // holds its `'''` markers.
            case SyntaxKind.DocumentationCommentTrivia:
                comments.Add($"[{offset + trivia.FullSpan.Start},{offset + trivia.FullSpan.End}]");
                break;
        }
    }
    foreach (var token in root.DescendantTokens())
    {
        if (IsLiteral(token)) literals.Add($"[{offset + token.Span.Start},{offset + token.Span.End}]");
    }
}

// The closing `"` of an interpolated string is a DoubleQuoteToken, which
// only interpolated strings hold.
static bool IsLiteral(SyntaxToken token) => token.Kind() is SyntaxKind.StringLiteralToken
    or SyntaxKind.CharacterLiteralToken or SyntaxKind.DateLiteralToken
    or SyntaxKind.DollarSignDoubleQuoteToken or SyntaxKind.InterpolatedStringTextToken
    || (token.IsKind(SyntaxKind.DoubleQuoteToken) && token.Parent.IsKind(SyntaxKind.InterpolatedStringExpression));

static string Quote(string s) => "\"" + s.Replace("\\", "\\\\").Replace("\"", "\\\"") + "\"";
