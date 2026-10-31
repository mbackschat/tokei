package parser

// Spans lexes input as go-jsonnet does and returns its comment and string
// ranges in byte offsets. lexer.go keeps comments as fodder without
// positions, but between two tokens there is only whitespace and comments,
// so each gap is scanned for them; strings are the tokens of string kinds,
// whose locations hold the line and the byte column.
func Spans(input string) (comments, literals [][2]int, err error) {
	tokens, err := Lex("", "", input)
	if err != nil {
		return nil, nil, err
	}
	lineStarts := []int{0}
	for i := 0; i < len(input); i++ {
		if input[i] == '\n' {
			lineStarts = append(lineStarts, i+1)
		}
	}
	offset := func(line, column int) int { return lineStarts[line-1] + column - 1 }
	gap := 0
	for _, t := range tokens {
		begin := offset(t.loc.Begin.Line, t.loc.Begin.Column)
		comments = append(comments, gapComments(input[:begin], gap)...)
		gap = offset(t.loc.End.Line, t.loc.End.Column)
		switch t.kind {
		case tokenStringBlock, tokenStringDouble, tokenStringSingle,
			tokenVerbatimStringDouble, tokenVerbatimStringSingle:
			literals = append(literals, [2]int{begin, gap})
		}
	}
	return comments, literals, nil
}

// gapComments returns the comments in input[from:], which holds only
// whitespace and comments: `#` and `//` run to the end of their line and
// `/*` to the next `*/`.
func gapComments(input string, from int) [][2]int {
	var comments [][2]int
	for i := from; i < len(input); i++ {
		switch {
		case input[i] == '#' || (input[i] == '/' && i+1 < len(input) && input[i+1] == '/'):
			end := i
			for end < len(input) && input[end] != '\n' {
				end++
			}
			comments = append(comments, [2]int{i, end})
			i = end
		case input[i] == '/' && i+1 < len(input) && input[i+1] == '*':
			end := i + 2
			for end+1 < len(input) && !(input[end] == '*' && input[end+1] == '/') {
				end++
			}
			comments = append(comments, [2]int{i, end + 2})
			i = end + 1
		}
	}
	return comments
}
