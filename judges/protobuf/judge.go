// Judges Protocol Buffers files with protocompile, the pure-Go compiler
// github.com/bufbuild/protocompile that the buf CLI compiles with in
// place of protoc: reads paths on stdin and prints, per file, the comment
// and literal ranges in byte offsets, as judgecmp's stream expects. Its
// lexer keeps every comment, `//` and `/* */`, as an item between the
// tokens; string literal tokens are literals. A file the parser rejects
// counts as one error.
package main

import (
	"bufio"
	"bytes"
	"encoding/json"
	"os"

	"github.com/bufbuild/protocompile/parser"
	"github.com/bufbuild/protocompile/reporter"
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
	data, err := os.ReadFile(path)
	if err != nil {
		j.Errors = 1
		return j
	}
	file, err := parser.Parse(path, bytes.NewReader(data), reporter.NewHandler(nil))
	if err != nil {
		j.Errors = 1
	}
	if file == nil {
		return j
	}
	items := file.Items()
	for item, ok := items.First(); ok; item, ok = items.Next(item) {
		token, comment := file.GetItem(item)
		if comment.IsValid() {
			j.Comments = append(j.Comments, [2]int{comment.Start().Offset, comment.End().Offset + 1})
			continue
		}
		info := file.TokenInfo(token)
		if text := info.RawText(); len(text) > 0 && (text[0] == '"' || text[0] == '\'') {
			j.Literals = append(j.Literals, [2]int{info.Start().Offset, info.End().Offset + 1})
		}
	}
	return j
}

func main() {
	in := bufio.NewScanner(os.Stdin)
	out := json.NewEncoder(os.Stdout)
	for in.Scan() {
		out.Encode(judge(in.Text()))
	}
}
