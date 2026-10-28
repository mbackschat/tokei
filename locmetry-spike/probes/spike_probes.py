"""Write spike probes with hand-derived counts, run the harness, and score each counter."""
import json, os, subprocess, sys, tempfile

P = [
 ("rust-char", "a.rs", 'fn main() {\n    let q = \'"\';\n    let s = "/*";\n    let a = 1;\n}\n', (5,0,0)),
 ("rust-raw-3-hashes", "a.rs", 'fn main() {\n    let s = r###"a "## /* "###;\n    let a = 1;\n}\n', (4,0,0)),
 ("rust-raw-backslash", "a.rs", 'fn main() {\n    let p = r"C:\\";\n    let s = "/*";\n    let a = 1;\n}\n', (5,0,0)),
 ("rust-lifetime", "a.rs", "fn f<'a>(x: &'a str) -> &'a str {\n    // note\n    let _s = \"/*\";\n    x\n}\n", (4,1,0)),
 ("rust-nested-comment", "a.rs", "/* a /* b */ still */\nfn main() {}\n", (1,1,0)),
 ("rust-multiline-string", "a.rs", 'fn main() {\n    let s = "a\n\n// not a comment\nb";\n}\n', (6,0,0)),
 ("go-raw-string", "a.go", 'package main\n\nimport "fmt"\n\nfunc main() {\n\ts := `/*`\n\tfmt.Println(s)\n}\n', (6,0,2)),
 ("go-rune", "a.go", 'package main\n\nfunc main() {\n\tq := \'"\'\n\ts := "/*"\n\t_, _ = q, s\n}\n', (6,0,1)),
 ("go-raw-multiline", "a.go", 'package main\n\nvar s = `\n// not a comment\n\n`\n', (5,0,1)),
 ("c-char", "a.c", 'int main(void) {\n    char q = \'"\';\n    const char *s = "/*";\n    return q + s[0];\n}\n', (5,0,0)),
 ("c-escaped-quote-char", "a.c", "int main(void) {\n    char q = '\\'';\n    const char *s = \"/*\";\n    // note\n    return q + s[0];\n}\n", (5,1,0)),
 ("java-char", "A.java", 'class A {\n    char q = \'"\';\n    String s = "/*";\n    int a = 1;\n}\n', (5,0,0)),
 ("java-text-block", "B.java", 'class B {\n    String t = """\n        He said "hi\n        """;\n    String s = "/*";\n    int a = 1;\n}\n', (7,0,0)),
 ("js-regex-slash-star", "a.js", 'const trim = (p) => p.replace(/\\/*$/, "");\nconst a = 1;\nexport { trim, a };\n', (3,0,0)),
 ("js-regex-quote", "b.js", 'const q = /"/;\nconst s = "/*";\nconst a = 1;\n', (3,0,0)),
 ("js-division", "c.js", 'const a = 4, b = 2, g = 1;\nconst r = a / b / g; // note\n/* c */\nconst s = (a) / 2;\n', (3,1,0)),
 ("js-template-nesting", "d.js", 'const x = 1;\nconst s = `a ${`/*${x}`} b\n\n// not a comment\n`;\n// note\n', (5,1,0)),
 ("ts-generic-regex", "a.ts", 'function f<T>(x: T): T { return x; }\nconst re = /[/*]/g;\n// note\nexport { f, re };\n', (3,1,0)),
 ("python-fstring-nested", "a.py", 'd = {"k": 1}\ns = f"{d["k"]}"\n# comment\na = 1\n', (3,1,0)),
 ("python-triple-hash", "b.py", 's = """\n# not a comment\n\n"""\n# comment\na = 1\n', (5,1,0)),
 ("python-raw-quote", "c.py", 's = r"a\\"b # x"\n# comment\na = 1\n', (2,1,0)),
 ("shell-heredoc-apostrophe", "a.sh", "cat <<EOF\nDon't panic\nEOF\n# comment\necho ok\n", (4,1,0)),
 ("shell-heredoc-quoted-hash", "b.sh", "cat <<'EOF'\n# not a comment\nEOF\n# comment\necho ok\n", (4,1,0)),
 ("shell-param-length", "c.sh", 'x=abc\necho ${#x} # note\n# comment\necho "$(echo "a # b")"\n', (3,1,0)),
 ("shell-dollar-hash", "d.sh", 'echo $# a#b\n# comment\n', (1,1,0)),
]

harness = sys.argv[1]
with tempfile.TemporaryDirectory() as root:
    for i, (pid, name, text, _) in enumerate(P):
        d = os.path.join(root, f"{i:02d}"); os.makedirs(d); open(os.path.join(d, name), "w").write(text)
    rows = {json.loads(l)["path"].split("/")[0]: json.loads(l) for l in subprocess.run([harness, "count", root], capture_output=True, text=True, check=True).stdout.splitlines()}
    score = {"tokei": 0, "ts": 0, "lex": 0, "fork": 0}
    print(f"{'probe':28} {'expected':10} {'tokei':10} {'tree-sitter':12} {'lexer':10} {'fork':10}")
    for i, (pid, _, _, exp) in enumerate(P):
        r = rows[f"{i:02d}"]
        cells = []
        for k in ("tokei", "ts", "lex", "fork"):
            got = tuple(r[k]); ok = got == exp; score[k] += ok
            cells.append(("ok" if ok else str(got)))
        err = " (ts parse error)" if r["ts_error"] else ""
        print(f"{pid:28} {str(exp):10} {cells[0]:10} {cells[1]:12} {cells[2]:10} {cells[3]:10}{err}")
    print(f"correct: tokei {score['tokei']}/{len(P)}, tree-sitter {score['ts']}/{len(P)}, lexer {score['lex']}/{len(P)}, fork {score['fork']}/{len(P)}")
