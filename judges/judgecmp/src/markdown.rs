//! `judgecmp markdown` compares the fork's Markdown counts with those that
//! follow from a judge's fenced code blocks, file by file.
//!
//! The judge, `markdown/judge.mjs`, prints each file's fenced code blocks as
//! CommonMark finds them. A block whose language the fork names counts its
//! fences as comment lines and its content as that language counts it; every
//! other line is Markdown text, a comment line, or blank when it holds only
//! spaces and tabs, as CommonMark's blank lines do (0.31.2, §2.1), so a line
//! of no-break spaces is text. The block's language is the fork's: the first word of its info
//! string, or the first of that word's comma-separated parts, that names a
//! language.

use std::collections::BTreeMap;
use std::io::BufRead;
use std::path::PathBuf;
use std::str::FromStr;

use rayon::prelude::*;
use tokei::{CodeStats, Config, LanguageType};

#[derive(serde::Deserialize)]
struct Block {
    open: usize,
    close: Option<usize>,
    end: usize,
    info: String,
    literal: String,
}

#[derive(serde::Deserialize)]
struct Judged {
    path: PathBuf,
    blocks: Vec<Block>,
}

/// The counts a file's lines and blocks imply: Markdown's comment and blank
/// lines, and each embedded language's code, comment and blank lines.
#[derive(Debug, Default, PartialEq)]
struct Counts {
    comments: usize,
    blanks: usize,
    blobs: BTreeMap<LanguageType, [usize; 3]>,
}

fn block_language(info: &str) -> Option<LanguageType> {
    info.split_whitespace()
        .next()?
        .split(',')
        .find_map(|part| LanguageType::from_str(part).ok())
}

fn expected(text: &str, blocks: &[Block], config: &Config) -> Counts {
    let lines: Vec<&str> = text.lines().collect();
    let mut counts = Counts::default();
    let mut in_block = vec![false; lines.len()];
    for block in blocks {
        let Some(language) = block_language(&block.info) else {
            continue;
        };
        for line in &mut in_block[block.open..block.end.min(lines.len())] {
            *line = true;
        }
        counts.comments += 1 + usize::from(block.close.is_some());
        let stats = language.parse_from_str(&block.literal, config).summarise();
        let blob = counts.blobs.entry(language).or_default();
        blob[0] += stats.code;
        blob[1] += stats.comments;
        blob[2] += stats.blanks;
    }
    for (line, _) in lines.iter().zip(&in_block).filter(|(_, inside)| !**inside) {
        if line.bytes().all(|b| matches!(b, b' ' | b'\t' | b'\r')) {
            counts.blanks += 1;
        } else {
            counts.comments += 1;
        }
    }
    counts
}

fn counted(stats: &CodeStats) -> Counts {
    Counts {
        comments: stats.comments + stats.code,
        blanks: stats.blanks,
        blobs: stats
            .blobs
            .iter()
            .map(|(language, blob)| {
                let blob = blob.summarise();
                (*language, [blob.code, blob.comments, blob.blanks])
            })
            .collect(),
    }
}

pub fn run() {
    let config = Config::default();
    let judged: Vec<Judged> = std::io::stdin()
        .lock()
        .lines()
        .map(|line| serde_json::from_str(&line.expect("stdin")).expect("judgement"))
        .collect();
    let differing: Vec<String> = judged
        .par_iter()
        .filter_map(|file| {
            let bytes = std::fs::read(&file.path).ok()?;
            let text = String::from_utf8(bytes).ok()?;
            let text = text.strip_prefix('\u{feff}').unwrap_or(&text);
            let want = expected(text, &file.blocks, &config);
            let got = counted(&LanguageType::Markdown.parse_from_str(text, &config));
            (want != got).then(|| format!("DIFF {}\n  judge {want:?}\n  fork  {got:?}", file.path.display()))
        })
        .collect();
    for diff in &differing {
        println!("{diff}");
    }
    println!("files {} | differing {}", judged.len(), differing.len());
}
