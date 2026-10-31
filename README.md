# Tokei ([時計](https://en.wiktionary.org/wiki/%E6%99%82%E8%A8%88))
[![Mean Bean CI](https://github.com/XAMPPRocky/tokei/workflows/Mean%20Bean%20CI/badge.svg)](https://github.com/XAMPPRocky/tokei/actions?query=workflow%3A%22Mean+Bean+CI%22)
[![Help Wanted](https://img.shields.io/github/issues/XAMPPRocky/tokei/help%20wanted?color=green)](https://github.com/XAMPPRocky/tokei/issues?q=is%3Aissue+is%3Aopen+label%3A%22help+wanted%22)
[![Documentation](https://docs.rs/tokei/badge.svg)](https://docs.rs/tokei/)
![](https://img.shields.io/crates/d/tokei?label=downloads%20%28crates.io%29)
![](https://img.shields.io/github/downloads/xampprocky/tokei/total?label=downloads%20%28GH%29)
![](https://img.shields.io/homebrew/installs/dy/tokei?color=brightgreen&label=downloads%20%28brew%29)
![Chocolatey Downloads](https://img.shields.io/chocolatey/dt/tokei?label=Downloads%20(Chocolately))
[![dependency status](https://deps.rs/repo/github/XAMPPRocky/tokei/status.svg)](https://deps.rs/repo/github/XAMPPRocky/tokei)
[![Packaging status](https://repology.org/badge/tiny-repos/tokei.svg)](https://repology.org/project/tokei/versions)


> **This is the [Locmetry](https://github.com/mbackschat/locmetry) fork of Tokei 15.0.0**, on branch `locmetry`. It corrects comments and blank lines that Tokei miscounts when a literal holds a comment marker or a quote, as recorded in Locmetry's [Tokei counting defects](https://github.com/mbackschat/locmetry/blob/main/docs/tokei-defects.md) and [fork plan](https://github.com/mbackschat/locmetry/blob/main/docs/plans/tokei-fork.md). Its version carries the build metadata `+locmetry.N`, raised with every change to counting. The rest of this README describes upstream Tokei.

## Changes From Upstream

Engine changes, each with a regression file in `tests/data`:

| Change | Commit | Upstream |
| --- | --- | --- |
| `character_literals`: an escape running to the closing delimiter on the same line, or exactly one character, so a quote inside a character literal cannot open a string while Rust lifetimes and Haskell primes stay code. | `9e65ec4` | Not offered yet; compare [#1389](https://github.com/XAMPPRocky/tokei/pull/1389) |
| Verbatim-quote openers join the per-line fast path, so a line that only opens a Go backtick string is analysed. | `2c95b8f` | Not offered yet; compare [#1351](https://github.com/XAMPPRocky/tokei/pull/1351) |
| `code_tokens` (named `comment_exceptions` before 15.0.0+locmetry.3): code tokens that start like a comment or string, also exempt from the line-start comment rule. | `a23b225` | Not offered yet |
| A line that starts inside a literal is code, even when its text starts with a comment marker. | `801aae6` | Not offered yet |
| `doubled_quote_escapes`: closing delimiters of verbatim quotes that stand for themselves when written twice. | `b773e13` | Not offered yet |
| Opener matching runs only at bytes that can start a token, and bytes in strings and code that can change nothing are passed over, keeping the analysis faster than upstream. | `f8a9a6d`, `3296611` | Not offered yet |
| `raw_strings` and `raw_comments`: literals and comments whose closer repeats the delimiter count or label of their opener, optionally with an escape, restricted to one line or closing only at the start of a line. | `1926802`, `39610e6`, `dcbd729` | Not offered yet |
| `interpolations`: holes in strings whose contents are code and can hold strings, comments and further holes; text that only looks like a hole, such as C#'s `{{`, stays in the string. | `e778ec0`, `d77916d`, `9c039dc` | Not offered yet |
| `regex_literals`: `/…/` literals, told from division by the previous token or keyword, looking past comments and whitespace, or by a line break where it ends a statement. | `e778ec0`, `d77916d`, `9c039dc`, `785d277` | Not offered yet |
| `line_comment_starts_word`: a line comment marker opens a comment only where a word can start, after whitespace or one of the language's `word_delimiters`. | `aaa4c32`, `d0f5219` | Not offered yet |
| `heredocs`: a body running from the next line to a line holding the word the opener names, optionally indented or quoted; an opener without such a line is code. | `785d277` | Not offered yet |
| A backslash in a string also escapes the string's own closer and verbatim-quote openers, and closing a verbatim string ends its verbatim mode. | `aaa4c32` | Not offered yet |
| `quotes_in_tags`: quotes open strings only inside markup tags, so an apostrophe in text is text. | `5dd4fd7` | Not offered yet |
| `quotes_after`: a quote opens a string only at the start of the line or after one of the listed texts, where a trailing space stands for any whitespace. | `d0f5219` | Not offered yet |
| `block_scalars`: the lines after a `\|` or `>` indicator that are indented deeper than its node, and at least as deep as the first of them, are text, without the trailing blank lines that the default chomping strips. | `d0f5219`, `b6cf65a` | Not offered yet |
| `jsx`: JSX elements, whose text between tags is text, whose attribute strings have no escapes and whose `{…}` braces hold code; an element starts only where an expression may and when a closing tag or `/>` follows, so type parameters such as `<T,>` stay code. | `356c845` | Not offered yet |
| Rust doc comments are comments: the Markdown re-parse of `///` and `//!` blocks, which dropped their edge lines and counted empty `///` lines as blank, is removed. | `5994462` | Not offered yet |
| A line is a comment line only when it holds nothing but comments, such as `/* a */ // b`; code between or after comments, as in `/* a */ x; /* b */` or `b */ x;`, makes it code. PlantUML, whose block comments are whole lines, is read by line instead. | `5399062`, `655d44c` | Not offered yet |
| An embedded script or style counts only the lines between its tags; `type` and `lang` values are read up to their closing quote; tag names match case-insensitively, a `>` inside a quoted attribute value does not end a start tag, and an end tag's name ends at whitespace, `/` or `>`. | `418dd12`, `44f2a48`, `2220fce`, `61279ab` | Not offered yet |
| `line_start_comments`: comment markers that open a comment only at the start of a line. | `de1ab9e` | Not offered yet |
| `line_comment_flags`: bytes that, with blanks among them, may come before a line comment or a line-start comment at the start of a line and belong to it, as make's recipe flags and cmd.exe's `@` do. | `a022f12`, `ea14733` | Not offered yet |
| YAML-style `block_scalars` languages also read plain scalars that continue on later lines as text, open quotes after `[`, `{` and `,` only inside flow collections, read every line of a flow collection that spans lines, and take no simple-parse prefix; a line leaves the per-line fast path for a `[`, `{`, `\|` or `>` only where a flow collection or block scalar can start. | `a1a1886`, `13d1920`, `d551ec3` | Not offered yet |
| `quotes_in_tags`: a tag that continues on later lines keeps its quoted attribute values. | `d2b0180` | Not offered yet |
| `pod` and `end_markers`: Perl and NQP POD blocks and the data after an end marker are regions of whole lines. | `ddbcd99`, `c19e61e` | Not offered yet |
| `blank_symbols`: texts beyond ASCII whitespace that count as whitespace, such as WenYan's statement-separating punctuation or Rust's Unicode line separators, so a line of them is blank and a line of comments and them is a comment line. | `273e1d0`, `44a536d`, `7976ce3` | Not offered yet |
| `frontmatter`: a manifest between fences of three or more dashes, opening a file after an optional shebang, whose lines count as a literal's. | `44a536d` | Not offered yet |
| `shebang`: a first line starting with `#!` is a comment line; Rust's excludes `#![`, an inner attribute, looking past comments other than doc comments. | `f50998f` | Not offered yet |
| `run_closers`: a verbatim closer followed by more of its last character closes only at the end of the run, the characters before it being text. | `f50998f` | Not offered yet |
| `line_splices`: a line comment whose line ends in a splice continues on the next line. `c`: a backslash before a newline, which is whitespace. `make`: an odd run of backslashes before a newline, as GNU make joins lines, except in a recipe or `define` body, whose comments the shell may read. | `7976ce3`, `9f25025` | Not offered yet |
| Outside literals, an escape character before a line comment marker makes it text, as in make's `\#`. | `9f25025` | Not offered yet |
| `run_opens` on an interpolation: a hole opener of one repeated byte opens at the end of a longer run, the bytes before it being text. | `fc99a62` | Not offered yet |
| `regex_literals` options: `spans_lines`, a regular expression that may span lines; `operator_symbols`, `:/` as a symbol; `command_arguments`, a `/` or `%` literal after a method name and a space but before none. | `0b633d1` | Not offered yet |
| `prefix_characters`: a prefix and one character or escape form a character literal: `?` by Ruby's rule, where it neither ends a name nor stands before whitespace, or by Elixir's, where it does not end a name, and Erlang's `$`. | `0b633d1`, `9c6d1c7`, `b167ba5`, `dcbd729` | Not offered yet |
| `code_tags`: a language read only between its code tags, such as PHP's `<?php` and `?>`; the text outside them is code, and a `?>` ends a line comment. | `1923467` | Not offered yet |
| `identifier_quote_prefix`: a quote directly after an identifier opens as the declared opener with this prefix, such as Scala's `s"`. | `b298558` | Not offered yet |
| `line_directives`: a line that starts at its first byte with one of these texts, such as R's `#line`, is a directive and counts as code. | `add99a7` | Not offered yet |
| `line_start_closers`: a closer that ends a literal only after nothing but whitespace on its line, such as an Elixir heredoc's `"""` or a PowerShell here-string's `"@`. | `9c6d1c7` | Not offered yet |
| `escape_character`: the character that escapes the next one in a string that is not verbatim, a backslash unless declared, such as PowerShell's backtick. | `bad5ddc` | Not offered yet |
| `quote_aliases`: texts that a closer's leading quote character may be written as, such as PowerShell's `“`, `”` and `„` for `"`. | `bad5ddc` | Not offered yet |
| `code_pragmas`: a block comment opener followed, after any whitespace, by one of these words in any case is code, as GHC reads `{-# INLINE f #-}`. | `df73c4e` | Not offered yet |
| `string_gaps`: a backslash, whitespace that may span lines and a backslash inside a string are skipped, so the closing backslash of a gap is no escape. | `df73c4e` | Not offered yet |
| `quote_like`: Perl quote-like operators and Ruby percent literals, whose delimiters the program chooses, nest brackets and may span lines; Elixir sigils close at their first unescaped delimiter, a heredoc sigil at its quotes starting a line; Haskell quasi-quotes close at their first `\|]` and open only where the file's LANGUAGE pragmas turn on `QuasiQuotes`; GNU make's `$(…)` and `${…}` references close at the closer that balances their opener and start no comment; Erlang sigils close at their delimiter's partner, with escapes only in the vanilla, `~b` and `~s` forms, or at the quotes of a triple-quoted one starting a line. | `56379a6`, `9c6d1c7`, `df73c4e`, `9f25025`, `dcbd729` | Not offered yet |
| `template_expressions`: a template expression inside a markup tag, such as `{{ $a->b }}`, is skipped whole, so its `>` does not end the tag. | `598ce84` | Not offered yet |
| `backslash_operator`: a backslash outside a literal is an operator, so the quote after it opens a literal instead of being escaped. | `daf1c1d` | Not offered yet |
| `content_checked_extensions`: an extension claimed only by files whose text matches one of its patterns, which decide before a language that lists the extension plainly; `exact_filenames`: file names matched in their own case only. | `e42f092`, `ea47c6a` | Not offered yet |
| `syntax_of`: a language whose files another language's lexer reads, such as Nextflow's Groovy, takes that language's lexical fields instead of a copy that can drift. | `5312a2e` | Not offered yet |
| `rem_comments`: `REM` opens a line comment in any case where a token starts and no identifier character follows; `xml_literals`: a `<` where an expression may start opens an XML literal when the text from it is an element, a document, a comment, a CDATA section or a processing instruction, whose lines are code, or blank when only whitespace. | `b11aa64` | Not offered yet |
| `comment_literals`: inside a block comment, a string or character literal that closes on its line, by F#'s or OCaml's rules, is comment text, so a `*)` or `(*` within it neither closes nor opens a comment; a code token is one token there too. | `8ac5216`, `e93caf1`, `3a5b9a4` | Not offered yet |
| `postfix_primes`: a character literal's opener directly after an operand, such as Julia's adjoint `x'`, is an operator. | `92d542b` | Not offered yet |
| `fixed_form`: a file is in Fortran's fixed source form unless its content is free form by fparser's rule, so a `C`, `c` or `*` in its first column opens a comment line and an `!` in the sixth column of a line whose first five are blank marks a continuation, not a comment. | `fa4eb14` | Not offered yet |
| `text_sections`: a section of character data whose text holds no markup up to its closer, such as XML's `<![CDATA[`…`]]>`; its lines are code, or blank when only whitespace. | `cfdcdc5` | Not offered yet |
| Inside a comment that does not nest, an opener no longer hides a closer it overlaps, so `<!-->` ends an HTML comment and `/*/` a C one. | `a0c8f52` | Not offered yet |
| `line_classes` classes the lines of an embedded script or style element as its own language counts them. | `3eda0aa` | Not offered yet |
| `markup_expressions`: an expression in markup text or a tag, such as Svelte's `{…}`, is read with another language's syntax up to its closer; an `interpolations` hole may name such a language too. `tag_line_comments` and `tag_multi_line_comments`: comments a tag holds where an attribute may start. | `3d035cd` | Not offered yet |
| `code_line_markers`: outside literals and comments, the rest of the line after the marker is code, as after the backslash of a psql meta-command. `dollar_quotes`: PostgreSQL's `$tag$…$tag$` strings, whose body is code when it lexes as SQL and a literal otherwise. The `hash` shebang: any first line starting with `#`. | `25583bf`, `64d070b` | Not offered yet |
| `nested_comments` nest only inside a comment of their own kind, so D's `/+ … +/` holds a `/*` as text and a `/* … */` holds a `/+`. A per-file rule may turn nesting off: Pascal's comments nest, as Free Pascal reads them, unless nesting would leave a comment open, as Delphi's do not nest. | `5a72cb6` | Not offered yet |
| `heredocs` closer `indented_line`: the line holds only the word, after any indentation, as Crystal's `<<-` heredoc closes. | `cc13f4f` | Not offered yet |
| `preprocessor_directives`: a `#` opening a line followed by one of these C preprocessor directives, in its own case, or by nothing is code where `#` otherwise opens a line-start comment; with C's `line_splices`, a line that a splice joins to the one before starts no line-start comment. | `203d357` | Not offered yet |
| `quotes_close_at_line_end`: a string that does not close on its line ends with it, as NASM reads one; `masm_comments`: MASM's `COMMENT` directive makes a comment of the lines through the next occurrence of its delimiter. | `347319e` | Not offered yet |
| Markdown's fenced code blocks are found by CommonMark's block structure, ported from commonmark.js's first pass, in the whole file: `~~~` fences too, none inside paragraphs or other blocks, and the content of a block in a list item or block quote without their prefixes; its blank lines count. Markdown lines are counted from that block list rather than the generic line loop. | `bdc6aff`, `72d47d8`, `6758800` | Not offered yet |

Declarations in `languages.json`, each grounded in the cited specification or reference lexer:

| Languages | Declared | Commit | Sources |
| --- | --- | --- | --- |
| C, C header | Character literals | `9e65ec4`, `6798eb4` | [N3220](https://www.open-std.org/jtc1/sc22/wg14/www/docs/n3220.pdf) §6.4.4.5, §6.4.5, §6.4.9 |
| C++, C++ header, C++ module | Character literals; `R"x(…)x"` raw strings with delimiters of up to 16 characters | `6798eb4`, `1926802` | [lex.ccon](https://eel.is/c++draft/lex.ccon), [lex.string](https://eel.is/c++draft/lex.string), [lex.comment](https://eel.is/c++draft/lex.comment) |
| Rust | Character literals; `r`, `br` and `cr` raw strings with any number of `#` | `9e65ec4`, `2c95b8f`, `1926802` | [Tokens](https://doc.rust-lang.org/reference/tokens.html#character-literals), [raw strings](https://doc.rust-lang.org/reference/tokens.html#raw-string-literals) |
| Go | Character (rune) literals; backtick raw strings as verbatim quotes | `2c95b8f` | [Go spec](https://go.dev/ref/spec#Rune_literals), [string literals](https://go.dev/ref/spec#String_literals) |
| Java | Character literals; `"""` text blocks | `2c95b8f` | [JLS SE 21 §3.10.4–3.10.7](https://docs.oracle.com/javase/specs/jls/se21/html/jls-3.html) |
| Kotlin | Character literals; `"""` raw strings as verbatim quotes instead of escaping quotes, closing at the end of a run of quotes; `${…}` templates; `$$"…"` and `$$$"…"` strings, whose holes open with as many `$`; backtick identifiers; shebangs | `6798eb4`, `e778ec0`, `f50998f` | [Kotlin spec, tokens](https://kotlinlang.org/spec/syntax-and-grammar.html#tokens), [string mode grammar](https://kotlinlang.org/spec/syntax-and-grammar.html#string-mode-grammar), [multi-dollar interpolation](https://kotlinlang.org/docs/strings.html#multi-dollar-string-interpolation), [KotlinLexer.flex](https://github.com/JetBrains/kotlin/blob/548305369/compiler/psi/parser/src/org/jetbrains/kotlin/lexer/Kotlin.flex) |
| Groovy | `'…'`, `'''…'''`, `"…"` and `"""…"""` strings with `${…}` holes in the double-quoted ones; slashy strings where an expression starts, spanning lines; `$/…/$` dollar-slashy strings; the shebang line | `a70c21f` | [Groovy Language Documentation, Syntax](https://groovy-lang.org/syntax.html) |
| JSON | `//` and `/* */` comments of JSON with Comments, which shares the `.json` extension, and `"…"` strings, so a comment marker in a string stays text | `fc28619` | [RFC 8259](https://www.rfc-editor.org/rfc/rfc8259), [VS Code, JSON with Comments](https://code.visualstudio.com/docs/languages/json#_json-with-comments) |
| TOML | `'…'` and `'''…'''` literal strings without escapes; one or two quotes before a multi-line string's closing delimiter belong to the string | `485bcb3` | [TOML 1.0, String](https://toml.io/en/v1.0.0#string) |
| Dockerfile | `#` comments only at the start of a line, and no string syntax, since a `#` elsewhere is an argument and quotes are the shell's; `<<WORD` and `<<-WORD` heredocs, whose bodies are text | `5ab59ff` | [Dockerfile reference, Format and Here-Documents](https://docs.docker.com/reference/dockerfile/#format) |
| Batch | `rem` remarks in any case, as a whole command after blanks and `@`, and `::` labels; `REM` no longer opens a comment mid-line or inside a longer command name | `ea14733` | [Microsoft Learn, rem](https://learn.microsoft.com/en-us/windows-server/administration/windows-commands/rem), [goto](https://learn.microsoft.com/en-us/windows-server/administration/windows-commands/goto), [echo](https://learn.microsoft.com/en-us/windows-server/administration/windows-commands/echo), Wine `programs/cmd/wcmdmain.c` |
| INI | Lines holding only the Unicode whitespace that Python's `str.strip` removes are blank | `7598c1b` | CPython 3.13 `Lib/configparser.py`, `_Line` |
| Makefile | Comments that an odd run of backslashes continues, except in recipes and `define` bodies; no comment in `\#` or in a `$(…)` or `${…}` reference; comment lines in recipes after any of make's `@`, `-` and `+` flags | `9f25025`, `a022f12` | [GNU make manual, 3.1](https://www.gnu.org/software/make/manual/html_node/Makefile-Contents.html), [5.1.1](https://www.gnu.org/software/make/manual/html_node/Splitting-Recipe-Lines.html), GNU make 4.4.1 `src/read.c` and `src/job.c` |
| C# | Character literals; `""` in `@"…"` strings; `"""…"""` raw strings with any number of quotes; `{…}` holes in `$"…"`, `$@"…"` and `$"""…"""` strings, with `{{` as text and as many braces as `$` signs in raw ones | `6798eb4`, `b773e13`, `1926802`, `e778ec0` | [C# standard §6.4.5.5–6.4.5.6](https://github.com/dotnet/csharpstandard/blob/draft-v8/standard/lexical-structure.md), [§12.8.3](https://github.com/dotnet/csharpstandard/blob/draft-v8/standard/expressions.md#1283-interpolated-string-expressions), [raw string literals](https://github.com/dotnet/csharplang/blob/main/proposals/csharp-11.0/raw-string-literal.md) |
| Scala | Character literals; `"""` strings as verbatim quotes; `${…}` holes in `s`, `f` and `raw` interpolated strings, with `$$` as text | `6798eb4`, `e778ec0` | [Scala 2.13 lexical syntax](https://scala-lang.org/files/archive/spec/2.13/01-lexical-syntax.html), [Scala 3 syntax](https://docs.scala-lang.org/scala3/reference/syntax.html) |
| OCaml | Character literals; nested comments; `{id\|…\|id}` quoted strings | `6798eb4`, `1926802` | [OCaml 5.2 lexical conventions](https://ocaml.org/manual/5.2/lex.html), [lexer.mll](https://github.com/ocaml/ocaml/blob/5.2/parsing/lexer.mll) |
| F# | Character literals; nested comments; `"""` and `@"…"` verbatim strings with `""` escapes; `(*)` is code | `a23b225`, `6798eb4`, `b773e13` | [F# spec, lexical analysis](https://github.com/fsharp/fslang-spec/blob/main/spec/lexical-analysis.md), [lex.fsl](https://github.com/dotnet/fsharp/blob/main/src/Compiler/lex.fsl) |
| Swift | `"""` strings; `#"…"#` and `#"""…"""#` raw strings with any number of `#` and their `\#` escapes; `\(…)` interpolation | `1926802`, `39610e6`, `e778ec0` | [Lexical structure, string literals](https://github.com/swiftlang/swift-book/blob/main/TSPL.docc/ReferenceManual/LexicalStructure.md), [swift-syntax Lexer/Cursor.swift](https://github.com/swiftlang/swift-syntax/blob/main/Sources/SwiftParser/Lexer/Cursor.swift) |
| Lua | `[==[…]==]` long strings and `--[==[…]==]` long comments of any level | `1926802` | [Lua 5.4 manual §3.1](https://www.lua.org/manual/5.4/manual.html#3.1), [llex.c](https://github.com/lua/lua/blob/v5.4/llex.c) |
| JavaScript, TypeScript, JSX, TSX | `${…}` holes in template literals; regular expression literals | `e778ec0`, `d77916d` | [ECMA-262 2025 §12.9.5](https://tc39.es/ecma262/2025/#sec-literals-regular-expression-literals), [§12.9.6](https://tc39.es/ecma262/2025/#sec-template-literal-lexical-components) |
| JavaScript, JSX, TSX | JSX elements, including TypeScript type arguments in tags | `356c845` | [JSX specification](https://facebook.github.io/jsx/#sec-jsx-elements), [TypeScript 2.9, generic type arguments in JSX elements](https://www.typescriptlang.org/docs/handbook/release-notes/typescript-2-9.html#generic-type-arguments-in-jsx-elements) |
| HTML, XML, SVG, XAML, Vue, Svelte, Ruby HTML | Quotes only in tags | `5dd4fd7` | [HTML Living Standard, attributes](https://html.spec.whatwg.org/multipage/syntax.html#attributes-2), [XML 1.0 §3.1](https://www.w3.org/TR/xml/#sec-starttags) |
| YAML | `'…'` strings as verbatim quotes with `''` as a quote; quotes only where a flow scalar can start, after `: `, `- ` or `? `, and after `[`, `{` or `,` inside a flow collection; `\|` and `>` block scalars, also after `?` and at document level, with keep chomping; plain scalars that continue on later lines | `d0f5219`, `a1a1886`, `d551ec3` | [YAML 1.2.2 §7.3](https://yaml.org/spec/1.2.2/#73-flow-scalar-styles), [§7.4](https://yaml.org/spec/1.2.2/#74-flow-collection-styles), [§8.1](https://yaml.org/spec/1.2.2/#81-block-scalar-styles) |
| Dart | `r` raw strings; `${…}` holes in single, double and triple-quoted strings | `e778ec0` | [Strings](https://dart.dev/language/built-in-types#strings) |
| Ruby | `#{…}` holes in double-quoted strings; regular expression literals, also at the start of a line; `$'`, `$"` and `` $` `` globals are code; `<<ID`, `<<-ID` and `<<~ID` heredocs | `e778ec0`, `d77916d`, `785d277` | [Literals](https://github.com/ruby/ruby/blob/v3_4_7/doc/syntax/literals.rdoc), [globals](https://docs.ruby-lang.org/en/3.4/globals_rdoc.html) |
| Shell, Bash, Zsh, Korn shell | `$(…)` holes in double-quoted strings; `'…'` as verbatim and `$'…'` as escaping strings; `#` comments only at the start of a word, after an opening backquote too; `<<WORD` and `<<-WORD` heredocs, but not `<<<` here-strings | `aaa4c32`, `785d277`, `213fd8e` | [POSIX.1-2024 Shell Command Language §2.2.2–2.2.4, §2.3, §2.6.3, §2.7.4](https://pubs.opengroup.org/onlinepubs/9799919799/utilities/V3_chap02.html), [Bash Reference Manual, Here Strings](https://www.gnu.org/software/bash/manual/html_node/Redirections.html#Here-Strings) |
| PHP | `#[` attributes are code; heredocs and nowdocs with indented closers | `a23b225`, `785d277` | [Attributes](https://www.php.net/manual/en/language.attributes.syntax.php), [heredoc](https://www.php.net/manual/en/language.types.string.php#language.types.string.syntax.heredoc), [zend_language_scanner.l](https://github.com/php/php-src/blob/php-8.5.12/Zend/zend_language_scanner.l) |
| Perl | `<<ID`, `<<"ID"` and `<<~ID` heredocs; POD blocks, ending at `=cut` or the end of the file; data after `__END__` and `__DATA__`; `q`, `qq`, `qw`, `qx`, `qr`, `m`, `s`, `tr` and `y` operators; regular expressions after listed keywords, spanning lines; `\"…"` and `\'…'` references to strings; `` `…` `` commands; the punctuation variables `$'`, `$"`, `` $` `` and `$#`; any delimiter for a quote-like operator, which is a name where PPI forces one | `785d277`, `ddbcd99`, `56379a6`, `daf1c1d`, `3738416` | [perlop, here-docs](https://perldoc.perl.org/perlop#%3C%3CEOF), [Quote and Quote-like Operators](https://perldoc.perl.org/perlop#Quote-and-Quote-like-Operators), [perlpod](https://perldoc.perl.org/perlpod), [perldata, special literals](https://perldoc.perl.org/perldata#Special-Literals), [perlref, making references](https://perldoc.perl.org/perlref#Making-References) |
| Haskell | Strings and character literals | `6798eb4` | [Haskell 2010 §2.6](https://www.haskell.org/onlinereport/haskell2010/haskellch2.html#x7-200002.6) |
| Elm | `"""` and `"` strings; character literals | `6798eb4` | [elm/compiler 0.19.1 Parse/String.hs](https://github.com/elm/compiler/blob/0.19.1/compiler/src/Parse/String.hs) |
| PureScript | Strings, `"""` raw strings and character literals; non-nesting comments | `6798eb4` | [Syntax](https://github.com/purescript/documentation/blob/master/language/Syntax.md), [Lexer.hs v0.15.16](https://github.com/purescript/purescript/blob/v0.15.16/src/Language/PureScript/CST/Lexer.hs) |
| Agda | Strings and character literals | `6798eb4` | [Lexical structure](https://agda.readthedocs.io/en/latest/language/lexical-structure.html), [Lexer.x v2.8.0.2](https://github.com/agda/agda/blob/v2.8.0.2/src/full/Agda/Syntax/Parser/Lexer.x) |
| Racket, Scheme, Lisp | Strings; `#\` character literals | `6798eb4` | [Racket reader](https://docs.racket-lang.org/reference/reader.html), [R7RS small](https://small.r7rs.org/attachment/r7rs.pdf) §2.1–2.3, [CLHS 2.4.8.1](https://www.lispworks.com/documentation/HyperSpec/Body/02_dha.htm) |
| Rust | Doc comments as comments | `5994462` | [Comments](https://doc.rust-lang.org/reference/comments.html) |
| Rust | Frontmatter; `#"` as `#` and a string, as before Rust 2024, where it is reserved; Unicode Pattern_White_Space as whitespace | `44a536d` | rustc_lexer `Cursor::frontmatter` and `GuardedStrPrefix`, [RFC 3503](https://rust-lang.github.io/rfcs/3503-frontmatter.html), [RFC 3593](https://rust-lang.github.io/rfcs/3593-reserved-guarded-strings.html), [Whitespace](https://doc.rust-lang.org/reference/whitespace.html) |
| JavaScript | `<!--` line comments, and `-->` line comments after nothing but whitespace and comments on the line | `de1ab9e` | [ECMA-262 2025 Annex B.1.1](https://tc39.es/ecma262/2025/#sec-html-like-comments) |
| JavaScript, JSX, TypeScript, TSX | Hashbangs as comments | `a751182` | [ECMA-262 2025 §12.5](https://tc39.es/ecma262/2025/#sec-hashbang) |
| C, C header, C++, C++ header, Objective-C, Objective-C++ | Line splices: a backslash before a newline is whitespace, and a line comment ending in one continues | `7976ce3` | [N3220](https://www.open-std.org/jtc1/sc22/wg14/www/docs/n3220.pdf) §5.1.1.2, [lex.phases](https://eel.is/c++draft/lex.phases) |
| Objective-C, Objective-C++ | Character literals; raw strings in Objective-C++ | `f22947c` | [Programming with Objective-C](https://developer.apple.com/library/archive/documentation/Cocoa/Conceptual/ProgrammingWithObjectiveC/Introduction/Introduction.html): a superset of C |
| C# | `{{{x}}` in `$$"""…"""` strings holds a brace of text before the hole, the innermost braces forming it | `fc99a62` | [raw string literals](https://github.com/dotnet/csharplang/blob/main/proposals/csharp-11.0/raw-string-literal.md) |
| Ruby | Regular expressions spanning lines; `:/` symbols; `%{…}` and `/…/` arguments after a method name; `?c` character literals; a regular expression after `<<` | `0b633d1` | [literals.rdoc](https://github.com/ruby/ruby/blob/v3_4_7/doc/syntax/literals.rdoc), parse.y `IS_SPCARG`, Prism |
| PHP | Only the text between `<?php`, `<?=` or `<?` and `?>` is PHP; inline HTML outside the tags is code with no strings or comments | `1923467` | [PHP tags](https://www.php.net/manual/en/language.basic-syntax.phptags.php), `token_get_all` |
| Dart | Nested block comments | `961bee1` | [Dart language specification](https://spec.dart.dev/DartLangSpecDraft.pdf), Comments |
| Scala | Nested block comments; a triple-quoted string ends at the last three quotes of a run; any identifier before a quote is an interpolator | `b298558` | [Scala 3 reference, Lexical Syntax](https://docs.scala-lang.org/scala3/reference/syntax.html) |
| R | `"…"`, `'…'` and backticked names that may span lines; raw strings `r"(…)"` with `[]`, `{}` and dashes in either quote and case; `#line` at a line's start is code | `add99a7` | [R Language Definition, Literal constants](https://cran.r-project.org/doc/manuals/r-release/R-lang.html#Literal-constants), [Quotes](https://stat.ethz.ch/R-manual/R-devel/library/base/html/Quotes.html), `gram.y` `processLineDirective` |
| Elixir | `#{…}` holes in strings, charlists and heredocs; heredocs close only at `"""` or `'''` starting a line; sigils such as `~r/…/i`, `~S(…)` and `~HTML"""`; `?c` character literals | `9c6d1c7` | [Binaries, strings, and charlists](https://hexdocs.pm/elixir/binaries-strings-and-charlists.html), [Sigils](https://hexdocs.pm/elixir/sigils.html), `elixir_tokenizer` |
| PowerShell | `@"…"@` here-strings the right way round, closing only at a line's start; the backtick, not the backslash, as escape; `$(…)` subexpressions in expandable strings; `“`, `”`, `„` and `‘`, `’`, `‚`, `‛` as quotes | `bad5ddc` | [about_Quoting_Rules](https://learn.microsoft.com/powershell/module/microsoft.powershell.core/about/about_quoting_rules), [PowerShell Language Specification 3.0](https://learn.microsoft.com/powershell/scripting/lang-spec/chapter-02) §2.3.1 |
| Haskell | Pragmas GHC's lexer knows, such as `{-# INLINE f #-}` and `{-# RULES … #-}`, are code; string gaps; quasi-quotes such as `[r|…|]` where `QuasiQuotes` is on | `df73c4e` | [Haskell 2010 report](https://www.haskell.org/onlinereport/haskell2010/haskellch2.html) §2.6, [GHC User's Guide, pragmas](https://downloads.haskell.org/ghc/latest/docs/users_guide/exts/pragmas.html), [quasi-quotation](https://downloads.haskell.org/ghc/latest/docs/users_guide/exts/template_haskell.html#template-haskell-quasi-quotation), `GHC.Parser.Lexer` |
| Python | `{…}` replacement fields in f-strings and t-strings with any `r` prefix, holding code, strings and comments, with `{{` and `}}` as text | `91e7eaa` | [Lexical analysis, f-strings](https://docs.python.org/3.14/reference/lexical_analysis.html#f-strings), [PEP 701](https://peps.python.org/pep-0701/), [PEP 750](https://peps.python.org/pep-0750/) |
| Ruby | Data after `__END__`; `%w[…]`, `%r{…}`, `%(…)` and the other percent literals | `ddbcd99`, `56379a6` | [DATA](https://docs.ruby-lang.org/en/3.4/Object.html), [Literals, percent literals](https://github.com/ruby/ruby/blob/v3_4_7/doc/syntax/literals.rdoc) |
| Blade, Twig, Handlebars | Quotes only in tags; template expressions inside tags | `598ce84` | [Blade, displaying data](https://laravel.com/docs/12.x/blade#displaying-data), [Twig synopsis](https://twig.symfony.com/doc/3.x/templates.html#synopsis), [Handlebars expressions](https://handlebarsjs.com/guide/expressions.html), [partials](https://handlebarsjs.com/guide/partials.html) |
| HTML and other markup with embedded scripts | Script and style elements end where the HTML tokenizer ends them | `44f2a48` | [HTML Living Standard, script data end tag name state](https://html.spec.whatwg.org/multipage/parsing.html#script-data-end-tag-name-state), [RAWTEXT end tag name state](https://html.spec.whatwg.org/multipage/parsing.html#rawtext-end-tag-name-state) |
| Not Quite Perl | POD blocks as NQP's grammar reads them, instead of `=begin`/`=end` comments | `c19e61e` | [src/NQP/Grammar.nqp `pod_comment`](https://github.com/Raku/nqp/blob/e5e5a90/src/NQP/Grammar.nqp) |
| WenYan | Comments as `注曰`, `疏曰` or `批曰` followed by a `「「…」」` or `『…』` string; `。` and `、` as whitespace | `273e1d0` | [wenyan `97f0a4b` parser and `IGNORE_SYMBOLS`](https://github.com/wenyan-lang/wenyan/tree/97f0a4b/src) |
| XML, SVG, XAML | CDATA sections as text | `cfdcdc5` | [XML 1.0 §2.7](https://www.w3.org/TR/xml/#sec-cdata-sect) |
| CSS | No `//` line comments | `76b5c44` | [CSS Syntax Level 3 §4.3.2](https://www.w3.org/TR/css-syntax-3/#consume-comment) |
| HTML | `<?…>` as a bogus comment; a script of a type neither JavaScript's nor a language's is a data block, counted as the page's text; `text/babel` and `text/jsx` scripts as JavaScript | `ecf6858`, `f5768c9` | [HTML Living Standard, tag open state](https://html.spec.whatwg.org/multipage/parsing.html#tag-open-state), [the script element](https://html.spec.whatwg.org/multipage/scripting.html#the-script-element), [Babel standalone](https://babeljs.io/docs/babel-standalone) |
| Vue, Svelte, HTML | A `lang` attribute names a language by name or file extension, as `<style lang="scss">` does | `0772c2f` | [Vue SFC specification, pre-processors](https://vuejs.org/api/sfc-spec.html#pre-processors) |
| Svelte | `{…}` in text and attributes as TypeScript expressions, `{/…}` as a block's end tag; `//` and `/* */` comments between attributes | `3d035cd` | [Svelte, Basic markup](https://svelte.dev/docs/svelte/basic-markup), [Svelte 5.53.0 changelog, comments in tags](https://github.com/sveltejs/svelte/pull/17671) |
| Lua | A first line starting with `#` is a comment, as `luaL_loadfile` skips it | `64d070b` | [Lua 5.4 `lauxlib.c`, `skipcomment`](https://www.lua.org/source/5.4/lauxlib.c.html#skipcomment), unchanged in 5.5 |
| SQL | `'…'` strings without backslash escapes, `E'…'` strings with them, `"…"` identifiers, nested `/* */` comments and `$tag$…$tag$` dollar quotes, as PostgreSQL's scanner reads them; a dollar-quoted body that lexes as SQL, such as a PL/pgSQL function's, is code; a psql meta-command's line is code | `25583bf` | [PostgreSQL, lexical structure](https://www.postgresql.org/docs/current/sql-syntax-lexical.html), [psql meta-commands](https://www.postgresql.org/docs/current/app-psql.html#APP-PSQL-META-COMMANDS) |
| YAML | A line of whitespace past a block scalar's indentation is its content | `150abcf` | [YAML 1.2.2 §8.1.2](https://yaml.org/spec/1.2.2/#812-literal-style) |
| Markdown | Fenced code blocks as CommonMark finds them | `bdc6aff` | [CommonMark 0.31.2 §4.5](https://spec.commonmark.org/0.31.2/#fenced-code-blocks) |
| MSBuild, Visual Studio C++ project, NuGet configuration, .NET resource, AXAML, XSL | XML's reading of quotes, which open strings only inside tags | `3fc5f6b` | [XML 1.0](https://www.w3.org/TR/xml/) |
| Protocol Buffers | `/* */` comments and `"…"` and `'…'` strings | `1091d87` | [Protocol Buffers proto3 specification](https://protobuf.dev/reference/protobuf/proto3-spec/) |
| CMake | `#[[…]]` bracket comments and `[[…]]` bracket arguments, closing at as many `=` as they open with | `3db24ae` | [cmake-language(7)](https://cmake.org/cmake/help/latest/manual/cmake-language.7.html#bracket-comment) |
| Cabal | Only `--` comments, without Haskell's `{- -}` | `ec7f406` | Cabal-syntax 3.18.2.0 `Distribution/Fields/Lexer.x` |
| Autoconf, Bitbake, RPM Specfile, Snakemake, Bazel | Autoconf claims `configure.ac` and `configure.in` instead of every `.in` file, Bazel its file names in their own case, and `.inc`, `.spec` and `.rules` files only when their text has the language's syntax | `e42f092` | [GitHub Linguist heuristics](https://github.com/github-linguist/linguist/blob/73ee9a3b27/lib/linguist/heuristics.yml) |
| Meson | `'''…'''` strings without escapes; `meson.options` files | `61f32a1` | [Meson, Syntax](https://mesonbuild.com/Syntax.html#strings) |
| Nextflow, Snakemake | Groovy's and Python's syntax, whose lexers read them | `5312a2e` | [Groovy syntax](https://groovy-lang.org/syntax.html), [Python lexical analysis](https://docs.python.org/3/reference/lexical_analysis.html) |
| Automake | Makefile syntax, since automake hands the file's text on to make | `5df7a24` | [GNU Automake manual](https://www.gnu.org/software/automake/manual/automake.html) |
| RPM Specfile | Sections split at rpm's section headers: build scripts and scriptlets as shell, `#` lines as comments and quotes as text elsewhere | `f26b942` | [RPM spec file format](https://rpm-software-management.github.io/rpm/manual/spec.html), rpm 6 `build/parseSpec.cc` |
| Autoconf, M4 | Quoted strings that nest and hold no comments, with `[`…`]` in Autoconf and `` ` ``…`'` in M4; a `.m4` file using Autoconf or m4sugar macros is Autoconf | `ea47c6a` | [GNU m4 manual, Quoted strings](https://www.gnu.org/software/m4/manual/m4.html#Quoted-strings), [Autoconf manual, M4 Quotation](https://www.gnu.org/software/autoconf/manual/autoconf.html#M4-Quotation) |
| Visual Basic | `"…"` strings with `""` as their only escape; `REM` comments; XML literals as code; `\` as an operator | `b11aa64` | [Visual Basic lexical grammar](https://learn.microsoft.com/en-us/dotnet/visual-basic/reference/language-specification/lexical-grammar), Roslyn `Scanner.vb` |
| F# | ` `` `-quoted identifiers and `@$"…"` strings are verbatim; shebangs; inside block comments, strings and character literals are text and `(*)` opens nothing | `8ac5216` | [lex.fsl](https://github.com/dotnet/fsharp/blob/main/src/Compiler/lex.fsl), rule `comment` |
| Julia | Character literals except after an operand, where `'` is the adjoint; `` ` `` and ```` ``` ```` commands; `$(…)` holes in strings and commands; raw string macros such as `raw"…"` | `92d542b` | JuliaSyntax `tokenize.jl` |
| Erlang | `"…"` strings and `'…'` quoted atoms with escapes; `$` character literals; verbatim triple-quoted strings; sigils | `dcbd729` | [Erlang Reference Manual, Data Types](https://www.erlang.org/doc/system/data_types.html), [EEP 64](https://www.erlang.org/eeps/eep-0064), [EEP 66](https://www.erlang.org/eeps/eep-0066), `erl_scan` |
| OCaml | Inside comments, strings, `{id\|…\|id}` quoted strings and character literals are text, as the compiler's lexer reads them | `3a5b9a4` | [lexer.mll](https://github.com/ocaml/ocaml/blob/5.5/parsing/lexer.mll), rule `comment` |
| FORTRAN Legacy, FORTRAN Modern | `"…"` and `'…'` strings with doubled quotes; `!` comments wherever they start; fixed source form, with first-column comments and column-6 continuations, unless free by content | `fa4eb14` | [Fortran 2018 (J3/18-007r1)](https://j3-fortran.org/doc/year/18/18-007r1.pdf) §6.3.2, §6.3.3, §7.4.4.3; fparser 0.2.5 `sourceinfo.py` |
| Pascal | `'…'` strings with `''` as their only escape; `{…}` and `(*…*)` comments nest within their own kind unless that leaves one open | `5a72cb6` | [fcl-passrc `pscanner.pp`](https://gitlab.com/freepascal.org/fpc/source/-/blob/fb4143a1ef/packages/fcl-passrc/src/pscanner.pp) |
| D | `/+ … +/` comments nest only within their own kind | `5a72cb6` | [D Language Specification, Lexical, Comments](https://dlang.org/spec/lex.html#comment) |
| Solidity | `'…'` strings beside `"…"` | `55e3ec7` | [Solidity documentation, Types, String Literals and Types](https://docs.soliditylang.org/en/latest/types.html#string-literals-and-types) |
| Nim | `#[ … ]#` and `##[ … ]##` comments, each nesting on its own opener; `r"…"` and generalized raw strings without escapes and with `""`; triple-quoted strings closing at the end of a run of quotes; character literals | `131696b` | [Nim Manual, Lexical Analysis](https://nim-lang.org/docs/manual.html#lexical-analysis), compiler `lexer.nim` |
| Crystal | `<<-` heredocs closing on an indented line; percent literals; regular expression literals; `#{…}` holes in strings and commands; `'` as a character literal only; `` def ` `` as code | `cc13f4f` | [Crystal reference, Literals](https://crystal-lang.org/reference/syntax_and_semantics/literals/), compiler `lexer.cr` |
| Clojure, ClojureScript, ClojureC | `#!` comments | `56b9d63` | [`LispReader.java`](https://github.com/clojure/clojure/blob/33fb064dee4cf8decdae71ed93f1a6262f7a4c28/src/jvm/clojure/lang/LispReader.java), `dispatchMacros['!']` |
| COBOL | Lines read in GnuCOBOL's reference format: in fixed form, the sequence and identification areas ignored, `*` and `/` comment lines, `-` continuations and `D` debugging lines, comments until `WITH DEBUGGING MODE`; `*>` floating comments; compiler directives as code; comment paragraphs such as `AUTHOR.` as comments; free form chosen as GnuCOBOL's default `auto` format chooses it, or by `>>SOURCE FORMAT` and `$SET SOURCEFORMAT`; `"…"` and `'…'` literals with doubled quotes | `2834e6c` | GnuCOBOL [`cobc/pplex.l`](https://github.com/OCamlPro/gnucobol/blob/eda8905e440494b7b0bb9cb47ea1bbcc3f4a5971/cobc/pplex.l) |
| GNU Style Assembly | `#`, `@` and `!` comment lines, as x86, Arm and SPARC assemblers read them, except preprocessor directives and lines a splice continues; C's line splices | `203d357` | [LLVM `AsmLexer`](https://github.com/llvm/llvm-project/blob/llvmorg-19.1.7/llvm/lib/MC/MCParser/AsmLexer.cpp), clang `Preprocessor::HandleDirective` |
| Assembly | `"…"` and `'…'` strings without escapes and backquote strings with them, each ending with its line when it does not close; MASM's `COMMENT` blocks | `347319e` | [NASM manual, 3.4.2 Character Strings](https://www.nasm.us/doc/nasmdoc3.html#section-3.4.2), MASM 6.1 Programmer's Guide, chapter 1 |
| Bitbake | Lines read as BitBake reads them: `#` comments only at a statement's first character, the bodies of shell and Python functions as Shell and Python | `f50b63c` | [BitBake User Manual, Syntax and Operators](https://docs.yoctoproject.org/bitbake/bitbake-user-manual/bitbake-user-manual-metadata.html), BitBake 2.16 `lib/bb/parse/parse_py/BBHandler.py` |
| Just | Lines read as just reads them: strings that span lines outside recipes, recipe lines as the shell reads each, and script recipes as the language of their `[script]` command, `script-interpreter` setting or `#!` line; `.justfile` counted | `8dfaf47`, `a069406` | [Just Programming Language manual](https://just.systems/man/en/), just 1.59.0 `src/lexer.rs` and `src/recipe.rs` |
| HCL | `<<` and `<<-` heredocs, whose closing line holds only the marker, and `${…}` interpolations and `%{…}` directives in strings, except the `$${` and `%%{` escapes | `1281560` | [HCL Native Syntax Specification](https://github.com/hashicorp/hcl/blob/v2.25.0/hclsyntax/spec.md), Template Expressions; `hclsyntax/scan_tokens.rl` |
| Nix | `''…''` strings with their `'''`, `''$` and `''\` escapes, `${…}` holes in both kinds of string, where `$$` is text, and identifiers such as `x''` that hold quotes | `559d1a4` | [Nix Reference Manual, String literals](https://nix.dev/manual/nix/2.35/language/string-literals); `src/libexpr/lexer.l`, Nix 2.35.4 |
| Jsonnet | `\|\|\|` text blocks and `@"…"` and `@'…'` verbatim strings, whose doubled quote stands for a quote and whose backslash is text | `215dc41` | [Jsonnet language reference, Lexing](https://jsonnet.org/ref/spec.html#lexing); go-jsonnet v0.22.0 `internal/parser/lexer.go` |
| Bicep | `'''` strings read without escapes and closed by the last three quotes of a run, as in `''''abc''''` | `ecf3ba1` | [Bicep data types, Strings](https://learn.microsoft.com/azure/azure-resource-manager/bicep/data-types#strings); `ScanMultilineString` in `src/Bicep.Core/Parsing/Lexer.cs`, Bicep v0.48.1 |
| LD Script | `//` comments, which the C preprocessor that builds run linker scripts through reads, so `//*lib.a:…` opens no block comment | `a7acd1c` | C17 6.4.9 Comments; U-Boot `scripts/Makefile.lib` `cmd_cpp_lds`, Zephyr and NuttX linker script preprocessing |
| Mustache | Quotes are text, since Mustache has no strings, so an apostrophe such as `it's` hides no comment or blank line | `afe278c` | [mustache(5)](https://mustache.github.io/mustache.5.html); [Mustache specification v1.4.3](https://github.com/mustache/spec/tree/v1.4.3/specs) |
| FreeMarker | `[#-- … --]` comments of the square-bracket tag syntax | `9d4eb34` | [FreeMarker Manual, Alternative (square bracket) syntax](https://freemarker.apache.org/docs/dgui_misc_alternativesyntax.html) |
| Apache Velocity | Quotes are text, since Velocity reads strings only in directive and reference arguments, so an apostrophe such as `don't` hides no comment or blank line | `4932872` | `STRING_LITERAL` in [Parser.jjt](https://github.com/apache/velocity-engine/blob/2.4.1/velocity-engine-core/src/main/parser/Parser.jjt) |
| PlantUML | Lines read as PlantUML's preprocessor reads them: a line whose first text is `'`, or that holds nothing but `/' … '/`, is a comment; a line whose first text is `/'` and that holds no `'/` opens a comment of whole lines, which the first line ending in `'/` closes; quotes span no lines | `9b24e4d` | `TLineType` and the comment iterators of `TContext` in [PlantUML v1.2026.9](https://github.com/plantuml/plantuml/tree/v1.2026.9/src/main/java/net/sourceforge/plantuml) |
| Zig | Character literals; strings that end with their line, since Zig allows no newline in any literal, so a `\\` multiline string line is code whatever quotes it holds | `6069a9f` | [Zig Language Reference 0.16.0, String Literals and Unicode Code Point Literals](https://ziglang.org/documentation/0.16.0/#String-Literals-and-Unicode-Code-Point-Literals); `lib/std/zig/tokenizer.zig` |

`locmetry-spike/` holds the development harness that compares these counts with tree-sitter on public repositories; it is not part of the crate.

Tokei is a program that displays statistics about your code. Tokei will show the number of files, total lines within those files and code, comments, and blanks grouped by language.

## Example
```console
━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━
 Language            Files        Lines         Code     Comments       Blanks
━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━
 BASH                    4           49           30           10            9
 JSON                    1         1332         1332            0            0
 Shell                   1           49           38            1           10
 TOML                    2           77           64            4            9
───────────────────────────────────────────────────────────────────────────────
 Markdown                5         1355            0         1074          281
 |- JSON                 1           41           41            0            0
 |- Rust                 2           53           42            6            5
 |- Shell                1           22           18            0            4
 (Total)                           1471          101         1080          290
───────────────────────────────────────────────────────────────────────────────
 Rust                   19         3416         2840          116          460
 |- Markdown            12          351            5          295           51
 (Total)                           3767         2845          411          511
━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━
 Total                  32         6745         4410         1506          829
━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━
```

## [API Documentation](https://docs.rs/tokei)

## Table of Contents

- [Features](#features)
- [Installation](#installation)
    - [Package Managers](#package-managers)
    - [Manual](#manual)
- [Configuration](#configuration)
- [How to use Tokei](#how-to-use-tokei)
- [Options](#options)
- [Supported Languages](#supported-languages)
- [Changelog](CHANGELOG.md)
- [Common Issues](#common-issues)
- [Canonical Source](#canonical-source)
- [Copyright and License](#copyright-and-license)

## Features

- Tokei is **very fast**, and is able to count millions of lines of code in seconds.
  Check out the [11.0.0 release](https://github.com/XAMPPRocky/tokei/releases/v11.0.0)
  to see how Tokei's speed compares to others.

- Tokei is **accurate**, Tokei correctly handles multi line comments,
  nested comments, and not counting comments that are in strings. Providing an
  accurate code statistics.

- Tokei has huge range of languages, supporting over **150** languages, and
  their various extensions.

- Tokei can output in multiple formats (**CBOR**, **JSON**, **YAML**)
  allowing Tokei's output to be easily stored, and reused. These can also be
  reused in tokei combining a previous run's statistics with another set.

- Tokei is available on **Mac**, **Linux**, and **Windows**. See [installation
  instructions](#installation) for how to get Tokei on your platform.

- Tokei is also a **library** allowing you to easily integrate it with other
  projects.

- Tokei comes with and without color. Set the env variable NO_COLOR to 1, and
  it'll be black and white.

## Installation

### Package Managers

#### Unix
```console
# Alpine Linux (since 3.13)
apk add tokei
# Arch Linux
pacman -S tokei
# Cargo
cargo install tokei
# Conda
conda install -c conda-forge tokei
# Fedora
sudo dnf install tokei
# FreeBSD
pkg install tokei
# NetBSD
pkgin install tokei
# Nix/NixOS
nix-env -i tokei
# OpenSUSE
sudo zypper install tokei
# Void Linux
sudo xbps-install tokei
```

#### macOS
```console
# Homebrew
brew install tokei
# MacPorts
sudo port selfupdate
sudo port install tokei
```

#### Windows
```console
# Winget
winget install XAMPPRocky.tokei
# Scoop
scoop install tokei
```

### Manual

#### Downloading
You can download prebuilt binaries in the
[releases section](https://github.com/XAMPPRocky/tokei/releases).

#### Building
You can also build and install from source (requires the latest stable [Rust] compiler.)
```console
cargo install --git https://github.com/XAMPPRocky/tokei.git tokei
```

[rust]: https://www.rust-lang.org


## Configuration

Tokei has a [configuration] file that allows you to change default behaviour.
The file can be named `tokei.toml` or `.tokeirc`. Currently tokei looks for
this file in three different places. The current directory, your home directory,
and your configuration directory.

## How to use Tokei

#### Basic usage

This is the basic way to use tokei. Which will report on the code in `./foo`
and all subfolders.

```shell
$ tokei ./foo
```

[configuration]: ./tokei.example.toml

#### Multiple folders
To have tokei report on multiple folders in the same call, simply add all the
folders you'd like tokei to examine.

```shell
$ tokei ./foo ./bar ./baz
```

#### Excluding folders
Tokei will respect all `.gitignore` and `.ignore` files, and you can use
the `--exclude` option to exclude any additional files. The `--exclude` flag has
the same semantics as `.gitignore`.

```shell
$ tokei ./foo --exclude *.rs
```

Paths to exclude can also be listed in a `.tokeignore` file, using the same
[syntax](https://git-scm.com/docs/gitignore) as .gitignore files.

#### Sorting output
By default tokei sorts alphabetically by language name, however using `--sort`
tokei can also sort by any of the columns.

`blanks, code, comments, lines`

```shell
$ tokei ./foo --sort code
```

#### Outputting file statistics
By default tokei only outputs the total of the languages, and using `--files`
flag tokei can also output individual file statistics.

```shell
$ tokei ./foo --files
```

#### Outputting into different formats
Tokei normally outputs into a nice human readable format designed for terminals.
There is also using the `--output` option various other formats that are more
useful for bringing the data into another program.

**Note:** This version of tokei was compiled without any serialization formats, to enable serialization, reinstall
tokei with the features flag.

```shell
  ALL:
  cargo install tokei --features all

  CBOR:
  cargo install tokei --features cbor

  YAML:
  cargo install tokei --features yaml
```

**Currently supported formats**
- JSON `--output json`
- YAML `--output yaml`
- CBOR `--output cbor`

```shell
$ tokei ./foo --output json
```

#### Reading in stored formats
Tokei can also take in the outputted formats added in the previous results to its
current run. Tokei can take either a path to a file, the format passed in as a
value to the option, or from stdin.

```shell
$ tokei ./foo --input ./stats.json
```

## Options

```
USAGE:
    tokei [FLAGS] [OPTIONS] [--] [input]...

FLAGS:
    -f, --files               Will print out statistics on individual files.
    -h, --help                Prints help information
        --hidden              Count hidden files.
    -l, --languages           Prints out supported languages and their extensions.
        --no-ignore           Don't respect ignore files (.gitignore, .ignore, etc.). This implies --no-ignore-parent,
                              --no-ignore-dot, and --no-ignore-vcs.
        --no-ignore-dot       Don't respect .ignore and .tokeignore files, including those in parent directories.
        --no-ignore-parent    Don't respect ignore files (.gitignore, .ignore, etc.) in parent directories.
        --no-ignore-vcs       Don't respect VCS ignore files (.gitignore, .hgignore, etc.), including those in parent
                              directories.
    -V, --version             Prints version information
    -v, --verbose             Set log output level:
                                          1: to show unknown file extensions,
                                          2: reserved for future debugging,
                                          3: enable file level trace. Not recommended on multiple files

OPTIONS:
    -c, --columns <columns>       Sets a strict column width of the output, only available for terminal output.
    -e, --exclude <exclude>...    Ignore all files & directories matching the pattern.
    -i, --input <file_input>      Gives statistics from a previous tokei run. Can be given a file path, or "stdin" to
                                  read from stdin.
    -o, --output <output>         Outputs Tokei in a specific format. Compile with additional features for more format
                                  support. [possible values: cbor, json, yaml]
    -s, --sort <sort>             Sort languages based on column [possible values: files, lines, blanks, code, comments]
    -t, --type <types>            Filters output by language type, separated by a comma. i.e. -t=Rust,Markdown

ARGS:
    <input>...    The path(s) to the file or directory to be counted.
```

## Dockerized version
Tokei is available in a small `alpine`-based docker image, buildable through [earthly](https://github.com/earthly/earthly):
```bash
earthly +docker
```

Once built, one can run the image with:
```bash
docker run --rm -v /path/to/analyze:/src tokei .
```

Or, to simply analyze the current folder (linux):
```bash
docker run --rm -v $(pwd):/src tokei .
```

## Supported Languages

If there is a language that you would to add to tokei feel free to make a pull
request. Languages are defined in [`languages.json`](./languages.json), and you can
read how to add and test your language in our [CONTRIBUTING.md](./CONTRIBUTING.md).

```
Abap
ActionScript
Ada
Agda
Alex
Alloy
APL
Asn1
Asp
AspNet
Assembly
AssemblyGAS
ATS
Autoconf
AutoHotKey
Automake
AWK
Bash
Batch
Bazel
Bean
Bicep
Bitbake
BQN
BrightScript
C
C3
Cabal
Cassius
Ceylon
CHeader
Cil
Clojure
ClojureC
ClojureScript
CMake
Cobol
CoffeeScript
Cogent
ColdFusion
ColdFusionScript
Coq
Cpp
CppHeader
Crystal
CSharp
CShell
Css
Cuda
CUE
Cython
D
D2
DAML
Dart
DeviceTree
Dhall
Dockerfile
DotNetResource
DreamMaker
Dust
Ebuild
EdgeDB
Edn
Elisp
Elixir
Elm
Elvish
EmacsDevEnv
Emojicode
Erlang
Factor
FEN
Fish
FlatBuffers
ForgeConfig
Forth
FortranLegacy
FortranModern
FreeMarker
FSharp
Fstar
GDB
GdScript
GdShader
Gherkin
Gleam
Glsl
Go
Graphql
Groovy
Gwion
Hamlet
Handlebars
Happy
Hare
Haskell
Haxe
Hcl
Hex
Hex0
Hex1
Hex2
HiCAD
hledger
Hlsl
HolyC
Html
Hy
Idris
Ini
IntelHex
Isabelle
Jai
Janet
Java
JavaScript
Jq
Json
Jsx
Julia
Julius
Just
KakouneScript
KaemFile
Koka
Kotlin
Lean
Less
Lingua Franca
LinkerScript
Liquid
Lisp
LLVM
Logtalk
Lua
Lucius
M1Assembly
Madlang
Max
Makefile
Markdown
Mdx
Meson
Mint
Mlatu
ModuleDef
MonkeyC
MoonScript
MsBuild
Mustache
Nim
Nix
NotQuitePerl
NuGetConfig
Nushell
ObjectiveC
ObjectiveCpp
OCaml
Odin
OpenSCAD
OpenQASM
Org
Oz
Pascal
Perl
Perl6
Pest
Phix
Php
Po
Poke
Polly
Pony
PostCss
PowerShell
Processing
Prolog
Protobuf
PRQL
PSL
PureScript
Pyret
Python
Qcl
Qml
R
Racket
Rakefile
Razor
Renpy
ReStructuredText
RON
RPMSpecfile
Ruby
RubyHtml
Rust
Sass
Scala
Scheme
Scons
Sh
ShaderLab
Slang
Sml
Solidity
SpecmanE
Spice
Sql
SRecode
Stata
Stratego
Svelte
Svg
Swift
Swig
SystemVerilog
Slint
Tact
Tcl
Templ
Tex
Text
Thrift
Toml
Tsx
Twig
TypeScript
UMPL
UnrealDeveloperMarkdown
UnrealPlugin
UnrealProject
UnrealScript
UnrealShader
UnrealShaderHeader
UrWeb
UrWebProject
Vala
VB6
VBScript
Velocity
Verilog
VerilogArgsFile
Vhdl
VimScript
VisualBasic
VisualStudioProject
VisualStudioSolution
Vue
WebAssembly
Wolfram
Xaml
XcodeConfig
Xml
XSL
Xtend
Yaml
ZenCode
Zig
ZoKrates
Zsh
```

## Common issues

### Tokei says I have a lot of D code, but I know there is no D code!
This is likely due to `gcc` generating `.d` files. Until the D people decide on
a different file extension, you can always exclude `.d` files using the
`-e --exclude` flag like so

```
$ tokei . -e *.d
```

## Canonical Source
The canonical source of this repo is hosted on
[GitHub](https://github.com/XAMPPRocky/tokei). If you have a GitHub account,
please make your issues, and pull requests there.

## Related Tools

- [tokei-pie](https://github.com/laixintao/tokei-pie): Render tokei's output to
  interactive sunburst chart.

## Copyright and License
(C) Copyright 2015 by XAMPPRocky and contributors

See [the graph](https://github.com/XAMPPRocky/tokei/graphs/contributors) for a full list of contributors.

Tokei is distributed under the terms of both the MIT license and the Apache License (Version 2.0).

See [LICENCE-APACHE](./LICENCE-APACHE), [LICENCE-MIT](./LICENCE-MIT) for more information.
