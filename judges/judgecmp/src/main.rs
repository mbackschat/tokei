//! Compares the fork's per-line classes with a language judge's, line by line.
//!
//! `judgecmp rust ROOT...` judges Rust files with rustc's own lexer
//! (`ra-ap-rustc_lexer`). `judgecmp stream LANGUAGE` reads judgements from
//! stdin, one JSON object per file: `path`, `comments` and `literals` as
//! `[start, end]` offsets into the text after a byte order mark, `units`
//! (`"bytes"` or `"utf16"`), and `errors`, the lexical errors the judge found.
//!
//! Lines follow the owner's rule of 2026-10-29: a line is a comment when it
//! holds nothing but comments; a line of whitespace takes the class of the
//! comment or literal it lies in, code for a literal, and is blank otherwise;
//! every other line is code.

use rayon::prelude::*;
use std::io::BufRead;
use std::path::{Path, PathBuf};
use tokei::{Config, LanguageType, LineClass};

const CODE: u8 = 0;
const COMMENT: u8 = 1;
const LITERAL: u8 = 2;

struct Judgement {
    path: PathBuf,
    /// Byte classes of the text, `CODE` where no comment or literal lies.
    classes: Vec<u8>,
    errors: usize,
}

struct Outcome {
    path: PathBuf,
    lines: usize,
    errors: usize,
    differences: Vec<(usize, char, char, String)>,
}

fn main() {
    let args: Vec<String> = std::env::args().collect();
    let (language, judgements): (Option<LanguageType>, Vec<Judgement>) = match args.get(1).map(String::as_str) {
        Some("rust") => (Some(LanguageType::Rust), judge_rust(&args[2..])),
        Some("stream") => {
            // `auto` takes each file's language from its path.
            let language = match args[2].as_str() {
                "auto" => None,
                name => Some(name.parse().expect("a Tokei language name")),
            };
            (language, read_stream())
        }
        _ => panic!("usage: judgecmp rust ROOT... | judgecmp stream LANGUAGE"),
    };
    let outcomes: Vec<Outcome> = judgements
        .into_par_iter()
        .filter_map(|j| compare(language, j))
        .collect();
    report(&outcomes);
}

fn report(outcomes: &[Outcome]) {
    let lines: usize = outcomes.iter().map(|o| o.lines).sum();
    let (invalid, valid): (Vec<&Outcome>, Vec<&Outcome>) = outcomes.iter().partition(|o| o.errors > 0);
    let differing: Vec<&&Outcome> = valid.iter().filter(|o| !o.differences.is_empty()).collect();
    let mut sorted = differing.clone();
    sorted.sort_by_key(|o| &o.path);
    for o in &sorted {
        println!("DIFF {} ({} lines)", o.path.display(), o.differences.len());
        for (line, judge, fork, text) in o.differences.iter().take(6) {
            println!("  {line:6} judge:{judge} fork:{fork} | {text}");
        }
    }
    if std::env::var_os("JUDGE_LIST_INVALID").is_some() {
        for o in &invalid {
            println!("INVALIDFILE {} ({} errors)", o.path.display(), o.errors);
        }
    }
    for o in invalid.iter().filter(|o| !o.differences.is_empty()) {
        println!("INVALID {} ({} errors, {} lines differ)", o.path.display(), o.errors, o.differences.len());
        for (line, judge, fork, text) in o.differences.iter().take(3) {
            println!("  {line:6} judge:{judge} fork:{fork} | {text}");
        }
    }
    println!(
        "files {} lines {} | lexically invalid {} | valid files differing {} lines differing {}",
        outcomes.len(),
        lines,
        invalid.len(),
        differing.len(),
        differing.iter().map(|o| o.differences.len()).sum::<usize>()
    );
}

/// The characters each language's lexer skips between tokens.
fn whitespace(language: LanguageType) -> fn(char) -> bool {
    use LanguageType::*;
    match language {
        Rust => ra_ap_rustc_lexer::is_whitespace,
        // Translation phase 2 deletes each backslash before a newline (C23
        // §5.1.1.2), and the clang judge leaves these splices outside its
        // tokens; elsewhere in valid code a backslash occurs only in literals.
        C | CHeader | Cpp | CppHeader | ObjectiveC | ObjectiveCpp => {
            |c: char| c.is_ascii_whitespace() || c == '\u{000B}' || c == '\\'
        }
        _ => |c: char| c.is_ascii_whitespace() || c == '\u{000B}',
    }
}

fn compare(language: Option<LanguageType>, judgement: Judgement) -> Option<Outcome> {
    let text = read(&judgement.path)?;
    let language = language.or_else(|| LanguageType::from_path(&judgement.path, &Config::default()))?;
    let whitespace = whitespace(language);
    let fork = language.line_classes(text.as_bytes(), &Config::default())?;
    let judged = line_classes(&text, &judgement.classes, whitespace);
    let mark = |c: LineClass| match c {
        LineClass::Code => 'C',
        LineClass::Comment => 'm',
        LineClass::Blank => '.',
    };
    let differences = text
        .split_inclusive('\n')
        .zip(judged.iter().zip(&fork))
        .enumerate()
        .filter(|(_, (_, (j, f)))| j != f)
        .map(|(n, (line, (j, f)))| (n + 1, mark(*j), mark(*f), line.trim_end().chars().take(100).collect()))
        .collect::<Vec<_>>();
    let differences = if judged.len() == fork.len() {
        differences
    } else {
        vec![(0, '#', '#', format!("judge has {} lines, fork {}", judged.len(), fork.len()))]
    };
    Some(Outcome { path: judgement.path, lines: judged.len(), errors: judgement.errors, differences })
}

fn line_classes(text: &str, classes: &[u8], whitespace: fn(char) -> bool) -> Vec<LineClass> {
    let mut out = Vec::new();
    let mut start = 0;
    for line in text.split_inclusive('\n') {
        let mut text_seen = false;
        let mut only_comments = true;
        for (offset, c) in line.char_indices() {
            if !whitespace(c) {
                text_seen = true;
                if classes[start + offset] != COMMENT {
                    only_comments = false;
                }
            }
        }
        out.push(match (text_seen, only_comments) {
            (true, true) => LineClass::Comment,
            (true, false) => LineClass::Code,
            // A line of whitespace takes the class of the comment or literal
            // its own bytes lie in, so a heredoc body's first line is code
            // and the line after a line comment is blank.
            (false, _) => match &classes[start..start + line.len()] {
                bytes if bytes.contains(&COMMENT) => LineClass::Comment,
                bytes if bytes.contains(&LITERAL) => LineClass::Code,
                _ => LineClass::Blank,
            },
        });
        start += line.len();
    }
    out
}

/// The file's text after a byte order mark, which Tokei's reader strips.
fn read(path: &Path) -> Option<String> {
    let text = String::from_utf8(std::fs::read(path).ok()?).ok()?;
    Some(text.strip_prefix('\u{feff}').map(str::to_owned).unwrap_or(text))
}

fn judge_rust(roots: &[String]) -> Vec<Judgement> {
    let files: Vec<PathBuf> = roots
        .iter()
        .flat_map(|root| walkdir::WalkDir::new(root).into_iter().filter_map(Result::ok))
        .filter(|e| e.file_type().is_file() && e.path().extension().is_some_and(|x| x == "rs"))
        .map(|e| e.into_path())
        .collect();
    files
        .into_par_iter()
        .filter_map(|path| {
            let text = read(&path)?;
            let (classes, errors) = rust_classes(&text);
            Some(Judgement { path, classes, errors })
        })
        .collect()
}

/// Byte classes from rustc's lexer, run as rustc runs it on a whole file:
/// the shebang stripped first and frontmatter allowed. A guarded-string
/// prefix, `#"` or `##`, is split as rustc_parse splits it before Rust 2024,
/// where valid code holds none: `#` and a fresh lexer from the next character.
fn rust_classes(text: &str) -> (Vec<u8>, usize) {
    use ra_ap_rustc_lexer::{FrontmatterAllowed, LiteralKind as L, TokenKind as T};
    let mut classes = vec![CODE; text.len()];
    let mut errors = 0;
    // rustc ignores a shebang as it ignores comments, and the shebang is a
    // comment in the other judged languages' lexers.
    let mut pos = ra_ap_rustc_lexer::strip_shebang(text).unwrap_or(0);
    classes[..pos].fill(COMMENT);
    let mut frontmatter = FrontmatterAllowed::Yes;
    'restart: loop {
        for token in ra_ap_rustc_lexer::tokenize(&text[pos..], frontmatter) {
            let end = pos + token.len as usize;
            let class = match token.kind {
                T::LineComment { .. } => Some(COMMENT),
                T::BlockComment { terminated, .. } => {
                    errors += usize::from(!terminated);
                    Some(COMMENT)
                }
                T::Literal { kind, .. } => match kind {
                    L::Char { terminated } | L::Byte { terminated } | L::Str { terminated } | L::ByteStr { terminated } | L::CStr { terminated } => {
                        errors += usize::from(!terminated);
                        Some(LITERAL)
                    }
                    L::RawStr { n_hashes } | L::RawByteStr { n_hashes } | L::RawCStr { n_hashes } => {
                        errors += usize::from(n_hashes.is_none());
                        Some(LITERAL)
                    }
                    L::Int { .. } | L::Float { .. } => None,
                },
                T::Frontmatter { has_invalid_preceding_whitespace, invalid_infostring } => {
                    let token_text = &text[pos..end];
                    let fence = token_text.trim_start().bytes().take_while(|b| *b == b'-').count();
                    // rustc_parse also rejects more than 255 dashes and a
                    // closing fence whose dashes differ from the opening's.
                    let closing = token_text.rsplit('\n').next().unwrap_or("");
                    let closed = token_text.contains('\n')
                        && closing.trim_end().len() == fence
                        && closing.bytes().take_while(|b| *b == b'-').count() == fence;
                    errors += usize::from(has_invalid_preceding_whitespace || invalid_infostring || !closed || fence > 255);
                    Some(LITERAL)
                }
                T::GuardedStrPrefix => {
                    pos += 1;
                    frontmatter = FrontmatterAllowed::No;
                    continue 'restart;
                }
                T::Unknown => {
                    errors += 1;
                    None
                }
                _ => None,
            };
            if let Some(class) = class {
                classes[pos..end].fill(class);
            }
            pos = end;
        }
        break;
    }
    (classes, errors)
}

#[derive(serde::Deserialize)]
struct Streamed {
    path: PathBuf,
    comments: Vec<[usize; 2]>,
    literals: Vec<[usize; 2]>,
    #[serde(default)]
    units: Option<String>,
    #[serde(default)]
    errors: usize,
}

fn read_stream() -> Vec<Judgement> {
    std::io::stdin()
        .lock()
        .lines()
        .map(|line| serde_json::from_str::<Streamed>(&line.unwrap()).expect("a judgement"))
        .filter_map(|s| {
            let text = read(&s.path)?;
            let to_byte = offsets(&text, s.units.as_deref() == Some("utf16"));
            let mut classes = vec![CODE; text.len()];
            for (ranges, class) in [(&s.literals, LITERAL), (&s.comments, COMMENT)] {
                for [a, b] in ranges {
                    classes[to_byte(*a)..to_byte(*b)].fill(class);
                }
            }
            Some(Judgement { path: s.path, classes, errors: s.errors })
        })
        .collect()
}

/// Maps a judge's offsets to byte offsets of `text`.
fn offsets(text: &str, utf16: bool) -> Box<dyn Fn(usize) -> usize> {
    if !utf16 {
        let len = text.len();
        return Box::new(move |n| n.min(len));
    }
    let mut map = Vec::with_capacity(text.len() + 1);
    for (byte, c) in text.char_indices() {
        for _ in 0..c.len_utf16() {
            map.push(byte);
        }
    }
    map.push(text.len());
    Box::new(move |n| map[n.min(map.len() - 1)])
}
