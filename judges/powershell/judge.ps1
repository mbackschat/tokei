# Judges PowerShell files with PowerShell's own parser,
# [System.Management.Automation.Language.Parser]::ParseFile: reads paths on
# stdin and prints, per file, the comment and literal ranges in UTF-16 offsets
# after a byte order mark and the number of errors that leave a string or
# comment open, as judgecmp's stream expects.
#
# Comment tokens are comments and quoted strings, here-strings included, are
# literals; bare words, which the parser also reads as strings, are code. The
# `$(…)` subexpressions and variables inside an expandable string are code,
# with their own strings and comments, as the string's nested tokens show.

using namespace System.Management.Automation.Language

$quoted = @(
    [TokenKind]::StringLiteral,
    [TokenKind]::StringExpandable,
    [TokenKind]::HereStringLiteral,
    [TokenKind]::HereStringExpandable
)

# Errors that leave a string, here-string or comment unterminated, so the
# file's literals and comments cannot be compared; other parse errors do not
# change where tokens start and end.
$lexical = @(
    'TerminatorExpectedAtEndOfString',
    'MissingTerminatorMultiLineComment',
    'WhitespaceBeforeHereStringFooter',
    'UnexpectedCharactersAfterHereStringHeader'
)

function Get-Holes([Token] $token) {
    $holes = [System.Collections.Generic.List[object]]::new()
    if ($token -isnot [StringExpandableToken] -or -not $token.NestedTokens) {
        return , $holes
    }
    $depth = 0
    $start = 0
    foreach ($nested in $token.NestedTokens) {
        if ($depth -eq 0) {
            if ($nested.Kind -eq [TokenKind]::DollarParen) {
                $depth = 1
                $start = $nested.Extent.StartOffset
            } else {
                $holes.Add(@($nested.Extent.StartOffset, $nested.Extent.EndOffset))
            }
        } elseif ($nested.Kind -in [TokenKind]::LParen, [TokenKind]::DollarParen, [TokenKind]::AtParen) {
            $depth++
        } elseif ($nested.Kind -eq [TokenKind]::RParen) {
            $depth--
            if ($depth -eq 0) {
                $holes.Add(@($start, $nested.Extent.EndOffset))
            }
        }
    }
    return , $holes
}

function Add-Ranges($tokens, $comments, $literals) {
    foreach ($token in $tokens) {
        if ($token.Kind -eq [TokenKind]::Comment) {
            $comments.Add("[$($token.Extent.StartOffset),$($token.Extent.EndOffset)]")
        } elseif ($token -is [StringToken]) {
            if ($token.Kind -in $quoted) {
                $from = $token.Extent.StartOffset
                foreach ($hole in (Get-Holes $token)) {
                    if ($hole[0] -gt $from) { $literals.Add("[$from,$($hole[0])]") }
                    $from = $hole[1]
                }
                if ($token.Extent.EndOffset -gt $from) { $literals.Add("[$from,$($token.Extent.EndOffset)]") }
            }
            if ($token -is [StringExpandableToken] -and $token.NestedTokens) {
                Add-Ranges $token.NestedTokens $comments $literals
            }
        }
    }
}

foreach ($path in [Console]::In.ReadToEnd().Split("`n", [StringSplitOptions]::RemoveEmptyEntries)) {
    $tokens = $null
    $errors = $null
    [void][Parser]::ParseFile($path, [ref] $tokens, [ref] $errors)
    $comments = [System.Collections.Generic.List[string]]::new()
    $literals = [System.Collections.Generic.List[string]]::new()
    Add-Ranges $tokens $comments $literals
    $open = @($errors | Where-Object { $_.ErrorId -in $lexical }).Count
    foreach ($parseError in $errors) { [Console]::Error.WriteLine("$path`t$($parseError.ErrorId)") }
    $name = ConvertTo-Json -Compress $path
    [Console]::Out.WriteLine("{`"path`":$name,`"units`":`"utf16`",`"errors`":$open,`"comments`":[$($comments -join ',')],`"literals`":[$($literals -join ',')]}")
}
