//! Judges justfiles with just 1.59.0's own lexer: reads paths on stdin and
//! prints, per file, the comment and literal ranges in byte offsets after a
//! byte order mark, as judgecmp's stream expects.
//!
//! Outside recipe bodies, comment tokens are comments, and strings,
//! backticks and the text of format strings are literals. Inside a body, just
//! hands each line to the shell (`Recipe::run_linewise` in `src/recipe.rs`),
//! after the `-`, `@` and `?` sigils, joined with the next when it ends in a
//! backslash; a line whose command starts with `#` after blanks is a shell
//! comment. A script recipe, one whose body starts with a `#!` line or that
//! carries the `[script]` attribute, runs its body as one script for its
//! interpreter: those bodies are printed as `scripts` for `scripts.py`, which
//! hands them to their interpreter's judge, and left as code here.

mod just;

use just::{Token, TokenKind};
use std::io::BufRead;

#[derive(serde::Serialize)]
struct Judgement {
    path: String,
    units: &'static str,
    errors: usize,
    /// The lexer's error, for diagnosis.
    #[serde(skip_serializing_if = "Option::is_none")]
    error: Option<String>,
    comments: Vec<[usize; 2]>,
    literals: Vec<[usize; 2]>,
    scripts: Vec<Script>,
}

/// A script recipe's body: its offsets, its `#!` line or `#!` and the
/// command of its `[script]` attribute, and
/// the spans of its `{{…}}` interpolations, which just evaluates first.
#[derive(serde::Serialize)]
struct Script {
    start: usize,
    end: usize,
    interpreter: String,
    interpolations: Vec<[usize; 2]>,
}

fn main() {
    for path in std::io::stdin().lock().lines() {
        let path = path.unwrap();
        let Ok(text) = std::fs::read_to_string(&path) else {
            continue;
        };
        let text = text.strip_prefix('\u{feff}').unwrap_or(&text);
        let mut judgement = Judgement {
            path: path.clone(),
            units: "bytes",
            errors: 0,
            error: None,
            comments: Vec::new(),
            literals: Vec::new(),
            scripts: Vec::new(),
        };
        match just::lex(path.as_ref(), text) {
            Ok(tokens) => judge(text, &tokens, &mut judgement),
            Err(error) => {
                judgement.errors = 1;
                judgement.error = Some(format!("line {}: {:?}", error.token.line + 1, error.kind));
            }
        }
        println!("{}", serde_json::to_string(&judgement).unwrap());
    }
}

fn judge(text: &str, tokens: &[Token], judgement: &mut Judgement) {
    use TokenKind::*;
    // `set script-interpreter := [...]` holds for every `[script]` recipe of
    // the file, wherever it stands.
    let setting = tokens.iter().enumerate().find_map(|(index, token)| {
        let next = tokens[index + 1..].iter().find(|t| t.kind != Whitespace);
        (token.kind == Identifier
            && token.lexeme() == "set"
            && next.is_some_and(|t| t.lexeme() == "script-interpreter"))
        .then(|| command(&tokens[index..]))
        .flatten()
    });
    // A `[script]` attribute's command, `Some(None)` when it names none.
    let mut attribute: Option<Option<String>> = None;
    let mut index = 0;
    while index < tokens.len() {
        let token = tokens[index];
        match token.kind {
            Comment => judgement.comments.push(span(&token)),
            StringToken | Backtick | FormatStringStart | FormatStringContinue | FormatStringEnd => {
                judgement.literals.push(span(&token))
            }
            BracketL if tokens.get(index + 1).is_some_and(|t| t.kind == Identifier && t.lexeme() == "script") => {
                let arguments = tokens.get(index + 2).is_some_and(|t| t.kind == ParenL);
                attribute = Some(if arguments { command(&tokens[index..]) } else { None });
            }
            Indent if tokens.get(index + 1).is_some_and(|t| matches!(t.kind, Text | InterpolationStart)) => {
                let end = tokens[index..].iter().position(|t| t.kind == Dedent).map_or(tokens.len(), |n| index + n);
                let script = attribute
                    .take()
                    .map(|command| command.or_else(|| setting.clone()).unwrap_or_else(|| "sh -eu".to_owned()));
                body(text, &tokens[index + 1..end], script, judgement);
                index = end;
                continue;
            }
            _ => {}
        }
        index += 1;
    }
}

/// The strings of the first bracketed or parenthesized list after the first
/// token, joined by spaces: the command and arguments an `Interpreter` holds.
fn command(tokens: &[Token]) -> Option<String> {
    use TokenKind::*;
    let open = 1 + tokens[1..].iter().position(|t| matches!(t.kind, BracketL | ParenL))?;
    let words: Vec<&str> = tokens[open + 1..]
        .iter()
        .take_while(|t| !matches!(t.kind, BracketR | ParenR))
        .filter(|t| t.kind == StringToken)
        .map(|t| {
            let quote = if t.lexeme().starts_with("'''") || t.lexeme().starts_with(r#"""""#) { 3 } else { 1 };
            &t.lexeme()[quote..t.lexeme().len() - quote]
        })
        .collect();
    (!words.is_empty()).then(|| words.join(" "))
}

fn span(token: &Token) -> [usize; 2] {
    [token.offset, token.offset + token.length]
}

/// Classes a recipe body's lines, from its tokens after the opening indent;
/// `script` is the command a `[script]` attribute runs it with.
fn body(text: &str, tokens: &[Token], script: Option<String>, judgement: &mut Judgement) {
    use TokenKind::*;
    let Some(first) = tokens.first() else { return };
    let end = tokens.last().map_or(first.offset, |t| t.offset + t.length);
    let shebang = first.kind == Text && first.lexeme().starts_with("#!");
    if shebang || script.is_some() {
        // `Recipe::run_script` prefers the attribute's command to a `#!` line.
        let interpreter = script.map_or_else(|| first.lexeme().to_owned(), |command| format!("#!{command}"));
        let mut interpolations = Vec::new();
        for token in tokens {
            match token.kind {
                InterpolationStart => interpolations.push([token.offset, token.offset]),
                InterpolationEnd => interpolations.last_mut().unwrap()[1] = token.offset + token.length,
                _ => {}
            }
        }
        judgement.scripts.push(Script { start: first.offset, end, interpreter, interpolations });
        return;
    }
    // The first token of each line decides it: a comment when it continues
    // a comment's last line, or is text whose command starts with `#`.
    let mut line_start = true;
    let mut continued_comment = false;
    for token in tokens {
        match token.kind {
            Eol if line_start => continued_comment = false,
            Eol => line_start = true,
            Whitespace if line_start => {}
            _ if line_start => {
                line_start = false;
                let command = if token.kind == Text { token.lexeme() } else { "" };
                let comment =
                    continued_comment || without_sigils(command).trim_start_matches([' ', '\t']).starts_with('#');
                let line_end = text[token.offset..].find('\n').map_or(text.len(), |n| token.offset + n);
                let line = text[token.offset..line_end].trim_end_matches('\r');
                if comment {
                    judgement.comments.push([token.offset, token.offset + line.len()]);
                }
                continued_comment = comment && line.ends_with('\\');
            }
            _ => {}
        }
    }
}

/// A recipe line's command after the sigils `Line::sigils` reads: `-`, `@`
/// and `?`, each at most once.
fn without_sigils(line: &str) -> &str {
    let mut seen = String::new();
    for (index, c) in line.char_indices() {
        if !matches!(c, '-' | '@' | '?') || seen.contains(c) {
            return &line[index..];
        }
        seen.push(c);
    }
    ""
}
