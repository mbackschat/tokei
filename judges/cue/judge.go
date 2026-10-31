// Judges CUE files with the scanner of cuelang.org/go v0.17.1, the one the
// cue command parses with: reads paths on stdin and prints, per file, the
// comment and literal ranges in byte offsets, as judgecmp's stream expects.
// Comment tokens are comments; strings are literals, while the `\(…)`
// interpolations they hold are code, read by resuming the string at the
// parenthesis that closes each one, as the scanner's contract asks of its
// caller (Scanner.Scan, "String Interpolations"). An attribute such as
// `@go(x)` is one token, read as code. A file with a scanning error counts
// as one error.
package main

import (
	"bufio"
	"encoding/json"
	"os"

	"cuelang.org/go/cue/scanner"
	"cuelang.org/go/cue/token"
)

type judgement struct {
	Path     string   `json:"path"`
	Units    string   `json:"units"`
	Errors   int      `json:"errors"`
	Comments [][2]int `json:"comments"`
	Literals [][2]int `json:"literals"`
}

func judge(path string) judgement {
	j := judgement{Path: path, Units: "bytes", Comments: [][2]int{}, Literals: [][2]int{}}
	src, err := os.ReadFile(path)
	if err != nil {
		j.Errors = 1
		return j
	}
	file := token.NewFile(path, -1, len(src))
	var s scanner.Scanner
	s.Init(file, src, func(token.Pos, string, []interface{}) { j.Errors = 1 }, scanner.ScanComments)
	// A token ends where the scanner stopped, which an interpolation's
	// opening parenthesis follows; the literal's own length would not do,
	// as the scanner strips carriage returns from it.
	span := func(start int, tok token.Token) [2]int {
		end := s.Offset()
		if tok == token.INTERPOLATION {
			end++
		}
		return [2]int{start, end}
	}
	// The parenthesis depth at which each open interpolation resumes its string.
	var holes []int
	depth := 0
	for {
		pos, tok, _ := s.Scan()
		start := file.Offset(pos)
		switch tok {
		case token.EOF:
			if len(holes) > 0 {
				j.Errors = 1
			}
			return j
		case token.COMMENT:
			j.Comments = append(j.Comments, span(start, tok))
		case token.STRING:
			j.Literals = append(j.Literals, span(start, tok))
		case token.INTERPOLATION:
			j.Literals = append(j.Literals, span(start, tok))
			holes = append(holes, depth)
		case token.LPAREN:
			depth++
		case token.RPAREN:
			depth--
			if len(holes) > 0 && holes[len(holes)-1] == depth {
				holes = holes[:len(holes)-1]
				// The segment starts at the closing parenthesis just read.
				start := s.Offset() - 1
				if segment := s.ResumeInterpolation(); segment[len(segment)-1] == '(' {
					j.Literals = append(j.Literals, span(start, token.INTERPOLATION))
					holes = append(holes, depth)
				} else {
					j.Literals = append(j.Literals, span(start, token.STRING))
				}
			}
		}
	}
}

func main() {
	in := bufio.NewScanner(os.Stdin)
	out := bufio.NewWriter(os.Stdout)
	defer out.Flush()
	encoder := json.NewEncoder(out)
	for in.Scan() {
		encoder.Encode(judge(in.Text()))
	}
}
