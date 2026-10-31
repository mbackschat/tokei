// Judges HCL files, Terraform's included, with hclsyntax's lexer, the one
// HashiCorp's HCL parser runs: reads paths on stdin and prints, per file, the
// comment and literal ranges in byte offsets after any byte order mark, as
// judgecmp's stream expects. Comment tokens are comments; quoted strings and
// heredocs, their delimiters and their text, are literals, while the `${…}`
// interpolations and `%{…}` directives they hold are code. A file with a
// lexical error counts as one error.
package main

import (
	"bufio"
	"bytes"
	"encoding/json"
	"os"

	"github.com/hashicorp/hcl/v2"
	"github.com/hashicorp/hcl/v2/hclsyntax"
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
	data = bytes.TrimPrefix(data, []byte("\xef\xbb\xbf"))
	tokens, diags := hclsyntax.LexConfig(data, path, hcl.Pos{Line: 1, Column: 1, Byte: 0})
	if diags.HasErrors() {
		j.Errors = 1
	}
	span := func(t hclsyntax.Token) [2]int { return [2]int{t.Range.Start.Byte, t.Range.End.Byte} }
	for _, t := range tokens {
		switch t.Type {
		case hclsyntax.TokenComment:
			j.Comments = append(j.Comments, span(t))
		case hclsyntax.TokenOQuote, hclsyntax.TokenCQuote, hclsyntax.TokenQuotedLit,
			hclsyntax.TokenOHeredoc, hclsyntax.TokenCHeredoc, hclsyntax.TokenStringLit:
			j.Literals = append(j.Literals, span(t))
		case hclsyntax.TokenInvalid, hclsyntax.TokenBadUTF8, hclsyntax.TokenNil:
			j.Errors = 1
		}
	}
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
