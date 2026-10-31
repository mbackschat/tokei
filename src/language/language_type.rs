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
        dialects::{bitbake, cobol, fortran, just, markdown, pascal, rpm},
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

        // A YAML line can continue the plain scalar of the line before it, so
        // no YAML line can be classified on its own; a shebang or frontmatter
        // is read by the full parse only, and so is a line before PHP's code
        // tags, which is output text. Whether a Markdown line opens a code
        // block depends on the lines before it, such as the list item that
        // holds its fence, so a Markdown file is read whole, and so is a COBOL
        // file, whose comment paragraphs and debugging lines depend on lines
        // before them.
        let simple_prefix = !syntax.shared.block_scalars
            && !matches!(self, Markdown | UnrealDeveloperMarkdown | Cobol)
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
        if matches!(self, Markdown | UnrealDeveloperMarkdown) {
            return markdown::parse(config, lines, stats, counted, on_line);
        }
        if self == RPMSpecfile {
            return rpm::parse(config, lines, stats, counted, on_line);
        }
        if self == Cobol {
            return cobol::parse(config, lines, stats, counted, on_line);
        }
        if self == Bitbake {
            return bitbake::parse(config, lines, stats, counted, on_line);
        }
        if self == Just {
            return just::parse(config, lines, stats, counted, on_line);
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
