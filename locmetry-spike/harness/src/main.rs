//! Spike: count files with Tokei 15.0.0, tree-sitter and family lexers.
//!
//! `spike count ROOT` prints one JSON line per file; `spike lines FILE` prints
//! each line's class per counter.

mod clike;
mod hash;
mod lines;
mod ts;

use lines::{Counts, Line, Region};
use rayon::prelude::*;
use std::path::Path;
use std::time::Instant;
use tokei::LanguageType;

#[derive(Clone, Copy, Debug)]
enum Lang {
    Rust,
    C,
    Cpp,
    Go,
    Java,
    Kotlin,
    CSharp,
    Scala,
    OCaml,
    FSharp,
    Haskell,
    Php,
    Swift,
    Lua,
    Dart,
    Ruby,
    JavaScript,
    TypeScript,
    Tsx,
    Python,
    Shell,
}

impl Lang {
    fn from_path(path: &Path) -> Option<Lang> {
        let name = path.file_name()?.to_str()?;
        let ext = name.rsplit_once('.')?.1;
        Some(match ext {
            "rs" => Lang::Rust,
            "c" | "h" => Lang::C,
            "cc" | "cpp" | "cxx" | "hh" | "hpp" | "hxx" => Lang::Cpp,
            "go" => Lang::Go,
            "java" => Lang::Java,
            "kt" | "kts" => Lang::Kotlin,
            "cs" => Lang::CSharp,
            "scala" => Lang::Scala,
            "ml" => Lang::OCaml,
            "fs" => Lang::FSharp,
            "hs" => Lang::Haskell,
            "php" => Lang::Php,
            "swift" => Lang::Swift,
            "lua" => Lang::Lua,
            "dart" => Lang::Dart,
            "rb" => Lang::Ruby,
            "js" | "mjs" | "cjs" | "jsx" => Lang::JavaScript,
            "ts" | "mts" | "cts" => Lang::TypeScript,
            "tsx" => Lang::Tsx,
            "py" => Lang::Python,
            "sh" | "bash" => Lang::Shell,
            _ => return None,
        })
    }

    fn grammar(self) -> tree_sitter::Language {
        match self {
            Lang::Rust => tree_sitter_rust::LANGUAGE.into(),
            Lang::C => tree_sitter_c::LANGUAGE.into(),
            Lang::Cpp => tree_sitter_cpp::LANGUAGE.into(),
            Lang::Go => tree_sitter_go::LANGUAGE.into(),
            Lang::Java => tree_sitter_java::LANGUAGE.into(),
            Lang::Kotlin => tree_sitter_kotlin_ng::LANGUAGE.into(),
            Lang::CSharp => tree_sitter_c_sharp::LANGUAGE.into(),
            Lang::Scala => tree_sitter_scala::LANGUAGE.into(),
            Lang::OCaml => tree_sitter_ocaml::LANGUAGE_OCAML.into(),
            Lang::FSharp => tree_sitter_fsharp::LANGUAGE_FSHARP.into(),
            Lang::Haskell => tree_sitter_haskell::LANGUAGE.into(),
            Lang::Php => tree_sitter_php::LANGUAGE_PHP.into(),
            Lang::Swift => tree_sitter_swift::LANGUAGE.into(),
            Lang::Lua => tree_sitter_lua::LANGUAGE.into(),
            Lang::Dart => tree_sitter_dart::LANGUAGE.into(),
            Lang::Ruby => tree_sitter_ruby::LANGUAGE.into(),
            Lang::JavaScript => tree_sitter_javascript::LANGUAGE.into(),
            Lang::TypeScript => tree_sitter_typescript::LANGUAGE_TYPESCRIPT.into(),
            Lang::Tsx => tree_sitter_typescript::LANGUAGE_TSX.into(),
            Lang::Python => tree_sitter_python::LANGUAGE.into(),
            Lang::Shell => tree_sitter_bash::LANGUAGE.into(),
        }
    }

    /// The spike's hand-written lexers, which exist only for its first eight languages.
    fn lexer(self, text: &str) -> Option<Vec<Region>> {
        Some(match self {
            Lang::Rust => clike::regions(text, clike::Lang::Rust),
            Lang::C => clike::regions(text, clike::Lang::C),
            Lang::Go => clike::regions(text, clike::Lang::Go),
            Lang::Java => clike::regions(text, clike::Lang::Java),
            Lang::JavaScript | Lang::TypeScript => clike::regions(text, clike::Lang::Js),
            Lang::Python => hash::python(text),
            Lang::Shell => hash::shell(text),
            _ => return None,
        })
    }
}

fn fork_lines(path: &Path, text: &str) -> Counts {
    let config = tokei_fork::Config::default();
    let kind = tokei_fork::LanguageType::from_path(path, &config).expect("fork language");
    let stats = kind.parse_from_str(text, &config).summarise();
    Counts { code: stats.code as u64, comments: stats.comments as u64, blanks: stats.blanks as u64 }
}

fn tokei_lines(path: &Path, text: &str) -> Counts {
    let config = tokei::Config::default();
    let kind = LanguageType::from_path(path, &config).expect("Tokei language");
    let stats = kind.parse_from_str(text, &config).summarise();
    Counts { code: stats.code as u64, comments: stats.comments as u64, blanks: stats.blanks as u64 }
}

/// Tokei's class of each line, read from the counts of growing prefixes.
fn tokei_per_line(path: &Path, text: &str) -> Vec<Line> {
    per_line(text, |prefix| tokei_lines(path, prefix))
}

fn fork_per_line(path: &Path, text: &str) -> Vec<Line> {
    per_line(text, |prefix| fork_lines(path, prefix))
}

fn per_line(text: &str, counter: impl Fn(&str) -> Counts) -> Vec<Line> {
    let mut out = Vec::new();
    let mut previous = Counts::default();
    let mut end = 0;
    for line in text.split_inclusive('\n') {
        end += line.len();
        let now = counter(&text[..end]);
        out.push(if now.code > previous.code {
            Line::Code
        } else if now.comments > previous.comments {
            Line::Comment
        } else {
            Line::Blank
        });
        previous = now;
    }
    out
}

fn json(c: Counts) -> serde_json::Value {
    serde_json::json!([c.code, c.comments, c.blanks])
}

fn count(root: &Path) {
    let files: Vec<_> = walkdir::WalkDir::new(root)
        .into_iter()
        .filter_entry(|e| !matches!(e.file_name().to_str(), Some(".git" | "node_modules")))
        .filter_map(Result::ok)
        .filter(|e| e.file_type().is_file())
        .filter_map(|e| Lang::from_path(e.path()).map(|lang| (e.into_path(), lang)))
        .collect();
    let rows: Vec<String> = files
        .par_iter()
        .filter_map(|(path, lang)| {
            let text = std::fs::read_to_string(path).ok()?;
            let t0 = Instant::now();
            let tokei = tokei_lines(path, &text);
            let t1 = Instant::now();
            let (ts_regions, ts_error) = ts::regions(&text, &lang.grammar());
            let ts = lines::count(&lines::lines(&text, &ts_regions));
            let t2 = Instant::now();
            let lex = lang.lexer(&text).map(|r| json(lines::count(&lines::lines(&text, &r))));
            let t3 = Instant::now();
            let fork = fork_lines(path, &text);
            let t4 = Instant::now();
            Some(
                serde_json::json!({
                    "path": path.strip_prefix(root).unwrap(),
                    "lang": format!("{lang:?}"),
                    "bytes": text.len(),
                    "tokei": json(tokei), "ts": json(ts), "lex": lex, "fork": json(fork),
                    "ts_error": ts_error,
                    "us": [(t1 - t0).as_micros(), (t2 - t1).as_micros(), (t3 - t2).as_micros(), (t4 - t3).as_micros()],
                })
                .to_string(),
            )
        })
        .collect();
    for row in rows {
        println!("{row}");
    }
}

fn show_lines(path: &Path) {
    let lang = Lang::from_path(path).expect("spike language");
    let text = std::fs::read_to_string(path).unwrap();
    let ts = lines::lines(&text, &ts::regions(&text, &lang.grammar()).0);
    let lex = lang.lexer(&text).map(|r| lines::lines(&text, &r));
    let tok = tokei_per_line(path, &text);
    let fork = fork_per_line(path, &text);
    let only_diff = std::env::args().any(|a| a == "--diff");
    let mark = |line: Line| match line {
        Line::Code => 'C',
        Line::Comment => 'm',
        Line::Blank => '.',
    };
    for (n, source) in text.lines().enumerate() {
        let (t, a, f) = (tok[n], ts[n], fork[n]);
        let b = lex.as_ref().map_or(a, |lex| lex[n]);
        if only_diff && t == a && a == b && b == f {
            continue;
        }
        let flag = if t != a || a != b || b != f { "≠" } else { " " };
        println!("{:5} tok:{} ts:{} lex:{} fork:{} {flag} {source}", n + 1, mark(t), mark(a), mark(b), mark(f));
    }
}

fn main() {
    let args: Vec<String> = std::env::args().collect();
    match args.get(1).map(String::as_str) {
        Some("count") => count(Path::new(&args[2])),
        Some("lines") => show_lines(Path::new(&args[2])),
        _ => eprintln!("usage: spike count ROOT | spike lines FILE"),
    }
}
