// Judges Go assembly files with the tokenizer Go's assembler uses: reads
// paths on stdin and prints, per file, the comment and literal ranges in
// byte offsets after a byte order mark, and the number of scanner errors, as
// judgecmp's stream expects.
//
// cmd/asm reads its input with text/scanner in the mode, whitespace and
// identifier characters of `NewTokenizer` in cmd/asm/internal/lex/tokenizer.go
// (Go 1.25): `//` and `/* */` comments, `"…"` strings and `'…'` characters,
// no raw strings. Only files that name a symbol as `name(SB)` are Go's;
// GNU-style assembly is for the gas judge.
package main

import (
	"bufio"
	"bytes"
	"encoding/json"
	"os"
	"text/scanner"
	"unicode"
)

type judgement struct {
	Path     string   `json:"path"`
	Units    string   `json:"units"`
	Errors   int      `json:"errors"`
	Comments [][2]int `json:"comments"`
	Literals [][2]int `json:"literals"`
}

// isIdentRune is cmd/asm's: a center dot and a division slash are identifier
// characters too.
func isIdentRune(ch rune, i int) bool {
	if unicode.IsLetter(ch) {
		return true
	}
	switch ch {
	case '_', '·', '∕':
		return true
	}
	return i > 0 && unicode.IsDigit(ch)
}

func judge(path string) *judgement {
	data, err := os.ReadFile(path)
	if err != nil || !bytes.Contains(data, []byte("(SB)")) {
		return nil
	}
	data = bytes.TrimPrefix(data, []byte("\xef\xbb\xbf"))
	result := &judgement{Path: path, Units: "bytes", Comments: [][2]int{}, Literals: [][2]int{}}
	var s scanner.Scanner
	s.Init(bytes.NewReader(data))
	s.Whitespace = 1<<'\t' | 1<<'\r' | 1<<' '
	s.Mode = scanner.ScanChars | scanner.ScanFloats | scanner.ScanIdents |
		scanner.ScanInts | scanner.ScanStrings | scanner.ScanComments
	s.IsIdentRune = isIdentRune
	s.Error = func(*scanner.Scanner, string) { result.Errors++ }
	for token := s.Scan(); token != scanner.EOF; token = s.Scan() {
		span := [2]int{s.Position.Offset, s.Pos().Offset}
		switch token {
		case scanner.Comment:
			result.Comments = append(result.Comments, span)
		case scanner.String, scanner.Char:
			result.Literals = append(result.Literals, span)
		}
	}
	return result
}

func main() {
	in := bufio.NewScanner(os.Stdin)
	out := bufio.NewWriter(os.Stdout)
	defer out.Flush()
	encoder := json.NewEncoder(out)
	for in.Scan() {
		if result := judge(in.Text()); result != nil {
			encoder.Encode(result)
		}
	}
}
