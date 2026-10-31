//! Justfiles, as just 1.59.0's lexer (`src/lexer.rs`) and recipe runner
//! (`src/recipe.rs`) read them. Outside recipe bodies, `#` starts a comment
//! to the end of the line, and `'`, `"`, `'''`, `"""`, `` ` `` and ```` ``` ````
//! delimit strings that may span lines, double-quoted ones with backslash
//! escapes and `f`-prefixed ones with `{{…}}` interpolations. A colon opens a
//! recipe, whose body is the indented lines after it. just hands each body
//! line to the shell after its `-`, `@` and `?` sigils, joined with the next
//! when it ends in a backslash, so a line whose command starts with `#` is a
//! comment. A recipe with the `[script]` attribute runs its body as one
//! script for the attribute's command, or the `script-interpreter`
//! setting's, `sh -eu` by default; one whose body starts with a `#!` line,
//! for the interpreter that line names (`Recipe::run_script`). The script is
//! the body without the recipe's indentation.

use std::ops::Range;

use grep_searcher::LineStep;

use crate::language::{language_type::LineReport, syntax::LineClass};
use crate::{utils::ext::SliceExt, CodeStats, Config, LanguageType};

/// What a line holds outside recipe bodies, by the bytes it holds.
#[derive(Clone, Copy, Default)]
struct Holds {
    code: bool,
    comment: bool,
    literal: bool,
}

impl Holds {
    fn class(self) -> LineClass {
        match self {
            Holds { code: true, .. } => LineClass::Code,
            Holds { comment: true, .. } => LineClass::Comment,
            Holds { literal: true, .. } => LineClass::Code,
            _ => LineClass::Blank,
        }
    }
}

/// How just runs a recipe body.
#[derive(Clone, Copy)]
enum Runner {
    /// Line by line, by the shell.
    Lines,
    /// As a script for the interpreter its `#!` line names.
    Shebang,
    /// As a script for the command of a `[script]` attribute, whose list
    /// starts at the given offset, or else of the `script-interpreter`
    /// setting.
    Script(Option<usize>),
}

/// A recipe body: its lines, its indentation and how it runs.
struct Body {
    lines: Range<usize>,
    indentation: Range<usize>,
    runner: Runner,
}

/// The string delimiter starting `rest`, longest first, as
/// `StringKind::from_token_start` finds it.
fn string_delimiter(rest: &[u8]) -> Option<&'static [u8]> {
    [&b"```"[..], b"`", b"\"\"\"", b"\"", b"'''", b"'"]
        .into_iter()
        .find(|delimiter| rest.starts_with(delimiter))
}

/// Reads a justfile's lines outside recipe bodies and finds the bodies.
struct Reader<'a> {
    text: &'a [u8],
    lines: &'a [Range<usize>],
    /// The line each byte lies on is found by walking forward from here.
    line: usize,
    holds: Vec<Holds>,
    /// Whether a recipe's colon has been read since the last body.
    pending: bool,
    /// The `[script]` attribute read since the last body, with the offset of
    /// its command list when it has one.
    attribute: Option<Option<usize>>,
    /// The offset of the `script-interpreter` setting's command list.
    setting: Option<usize>,
}

impl<'a> Reader<'a> {
    fn mark(&mut self, range: Range<usize>, mark: fn(&mut Holds, bool)) {
        for at in range {
            let line = self.line_of(at);
            mark(&mut self.holds[line], self.text[at].is_ascii_whitespace());
        }
    }

    fn code(&mut self, at: usize) {
        self.mark(at..at + 1, |holds, blank| holds.code |= !blank);
    }

    /// Reads the string at `start`, as `Lexer::lex_string` does, and returns
    /// its end; a format string's interpolations are read as code.
    fn string(&mut self, start: usize, format: bool) -> usize {
        let delimiter = string_delimiter(&self.text[start..]).unwrap_or(b"'");
        let escapes = delimiter[0] == b'"';
        let format = format && delimiter[0] != b'`';
        let mut at = start + delimiter.len();
        let mut from = start;
        loop {
            let rest = self.text.get(at..).unwrap_or_default();
            if rest.is_empty() {
                break;
            } else if escapes && rest[0] == b'\\' {
                at += 2;
            } else if format && rest.starts_with(b"{{{{") {
                at += 4;
            } else if rest.starts_with(delimiter) {
                at += delimiter.len();
                break;
            } else if format && rest.starts_with(b"{{") {
                self.mark(from..at + 2, |holds, _| holds.literal = true);
                at = self.expression(at + 2, b"}}");
                from = at;
                continue;
            } else {
                at += 1;
            }
        }
        let end = at.min(self.text.len());
        self.mark(from..end, |holds, _| holds.literal = true);
        end
    }

    /// Reads code from `at` to the `close` that ends it outside strings and
    /// brackets, and returns the end of `close`.
    fn expression(&mut self, mut at: usize, close: &[u8]) -> usize {
        let mut depth = 0usize;
        while at < self.text.len() {
            if depth == 0 && self.text[at..].starts_with(close) {
                self.mark(at..at + close.len(), |holds, _| holds.code = true);
                return at + close.len();
            }
            at = self.token(at, &mut depth);
        }
        at
    }

    /// Reads the comment, string, bracket or other byte at `at`, keeping the
    /// bracket `depth`, and returns where the next one starts.
    fn token(&mut self, at: usize, depth: &mut usize) -> usize {
        match self.text[at] {
            b'#' => return self.comment(at),
            b'\'' | b'"' | b'`' => return self.string(at, self.format_prefix(at)),
            b'(' | b'[' | b'{' => *depth += 1,
            b')' | b']' | b'}' => *depth = depth.saturating_sub(1),
            _ => {}
        }
        self.code(at);
        at + 1
    }

    fn comment(&mut self, at: usize) -> usize {
        let end = self.lines[self.line_of(at)].end;
        let end = if self.text[..end].ends_with(b"\n") {
            end - 1
        } else {
            end
        };
        self.mark(at..end, |holds, blank| holds.comment |= !blank);
        end
    }

    fn line_of(&mut self, at: usize) -> usize {
        while at >= self.lines[self.line].end {
            self.line += 1;
        }
        self.line
    }

    /// Whether the string at `at` follows the identifier `f`, which makes it a
    /// format string.
    fn format_prefix(&self, at: usize) -> bool {
        let is_identifier = |b: &u8| b.is_ascii_alphanumeric() || matches!(b, b'_' | b'-');
        at >= 1
            && self.text[at - 1] == b'f'
            && !self.text[..at - 1].last().is_some_and(is_identifier)
    }

    /// Reads a line outside bodies from `at`, and returns where reading
    /// stopped: the end of the line, or later when a bracket, string or
    /// escaped newline carries it on.
    fn statement(&mut self, mut at: usize) -> usize {
        if let Some(after) = self.text[at..].strip_prefix(b"set") {
            let name = after.trim_start();
            if name.len() < after.len() && name.starts_with(b"script-interpreter") {
                self.setting = Some(self.text.len() - name.len() + b"script-interpreter".len());
            }
        }
        let mut depth = 0usize;
        while at < self.text.len() {
            let rest = &self.text[at..];
            match rest[0] {
                b'\n' if depth == 0 => return at + 1,
                b':' if rest.starts_with(b":=") || rest.starts_with(b"::") => {
                    self.mark(at..at + 2, |holds, _| holds.code = true);
                    at += 2;
                }
                b'\\' if rest[1..].starts_with(b"\n") || rest[1..].starts_with(b"\r\n") => {
                    self.code(at);
                    at += if rest[1] == b'\r' { 3 } else { 2 };
                }
                byte => {
                    self.pending |= byte == b':';
                    if rest.starts_with(b"[script")
                        && !rest
                            .get(7)
                            .is_some_and(|b| b.is_ascii_alphanumeric() || matches!(b, b'_' | b'-'))
                    {
                        self.attribute = Some((rest.get(7) == Some(&b'(')).then_some(at + 8));
                    }
                    at = self.token(at, &mut depth);
                }
            }
        }
        at
    }
}

/// The leading blanks of a line.
fn leading_blanks(line: &[u8]) -> usize {
    line.iter()
        .take_while(|b| matches!(b, b' ' | b'\t'))
        .count()
}

/// Whether only blank lines lie between `at` and the next line that starts
/// with something other than a blank, as `DEDENT_RE` matches.
fn dedents(text: &[u8], at: usize) -> bool {
    let rest = &text[at..];
    let Some(last_newline) = rest
        .iter()
        .position(|b| !b.is_ascii_whitespace())
        .and_then(|first| rest[..first].iter().rposition(|&b| b == b'\n'))
    else {
        return false;
    };
    !matches!(rest[last_newline + 1], b' ' | b'\t')
}

/// The recipe bodies of a justfile, the classes of its other lines and the
/// offset of its `script-interpreter` setting's command list.
fn read(text: &[u8], lines: &[Range<usize>]) -> (Vec<LineClass>, Vec<Body>, Option<usize>) {
    let mut reader = Reader {
        text,
        lines,
        line: 0,
        holds: vec![Holds::default(); lines.len()],
        pending: false,
        attribute: None,
        setting: None,
    };
    let mut bodies = Vec::new();
    let mut index = 0;
    while index < lines.len() {
        let line = lines[index].clone();
        let bytes = &text[line.clone()];
        let blanks = leading_blanks(bytes);
        let blank = bytes[blanks..].iter().all(u8::is_ascii_whitespace);
        if blank {
            index += 1;
            continue;
        }
        if blanks > 0 && reader.pending {
            // A body: the lines that keep its indentation, and the blank lines
            // between them that no dedenting line follows.
            let first = index;
            let indent = line.start..line.start + blanks;
            let mut end = index + 1;
            while end < lines.len() {
                let next = &text[lines[end].clone()];
                let next_blank = next.iter().all(u8::is_ascii_whitespace);
                if next_blank && dedents(text, lines[end].start)
                    || !next_blank && !next.starts_with(&text[indent.clone()])
                {
                    break;
                }
                end += 1;
            }
            // `Recipe::run_script` prefers an attribute to a `#!` line.
            let runner = match reader.attribute.take() {
                Some(command) => Runner::Script(command),
                None if bytes[blanks..].starts_with(b"#!") => Runner::Shebang,
                None => Runner::Lines,
            };
            bodies.push(Body {
                lines: first..end,
                indentation: indent,
                runner,
            });
            reader.pending = false;
            index = end;
            continue;
        }
        let stop = reader.statement(line.start + blanks);
        index = reader.line_of(stop.saturating_sub(1).max(line.start)) + 1;
    }
    let setting = reader.setting;
    (
        reader.holds.into_iter().map(Holds::class).collect(),
        bodies,
        setting,
    )
}

/// A body line's command after the sigils `Line::sigils` reads, each of
/// `-`, `@` and `?` at most once.
fn without_sigils(mut command: &[u8]) -> &[u8] {
    let mut seen = Vec::with_capacity(3);
    while let Some(&sigil @ (b'-' | b'@' | b'?')) = command.first() {
        if seen.contains(&sigil) {
            break;
        }
        seen.push(sigil);
        command = &command[1..];
    }
    command
}

/// The classes of a body run line by line by the shell.
fn linewise_classes(text: &[u8], lines: &[Range<usize>], indentation: usize) -> Vec<LineClass> {
    let mut continued_comment = false;
    lines
        .iter()
        .map(|line| {
            let bytes = text[line.clone()].trim_end();
            if bytes.iter().all(u8::is_ascii_whitespace) {
                continued_comment = false;
                return LineClass::Blank;
            }
            let command = without_sigils(&bytes[indentation.min(bytes.len())..]);
            let comment = continued_comment || command.trim_start().starts_with(b"#");
            continued_comment = comment && bytes.ends_with(b"\\");
            if comment {
                LineClass::Comment
            } else {
                LineClass::Code
            }
        })
        .collect()
}

/// Replaces each `{{…}}` interpolation of a script with `x` characters but
/// its newlines, since just evaluates them before the interpreter reads the
/// script; `{{{{` is an escaped `{{`.
fn blank_interpolations(script: &mut [u8]) {
    let mut at = 0;
    while let Some(found) = script[at..].windows(2).position(|pair| pair == b"{{") {
        let start = at + found;
        if script[start..].starts_with(b"{{{{") {
            at = start + 4;
            continue;
        }
        let mut end = start + 2;
        let mut quote = None;
        while end < script.len() && (quote.is_some() || !script[end..].starts_with(b"}}")) {
            match (quote, script[end]) {
                (None, q @ (b'\'' | b'"' | b'`')) => quote = Some(q),
                (Some(b'"'), b'\\') => end += 1,
                (Some(q), b) if b == q => quote = None,
                _ => {}
            }
            end += 1;
        }
        let end = (end + 2).min(script.len());
        for byte in &mut script[start..end] {
            if *byte != b'\n' {
                *byte = b'x';
            }
        }
        at = end;
    }
}

/// The quoted words of the command list that `text` starts, up to the
/// bracket or parenthesis that closes it.
fn command_words(text: &[u8]) -> Vec<&str> {
    let mut words = Vec::new();
    let mut rest = text;
    while let Some(at) = rest
        .iter()
        .position(|b| matches!(b, b'\'' | b'"' | b')' | b']'))
    {
        let quote = rest[at];
        if !matches!(quote, b'\'' | b'"') {
            break;
        }
        let Some(end) = rest[at + 1..].iter().position(|&b| b == quote) else {
            break;
        };
        words.extend(std::str::from_utf8(&rest[at + 1..at + 1 + end]).ok());
        rest = &rest[at + 2 + end..];
    }
    words
}

/// The language of the script a command runs: that of its program, or of
/// the program `env` runs, as `#!/usr/bin/env` would name it; the shell's
/// when it names none Tokei knows.
fn command_language<'a>(mut words: impl Iterator<Item = &'a str>) -> LanguageType {
    let basename = |word: &'a str| word.rsplit('/').next().unwrap_or(word);
    let mut program = words.next().map(basename);
    if program == Some("env") {
        program = words.find(|word| !word.starts_with('-')).map(basename);
    }
    program
        .and_then(|program| LanguageType::from_shebang_line(&format!("#!/usr/bin/env {program}")))
        .unwrap_or(LanguageType::Sh)
}

/// The language a `#!` line names, by its program when Tokei knows no such
/// line.
fn shebang_language(line: &[u8]) -> LanguageType {
    let line = std::str::from_utf8(line.trim_end()).unwrap_or("#!");
    LanguageType::from_shebang_line(line)
        .unwrap_or_else(|| command_language(line[2..].split_whitespace()))
}

/// The classes of a script body, as its interpreter reads the script just
/// writes: the lines without the recipe's indentation.
fn script_classes(
    config: &Config,
    language: LanguageType,
    text: &[u8],
    lines: &[Range<usize>],
    indentation: &[u8],
) -> Vec<LineClass> {
    let mut script = Vec::with_capacity(lines.iter().map(|line| line.len()).sum());
    for line in lines {
        let bytes = &text[line.clone()];
        let unindented = bytes
            .strip_prefix(indentation)
            .unwrap_or(&bytes[leading_blanks(bytes)..]);
        // A last line of blanks keeps one, so that the script keeps the line.
        script.extend_from_slice(if unindented.is_empty() {
            b" "
        } else {
            unindented
        });
    }
    blank_interpolations(&mut script);
    language
        .line_classes(&script, config)
        .filter(|classes| classes.len() == lines.len())
        .unwrap_or_else(|| vec![LineClass::Code; lines.len()])
}

pub(crate) fn parse(
    config: &Config,
    text: &[u8],
    mut stats: CodeStats,
    counted: Range<usize>,
    on_line: &mut impl FnMut(LineReport<'_>),
) -> CodeStats {
    let mut lines = Vec::new();
    let mut stepper = LineStep::new(b'\n', 0, text.len());
    while let Some((start, end)) = stepper.next(text) {
        lines.push(start..end);
    }
    if lines.is_empty() {
        return stats;
    }
    let (mut classes, bodies, setting) = read(text, &lines);
    for body in bodies {
        let body_lines = &lines[body.lines.clone()];
        let language = match body.runner {
            Runner::Lines => None,
            Runner::Shebang => Some(shebang_language(
                &text[body.indentation.end..body_lines[0].end],
            )),
            Runner::Script(command) => Some(command.or(setting).map_or(LanguageType::Sh, |list| {
                command_language(command_words(&text[list..]).into_iter())
            })),
        };
        let body_classes = match language {
            None => linewise_classes(text, body_lines, body.indentation.len()),
            Some(language) => script_classes(
                config,
                language,
                text,
                body_lines,
                &text[body.indentation.clone()],
            ),
        };
        classes[body.lines].copy_from_slice(&body_classes);
    }
    for (line, class) in lines.iter().zip(classes) {
        if counted.contains(&line.start) {
            stats.add_line(class);
            on_line(LineReport::Class(Some(class)));
        }
    }
    stats
}
