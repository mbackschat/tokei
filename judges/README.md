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
| Swift | swift-syntax, the compiler's parser, from the Xcode toolchain's `usr/lib/swift/host` (measured with Apple Swift 6.4) | `swift/build.sh`, then `… \| swift/judge \| judgecmp stream Swift` |
| Dart | the scanner of the Dart SDK's front end, `_fe_analyzer_shared` 111.0.0 (measured with Dart 3.13.7) | `dart pub get` and `dart compile exe -o judge bin/judge.dart` in `dart/`, as in the `dart:stable` image, then `… \| dart/judge \| judgecmp stream Dart` |
| Scala | the Scala 3.10.0 compiler's parser and scanner, run on the JDK; Scala 2 code reads as Scala 3, with the scanner continuing where the parser gives up | `… \| scala/run.sh \| judgecmp stream Scala`, which downloads the compiler from Maven Central on first use |
| R | R's own parser, through the parse data of `parse(keep.source = TRUE)` (measured with R 4.6.1) | `… \| Rscript r/judge.R \| judgecmp stream R`, as in the `r-base` image with `jsonlite` installed |
| Elixir | Elixir's own tokenizer, `:elixir_tokenizer`, the one `Code.string_to_quoted_with_comments/2` runs (measured with Elixir 1.20.4 on OTP 29); a literal ends where the next token or comment starts, less the whitespace before it | `… \| elixir elixir/judge.exs \| judgecmp stream Elixir`, as in the `elixir` image |
| PowerShell | PowerShell's own parser, `Parser.ParseFile` (measured with PowerShell 7.5.0); bare words, which it also types as strings, are code | `… \| pwsh -NoProfile -File powershell/judge.ps1 \| judgecmp stream PowerShell`, as in the `mcr.microsoft.com/powershell:azurelinux-3.0-arm64` image |
| Haskell | GHC's own lexer, `lexTokenStream` from the `ghc` library (measured with GHC 9.14.1), with GHC2021 and each file's LANGUAGE pragmas; files that use the C preprocessor are lexed unprocessed and mostly fail | build with `ghc -O1 -package ghc Judge.hs -o judge` in `haskell/`, as in the `haskell` image, then `… \| haskell/judge \| judgecmp stream Haskell` |
| Lua | Lua's own lexer, `luaX_next` from `llex.c`, linked with Lua 5.5.1's core; the comments it skips are read from the text between tokens by its rules, and a first line starting with `#` is a comment, as `luaL_loadfile` skips it | `sh build.sh` in `lua/`, as in the `golang:1.25` image, then `… \| lua/judge-linux \| judgecmp stream Lua`; LuaJIT's integer suffixes, such as `1ULL`, are errors |
| Python | CPython's `tokenize` (measured with 3.14) | `… \| python3 python/judge.py \| judgecmp stream Python` |
| Lean | Lean 4.31.0's own lexer functions | see below |

Judges read paths on stdin and print one JSON object per file: `path`, `comments` and `literals` as `[start, end]` offsets after a byte order mark, `units` (`bytes` or `utf16`) and `errors`. `judgecmp stream LANGUAGE` reads them, `auto` takes each file's language from its path, and `JUDGE_LIST_INVALID=1` also lists the invalid files. Build `judgecmp` with `cargo build --release` in `judgecmp/`; it builds this repository as its `tokei`.

The Lean judge checks Locmetry's own Lean counter, which counts Lean in place of the fork. `python3 lean/leanlines/gen.py LOCMETRY/src/backend/lean.rs lean/leanlines/src/main.rs` generates a per-line copy of that counter; build it with `cargo build --release` in `lean/leanlines/` and the judge with `lake build` in `lean/`, then run `python3 lean/leancmp.py OUT.json ROOT...` over Git checkouts.

## Speed

Every release must parse within 10% of upstream Tokei's time on each measured repository, as Locmetry's owner decided on 2026-10-30. `python3 speed/check.py ROOT...` builds the same in-memory benchmark against this working tree and against upstream's `v15.0.0` tag, runs both 15 times interleaved on each root, compares their minimum user CPU time, and exits with an error when any root exceeds the threshold. `--threshold`, `--runs` and `--base` change those defaults; builds are cached in `speed/target`.

Upstream is given the string quotes listed in [baseline-quotes.json](speed/baseline-quotes.json) for languages whose upstream definition has none, so the check measures the fork's own overhead rather than the cost of reading strings at all; Locmetry's owner decided this on 2026-10-30, after upstream's quote-less Haskell made pandoc and cabal unreachable. Without the quotes, upstream analyses none of the lines holding strings, a quarter of cabal's Haskell lines and a third of pandoc's; with them, its own engine took 3.4 times as long on cabal's Haskell files.
