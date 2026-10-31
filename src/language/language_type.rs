use std::{
    borrow::Cow,
    fmt,
    fs::File,
    io::{self, Read},
    path::{Path, PathBuf},
    str::FromStr,
};

use crate::{
    config::Config,
    language::{
        dialects::{bitbake, cobol, fortran, haml, just, liquid, markdown, pascal, plantuml, rpm},
        line_splices::LineSplices,
        syntax::{line_end, FileContext, LanguageContext, LineClass, SyntaxCounter},
    },
    stats::{CodeStats, Report},
    utils::{ext::SliceExt, fs as fsutils},
};

use encoding_rs_io::DecodeReaderBytesBuilder;
use grep_searcher::{LineIter, LineStep};
use once_cell::sync::Lazy;
use rayon::prelude::*;
use serde::Serialize;
use std::ops::Range;

use self::LanguageType::*;

include!(concat!(env!("OUT_DIR"), "/language_type.rs"));

impl Serialize for LanguageType {
    fn serialize<S>(&self, serializer: S) -> Result<S::Ok, S::Error>
    where
        S: serde::Serializer,
    {
        serializer.serialize_str(self.name())
    }
}

/// What `parse_lines` reports for each counted line.
pub(crate) enum LineReport<'a> {
    /// The line's class, or `None` when another embedded language counts it.
    Class(Option<LineClass>),
    /// The text of an element embedded in markup, from the end of its
    /// opening tag to the start of its closing tag, whose lines its language
    /// counts, or, for a data block, the host.
    Embedded {
        language: Option<LanguageType>,
        text: &'a [u8],
    },
}

/// A language whose own module reads a file whole, since the class of its
/// lines depends on the lines before them. Neither the generic engine nor its
/// shortcut for the lines before a file's first quote reads such a file, and
/// this is the one list that both consult.
#[derive(Clone, Copy)]
enum WholeFile {
    Markdown,
    Rpm,
    Cobol,
    Bitbake,
    Just,
    PlantUml,
    Liquid,
    Haml,
}

impl WholeFile {
    fn of(language: LanguageType) -> Option<Self> {
        match language {
            Markdown | UnrealDeveloperMarkdown => Some(Self::Markdown),
            RPMSpecfile => Some(Self::Rpm),
            Cobol => Some(Self::Cobol),
            Bitbake => Some(Self::Bitbake),
            Just => Some(Self::Just),
            PlantUml => Some(Self::PlantUml),
            Liquid => Some(Self::Liquid),
            Haml => Some(Self::Haml),
            _ => None,
        }
    }

    fn parse(
        self,
        config: &Config,
        lines: &[u8],
        stats: CodeStats,
        counted: Range<usize>,
        on_line: &mut impl FnMut(LineReport<'_>),
    ) -> CodeStats {
        match self {
            Self::Markdown => markdown::parse(config, lines, stats, counted, on_line),
            Self::Rpm => rpm::parse(config, lines, stats, counted, on_line),
            Self::Cobol => cobol::parse(config, lines, stats, counted, on_line),
            Self::Bitbake => bitbake::parse(config, lines, stats, counted, on_line),
            Self::Just => just::parse(config, lines, stats, counted, on_line),
            Self::PlantUml => plantuml::parse(config, lines, stats, counted, on_line),
            Self::Liquid => liquid::parse(config, lines, stats, counted, on_line),
            Self::Haml => haml::parse(config, lines, stats, counted, on_line),
        }
    }
}

/// The classes of the lines of a data block's `text` that the host counts:
/// its text is code, or blank where only whitespace.
fn data_line_classes(text: &[u8]) -> impl Iterator<Item = LineClass> + '_ {
    text[embedded_lines(text)]
        .split_inclusive(|b| *b == b'\n')
        .map(|line| {
            if line.trim().is_empty() {
                LineClass::Blank
            } else {
                LineClass::Code
            }
        })
}

/// The lines of an element's `text` that the element's language counts:
/// those after the line of its opening tag, up to the line of its closing
/// tag, which the host counts; none for an element on one line.
fn embedded_lines(text: &[u8]) -> Range<usize> {
    let after_first = text
        .iter()
        .position(|b| *b == b'\n')
        .map_or(text.len(), |n| n + 1);
    let last_start = text.iter().rposition(|b| *b == b'\n').map_or(0, |n| n + 1);
    after_first..last_start.max(after_first)
}

impl LanguageType {
    /// Parses a given [`Path`] using the [`LanguageType`]. Returning [`Report`]
    /// on success and giving back ownership of [`PathBuf`] on error.
    pub fn parse(self, path: PathBuf, config: &Config) -> Result<Report, (io::Error, PathBuf)> {
        let text = {
            let f = match File::open(&path) {
                Ok(f) => f,
                Err(e) => return Err((e, path)),
            };
            let mut s = Vec::new();
            let mut reader = DecodeReaderBytesBuilder::new().build(f);

            if let Err(e) = reader.read_to_end(&mut s) {
                return Err((e, path));
            }
            s
        };

        let mut stats = Report::new(path);

        stats += self.parse_from_slice(text, config);

        Ok(stats)
    }

    /// Parses the text provided as the given [`LanguageType`].
    pub fn parse_from_str<A: AsRef<str>>(self, text: A, config: &Config) -> CodeStats {
        self.parse_from_slice(text.as_ref().as_bytes(), config)
    }

    /// Parses the bytes provided as the given [`LanguageType`].
    pub fn parse_from_slice<A: AsRef<[u8]>>(self, text: A, config: &Config) -> CodeStats {
        let text = text.as_ref();

        if self == Jupyter {
            return self
                .parse_jupyter(text.as_ref(), config)
                .unwrap_or_default();
        }

        let syntax = self.syntax_counter_for(text);

        // The lines before the first important syntax are classified one by
        // one, which holds only where no earlier line decides a later one and
        // the search finds every construct; the guard that counts every
        // fixture from each of its lines checks it. It does not hold for a
        // YAML line, which can continue the plain scalar before it; a shebang
        // or frontmatter, read by the full parse only; a line before PHP's
        // code tags, which is output text; a language read whole by its own
        // module, such as Markdown, whose code blocks depend on the list items
        // that hold their fences; a MASM `COMMENT` block, which opens with a
        // word in any case that the case-sensitive search cannot find; or a
        // Visual Basic XML literal, which only the code before its `<` opens.
        let simple_prefix = !syntax.shared.block_scalars
            && WholeFile::of(self).is_none()
            && !syntax.shared.masm_comments
            && !syntax.shared.xml_literals
            && syntax.shared.code_tags.is_none()
            && !syntax.opens_with_whole_lines(text);
        if let Some(end) = simple_prefix
            .then(|| syntax.shared.important_syntax.as_ref()?.find(text))
            .flatten()
            .and_then(|m| {
                // Get the position of the last line before the important
                // syntax.
                text[..=m.start()]
                    .iter()
                    .rev()
                    .position(|&c| c == b'\n')
                    .filter(|&p| p != 0)
                    .map(|p| m.start() - p)
            })
        {
            let (skippable_text, rest) = text.split_at(end + 1);
            let shared = syntax.shared.clone();
            let fixed_form = syntax.fixed_form;
            trace!(
                "Using Simple Parse on {:?}",
                String::from_utf8_lossy(skippable_text)
            );
            let parse_lines = move || {
                self.parse_lines(
                    config,
                    rest,
                    CodeStats::new(),
                    syntax,
                    0..rest.len(),
                    &mut |_| {},
                )
            };
            let simple_parse = move || {
                LineIter::new(b'\n', skippable_text)
                    .par_bridge()
                    .map(|line| {
                        let class = fixed_form
                            .then(|| fortran::line_class(line))
                            .flatten()
                            .unwrap_or_else(|| shared.plain_line_class(line.trim()));
                        match class {
                            LineClass::Blank => (1, 0, 0),
                            LineClass::Code => (0, 1, 0),
                            LineClass::Comment => (0, 0, 1),
                        }
                    })
                    .reduce(|| (0, 0, 0), |a, b| (a.0 + b.0, a.1 + b.1, a.2 + b.2))
            };

            let (mut stats, (blanks, code, comments)) = rayon::join(parse_lines, simple_parse);

            stats.blanks += blanks;
            stats.code += code;
            stats.comments += comments;
            stats
        } else {
            self.parse_lines(
                config,
                text,
                CodeStats::new(),
                syntax,
                0..text.len(),
                &mut |_| {},
            )
        }
    }

    /// The class of each line of `text`, in order, as `parse_from_slice`
    /// counts it, so that a reference lexer can be compared with it line by
    /// line.
    ///
    /// `None` when lines are counted elsewhere: for Jupyter notebooks, and for
    /// a Markdown text with a code block in a named language, whose lines that
    /// language counts. The lines of a script or style element embedded in
    /// markup take the classes its own language gives them.
    pub fn line_classes(self, text: &[u8], config: &Config) -> Option<Vec<LineClass>> {
        if self == Jupyter {
            return None;
        }
        let syntax = self.syntax_counter_for(text);
        // The simple parse of the lines before the first important syntax is
        // a shortcut that counts them as this full parse does.
        self.collect_line_classes(config, text, syntax, 0..text.len())
    }

    /// The classes of the lines of `text` that start within `counted`, with
    /// those of elements embedded in markup as their own language counts
    /// them; `None` when another embedded language, such as a Markdown code
    /// block, counts lines.
    fn collect_line_classes(
        self,
        config: &Config,
        text: &[u8],
        syntax: SyntaxCounter,
        counted: Range<usize>,
    ) -> Option<Vec<LineClass>> {
        let mut classes = Some(Vec::new());
        self.parse_lines(
            config,
            text,
            CodeStats::new(),
            syntax,
            counted,
            &mut |report| {
                let Some(list) = classes.as_mut() else {
                    return;
                };
                let complete = match report {
                    LineReport::Class(Some(class)) => {
                        list.push(class);
                        true
                    }
                    LineReport::Class(None) => false,
                    LineReport::Embedded {
                        language: Some(language),
                        text,
                    } => language
                        .collect_line_classes(
                            config,
                            text,
                            SyntaxCounter::new(language),
                            embedded_lines(text),
                        )
                        .map(|lines| list.extend(lines))
                        .is_some(),
                    LineReport::Embedded {
                        language: None,
                        text,
                    } => {
                        list.extend(data_line_classes(text));
                        true
                    }
                };
                if !complete {
                    classes = None;
                }
            },
        );
        classes
    }

    /// Parses the text of an element embedded in markup, from the end of its
    /// opening tag to the start of its closing tag. The host counts the lines
    /// that hold the tags, so only the lines between them are counted here;
    /// the rest of the opening tag's line is still read, so a comment or
    /// string opened there carries on.
    pub(crate) fn parse_embedded(self, text: &[u8], config: &Config) -> CodeStats {
        let syntax = SyntaxCounter::new(self);
        self.parse_lines(
            config,
            text,
            CodeStats::new(),
            syntax,
            embedded_lines(text),
            &mut |_| {},
        )
    }

    /// Counts the lines of `lines` that start within `counted`; the others
    /// are read only for the state they leave. `on_line` receives the class
    /// of each counted line, `None` for lines another embedded language
    /// counts, and the text of each element embedded in markup after the
    /// class of the line that opens it.
    #[inline]
    fn parse_lines(
        self,
        config: &Config,
        lines: &[u8],
        mut stats: CodeStats,
        mut syntax: SyntaxCounter,
        mut counted: Range<usize>,
        on_line: &mut impl FnMut(LineReport<'_>),
    ) -> CodeStats {
        if let Some(dialect) = WholeFile::of(self) {
            return dialect.parse(config, lines, stats, counted, on_line);
        }
        let mut stepper = LineStep::new(b'\n', 0, lines.len());
        let splices_continue_lines = syntax.shared.line_splices == Some(LineSplices::C)
            && !syntax.shared.line_start_comments.is_empty();
        let mut previous = None;

        while let Some((start, end)) = stepper.next(lines) {
            syntax.continues_line = splices_continue_lines
                && previous.is_some_and(|previous| LineSplices::C.splices(&lines[previous..start]));
            previous = Some(start);
            if syntax.shared.quotes_close_at_line_end && syntax.quote.is_some() {
                syntax.end_quote();
            }
            let before = (stats.code, stats.comments, stats.blanks);
            let mut embedded = false;
            let mut element = None;
            let is_counted = counted.contains(&start);
            let whole_line = syntax.whole_line_class(lines, start, end);
            let reads_line = whole_line.is_none() && syntax.quote.is_none();
            'line: {
                if let Some(class) = whole_line {
                    stats.add_line(class);
                    trace!("{:?} No.{} as a whole line", class, stats.lines());
                    // A literal that closes inside this line leaves the rest
                    // of it to be read.
                    if (start + 1..end).contains(&syntax.literal_end) {
                        syntax.line_has_code = true;
                        let rest = syntax.literal_end;
                        syntax.perform_multi_line_analysis(lines, rest, end, config);
                    }
                    break 'line;
                }
                let line = lines[start..end].trim();
                trace!("{}", String::from_utf8_lossy(line));

                if syntax.try_perform_single_line_analysis(lines, line, &mut stats) {
                    break 'line;
                }

                syntax.line_has_code = false;
                let started_in_literal = syntax.quote.is_some() || syntax.in_jsx_text();
                let started_in_comments = !syntax.stack.is_empty()
                    || (config.treat_doc_strings_as_comments == Some(true)
                        && syntax.quote.is_some()
                        && syntax.quote_is_doc_quote);
                let ended_with_comments =
                    match syntax.perform_multi_line_analysis(lines, start, end, config) {
                        crate::language::syntax::AnalysisReport::Normal(end) => end,
                        crate::language::syntax::AnalysisReport::ChildLanguage(FileContext {
                            language,
                            end,
                            stats: blob,
                        }) => {
                            match language {
                                LanguageContext::LinguaFranca => {
                                    let child_lang = syntax.get_lf_target_language();
                                    *stats.blobs.entry(child_lang).or_default() += blob;
                                }
                                LanguageContext::Html {
                                    language,
                                    start: text_start,
                                } => {
                                    stats.code += 1;
                                    let text = &lines[text_start..end];
                                    element = Some((language, text));
                                    match language {
                                        Some(language) => {
                                            *stats.blobs.entry(language).or_default() += blob;
                                        }
                                        None => {
                                            for class in data_line_classes(text) {
                                                match class {
                                                    LineClass::Blank => stats.blanks += 1,
                                                    _ => stats.code += 1,
                                                }
                                            }
                                        }
                                    }
                                    // A closing tag on this line leaves the rest
                                    // of it, already counted, to be read.
                                    let opening_line_end = line_end(lines, start);
                                    if end <= opening_line_end {
                                        counted.start = counted.start.max(opening_line_end + 1);
                                    }
                                }
                            }

                            embedded = true;
                            // Advance to after the language code and the delimiter..
                            stepper = LineStep::new(b'\n', end, lines.len());
                            break 'line;
                        }
                    };
                trace!("{}", String::from_utf8_lossy(line));

                if syntax.shared.is_literate
                    || syntax.line_is_comment(
                        line,
                        config,
                        ended_with_comments,
                        started_in_comments,
                        started_in_literal,
                    )
                {
                    stats.comments += 1;
                    trace!("Comment No.{}", stats.comments);
                    trace!("Was the Comment stack empty?: {}", !started_in_comments);
                } else {
                    stats.code += 1;
                    trace!("Code No.{}", stats.code);
                }
            }
            // Only a block scalar's plain scalar continues past its line.
            if reads_line && syntax.shared.block_scalars {
                syntax.end_line(lines, start);
            }
            if !is_counted {
                (stats.code, stats.comments, stats.blanks) = before;
            } else if let Some((language, text)) = element {
                on_line(LineReport::Class(Some(LineClass::Code)));
                on_line(LineReport::Embedded { language, text });
            } else {
                on_line(LineReport::Class(
                    match (
                        stats.code - before.0,
                        stats.comments - before.1,
                        stats.blanks - before.2,
                        embedded,
                    ) {
                        (1, 0, 0, false) => Some(LineClass::Code),
                        (0, 1, 0, false) => Some(LineClass::Comment),
                        (0, 0, 1, false) => Some(LineClass::Blank),
                        _ => None,
                    },
                ));
            }
        }

        stats
    }

    fn parse_jupyter(&self, json: &[u8], config: &Config) -> Option<CodeStats> {
        #[derive(Deserialize)]
        struct Jupyter {
            cells: Vec<JupyterCell>,
            metadata: JupyterMetadata,
        }

        #[derive(Clone, Copy, Deserialize, PartialEq, Eq)]
        #[serde(rename_all = "lowercase")]
        enum CellType {
            Markdown,
            Code,
        }

        #[derive(Deserialize)]
        struct JupyterCell {
            cell_type: CellType,
            source: Vec<String>,
        }

        #[derive(Deserialize)]
        struct JupyterMetadata {
            kernelspec: serde_json::Value,
            language_info: serde_json::Value,
        }

        let jupyter: Jupyter = serde_json::from_slice(json).ok()?;

        let mut jupyter_stats = CodeStats::new();

        let language = jupyter
            .metadata
            .kernelspec
            .get("language")
            .and_then(serde_json::Value::as_str)
            .and_then(|v| LanguageType::from_str(v).ok())
            .or_else(|| {
                jupyter
                    .metadata
                    .language_info
                    .get("file_extension")
                    .and_then(serde_json::Value::as_str)
                    .and_then(LanguageType::from_file_extension)
            })
            .unwrap_or(LanguageType::Python);

        let iter = jupyter
            .cells
            .par_iter()
            .map(|cell| match cell.cell_type {
                CellType::Markdown => (
                    LanguageType::Markdown,
                    LanguageType::Markdown.parse_from_str(cell.source.join(""), config),
                ),
                CellType::Code => (
                    language,
                    language.parse_from_str(cell.source.join(""), config),
                ),
            })
            .collect::<Vec<_>>();

        for (language, stats) in iter {
            *jupyter_stats.blobs.entry(language).or_default() += &stats;
            jupyter_stats += &stats;
        }

        Some(jupyter_stats)
    }

    /// The syntax counter for reading `text` as this language, with the
    /// settings its content decides: the target language of a LinguaFranca
    /// file, the source form of a fixed-form Fortran one, and whether the
    /// comments of a Pascal one nest.
    fn syntax_counter_for(self, text: &[u8]) -> SyntaxCounter {
        let mut syntax = SyntaxCounter::new(self);
        if self == LinguaFranca {
            syntax.lf_embedded_language = self.find_lf_target_language(text);
        }
        if syntax.fixed_form {
            syntax.fixed_form = !fortran::is_free_form(text);
        }
        if self == Pascal {
            syntax.comments_nest = pascal::nested_comments_close(text);
        }
        syntax
    }

    /// The embedded language in LF is declared in a construct that looks like this: `target C;`, `target Python`.
    /// This is the first thing in the file (although there may be comments before).
    fn find_lf_target_language(&self, bytes: &[u8]) -> Option<LanguageType> {
        use regex::bytes::Regex;
        static LF_TARGET_REGEX: Lazy<Regex> =
            Lazy::new(|| Regex::new(r#"(?m)\btarget\s+(\w+)\s*($|;|\{)"#).unwrap());
        LF_TARGET_REGEX.captures(bytes).and_then(|captures| {
            let name = captures.get(1).unwrap().as_bytes();
            if name == b"CCpp" {
                // this is a special alias for the C target in LF
                Some(C)
            } else {
                let name_str = &String::from_utf8_lossy(name);
                let by_name = LanguageType::from_name(name_str);
                if by_name.is_none() {
                    trace!("LF target not recognized: {}", name_str);
                }
                by_name
            }
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    use std::{fs, path::Path};

    #[test]
    fn rust_allows_nested() {
        assert!(LanguageType::Rust.allows_nested());
    }

    fn assert_stats(stats: &CodeStats, blanks: usize, code: usize, comments: usize) {
        assert_eq!(stats.blanks, blanks, "expected {} blank lines", blanks);
        assert_eq!(stats.code, code, "expected {} code lines", code);
        assert_eq!(
            stats.comments, comments,
            "expected {} comment lines",
            comments
        );
    }

    #[test]
    fn jupyter_notebook_has_correct_totals() {
        let sample_notebook =
            fs::read_to_string(Path::new("tests").join("data").join("jupyter.ipynb")).unwrap();

        let stats = LanguageType::Jupyter
            .parse_jupyter(sample_notebook.as_bytes(), &Config::default())
            .unwrap();

        assert_stats(&stats, 115, 528, 333);
    }

    /// The language `from_path` detects for a file `name` holding `text`.
    fn detect(name: &str, text: &str) -> Option<LanguageType> {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join(name);
        fs::write(&path, text).unwrap();
        LanguageType::from_path(&path, &Config::default())
    }

    #[test]
    fn only_configure_scripts_are_autoconf_not_every_template() {
        assert_eq!(
            detect("configure.ac", "AC_INIT([x], [1])\n"),
            Some(Autoconf)
        );
        assert_eq!(detect("configure.in", "AC_INIT(x, 1)\n"), Some(Autoconf));
        for template in [
            "Makefile.in",
            "Config.in",
            "MANIFEST.in",
            "foo.cmake.in",
            "config.h.in",
        ] {
            assert_eq!(detect(template, "@VERSION@\n"), None, "{template}");
        }
    }

    #[test]
    fn bazel_file_names_match_in_their_own_case() {
        assert_eq!(detect("BUILD", "cc_library(name = \"x\")\n"), Some(Bazel));
        assert_eq!(
            detect("WORKSPACE", "workspace(name = \"x\")\n"),
            Some(Bazel)
        );
        assert_eq!(
            detect("MODULE.bazel", "module(name = \"x\")\n"),
            Some(Bazel)
        );
        // A lowercase `build` is usually a script, as in ShellCheck's `builders/`.
        assert_eq!(detect("build", "#!/bin/sh\nmake\n"), Some(Sh));
        assert_eq!(detect("module", "x\n"), None);
    }

    #[test]
    fn just_reads_justfile_and_dot_justfile_in_any_case() {
        // just 1.59.0, `JUSTFILE_NAMES` in `src/search.rs`, compared ignoring case.
        for name in ["justfile", "Justfile", ".justfile", ".Justfile"] {
            assert_eq!(
                detect(name, "build:\n    cc main.c\n"),
                Some(Just),
                "{name}"
            );
        }
    }

    #[test]
    fn meson_reads_options_from_meson_options_since_1_1() {
        // https://mesonbuild.com/Build-options.html
        assert_eq!(
            detect("meson.options", "option('x', type: 'string')\n"),
            Some(Meson)
        );
        assert_eq!(
            detect("meson_options.txt", "option('x', type: 'string')\n"),
            Some(Meson)
        );
    }

    #[test]
    fn a_spec_file_is_rpm_only_with_an_rpm_preamble_and_section() {
        let rpm = "Name: x\nVersion: 1\nSummary: s\n\n%description\nText.\n";
        assert_eq!(detect("x.spec", rpm), Some(RPMSpecfile));
        // PostgreSQL's isolation tester reads `.spec` files too.
        let isolation = "setup { CREATE TABLE t (a int); }\nsession s1\nstep s1a { SELECT 1; }\n";
        assert_eq!(detect("lock.spec", isolation), None);
    }

    #[test]
    fn an_m4_file_is_autoconf_when_it_uses_autoconf_or_m4sugar_macros() {
        for (name, text) in [
            ("ax_check.m4", "AC_DEFUN([AX_CHECK], [true])\n"),
            ("version.m4", "m4_define([VERSION], [1.0])\n"),
            ("prereq.m4", "AC_PREREQ([2.69])\n"),
        ] {
            assert_eq!(detect(name, text), Some(Autoconf), "{name}");
        }
        assert_eq!(detect("plain.m4", "define(`greet', `Hello')\n"), Some(M4));
    }

    #[test]
    fn an_inc_file_is_bitbake_only_with_bitbake_syntax() {
        for (name, text) in [
            ("a.inc", "SUMMARY = \"x\"\nLICENSE = \"MIT\"\n"),
            ("b.inc", "inherit autotools pkgconfig\n"),
            ("c.inc", "require recipes-core/foo.inc\n"),
            ("d.inc", "do_install:append() {\n}\n"),
            ("e.inc", "python do_patch() {\n}\n"),
        ] {
            assert_eq!(detect(name, text), Some(Bitbake), "{name}");
        }
        for (name, text) in [
            (
                "mg_names.inc",
                "/* -*- buffer-read-only: t -*- */\n#define X 1\n",
            ),
            ("t.inc", "<?php\nrequire 'x.php';\n$a = \"b\";\n"),
            ("version.inc", "VERSION=\"1.0\"\n"),
            ("config.inc", "config.name = \"x\"\n"),
        ] {
            assert_eq!(detect(name, text), None, "{name}");
        }
    }

    #[test]
    fn a_rules_file_is_snakemake_only_with_snakemake_directives() {
        assert_eq!(
            detect("tex.rules", "rule pdf:\n    input: \"x.tex\"\n"),
            Some(Snakemake)
        );
        assert_eq!(
            detect("test.rules", "configfile: \"x.json\"\n"),
            Some(Snakemake)
        );
        assert_eq!(
            detect("90-kvm.rules", "KERNEL==\"kvm\", GROUP=\"kvm\"\n"),
            None
        );
        assert_eq!(
            detect("p.rules", "polkit.addRule(function(action, subject) {});\n"),
            None
        );
    }

    #[test]
    fn a_v_file_is_coq_or_verilog_by_its_text() {
        assert_eq!(
            detect(
                "one.v",
                "Require Import Arith.\n\nLemma one : 1 = 1.\nProof. reflexivity. Qed.\n"
            ),
            Some(Coq)
        );
        assert_eq!(
            detect(
                "top.v",
                "`timescale 1ns/1ps\nmodule top (input clk);\nendmodule\n"
            ),
            Some(Verilog)
        );
        // V (vlang.io) writes `.v` files too, and Tokei has no V definition.
        assert_eq!(
            detect("main.v", "module main\n\nfn main() {\n\tprintln('hi')\n}\n"),
            None
        );
    }

    #[test]
    fn an_fs_file_is_forth_or_glsl_by_its_text_and_otherwise_fsharp() {
        assert_eq!(
            detect("sq.fs", ": square ( n -- n*n ) dup * ;\n"),
            Some(Forth)
        );
        assert_eq!(
            detect(
                "frag.fs",
                "#version 330 core\nout vec4 color;\nvoid main() { color = vec4(1.0); }\n"
            ),
            Some(Glsl)
        );
        assert_eq!(
            detect("Program.fs", "module Program\nlet x = 1\n"),
            Some(FSharp)
        );
    }

    #[test]
    fn an_e_file_is_phix_or_specman_by_its_text() {
        assert_eq!(
            detect(
                "app.e",
                "include std/console.e\nglobal function f()\n    return 1\nend function\n"
            ),
            Some(Phix)
        );
        assert_eq!(
            detect("packet.e", "<'\nstruct packet {\n};\n'>\n"),
            Some(SpecmanE)
        );
        // Eiffel classes are `.e` files too, and Tokei has no Eiffel definition.
        assert_eq!(
            detect(
                "application.e",
                "class\n\tAPPLICATION\ncreate\n\tmake\nfeature\n\tmake\n\t\tdo\n\t\tend\nend\n"
            ),
            None
        );
    }

    #[test]
    fn an_mc_file_is_m4_or_monkey_c_by_its_text() {
        assert_eq!(
            detect("sendmail.mc", "divert(-1)\ndefine(`VERSION', `1.0')\n"),
            Some(M4)
        );
        assert_eq!(
            detect(
                "View.mc",
                "import Toybox.WatchUi;\n\nclass View extends WatchUi.View {\n}\n"
            ),
            Some(MonkeyC)
        );
        // Windows message texts, compiled by mc.exe.
        assert_eq!(
            detect("msg.mc", "MessageIdTypedef=DWORD\nMessageId=0x1\nSymbolicName=MSG_X\nLanguage=English\nText.\n.\n"),
            None
        );
    }

    #[test]
    fn pl_and_pro_files_are_prolog_only_with_prolog_clauses() {
        assert_eq!(
            detect("m.pl", ":- module(m, [p/1]).\np(X) :- q(X).\n"),
            Some(Prolog)
        );
        assert_eq!(detect("family.pro", "parent(tom, bob).\n"), Some(Prolog));
        assert_eq!(
            detect("x.pl", "use strict;\nmy $x = 1;\nprint \"done.\\n\";\n"),
            Some(Perl)
        );
        // A sentence of POD prose is no Prolog fact, as perl5's own scripts show.
        assert_eq!(
            detect("y.pl", "=pod\n\notherwise.\n\n=cut\nprint 1;\n"),
            Some(Perl)
        );
        // QMake projects and ProGuard rules are `.pro` files too.
        assert_eq!(detect("app.pro", "QT += core\nSOURCES += main.cpp\n"), None);
        assert_eq!(
            detect("proguard-rules.pro", "-keep class com.example.** { *; }\n"),
            None
        );
    }

    #[test]
    fn a_qt_translation_ts_file_is_xml() {
        assert_eq!(
            detect(
                "app_de.ts",
                "<?xml version=\"1.0\" encoding=\"utf-8\"?>\n<!DOCTYPE TS>\n<TS version=\"2.1\" language=\"de\">\n</TS>\n"
            ),
            Some(Xml)
        );
        assert_eq!(
            detect("main.ts", "const x: number = 1;\n"),
            Some(TypeScript)
        );
    }

    #[test]
    fn a_tpl_file_is_pan_only_with_a_template_declaration() {
        assert_eq!(
            detect("schema.tpl", "structure template components/foo/schema;\n"),
            Some(Pan)
        );
        assert_eq!(detect("index.tpl", "{if $x}<p>{$x}</p>{/if}\n"), None);
        assert_eq!(
            detect(
                "_helpers.tpl",
                "{{- define \"x.name\" -}}\n{{ .Chart.Name }}\n{{- end }}\n"
            ),
            None
        );
    }

    #[test]
    fn an_m_file_is_objective_c_only_with_objective_c_syntax() {
        assert_eq!(
            detect(
                "A.m",
                "#import <Foundation/Foundation.h>\n@implementation A\n@end\n"
            ),
            Some(ObjectiveC)
        );
        // A unity build, as protobuf's GPBProtocolBuffers.m, imports only `.m` files.
        assert_eq!(
            detect(
                "All.m",
                "#import \"GPBArray.m\"\n#import \"GPBMessage.m\"\n"
            ),
            Some(ObjectiveC)
        );
        // MATLAB and Mercury write `.m` files too.
        assert_eq!(
            detect("f.m", "function y = f(x)\n  y = x + 1; % add\nend\n"),
            None
        );
        assert_eq!(detect("hello.m", ":- module hello.\n:- interface.\n"), None);
    }

    #[test]
    fn a_pp_file_is_pascal_with_pascal_syntax_and_otherwise_puppet() {
        assert_eq!(
            detect(
                "hello.pp",
                "program Hello;\nbegin\n  writeln('Hi');\nend.\n"
            ),
            Some(Pascal)
        );
        assert_eq!(
            detect(
                "init.pp",
                "class ntp {\n  package { 'ntp': ensure => installed }\n}\n"
            ),
            Some(Puppet)
        );
    }

    #[test]
    fn an_srt_subtitle_file_is_text_and_otherwise_srecode() {
        assert_eq!(
            detect("movie.srt", "1\n00:00:01,000 --> 00:00:02,500\nHello.\n\n"),
            Some(Text)
        );
        assert_eq!(
            detect(
                "x.srt",
                ";; x.srt\nset mode \"c-mode\"\ncontext file\ntemplate empty\n----\n----\n"
            ),
            Some(SRecode)
        );
    }

    #[test]
    fn lf_embedded_language_is_counted() {
        let file_text =
            fs::read_to_string(Path::new("tests").join("data").join("linguafranca.lf")).unwrap();

        let stats = LinguaFranca.parse_from_str(file_text, &Config::default());

        assert_stats(&stats, 9, 11, 8);

        assert_eq!(stats.blobs.len(), 1, "num embedded languages");
        let rust_stats = stats.blobs.get(&Rust).expect("should have a Rust entry");
        assert_stats(rust_stats, 2, 5, 1);
    }
}
