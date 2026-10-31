//! BitBake recipes, classes, appends and includes, as BitBake 2.16 reads
//! them (`lib/bb/parse/parse_py/BBHandler.py`, which `lib/bb/parse/ast.py`
//! also runs on `.bbappend` files): each line with its trailing whitespace
//! stripped, a trailing backslash joining the next line to the statement. A
//! statement whose first character is `#` is a comment. A statement that
//! ends in `() {` opens a function whose body runs to a line that is exactly
//! `}`, run by Python when `python` comes first or the function has no
//! name and by the shell otherwise; one that
//! starts with `def` opens a Python function whose body runs while lines are
//! indented, empty or start with `#`. The bodies' lines are classed as their
//! interpreter reads them, and every other statement's lines are code, since
//! an assignment's quoted value must end its statement.

use std::ops::Range;

use grep_searcher::LineStep;

use crate::language::{language_type::LineReport, syntax::LineClass};
use crate::{utils::ext::SliceExt, CodeStats, Config, LanguageType};

/// A function body being read: its interpreter and first line.
#[derive(Clone, Copy)]
enum Body {
    /// A `name() {` function, closed by a line that is exactly `}`.
    Braced(LanguageType, usize),
    /// A `def` function, open while lines match `__python_func_regexp__`.
    Def(usize),
}

/// The interpreter of the function a statement opens, as
/// `__func_start_regexp__` finds it: `python` or `fakeroot` flags, then an
/// optional name of `[\w.\-+{}$:]` characters, then `()` and `{` at the end.
/// Python runs a function with the `python` flag and, as `MethodNode.eval`
/// in `lib/bb/parse/ast.py` makes it, one without a name.
fn function_start(statement: &[u8]) -> Option<LanguageType> {
    let mut rest = statement
        .strip_suffix(b"{")?
        .trim_end()
        .strip_suffix(b")")?
        .trim_end()
        .strip_suffix(b"(")?;
    let mut python = false;
    loop {
        let flag = [&b"python"[..], b"fakeroot"]
            .into_iter()
            .find(|flag| rest.starts_with(flag));
        let Some(flag) = flag else { break };
        // `python` may stand right before the parentheses, `fakeroot` not.
        match rest.get(flag.len()) {
            Some(next) if next.is_ascii_whitespace() => {}
            None if flag == b"python" => {}
            _ => break,
        }
        python |= flag == b"python";
        rest = rest[flag.len()..].trim_start();
    }
    let name = rest.trim_end();
    let is_name = |&b: &u8| b.is_ascii_alphanumeric() || b >= 0x80 || b"_.-+{}$:".contains(&b);
    name.iter()
        .all(is_name)
        .then_some(if python || name.is_empty() {
            LanguageType::Python
        } else {
            LanguageType::Sh
        })
}

/// Whether a statement opens a `def` function, as `__def_regexp__`,
/// `def\s+(\w+).*:`, matches it.
fn starts_def(statement: &[u8]) -> bool {
    let Some(rest) = statement.strip_prefix(b"def") else {
        return false;
    };
    let name = rest.trim_start();
    name.len() < rest.len()
        && name
            .first()
            .is_some_and(|&b| b.is_ascii_alphanumeric() || b == b'_' || b >= 0x80)
        && name[1..].contains(&b':')
}

/// Whether a stripped line continues a `def` body, as
/// `__python_func_regexp__`, `(\s+.*)|(^$)|(^#)`, matches it.
fn continues_def(line: &[u8]) -> bool {
    line.first()
        .map_or(true, |&b| b.is_ascii_whitespace() || b == b'#')
}

/// The class of each line of a statement outside functions.
fn statement_class(statement: &[u8]) -> LineClass {
    match statement.first() {
        None => LineClass::Blank,
        Some(b'#') => LineClass::Comment,
        Some(_) => LineClass::Code,
    }
}

/// The classes of a body's lines, as its interpreter reads them.
fn body_classes(
    config: &Config,
    language: LanguageType,
    text: &[u8],
    lines: &[Range<usize>],
) -> Vec<LineClass> {
    let Some((first, last)) = lines.first().zip(lines.last()) else {
        return Vec::new();
    };
    language
        .line_classes(&text[first.start..last.end], config)
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
    let mut classes = Vec::with_capacity(lines.len());
    let mut body: Option<Body> = None;
    // The statement read so far, joined, and its first line.
    let mut joined = Vec::new();
    let mut first = None;
    for (index, line) in lines.iter().enumerate() {
        let stripped = text[line.clone()].trim_end();
        match body {
            Some(Body::Braced(language, start)) => {
                if stripped == b"}" {
                    classes.extend(body_classes(config, language, text, &lines[start..index]));
                    classes.push(LineClass::Code);
                    body = None;
                }
                continue;
            }
            Some(Body::Def(_)) if continues_def(stripped) => continue,
            Some(Body::Def(start)) => {
                classes.extend(body_classes(
                    config,
                    LanguageType::Python,
                    text,
                    &lines[start..index],
                ));
                body = None;
            }
            None => {}
        }
        let start = *first.get_or_insert(index);
        if let Some(continued) = stripped.strip_suffix(b"\\") {
            joined.extend_from_slice(continued);
            continue;
        }
        joined.extend_from_slice(stripped);
        first = None;
        if starts_def(&joined) {
            body = Some(Body::Def(start));
        } else {
            let class = statement_class(&joined);
            classes.extend(std::iter::repeat(class).take(index + 1 - start));
            if class == LineClass::Code {
                body = function_start(&joined).map(|language| Body::Braced(language, index + 1));
            }
        }
        joined.clear();
    }
    match (body, first) {
        (Some(Body::Braced(language, start)), _) => {
            classes.extend(body_classes(config, language, text, &lines[start..]))
        }
        (Some(Body::Def(start)), _) => classes.extend(body_classes(
            config,
            LanguageType::Python,
            text,
            &lines[start..],
        )),
        (None, Some(start)) => {
            let class = statement_class(&joined);
            classes.extend(std::iter::repeat(class).take(lines.len() - start));
        }
        (None, None) => {}
    }
    for (line, class) in lines.iter().zip(classes) {
        if counted.contains(&line.start) {
            stats.add_line(class);
            on_line(LineReport::Class(Some(class)));
        }
    }
    stats
}
