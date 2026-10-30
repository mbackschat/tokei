# Language Judges

Dev tools for step 6 of Locmetry's [Tokei fork plan](https://github.com/mbackschat/locmetry/blob/main/docs/plans/tokei-fork.md): each judge reads files with the language's own lexer or parser, and `judgecmp` compares its verdicts with the fork's [`LanguageType::line_classes`](../src/language/language_type.rs) line by line. A line is a comment when it holds nothing but comments; a line of whitespace takes the class of the comment or literal its own bytes lie in, code for a literal, and is blank otherwise; every other line is code. A file the judge rejects is lexically invalid and listed rather than matched. Locmetry's [step 6 record](https://github.com/mbackschat/locmetry/blob/main/docs/research/tokei-fork-step-6.md) holds the measured repositories and results.

| Language | Judge | Run |
| --- | --- | --- |
| Rust | rustc's lexer, `ra-ap-rustc_lexer` 0.179.0, built into `judgecmp` | `judgecmp rust ROOT...` |
| Java | javac's scanner from the running JDK (measured with 27.0.1) | `find … -name '*.java' \| java/run.sh \| judgecmp stream Java` |
| Kotlin | `KotlinLexer` from `kotlin-compiler-embeddable` 2.4.0 | `… \| kotlin/run.sh \| judgecmp stream Kotlin` |
| JavaScript, JSX, TypeScript, TSX | Babel 7.28.5 with JSX, Flow and proposal plugins, then Hermes 0.36.1; the TypeScript 6.0.3 parser for TypeScript, TSX and JavaScript no other parser accepts | `pnpm install` in `js/`, then `… \| node js/judge.mjs \| judgecmp stream auto` |
| C, C++, Objective-C, Objective-C++ | clang's raw lexer, `clang -cc1 -dump-raw-tokens` (measured with Apple clang 21.0.0); headers lex as C++ | `… \| python3 c/judge.py \| judgecmp stream auto` |
| Go | the standard library's `go/scanner` (measured with Go 1.25.14) | build with `go build -o judge-linux judge.go` in `go/`, as in the `golang:1.25` image, then `… \| go/judge-linux \| judgecmp stream Go` |
| C# | Roslyn's parser, `Microsoft.CodeAnalysis.CSharp` 5.9.0; text in inactive `#if` regions parsed on its own | `dotnet build -c Release -o out` in `csharp/`, then `… \| dotnet csharp/out/Judge.dll \| judgecmp stream auto` |
| Ruby | Prism, `Prism.lex`, Ruby 3.4's default parser (measured with 3.4.12) | `… \| ruby ruby/judge.rb \| judgecmp stream Ruby`, as in the `ruby:3.4` image |
| PHP | PHP's `token_get_all` (measured with 8.4.27, `short_open_tag` on by default); text outside the PHP tags is in no range | `… \| php php/judge.php \| judgecmp stream PHP`, as in the `php:8.4-cli` image; Blade templates (`.blade.php`) are not PHP |
| Python | CPython's `tokenize` (measured with 3.14) | `… \| python3 python/judge.py \| judgecmp stream Python` |
| Lean | Lean 4.31.0's own lexer functions | see below |

Judges read paths on stdin and print one JSON object per file: `path`, `comments` and `literals` as `[start, end]` offsets after a byte order mark, `units` (`bytes` or `utf16`) and `errors`. `judgecmp stream LANGUAGE` reads them, `auto` takes each file's language from its path, and `JUDGE_LIST_INVALID=1` also lists the invalid files. Build `judgecmp` with `cargo build --release` in `judgecmp/`; it builds this repository as its `tokei`.

The Lean judge checks Locmetry's own Lean counter, which counts Lean in place of the fork. `python3 lean/leanlines/gen.py LOCMETRY/src/backend/lean.rs lean/leanlines/src/main.rs` generates a per-line copy of that counter; build it with `cargo build --release` in `lean/leanlines/` and the judge with `lake build` in `lean/`, then run `python3 lean/leancmp.py OUT.json ROOT...` over Git checkouts.
