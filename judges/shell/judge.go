// Judges shell scripts with mvdan.cc/sh's parser, the one shfmt runs, since
// a shell reads a script only as it runs it: reads paths on stdin and prints,
// per file, the comment and literal ranges in byte offsets after a byte order
// mark, as judgecmp's stream expects. Files ending in `.zsh` are parsed as
// Zsh, all others as Bash, whose syntax extends POSIX sh's; a file one of
// them rejects is parsed as the other, since zsh scripts are often named
// `.sh` and completion scripts hold branches for both. Single-quoted
// and double-quoted strings, `$'…'` included, and heredoc bodies are
// literals, except the command substitutions a double-quoted string holds,
// which are code. A file the parser rejects counts as one error.
package main

import (
	"bufio"
	"bytes"
	"encoding/json"
	"os"
	"strings"
	"unicode/utf8"

	"mvdan.cc/sh/v3/syntax"
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
		encoder.Encode(judge(path, src))
	}
}

func judge(path string, src []byte) judgement {
	j := judgement{Path: path, Units: "bytes", Comments: [][2]int{}, Literals: [][2]int{}}
	variants := []syntax.LangVariant{syntax.LangBash, syntax.LangZsh}
	if strings.HasSuffix(path, ".zsh") {
		variants[0], variants[1] = variants[1], variants[0]
	}
	var file *syntax.File
	var err error
	for _, variant := range variants {
		parser := syntax.NewParser(syntax.KeepComments(true), syntax.Variant(variant))
		if file, err = parser.Parse(bytes.NewReader(src), path); err == nil {
			break
		}
	}
	if err != nil {
		j.Errors = 1
		return j
	}
	span := func(from, to syntax.Pos) [2]int { return [2]int{int(from.Offset()), int(to.Offset())} }
	syntax.Walk(file, func(node syntax.Node) bool {
		switch node := node.(type) {
		case *syntax.Comment:
			j.Comments = append(j.Comments, span(node.Pos(), node.End()))
		case *syntax.SglQuoted:
			j.Literals = append(j.Literals, span(node.Pos(), node.End()))
		case *syntax.DblQuoted:
			start := node.Pos()
			for _, part := range node.Parts {
				if hole, ok := part.(*syntax.CmdSubst); ok {
					j.Literals = append(j.Literals, span(start, hole.Pos()))
					start = hole.End()
				}
			}
			j.Literals = append(j.Literals, span(start, node.End()))
		case *syntax.Redirect:
			if node.Hdoc != nil && node.Hdoc.Pos().IsValid() {
				j.Literals = append(j.Literals, span(node.Hdoc.Pos(), node.Hdoc.End()))
			}
		}
		return true
	})
	return j
}
