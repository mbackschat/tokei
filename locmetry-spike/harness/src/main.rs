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
    Perl,
    Html,
    Xml,
    Yaml,
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
            "pl" | "pm" => Lang::Perl,
            "html" | "htm" => Lang::Html,
            "xml" | "svg" | "xaml" => Lang::Xml,
            "yaml" | "yml" => Lang::Yaml,
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
            Lang::Perl => ts_parser_perl::LANGUAGE.into(),
            Lang::Html => tree_sitter_html::LANGUAGE.into(),
            Lang::Xml => tree_sitter_xml::LANGUAGE_XML.into(),
            Lang::Yaml => tree_sitter_yaml::LANGUAGE.into(),
        }
    }

    fn regions(self, text: &str) -> (Vec<Region>, bool) {
        match self {
            Lang::Html => ts::html_regions(text),
            _ => ts::regions(text, &self.grammar()),
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
    let stats = kind.parse_from_str(text, &config);
    let mut counts = Counts { code: stats.code as u64, comments: stats.comments as u64, blanks: stats.blanks as u64 };
    for (child, blob) in &stats.blobs {
        let blob = blob.summarise();
        // Locmetry folds Markdown blobs, which Tokei makes of Rust doc
        // comments, into the host's comments (src/backend/records.rs).
        if *child == tokei_fork::LanguageType::Markdown {
            counts.comments += (blob.code + blob.comments) as u64;
        } else {
            counts.code += blob.code as u64;
            counts.comments += blob.comments as u64;
        }
        counts.blanks += blob.blanks as u64;
    }
    counts
}

fn tokei_lines(path: &Path, text: &str) -> Counts {
    let config = tokei::Config::default();
    let kind = LanguageType::from_path(path, &config).expect("Tokei language");
    let stats = kind.parse_from_str(text, &config);
    let mut counts = Counts { code: stats.code as u64, comments: stats.comments as u64, blanks: stats.blanks as u64 };
    for (child, blob) in &stats.blobs {
        let blob = blob.summarise();
        if *child == LanguageType::Markdown {
            counts.comments += (blob.code + blob.comments) as u64;
        } else {
            counts.code += blob.code as u64;
            counts.comments += blob.comments as u64;
        }
        counts.blanks += blob.blanks as u64;
    }
    counts
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
            let text = read(path)?;
            let t0 = Instant::now();
            let tokei = tokei_lines(path, &text);
            let t1 = Instant::now();
            let (ts_regions, ts_error) = lang.regions(&text);
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

/// The file's text without a leading byte order mark, which Tokei's file
/// reader strips (encoding_rs_io BOM sniffing) before counting.
fn read(path: &Path) -> Option<String> {
    let text = std::fs::read_to_string(path).ok()?;
    Some(text.strip_prefix('\u{feff}').map(str::to_owned).unwrap_or(text))
}

fn show_lines(path: &Path) {
    let lang = Lang::from_path(path).expect("spike language");
    let text = read(path).unwrap();
    let ts = lines::lines(&text, &lang.regions(&text).0);
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

/// Prints the tree-sitter nodes that intersect lines FROM to TO, one-based,
/// with their depth, kind, rows and error flags.
fn show_tree(path: &Path, from: usize, to: usize) {
    let lang = Lang::from_path(path).expect("spike language");
    let text = read(path).unwrap();
    let mut parser = tree_sitter::Parser::new();
    parser.set_language(&lang.grammar()).unwrap();
    let tree = parser.parse(&text, None).unwrap();
    let mut stack = vec![(tree.root_node(), 0)];
    while let Some((node, depth)) = stack.pop() {
        let (start, end) = (node.start_position().row + 1, node.end_position().row + 1);
        if end < from || start > to {
            continue;
        }
        let snippet: String = node.utf8_text(text.as_bytes()).unwrap_or("").chars().take(40).collect();
        println!("{}{} {}-{}{}{} {:?}", "  ".repeat(depth), node.kind(), start, end,
            if node.is_error() { " ERROR" } else { "" }, if node.is_missing() { " MISSING" } else { "" }, snippet);
        let mut cursor = node.walk();
        let children: Vec<_> = node.children(&mut cursor).collect();
        for child in children.into_iter().rev() {
            stack.push((child, depth + 1));
        }
    }
}

fn main() {
    let args: Vec<String> = std::env::args().collect();
    match args.get(1).map(String::as_str) {
        Some("count") => count(Path::new(&args[2])),
        Some("lines") => show_lines(Path::new(&args[2])),
        Some("tree") => show_tree(Path::new(&args[2]), args[3].parse().unwrap(), args[4].parse().unwrap()),
        _ => eprintln!("usage: spike count ROOT | spike lines FILE"),
    }
}
