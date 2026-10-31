// Judges Bicep files with the lexer of the Bicep compiler, Azure.Bicep.Core
// 0.48.1: reads paths from stdin and prints, per file, the comment and
// literal ranges in UTF-16 offsets after a byte order mark and the number of
// lexing errors, as judgecmp's stream expects. Comment trivia are comments;
// string tokens, `'''` strings included, are literals, the pieces of an
// interpolated string among them, while the `${…}` holes between the pieces
// are code.
using System.Text;
using Bicep.Core.Diagnostics;
using Bicep.Core.Parsing;
using Bicep.Core.Syntax;

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
    if (text.StartsWith('\uFEFF')) text = text[1..];
    var diagnostics = ToListDiagnosticWriter.Create();
    var lexer = new Lexer(new SlidingTextWindow(text), diagnostics);
    lexer.Lex();
    var comments = new List<int[]>();
    var literals = new List<int[]>();
    void AddTrivia(IEnumerable<SyntaxTrivia> trivia)
    {
        foreach (var t in trivia)
        {
            if (t.Type is SyntaxTriviaType.SingleLineComment or SyntaxTriviaType.MultiLineComment)
                comments.Add(new[] { t.Span.Position, t.Span.Position + t.Span.Length });
        }
    }
    foreach (var token in lexer.GetTokens())
    {
        AddTrivia(token.LeadingTrivia);
        if (token.Type is TokenType.StringComplete or TokenType.StringLeftPiece or TokenType.StringMiddlePiece
            or TokenType.StringRightPiece)
            literals.Add(new[] { token.Span.Position, token.Span.Position + token.Span.Length });
        AddTrivia(token.TrailingTrivia);
    }
    var errors = diagnostics.GetDiagnostics().Count(d => d.Level == DiagnosticLevel.Error);
    output.WriteLine(System.Text.Json.JsonSerializer.Serialize(new { path, units = "utf16", errors, comments, literals }));
}
output.Flush();
