// Judges C# files with Roslyn's own parser: reads paths from stdin and
// prints, per file, the comment and literal ranges in UTF-16 offsets after a
// byte order mark and the number of syntax errors, as judgecmp's stream
// expects. Strings, characters and the text parts of interpolated and raw
// strings are literals; interpolation holes are code with their own strings
// and comments. Text that an `#if` leaves inactive is parsed on its own, as
// Roslyn reads it as disabled text without comments or literals, while its
// author wrote them as such. Directives other than conditional ones split
// disabled text, even inside a string, so each run of disabled text and
// such directives is parsed whole.
using System.Text;
using Microsoft.CodeAnalysis;
using Microsoft.CodeAnalysis.CSharp;
using Microsoft.CodeAnalysis.CSharp.Syntax;

var options = new CSharpParseOptions(LanguageVersion.Preview);
var decoder = new UTF8Encoding(false, true);
var output = new StreamWriter(Console.OpenStandardOutput(), new UTF8Encoding(false));
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
    var tree = CSharpSyntaxTree.ParseText(text, options);
    var errors = tree.GetDiagnostics().Count(d => d.Severity == DiagnosticSeverity.Error);
    var comments = new List<string>();
    var literals = new List<string>();
    Collect(tree.GetRoot(), 0, comments, literals, options);
    output.WriteLine($"{{\"path\":{Quote(path)},\"units\":\"utf16\",\"errors\":{errors},\"comments\":[{string.Join(",", comments)}],\"literals\":[{string.Join(",", literals)}]}}");
}
output.Flush();

static void Collect(SyntaxNode root, int offset, List<string> comments, List<string> literals, CSharpParseOptions options)
{
    var disabled = new List<(int Start, int End)>();
    foreach (var trivia in root.DescendantTrivia())
    {
        var continues = disabled.Count > 0 && disabled[^1].End == trivia.FullSpan.Start;
        var inactive = trivia.IsKind(SyntaxKind.DisabledTextTrivia)
            || (continues && trivia.Kind() is SyntaxKind.WhitespaceTrivia or SyntaxKind.EndOfLineTrivia)
            || (continues && trivia.GetStructure() is DirectiveTriviaSyntax { IsActive: false } directive
                && !IsConditional(directive));
        if (!inactive) continue;
        if (continues)
            disabled[^1] = (disabled[^1].Start, trivia.FullSpan.End);
        else
            disabled.Add((trivia.FullSpan.Start, trivia.FullSpan.End));
    }
    var text = root.ToFullString();
    foreach (var (start, end) in disabled)
    {
        var tree = CSharpSyntaxTree.ParseText(text[start..end], options);
        Collect(tree.GetRoot(), offset + start, comments, literals, options);
    }
    bool InDisabled(int position) => disabled.Any(d => d.Start <= position && position < d.End);
    foreach (var trivia in root.DescendantTrivia(descendIntoTrivia: true))
    {
        if (InDisabled(trivia.FullSpan.Start)) continue;
        switch (trivia.Kind())
        {
            case SyntaxKind.SingleLineCommentTrivia:
            case SyntaxKind.MultiLineCommentTrivia:
                comments.Add($"[{offset + trivia.Span.Start},{offset + trivia.Span.End}]");
                break;
            // A documentation comment's span leaves out its `///` or `/**`,
            // the leading trivia of its first token, and takes in the line
            // break after it, which ends it as it ends a line comment.
            case SyntaxKind.SingleLineDocumentationCommentTrivia:
            case SyntaxKind.MultiLineDocumentationCommentTrivia:
                var end = trivia.FullSpan.Start + trivia.ToFullString().TrimEnd('\r', '\n').Length;
                comments.Add($"[{offset + trivia.FullSpan.Start},{offset + end}]");
                break;
        }
    }
    foreach (var token in root.DescendantTokens())
    {
        if (IsLiteral(token.Kind())) literals.Add($"[{offset + token.Span.Start},{offset + token.Span.End}]");
    }
}

static bool IsConditional(DirectiveTriviaSyntax directive) => directive.Kind() is SyntaxKind.IfDirectiveTrivia
    or SyntaxKind.ElifDirectiveTrivia or SyntaxKind.ElseDirectiveTrivia or SyntaxKind.EndIfDirectiveTrivia;

static bool IsLiteral(SyntaxKind kind) => kind is SyntaxKind.StringLiteralToken or SyntaxKind.Utf8StringLiteralToken
    or SyntaxKind.CharacterLiteralToken or SyntaxKind.SingleLineRawStringLiteralToken
    or SyntaxKind.Utf8SingleLineRawStringLiteralToken or SyntaxKind.MultiLineRawStringLiteralToken
    or SyntaxKind.Utf8MultiLineRawStringLiteralToken or SyntaxKind.InterpolatedStringStartToken
    or SyntaxKind.InterpolatedVerbatimStringStartToken or SyntaxKind.InterpolatedSingleLineRawStringStartToken
    or SyntaxKind.InterpolatedMultiLineRawStringStartToken or SyntaxKind.InterpolatedStringTextToken
    or SyntaxKind.InterpolatedStringEndToken or SyntaxKind.InterpolatedRawStringEndToken;

static string Quote(string s) => "\"" + s.Replace("\\", "\\\\").Replace("\"", "\\\"") + "\"";
