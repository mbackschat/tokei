// Judges Jsonnet files with go-jsonnet v0.22.0's lexer, vendored in parser/
// because go-jsonnet keeps it internal: reads paths on stdin and prints, per
// file, the comment and literal ranges in byte offsets, as judgecmp's stream
// expects. A file the lexer rejects counts as one error.
package main

import (
	"bufio"
	"encoding/json"
	"os"

	"jsonnet-judge/parser"
)

type judgement struct {
	Path     string   `json:"path"`
	Units    string   `json:"units"`
	Errors   int      `json:"errors"`
	Comments [][2]int `json:"comments"`
	Literals [][2]int `json:"literals"`
	// Diagnosis tells why a file was rejected; judgecmp ignores it.
	Diagnosis string `json:"diagnosis,omitempty"`
}

func judge(path string) judgement {
	j := judgement{Path: path, Units: "bytes", Comments: [][2]int{}, Literals: [][2]int{}}
	data, err := os.ReadFile(path)
	if err != nil {
		j.Errors = 1
		return j
	}
	comments, literals, err := parser.Spans(string(data))
	if err != nil {
		j.Errors = 1
		j.Diagnosis = err.Error()
		return j
	}
	j.Comments = append(j.Comments, comments...)
	j.Literals = append(j.Literals, literals...)
	return j
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
