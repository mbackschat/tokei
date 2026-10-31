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


> **This is the [LOCmetry](https://github.com/mbackschat/locmetry) fork of Tokei 15.0.0**, on branch `locmetry`. It corrects comments and blank lines that Tokei miscounts when a literal holds a comment marker or a quote, as recorded in LOCmetry's [Tokei counting defects](https://github.com/mbackschat/locmetry/blob/main/docs/tokei-defects.md) and [fork plan](https://github.com/mbackschat/locmetry/blob/main/docs/plans/archive/tokei-fork.md). Its version carries the build metadata `+locmetry.N`, raised with every change to counting. The rest of this README describes upstream Tokei.

## Why This Fork

Tokei declares most languages by their comment markers and quote pairs and classifies each line by them. Measured against each language's own lexer or parser on public repositories, that declaration misreads every literal it does not know: a character literal such as `'"'`, a raw string such as Rust `r#"…"#`, a C# or Kotlin interpolation hole, a heredoc, a regular expression such as `/"/`, a YAML block scalar or an apostrophe in HTML text opens a string that never closes, and a comment marker inside a literal opens a comment. Each such misreading turns the comments and blanks after it into code, or code into comments, often for the rest of the file. Tokei also gives every file of an extension that several languages share to one of them, so MATLAB `.m` files count as Objective-C and Pascal `.pp` files as Puppet.

LOCmetry reports code, comment and blank lines as evidence, so these counts have to match what the language's compiler reads. Upstream Tokei has its issue tracker disabled; the corrections are offered upstream, one pull request per defect on request, in [discussion #1394](https://github.com/XAMPPRocky/tokei/discussions/1394).

## What Changed

- **A lexing engine that reads literals as languages define them.** New `languages.json` fields declare character literals, raw strings, interpolation holes, heredocs, regular expression literals, nested comments, quote-like operators and the like, so most languages stay declarations rather than code. Each field is listed in the [engine table](CHANGES-FROM-UPSTREAM.md).
- **Language-specific readers for languages that are not token streams.** Markdown's fenced blocks are found by CommonMark's block structure, and RPM spec files, COBOL, BitBake, Just, PlantUML, Liquid and Haml are read whole, as their own tools read them, with embedded scripts classified in their own language.
- **One rule for mixed lines.** A line counts as a comment only when it holds nothing but comments, so `/* a */ x;` is code; one Tokei rule is kept on purpose: a line that starts inside a block comment is a comment even when code follows its end.
- **File attribution by content.** An extension that several languages share is decided by patterns over the file's text, starting from GitHub Linguist's heuristics, and build languages claim only their own file names, such as Autoconf's `configure.ac` instead of every `.in` template.
- **Library additions.** `LanguageType::line_classes` returns each line's class, `LineClass`, for embedders that classify parts of a file; `LanguageType::from_path_with_bytes` and `parse_bytes` let a caller count a content-checked file from the bytes detection already read.
- **Speed.** Every release must parse within 10% of upstream's time on each of 69 public repositories, by `judges/speed/check.py`; for release `15.0.0+locmetry.43`, 67 of them took half of upstream's time or less (measured 2026-10-31 on macOS), as the per-byte loop skips bytes that cannot start a token.

Unchanged: the command line, configuration and output formats, and upstream's language list. Counts change only where upstream misread a file.

## How It Is Verified

Every correction is a commit with a regression file in `tests/data`, and every declaration cites the specification or reference lexer it follows, in [CHANGES-FROM-UPSTREAM.md](CHANGES-FROM-UPSTREAM.md). The [judges](judges/README.md) read the files of public repositories with each language's own lexer or parser, such as `rustc_lexer`, `javac`, Roslyn, the TypeScript parser or CPython's `tokenize`, and compare every line's class with the fork's; LOCmetry's [step 6 record](https://github.com/mbackschat/locmetry/blob/main/docs/research/tokei-fork-step-6.md) lists the repositories, commits and results, and its [Tokei defects](https://github.com/mbackschat/locmetry/blob/main/docs/tokei-defects.md) names each defect with its effect. Languages without a judge yet are listed in LOCmetry's [deferred work](https://github.com/mbackschat/locmetry/blob/main/docs/DEFERRED.md).

The fork is not published on crates.io; depend on it by commit, as LOCmetry does:

```toml
tokei = { git = "https://github.com/mbackschat/tokei", rev = "<commit>", default-features = false }
```

## Releases

Each change to counting is released as `15.0.0+locmetry.N`, with `N` raised by one:

1. Pass the test suite and the speed gate, `python3 judges/speed/check.py` over the benchmark repositories.
2. Set the version in `Cargo.toml`, `Cargo.lock`, `judges/speed/fork/Cargo.lock`, `judges/judgecmp/Cargo.lock` and `locmetry-spike/harness/Cargo.lock`, and commit it as `build: release 15.0.0+locmetry.N`.
3. Tag that commit with the annotated tag `v15.0.0+locmetry.N` and push the branch and the tag. Releases are tagged from `15.0.0+locmetry.43` on, as LOCmetry's owner decided on 2026-11-01.

There are no GitHub release pages or binaries; [CHANGES-FROM-UPSTREAM.md](CHANGES-FROM-UPSTREAM.md) lists the changes.

## Changes From Upstream

[CHANGES-FROM-UPSTREAM.md](CHANGES-FROM-UPSTREAM.md) lists every engine change with its commit and upstream status, and every `languages.json` declaration with the specification or reference lexer it follows.

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
