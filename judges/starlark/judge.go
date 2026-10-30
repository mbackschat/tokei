// Judges Bazel files with starlark-go, the Go implementation of the
// Starlark language that Bazel's BUILD and .bzl files are written in, which
// follows the same specification as Bazel's own Java implementation: reads
// paths on stdin and prints, per file, the comment and literal ranges in
// byte offsets after any byte order mark, as judgecmp's stream expects. The
// parser, run with every dialect option on and with comments retained,
// attaches each comment to a node; string and bytes literal tokens are
// literals. A file the parser rejects counts as one error.
package main

import (
	"bufio"
	"bytes"
	"encoding/json"
	"os"
	"unicode/utf8"

	"go.starlark.net/syntax"
)

type judgement struct {
	Path     string   `json:"path"`
	Units    string   `json:"units"`
	Errors   int      `json:"errors"`
	Comments [][2]int `json:"comments"`
	Literals [][2]int `json:"literals"`
}

// offsets maps starlark-go's positions, whose columns count runes, to byte
// offsets.
type offsets struct {
	data       []byte
	lineStarts []int
}

func newOffsets(data []byte) offsets {
	starts := []int{0}
	for i, b := range data {
		if b == '\n' {
			starts = append(starts, i+1)
		}
	}
	return offsets{data, starts}
}

func (o offsets) of(p syntax.Position) int {
	at := o.lineStarts[p.Line-1]
	for col := int32(1); col < p.Col; col++ {
		_, size := utf8.DecodeRune(o.data[at:])
		at += size
	}
	return at
}

func judge(path string) judgement {
	j := judgement{Path: path, Units: "bytes", Comments: [][2]int{}, Literals: [][2]int{}}
	data, err := os.ReadFile(path)
	if err != nil {
		j.Errors = 1
		return j
	}
	data = bytes.TrimPrefix(data, []byte("\xef\xbb\xbf"))
	options := syntax.FileOptions{Set: true, While: true, TopLevelControl: true, GlobalReassign: true, Recursion: true}
	file, err := options.Parse(path, data, syntax.RetainComments)
	if err != nil {
		j.Errors = 1
		return j
	}
	at := newOffsets(data)
	comment := func(comments []syntax.Comment) {
		for _, c := range comments {
			start := at.of(c.Start)
			j.Comments = append(j.Comments, [2]int{start, start + len(c.Text)})
		}
	}
	syntax.Walk(file, func(n syntax.Node) bool {
		if n == nil {
			return false
		}
		if c := n.Comments(); c != nil {
			comment(c.Before)
			comment(c.Suffix)
			comment(c.After)
		}
		if lit, ok := n.(*syntax.Literal); ok && (lit.Token == syntax.STRING || lit.Token == syntax.BYTES) {
			start := at.of(lit.TokenPos)
			j.Literals = append(j.Literals, [2]int{start, start + len(lit.Raw)})
		}
		return true
	})
	return j
}

func main() {
	in := bufio.NewScanner(os.Stdin)
	out := json.NewEncoder(os.Stdout)
	for in.Scan() {
		if err := out.Encode(judge(in.Text())); err != nil {
			panic(err)
		}
	}
}
