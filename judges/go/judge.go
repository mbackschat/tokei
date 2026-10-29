// Judges Go files with the standard library's own scanner, go/scanner:
// reads paths on stdin and prints, per file, the comment and literal ranges
// in byte offsets after a byte order mark, and the number of scanner errors,
// as judgecmp's stream expects. String and rune literals are literals. The
// scanner drops carriage returns from the text of comments and raw strings
// (go/scanner, Scanner.Scan), so their ends are found in the source.
package main

import (
	"bufio"
	"bytes"
	"encoding/json"
	"go/scanner"
	"go/token"
	"os"
	"unicode/utf8"
)

type judgement struct {
	Path     string   `json:"path"`
	Units    string   `json:"units"`
	Errors   int      `json:"errors"`
	Comments [][2]int `json:"comments"`
	Literals [][2]int `json:"literals"`
}

func main() {
	in := bufio.NewScanner(os.Stdin)
	out := bufio.NewWriter(os.Stdout)
	defer out.Flush()
	encoder := json.NewEncoder(out)
	for in.Scan() {
		path := in.Text()
		src, err := os.ReadFile(path)
		if err != nil || !utf8.Valid(src) {
			continue
		}
		src = bytes.TrimPrefix(src, []byte("\xef\xbb\xbf"))
		j := judgement{Path: path, Units: "bytes", Comments: [][2]int{}, Literals: [][2]int{}}
		fset := token.NewFileSet()
		file := fset.AddFile(path, -1, len(src))
		var s scanner.Scanner
		s.Init(file, src, func(token.Position, string) { j.Errors++ }, scanner.ScanComments)
		for {
			pos, tok, lit := s.Scan()
			if tok == token.EOF {
				break
			}
			start := file.Offset(pos)
			switch {
			case tok == token.COMMENT && lit[1] == '/':
				j.Comments = append(j.Comments, [2]int{start, end(src, start, "\n", 0)})
			case tok == token.COMMENT:
				j.Comments = append(j.Comments, [2]int{start, end(src, start+2, "*/", 2)})
			case tok == token.STRING && lit[0] == '`':
				j.Literals = append(j.Literals, [2]int{start, end(src, start+1, "`", 1)})
			case tok == token.STRING || tok == token.CHAR:
				j.Literals = append(j.Literals, [2]int{start, start + len(lit)})
			}
		}
		encoder.Encode(j)
	}
}

// end returns the offset of the first closer after from, plus keep of its
// bytes, or the end of the source when it never closes.
func end(src []byte, from int, closer string, keep int) int {
	if i := bytes.Index(src[from:], []byte(closer)); i >= 0 {
		return from + i + keep
	}
	return len(src)
}
