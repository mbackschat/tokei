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
| `comment_exceptions`: code tokens that start like a comment, also exempt from the line-start comment rule. | `a23b225` | Not offered yet |
| A line that starts inside a literal is code, even when its text starts with a comment marker. | `801aae6` | Not offered yet |
| `doubled_quote_escapes`: closing delimiters of verbatim quotes that stand for themselves when written twice. | `b773e13` | Not offered yet |
| Opener matching runs only at bytes that can start a token, keeping the analysis at upstream speed. | `f8a9a6d` | Not offered yet |

Declarations in `languages.json`, each grounded in the cited specification or reference lexer:

| Languages | Declared | Commit | Sources |
| --- | --- | --- | --- |
| C, C header | Character literals | `9e65ec4`, `6798eb4` | [N3220](https://www.open-std.org/jtc1/sc22/wg14/www/docs/n3220.pdf) §6.4.4.5, §6.4.5, §6.4.9 |
| C++, C++ header, C++ module | Character literals | `6798eb4` | [lex.ccon](https://eel.is/c++draft/lex.ccon), [lex.string](https://eel.is/c++draft/lex.string), [lex.comment](https://eel.is/c++draft/lex.comment) |
| Rust | Character literals; `r`, `br` and `cr` raw strings with up to three `#` | `9e65ec4`, `2c95b8f` | [Tokens](https://doc.rust-lang.org/reference/tokens.html#character-literals), [raw strings](https://doc.rust-lang.org/reference/tokens.html#raw-string-literals) |
| Go | Character (rune) literals; backtick raw strings as verbatim quotes | `2c95b8f` | [Go spec](https://go.dev/ref/spec#Rune_literals), [string literals](https://go.dev/ref/spec#String_literals) |
| Java | Character literals; `"""` text blocks | `2c95b8f` | [JLS SE 21 §3.10.4–3.10.7](https://docs.oracle.com/javase/specs/jls/se21/html/jls-3.html) |
| Kotlin | Character literals; `"""` raw strings as verbatim quotes instead of escaping quotes | `6798eb4` | [Kotlin spec, tokens](https://kotlinlang.org/spec/syntax-and-grammar.html#tokens) |
| C# | Character literals; `""` in `@"…"` strings | `6798eb4`, `b773e13` | [C# standard §6.4.5.5–6.4.5.6](https://github.com/dotnet/csharpstandard/blob/draft-v8/standard/lexical-structure.md) |
| Scala | Character literals; `"""` strings as verbatim quotes | `6798eb4` | [Scala 2.13 lexical syntax](https://scala-lang.org/files/archive/spec/2.13/01-lexical-syntax.html), [Scala 3 syntax](https://docs.scala-lang.org/scala3/reference/syntax.html) |
| OCaml | Character literals; nested comments | `6798eb4` | [OCaml 5.2 lexical conventions](https://ocaml.org/manual/5.2/lex.html), [lexer.mll](https://github.com/ocaml/ocaml/blob/5.2/parsing/lexer.mll) |
| F# | Character literals; nested comments; `"""` and `@"…"` verbatim strings with `""` escapes; `(*)` is code | `a23b225`, `6798eb4`, `b773e13` | [F# spec, lexical analysis](https://github.com/fsharp/fslang-spec/blob/main/spec/lexical-analysis.md), [lex.fsl](https://github.com/dotnet/fsharp/blob/main/src/Compiler/lex.fsl) |
| PHP | `#[` attributes are code | `a23b225` | [Attributes](https://www.php.net/manual/en/language.attributes.syntax.php), [zend_language_scanner.l](https://github.com/php/php-src/blob/php-8.5.12/Zend/zend_language_scanner.l) |
| Haskell | Strings and character literals | `6798eb4` | [Haskell 2010 §2.6](https://www.haskell.org/onlinereport/haskell2010/haskellch2.html#x7-200002.6) |
| Elm | `"""` and `"` strings; character literals | `6798eb4` | [elm/compiler 0.19.1 Parse/String.hs](https://github.com/elm/compiler/blob/0.19.1/compiler/src/Parse/String.hs) |
| PureScript | Strings, `"""` raw strings and character literals; non-nesting comments | `6798eb4` | [Syntax](https://github.com/purescript/documentation/blob/master/language/Syntax.md), [Lexer.hs v0.15.16](https://github.com/purescript/purescript/blob/v0.15.16/src/Language/PureScript/CST/Lexer.hs) |
| Agda | Strings and character literals | `6798eb4` | [Lexical structure](https://agda.readthedocs.io/en/latest/language/lexical-structure.html), [Lexer.x v2.8.0.2](https://github.com/agda/agda/blob/v2.8.0.2/src/full/Agda/Syntax/Parser/Lexer.x) |
| Racket, Scheme, Lisp | Strings; `#\` character literals | `6798eb4` | [Racket reader](https://docs.racket-lang.org/reference/reader.html), [R7RS small](https://small.r7rs.org/attachment/r7rs.pdf) §2.1–2.3, [CLHS 2.4.8.1](https://www.lispworks.com/documentation/HyperSpec/Body/02_dha.htm) |

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
