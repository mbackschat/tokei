# Judges Julia files with JuliaSyntax, the tokenizer and parser that Julia
# itself runs since 1.10 (`Base.JuliaSyntax`): reads paths on stdin and
# prints, per file, the comment and literal ranges in byte offsets after a
# byte order mark and the number of parse errors, as judgecmp's stream
# expects.
#
# Comment tokens, `#` lines and nested `#= … =#` blocks, are comments. The
# delimiters and text of strings, commands and characters are literals, the
# adjacent tokens of one joined into one range; a string macro's prefix,
# such as the `r` of `r"…"`, and `$` interpolations are code with their own
# literals and comments.
using Base.JuliaSyntax: @K_str, ParseStream, kind, parse!, tokenize

const LITERALS = (
    K"\"", K"\"\"\"", K"`", K"```", K"'",
    K"String", K"CmdString", K"Char",
)

function judge(path)
    bytes = read(path)
    if length(bytes) >= 3 && bytes[1:3] == [0xEF, 0xBB, 0xBF]
        bytes = bytes[4:end]
    end
    text = String(bytes)
    isvalid(text) || return
    comments = Tuple{Int,Int}[]
    literals = Tuple{Int,Int}[]
    for token in tokenize(text)
        first, last = Int(token.range.start) - 1, Int(token.range.stop)
        spans = kind(token) == K"Comment" ? comments :
            kind(token) in LITERALS ? literals : nothing
        spans === nothing && continue
        if !isempty(spans) && spans[end][2] == first
            spans[end] = (spans[end][1], last)
        else
            push!(spans, (first, last))
        end
    end
    stream = ParseStream(text)
    parse!(stream; rule = :all)
    errors = count(d -> d.level == :error, stream.diagnostics)
    ranges(spans) = join(("[$a,$b]" for (a, b) in spans), ",")
    quoted = replace(path, "\\" => "\\\\", "\"" => "\\\"")
    println(
        "{\"path\":\"", quoted, "\"", ",\"units\":\"bytes\",\"errors\":", errors,
        ",\"comments\":[", ranges(comments), "],\"literals\":[", ranges(literals), "]}",
    )
    flush(stdout)
end

for path in eachline(stdin)
    judge(path)
end
