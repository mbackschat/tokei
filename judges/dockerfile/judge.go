// Judges Dockerfiles with BuildKit's own Dockerfile parser,
// github.com/moby/buildkit/frontend/dockerfile/parser: reads paths on stdin
// and prints, per file, the comment and literal ranges in byte offsets after
// a byte order mark, as judgecmp's stream expects. A line whose first
// non-whitespace character is `#` is a comment, parser directives included,
// also inside an instruction continued over lines, which the parser drops;
// a `#` anywhere else is an argument (Dockerfile reference, "Format"). The
// body and terminator of a heredoc, as the parser finds them, are literals,
// since BuildKit passes them on verbatim. A file the parser rejects counts
// as one error.
package main

import (
	"bufio"
	"bytes"
	"encoding/json"
	"os"
	"regexp"
	"strings"
	"unicode/utf8"

	"github.com/moby/buildkit/frontend/dockerfile/parser"
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
	result, err := parser.Parse(bytes.NewReader(src))
	if err != nil {
		j.Errors = 1
		return j
	}
	// starts[n] is the offset of line n+1; the last entry is the end.
	starts := []int{0}
	for i, b := range src {
		if b == '\n' {
			starts = append(starts, i+1)
		}
	}
	if starts[len(starts)-1] != len(src) {
		starts = append(starts, len(src))
	}
	lines := len(starts) - 1
	text := func(n int) string { return strings.TrimRight(string(src[starts[n]:starts[n+1]]), "\r\n") }
	heredoc := make([]bool, lines)
	for _, node := range result.AST.Children {
		from := node.StartLine - 1
		for _, doc := range node.Heredocs {
			opener := regexp.MustCompile(`<<-?["']?` + regexp.QuoteMeta(doc.Name) + `\b`)
			for from < node.EndLine && !opener.MatchString(text(from)) {
				from++
			}
			// The body starts on the line after its opener's; bodies follow one
			// another in the order of their openers.
			body := from + 1
			end := body
			for end < lines {
				line := text(end)
				if doc.Chomp {
					line = strings.TrimLeft(line, "\t")
				}
				if line == doc.Name {
					break
				}
				end++
			}
			for n := body; n <= end && n < lines; n++ {
				heredoc[n] = true
			}
			from = end
		}
	}
	for n := 0; n < lines; n++ {
		if heredoc[n] {
			j.Literals = append(j.Literals, [2]int{starts[n], starts[n+1]})
			continue
		}
		line := text(n)
		trimmed := strings.TrimLeft(line, " \t")
		if strings.HasPrefix(trimmed, "#") {
			j.Comments = append(j.Comments, [2]int{starts[n] + len(line) - len(trimmed), starts[n] + len(line)})
		}
	}
	return j
}
