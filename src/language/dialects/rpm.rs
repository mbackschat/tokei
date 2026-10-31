//! RPM spec files' sections, as rpmbuild reads them (rpm 6,
//! `build/parseSpec.cc`): a section starts at a line whose start names one of
//! `partList`'s parts, in any case, followed by a blank or the line's end.
//! The preamble and the `%description`, `%files`, `%changelog` and list
//! sections are read with `STRIP_COMMENTS`, so a line whose first non-blank
//! character is `#` is a comment and quotes are text; the build scripts and
//! scriptlets are handed to the shell, or to Lua for `-p <lua>`, without the
//! `%if`, `%else`, `%endif` and `%include` lines rpm consumes, so their lines
//! are classed as that interpreter reads them.

use std::ops::Range;

use crate::language::{language_type::LineReport, syntax::LineClass};
use crate::{utils::ext::SliceExt, CodeStats, Config, LanguageType};

/// The parts of `partList` whose lines rpm reads as text rather than run.
const TEXT_PARTS: &[&[u8]] = &[
    b"%package",
    b"%files",
    b"%changelog",
    b"%description",
    b"%sepolicy",
    b"%end",
    b"%patchlist",
    b"%sourcelist",
];

/// The parts of `partList` whose lines an interpreter runs.
const SCRIPT_PARTS: &[&[u8]] = &[
    b"%prep",
    b"%generate_buildrequires",
    b"%conf",
    b"%build",
    b"%install",
    b"%check",
    b"%clean",
    b"%preun",
    b"%postun",
    b"%pretrans",
    b"%posttrans",
    b"%preuntrans",
    b"%postuntrans",
    b"%pre",
    b"%post",
    b"%triggerpostun",
    b"%triggerprein",
    b"%triggerun",
    b"%triggerin",
    b"%trigger",
    b"%verifyscript",
    b"%filetriggerin",
    b"%filetrigger",
    b"%filetriggerun",
    b"%filetriggerpostun",
    b"%transfiletriggerin",
    b"%transfiletrigger",
    b"%transfiletriggerun",
    b"%transfiletriggerpostun",
];

/// rpm's `lineTypes` directives: those `ISMACRO` matches, followed by no
/// letter, and those `ISMACROWITHARG` matches, followed by a blank or the
/// line's end.
const DIRECTIVES: &[&[u8]] = &[b"%endif", b"%else"];
const DIRECTIVES_WITH_ARGUMENTS: &[&[u8]] = &[
    b"%if",
    b"%ifarch",
    b"%ifnarch",
    b"%ifos",
    b"%ifnos",
    b"%include",
    b"%elifarch",
    b"%elifos",
    b"%elif",
];

/// Whether `name` starts `line`, in any case, followed by a byte `ends` accepts
/// or the line's end.
fn starts_with_name(line: &[u8], name: &[u8], ends: impl Fn(u8) -> bool) -> bool {
    line.len() >= name.len()
        && line[..name.len()].eq_ignore_ascii_case(name)
        && line
            .get(name.len())
            .map_or(true, |&next| matches!(next, b'\r' | b'\n') || ends(next))
}

/// A section of a spec file.
#[derive(Clone, Copy)]
enum Section {
    /// Lines rpm reads as text.
    Text,
    /// Lines the given interpreter runs.
    Script(LanguageType),
}

/// The section a line starts, as `isPart` finds it.
fn section(line: &[u8]) -> Option<Section> {
    let named = |names: &[&[u8]]| {
        names
            .iter()
            .any(|name| starts_with_name(line, name, |next| next.is_ascii_whitespace()))
    };
    if named(TEXT_PARTS) {
        Some(Section::Text)
    } else if named(SCRIPT_PARTS) {
        let lua = line
            .split(|byte| byte.is_ascii_whitespace())
            .collect::<Vec<_>>()
            .windows(2)
            .any(|pair| pair[0] == b"-p" && pair[1] == b"<lua>");
        Some(Section::Script(if lua {
            LanguageType::Lua
        } else {
            LanguageType::Sh
        }))
    } else {
        None
    }
}

/// Whether rpm consumes `line` as a conditional or `%include` directive.
fn is_directive(line: &[u8]) -> bool {
    let line = line.trim_start();
    DIRECTIVES
        .iter()
        .any(|name| starts_with_name(line, name, |next| !next.is_ascii_alphabetic()))
        || DIRECTIVES_WITH_ARGUMENTS
            .iter()
            .any(|name| starts_with_name(line, name, |next| matches!(next, b' ' | b'\t')))
}

/// A script section's lines, as its interpreter classes them, with the
/// directive lines it never sees blanked for it and counted as code.
fn script_classes(
    config: &Config,
    language: LanguageType,
    text: &[u8],
    lines: &[Range<usize>],
) -> Vec<LineClass> {
    let mut view = Vec::with_capacity(lines.iter().map(|line| line.len()).sum());
    let mut directives = Vec::with_capacity(lines.len());
    for line in lines {
        let bytes = &text[line.clone()];
        let directive = is_directive(bytes);
        directives.push(directive);
        if directive {
            view.extend(bytes.iter().map(|&b| if b == b'\n' { b'\n' } else { b' ' }));
        } else {
            view.extend_from_slice(bytes);
        }
    }
    let mut classes = super::classes_or_code(config, language, &view, lines.len());
    for (class, directive) in classes.iter_mut().zip(directives) {
        if directive {
            *class = LineClass::Code;
        }
    }
    classes
}

/// The class of a line of a text section: a comment when its first
/// non-blank character is `#`, as `handleComments` strips it.
fn text_class(line: &[u8]) -> LineClass {
    match line.trim_start().first() {
        None => LineClass::Blank,
        Some(b'#') => LineClass::Comment,
        Some(_) => LineClass::Code,
    }
}

pub(crate) fn parse(
    config: &Config,
    text: &[u8],
    stats: CodeStats,
    counted: Range<usize>,
    on_line: &mut impl FnMut(LineReport<'_>),
) -> CodeStats {
    let lines: Vec<_> = super::lines(text).collect();
    let mut classes = Vec::with_capacity(lines.len());
    let mut script: Option<(LanguageType, usize)> = None;
    for (index, line) in lines.iter().enumerate() {
        let bytes = &text[line.clone()];
        match section(bytes) {
            Some(next) => {
                if let Some((language, first)) = script.take() {
                    classes.extend(script_classes(config, language, text, &lines[first..index]));
                }
                classes.push(LineClass::Code);
                script = match next {
                    Section::Text => None,
                    Section::Script(language) => Some((language, index + 1)),
                };
            }
            None if script.is_some() => {}
            None => classes.push(text_class(bytes)),
        }
    }
    if let Some((language, first)) = script {
        classes.extend(script_classes(config, language, text, &lines[first..]));
    }
    super::report(lines.into_iter().zip(classes), stats, counted, on_line)
}
