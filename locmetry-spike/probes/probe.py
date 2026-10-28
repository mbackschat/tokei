"""Measure Tokei-counted probe files with locmetry and compare with hand-derived counts."""
import json, os, subprocess, sys, tempfile

C = [
 # id, file, text, expected (code, comments, blanks), validator
 ("rust-char", "main.rs", 'fn main() {\n    let q = \'"\';\n    let s = "/*";\n    let a = 1;\n}\n', (5,0,0), "rustc --edition 2021 --emit=metadata -o {d}/out.rmeta {f}"),
 ("rust-raw-3-hashes", "main.rs", 'fn main() {\n    let s = r###"a "## /* "###;\n    let a = 1;\n}\n', (4,0,0), "rustc --edition 2021 --emit=metadata -o {d}/out.rmeta {f}"),
 ("rust-raw-backslash", "main.rs", 'fn main() {\n    let p = r"C:\\";\n    let s = "/*";\n    let a = 1;\n}\n', (5,0,0), "rustc --edition 2021 --emit=metadata -o {d}/out.rmeta {f}"),
 ("swift-raw", "main.swift", 'let s = #"a "/*" b"#\nlet a = 1\nprint(s, a)\n', (3,0,0), "swiftc -parse {f}"),
 ("swift-multiline-lone-quote", "main.swift", 'let s = """\nHe said "hi\n"""\nlet t = "/*"\nlet a = 1\nprint(s, t, a)\n', (6,0,0), "swiftc -parse {f}"),
 ("go-raw-string", "main.go", 'package main\n\nimport "fmt"\n\nfunc main() {\n\ts := `/*`\n\tfmt.Println(s)\n}\n', (6,0,2), None),
 ("go-rune", "main.go", 'package main\n\nfunc main() {\n\tq := \'"\'\n\ts := "/*"\n\t_, _ = q, s\n}\n', (6,0,1), None),
 ("c-char", "main.c", 'int main(void) {\n    char q = \'"\';\n    const char *s = "/*";\n    return q + s[0];\n}\n', (5,0,0), "cc -fsyntax-only {f}"),
 ("cpp-raw-delimiter", "main.cpp", 'const char *s = R"sql(a "/*" b)sql";\nint a = 1;\nint main() { return a + s[0]; }\n', (3,0,0), "c++ -std=c++17 -fsyntax-only {f}"),
 ("java-char", "A.java", 'class A {\n    char q = \'"\';\n    String s = "/*";\n    int a = 1;\n}\n', (5,0,0), "javac -d {d}/out {f}"),
 ("java-text-block", "B.java", 'class B {\n    String t = """\n        He said "hi\n        """;\n    String s = "/*";\n    int a = 1;\n}\n', (7,0,0), "javac -d {d}/out {f}"),
 ("kotlin-char", "main.kt", 'val q = \'"\'\nval s = "/*"\nval a = 1\n', (3,0,0), None),
 ("csharp-raw-string", "C.cs", 'class C {\n    string r = """a "/*" b""";\n    int a = 1;\n}\n', (4,0,0), None),
 ("csharp-interpolation", "D.cs", 'class D {\n    static bool f = true;\n    string s = $"{(f ? "/*" : "")}";\n    int a = 1;\n}\n', (5,0,0), None),
 ("js-regex-slash-star", "a.js", 'const trim = (p) => p.replace(/\\/*$/, "");\nconst a = 1;\nexport { trim, a };\n', (3,0,0), "node --check --input-type=module < {f}"),
 ("js-regex-quote", "b.js", 'const q = /"/;\nconst s = "/*";\nconst a = 1;\n', (3,0,0), "node --check {f}"),
 ("jsx-apostrophe", "a.jsx", "export const A = () => <p>Don't panic</p>;\n// comment\nexport const b = 1;\n", (2,1,0), None),
 ("tsx-apostrophe", "a.tsx", "export const A = () => <p>Don't panic</p>;\n// comment\nexport const b = 1;\n", (2,1,0), None),
 ("html-apostrophe", "a.html", "<p>Don't panic</p>\n<!-- comment -->\n<p>ok</p>\n", (2,1,0), None),
 ("vue-apostrophe", "a.vue", "<template>\n  <p>Don't panic</p>\n  <!-- comment -->\n</template>\n", (3,1,0), None),
 ("python-fstring-nested", "a.py", 'd = {"k": 1}\ns = f"{d["k"]}"\n# comment\na = 1\n', (3,1,0), None),
 ("ruby-heredoc-apostrophe", "a.rb", "text = <<~EOS\n  Don't panic\nEOS\n# comment\na = 1\n", (4,1,0), "ruby -c {f}"),
 ("shell-heredoc-apostrophe", "a.sh", "cat <<EOF\nDon't panic\nEOF\n# comment\necho ok\n", (4,1,0), "bash -n {f}"),
 ("php-attribute", "a.php", "<?php\n#[Attribute]\nclass A {}\n", (3,0,0), None),
 ("haskell-string", "Main.hs", 'main :: IO ()\nmain = putStrLn "{-"\n-- comment\nx :: Int\nx = 1\n', (4,1,0), None),
 ("elm-string", "Main.elm", 'module Main exposing (s, a)\ns = "{-"\n-- comment\na = 1\n', (3,1,0), None),
 ("racket-string", "a.rkt", '#lang racket\n(define s "#|")\n; comment\n(define a 1)\n', (3,1,0), None),
 ("ocaml-char", "a.ml", 'let q = \'"\'\nlet s = "(*"\nlet a = 1\n', (3,0,0), None),
 ("ocaml-quoted-string", "b.ml", 'let s = {|(*|}\nlet a = 1\n', (2,0,0), None),
 ("fsharp-multiply-operator", "a.fs", 'let product = List.fold (*) 1 [1; 2; 3]\nlet a = 1\n', (2,0,0), None),
 ("yaml-apostrophe", "a.yaml", "description: Don't panic\n# comment\nname: x\n", (2,1,0), None),
 ("lean-string (fixed)", "Main.lean", 'def s : String := "/-"\n-- comment\ndef a : Nat := 1\n', (2,1,0), None),
]

binary = sys.argv[1]
with tempfile.TemporaryDirectory() as root:
    for i, (cid, name, text, _, check) in enumerate(C):
        d = os.path.join(root, f"{i:02d}")
        os.makedirs(d)
        f = os.path.join(d, name)
        open(f, "w").write(text)
        C[i] = C[i] + ((None if check is None else subprocess.run(check.format(f=f, d=d), shell=True, capture_output=True).returncode == 0),)
    out = json.loads(subprocess.run([binary, "--no-user-config", "--files", "--format", "json", "--root", root], capture_output=True, check=True).stdout)
    got = {r["path"].split("/")[0]: r for r in out["files"]}
    for i, (cid, name, text, exp, _, valid) in enumerate(C):
        r = got.get(f"{i:02d}")
        if r is None:
            print(f"{cid:30} NOT COUNTED"); continue
        c = r["counts"]; act = (c["code"], c["comments"], c["blanks"])
        verdict = "ok" if act == exp else ("UNDERCOUNTS CODE" if act[0] < exp[0] else "overcounts code")
        v = {True: "valid", False: "INVALID", None: "unchecked"}[valid]
        print(f"{cid:30} {r['language']:11} expected {exp} got {act}  {verdict:17} [{v}]")
