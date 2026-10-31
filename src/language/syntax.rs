use std::{borrow::Cow, sync::Arc};

use aho_corasick::AhoCorasick;
use dashmap::DashMap;
use log::Level::Trace;
use once_cell::sync::Lazy;

use super::blank_symbols::BlankSymbols;
use super::dialects::haskell::CodePragmas;
use super::dialects::haskell::QuasiQuoting;
use super::dialects::jsx::JsxFrame;
use super::dialects::perl::{pod_block_end, PodSyntax};
use super::dialects::php::CodeTags;
use super::dialects::rust::frontmatter_end;
use super::dialects::sql::DollarQuote;
use super::dialects::svelte::may_start_tag_comment;
use super::dialects::yaml::may_start_block_scalar;
use super::dialects::CommentLiterals;
use super::dialects::{batch, elixir, erlang, haskell, julia, m4, make, perl, ruby, visual_basic};
use super::embedding::{
    RegexCache, RegexFamily, ENDING_LF_BLOCK_REGEX, END_SCRIPT, END_STYLE, END_TEMPLATE,
};
use super::heredoc::Heredoc;
use super::interpolation::{Interpolation, MarkupExpressions, OpenHole};
use super::line_splices::LineSplices;
use super::quote_like::{delimited_len, PrefixCharacterSyntax, QuoteLikeSyntax};
use super::raw_delimiter::RawDelimiter;
use super::regex_literal::{is_word_byte, regex_literal_len, RegexContext};
use super::shebang::{shebang_len, ShebangSyntax};
use crate::LanguageType::LinguaFranca;
use crate::{
    stats::CodeStats,
    utils::ext::{utf8_len, SliceExt},
    Config, LanguageType,
};

/// Tracks the syntax of the language as well as the current state in the file.
/// Current has what could be consider three types of mode.
/// - `plain` mode: This is the normal state, blanks are counted as blanks,
///   string literals can trigger `string` mode, and comments can trigger
///   `comment` mode.
/// - `string` mode: This when the state machine is current inside a string
///   literal for a given language, comments cannot trigger `comment` mode while
///   in `string` mode.
/// - `comment` mode: This when the state machine is current inside a comment
///   for a given language, strings cannot trigger `string` mode while in
///   `comment` mode.
#[derive(Clone, Debug)]
pub(crate) struct SyntaxCounter {
    pub(crate) shared: Arc<SharedMatchers>,
    /// The closer of the current literal; owned when computed from a raw opener.
    pub(crate) quote: Option<Cow<'static, str>>,
    /// The escape of the current raw literal, such as Swift's `\#` in `#"…"#`.
    pub(crate) quote_escape: Option<String>,
    /// Whether the open quote's closer closes only at the start of a line.
    quote_closes_at_line_start: bool,
    /// The opener of the current literal, which selects its interpolations.
    pub(crate) quote_opener: &'static str,
    /// Strings suspended while the code of their holes is read, innermost last.
    pub(crate) holes: Vec<OpenHole>,
    /// The end of the last heredoc or block scalar body; lines before it are
    /// literal.
    pub(crate) literal_end: usize,
    /// The end of the last text section that spans lines, which ends a
    /// literal at `literal_end` whose lines of whitespace are blank.
    pub(crate) text_end: usize,
    /// The bracket quotes that the file's pragmas turn on, read at the
    /// first quasi-quote opener of a language that has them.
    pub(crate) quasi_quoting: Option<QuasiQuoting>,
    /// The end of the comment lines of a Perl POD block being read.
    comment_end: usize,
    /// Whether an end marker such as `__END__` has made the rest data.
    in_data_section: bool,
    /// Whether the text read is outside the code tags, as before PHP's
    /// `<?php`.
    outside_code: bool,
    /// The YAML flow collections, `[…]` and `{…}`, open at this point.
    pub(crate) flow_depth: usize,
    /// Whether a markup tag is open, in which quotes open strings.
    pub(crate) in_tag: bool,
    /// The delimiters of the dollar-quoted bodies read as SQL at this point,
    /// innermost last.
    pub(crate) dollar_bodies: Vec<Vec<u8>>,
    /// The JSX elements and expressions open at this point, innermost last.
    pub(crate) jsx: Vec<JsxFrame>,
    /// How far the makefile's `define` bodies have been read.
    pub(crate) make_defines: make::Defines,
    pub(crate) quote_is_doc_quote: bool,
    pub(crate) stack: Vec<Cow<'static, str>>,
    pub(crate) quote_is_verbatim: bool,
    pub(crate) lf_embedded_language: Option<LanguageType>,
    /// Whether a non-blank byte of the line being read lies outside every
    /// comment, which makes the line code.
    pub(crate) line_has_code: bool,
}

#[derive(Clone, Debug)]
pub(crate) struct FileContext {
    pub(crate) language: LanguageContext,
    pub(crate) stats: CodeStats,
    pub(crate) end: usize,
}

impl FileContext {
    pub fn new(language: LanguageContext, end: usize, stats: CodeStats) -> Self {
        Self {
            language,
            stats,
            end,
        }
    }
}

#[derive(Clone, Debug)]
pub(crate) enum LanguageContext {
    Html {
        /// `None` for a script that is a data block, whose text is the
        /// host's.
        language: Option<LanguageType>,
        /// Where the element's text starts, after its opening tag.
        start: usize,
    },
    LinguaFranca,
}

#[derive(Clone, Debug)]
pub(crate) struct SharedMatchers {
    pub language: LanguageType,
    pub allows_nested: bool,
    pub doc_quotes: &'static [(&'static str, &'static str)],
    /// None for a language without any: searching an automaton of no patterns
    /// still reads every byte, which took half the time of Linux's makefiles
    /// at `d89e1d1`, concatenated (measured 2026-10-30).
    pub important_syntax: Option<AhoCorasick>,
    #[allow(dead_code)]
    pub any_comments: &'static [&'static str],
    pub is_fortran: bool,
    pub is_literate: bool,
    pub line_comments: &'static [&'static str],
    pub line_start_comments: &'static [&'static str],
    pub line_comment_flags: &'static [u8],
    pub batch_remarks: bool,
    pub rem_comments: bool,
    pub comment_literals: Option<CommentLiterals>,
    pub postfix_primes: bool,
    pub xml_literals: bool,
    pub line_start_closers: &'static [&'static str],
    pub pod: Option<PodSyntax>,
    pub code_tags: Option<CodeTags>,
    pub frontmatter: bool,
    pub line_splices: Option<LineSplices>,
    pub shebang: Option<ShebangSyntax>,
    pub run_closers: &'static [&'static str],
    pub identifier_quote_prefix: Option<&'static str>,
    pub line_directives: &'static [&'static str],
    pub backslash_operator: bool,
    pub string_gaps: bool,
    pub escape: u8,
    pub quote_like: Option<QuoteLikeSyntax>,
    pub end_markers: &'static [&'static str],
    pub any_multi_line_comments: &'static [(&'static str, &'static str)],
    pub multi_line_comments: &'static [(&'static str, &'static str)],
    pub nested_comments: &'static [(&'static str, &'static str)],
    pub text_sections: &'static [(&'static str, &'static str)],
    pub markup_expressions: Option<MarkupExpressions>,
    pub dollar_quotes: bool,
    pub tag_line_comments: &'static [&'static str],
    pub tag_multi_line_comments: &'static [(&'static str, &'static str)],
    pub string_literals: &'static [(&'static str, &'static str)],
    pub character_literals: &'static [(&'static str, &'static str)],
    pub code_tokens: &'static [&'static str],
    pub code_line_markers: &'static [&'static str],
    pub code_pragmas: Option<CodePragmas>,
    pub blank_symbols: BlankSymbols,
    pub doubled_quote_escapes: &'static [&'static str],
    pub verbatim_string_literals: &'static [(&'static str, &'static str)],
    pub raw_strings: &'static [RawDelimiter],
    pub raw_comments: &'static [RawDelimiter],
    pub interpolations: &'static [Interpolation],
    pub heredocs: &'static [Heredoc],
    /// Keywords after which `/` starts a regular expression, for languages
    /// that have regular expression literals.
    pub regex_keywords: Option<&'static [&'static str]>,
    pub line_comments_start_words: bool,
    pub regex_after_line_break: bool,
    pub regex_spans_lines: bool,
    pub regex_operator_symbols: bool,
    pub regex_command_arguments: bool,
    pub prefix_characters: Option<PrefixCharacterSyntax>,
    pub quotes_in_tags: bool,
    pub template_expressions: &'static [(&'static str, &'static str)],
    pub word_delimiters: &'static [u8],
    pub quotes_after: Option<&'static [&'static str]>,
    pub block_scalars: bool,
    pub jsx: bool,
    /// Bytes that can begin a comment, literal or comment exception, so the
    /// analysis loop can skip every other position without prefix matching.
    pub opener_bytes: [bool; 256],
    /// The bytes that can follow each opener byte, so an opener byte such
    /// as Python's `f` is matched only before a byte some opener continues
    /// with.
    pub opener_pairs: OpenerPairs,
    /// Bytes that can begin a hole, a text that only looks like one or an
    /// escape, so strings skip the hole check at every other position.
    pub hole_bytes: [bool; 256],
    pub quote_aliases: &'static [(&'static str, &'static str)],
    /// Bytes that begin a quote alias, so strings skip the alias check at
    /// every other position.
    pub alias_bytes: [bool; 256],
    /// Bytes that begin an interpolating quote, so a string opened by any
    /// other byte skips the search of the interpolations, which for Python's
    /// 80 f-string and t-string kinds ran at every escape in a plain string.
    pub interpolating_quote_bytes: [bool; 256],
    /// Bytes that change nothing outside literals, comments, holes and JSX:
    /// no opener, hole, escape or JSX tag starts at them.
    pub inert_bytes: [bool; 256],
    /// Bytes that change nothing unless an opener starts at them, so an
    /// opener byte is inert where no opener pair starts; otherwise each `f`,
    /// `r` and `t` in Python code would take the full check for its string
    /// prefixes.
    pub pair_inert_bytes: [bool; 256],
}

impl SharedMatchers {
    /// The class of a line that opens no comment or literal other than a
    /// line comment at its start: blank, a comment, or code. The full parse
    /// and the fast path over the lines before a file's first important
    /// syntax both classify such lines with it, so the two cannot disagree.
    pub(crate) fn plain_line_class(&self, line: &[u8]) -> LineClass {
        if self.blank_symbols.trim_start(line).is_empty() {
            LineClass::Blank
        } else if self.is_literate
            || self
                .line_comments
                .iter()
                .any(|c| line.starts_with(c.as_bytes()))
            || (self.rem_comments && visual_basic::starts_rem(line, 0))
            || self.starts_with_line_start_comment(line)
        {
            LineClass::Comment
        } else {
            LineClass::Code
        }
    }

    /// Whether `text`, with nothing but comments before it on its line, opens
    /// a comment that opens one only there: a line-start comment or a batch
    /// remark, either after any line comment flags, or a line comment after
    /// at least one.
    pub(crate) fn starts_with_line_start_comment(&self, text: &[u8]) -> bool {
        let flags = match self.line_comment_flags {
            [] => 0,
            flags => text
                .iter()
                .take_while(|b| flags.contains(b) || matches!(b, b' ' | b'\t'))
                .count(),
        };
        let flagless = &text[flags..];
        self.line_start_comments
            .iter()
            .any(|c| flagless.starts_with(c.as_bytes()))
            || (self.batch_remarks && batch::starts_remark(flagless))
            || (flags > 0
                && self
                    .line_comments
                    .iter()
                    .any(|c| flagless.starts_with(c.as_bytes())))
    }

    pub fn new(language: LanguageType) -> Arc<Self> {
        static MATCHERS: Lazy<DashMap<LanguageType, Arc<SharedMatchers>>> = Lazy::new(DashMap::new);

        MATCHERS
            .entry(language)
            .or_insert_with(|| Arc::new(Self::init(language)))
            .value()
            .clone()
    }

    pub fn init(language: LanguageType) -> Self {
        // A line holding a pattern that contains another holds the other too, on
        // the same line, so the longer one cannot change which lines are
        // important; dropping it keeps prefix letters such as Python's `f` in
        // `f"` out of the search's start bytes.
        fn without_superstrings(patterns: Vec<String>) -> Vec<String> {
            patterns
                .iter()
                .filter(|long| {
                    long.contains('\n')
                        || !patterns
                            .iter()
                            .any(|short| short.len() < long.len() && long.contains(short.as_str()))
                })
                .cloned()
                .collect()
        }

        fn init_corasick<P: AsRef<[u8]>>(pattern: &[P]) -> AhoCorasick {
            AhoCorasick::builder()
                .match_kind(aho_corasick::MatchKind::LeftmostLongest)
                .start_kind(aho_corasick::StartKind::Unanchored)
                .prefilter(true)
                .kind(Some(aho_corasick::AhoCorasickKind::DFA))
                .build(pattern)
                .unwrap()
        }

        let starts = |pairs: &'static [(&'static str, &'static str)]| pairs.iter().map(|(s, _)| *s);
        let mut opener_pairs = OpenerPairs::default();
        for opener in language
            .line_comments()
            .iter()
            .chain(language.line_start_comments())
            .copied()
            .chain(starts(language.multi_line_comments()))
            .chain(starts(language.nested_comments()))
            .chain(starts(language.text_sections()))
            .chain(starts(language.quotes()))
            .chain(starts(language.doc_quotes()))
            .chain(starts(language.verbatim_quotes()))
            .chain(starts(language.character_literals()))
            .chain(language.code_tokens().iter().copied())
            .chain(language.code_line_markers().iter().copied())
        {
            match *opener.as_bytes() {
                [] => opener_pairs = OpenerPairs::any(),
                [first] => opener_pairs.allow_any(first),
                [first, second, ..] => opener_pairs.allow(first, second),
            }
        }
        let raw = || language.raw_strings().iter().chain(language.raw_comments());
        for b in raw().flat_map(RawDelimiter::first_bytes) {
            opener_pairs.allow_any(b);
        }
        if let Some(syntax) = language.prefix_characters() {
            opener_pairs.allow_any(syntax.prefix());
        }
        if language.xml_literals() {
            opener_pairs.allow_any(b'<');
        }
        if language.rem_comments() {
            for (first, second) in [(b'R', b'E'), (b'R', b'e'), (b'r', b'E'), (b'r', b'e')] {
                opener_pairs.allow(first, second);
            }
        }
        if language.regex_keywords().is_some() {
            opener_pairs.allow_any(b'/');
        }
        for heredoc in language.heredocs() {
            opener_pairs.allow_any(heredoc.start.as_bytes()[0]);
        }
        if language.block_scalars() {
            opener_pairs.allow_any(b'|');
            opener_pairs.allow_any(b'>');
        }
        match language.quote_like() {
            Some(QuoteLikeSyntax::Perl) => {
                for b in *b"qmsty" {
                    opener_pairs.allow_any(b);
                }
            }
            Some(QuoteLikeSyntax::RubyPercent) => {
                opener_pairs.allow_any(b'%');
            }
            Some(QuoteLikeSyntax::ElixirSigil | QuoteLikeSyntax::ErlangSigil) => {
                opener_pairs.allow_any(b'~');
            }
            Some(QuoteLikeSyntax::MakeReference) => {
                opener_pairs.allow_any(b'$');
            }
            Some(QuoteLikeSyntax::M4 { open, .. }) => {
                opener_pairs.allow_any(open.as_bytes()[0]);
            }
            Some(QuoteLikeSyntax::HaskellQuasiQuote) => {
                for b in (b'a'..=b'z').chain(b'A'..=b'Z').chain(*b"_") {
                    opener_pairs.allow(b'[', b);
                }
            }
            None => {}
        }
        if language.quotes_after().is_some() {
            for b in *b"[]{}" {
                opener_pairs.allow_any(b);
            }
        }
        if language.quotes_in_tags() {
            opener_pairs.allow_any(b'<');
            opener_pairs.allow_any(b'>');
        }
        for (open, _) in language.template_expressions() {
            opener_pairs.allow_any(open.as_bytes()[0]);
        }
        if let Some(expressions) = language.markup_expressions() {
            opener_pairs.allow_any(expressions.open.as_bytes()[0]);
        }
        if language.dollar_quotes() {
            opener_pairs.allow_any(b'$');
        }
        for open in language
            .tag_line_comments()
            .iter()
            .copied()
            .chain(starts(language.tag_multi_line_comments()))
        {
            opener_pairs.allow_any(open.as_bytes()[0]);
        }
        let opener_bytes = std::array::from_fn(|b| opener_pairs.may_start(b as u8));
        let mut hole_bytes = [false; 256];
        let mut interpolating_quote_bytes = [false; 256];
        for kind in language.interpolations() {
            interpolating_quote_bytes[usize::from(kind.quote.as_bytes()[0])] = true;
            for text in kind.literal.iter().chain([&kind.open]) {
                if let Some(&b) = text.as_bytes().first() {
                    hole_bytes[usize::from(b)] = true;
                }
            }
            hole_bytes[usize::from(language.escape_character())] = true;
        }
        let blank_symbols = BlankSymbols::new(language.blank_symbols());
        // A run of passed-over bytes skips the checks for blank symbols and
        // code tags that the analysis makes before it, so their bytes end it.
        let close_tag = language.code_tags().map(|tags| tags.close().as_bytes()[0]);
        let pair_inert_bytes: [bool; 256] = std::array::from_fn(|i| {
            let b = i as u8;
            !hole_bytes[i]
                && b != language.escape_character()
                && !(language.jsx() && b == b'<')
                && !blank_symbols.may_start(b)
                && Some(b) != close_tag
        });
        let inert_bytes = std::array::from_fn(|i| pair_inert_bytes[i] && !opener_bytes[i]);
        let important_syntax: Vec<String> = language
            .important_syntax()
            .iter()
            .map(|text| (*text).to_owned())
            .chain(raw().flat_map(RawDelimiter::important_syntax))
            .chain(language.heredocs().iter().map(|h| h.start.to_owned()))
            .chain((language.jsx() || language.quotes_in_tags()).then(|| "<".to_owned()))
            .chain(language.pod().is_some().then(|| "=".to_owned()))
            // An XML literal decides the lines it spans.
            .chain(language.xml_literals().then(|| "<".to_owned()))
            .chain(
                language
                    .code_tags()
                    .into_iter()
                    .flat_map(|tags| [tags.open().to_owned(), tags.close().to_owned()]),
            )
            // A regular expression that spans lines decides the next line.
            .chain(language.regex_spans_lines().then(|| "/".to_owned()))
            // A line comment that a splice continues decides the next line.
            .chain(
                language
                    .line_splices()
                    .and_then(LineSplices::important_syntax)
                    .map(str::to_owned),
            )
            .chain(
                match language.quote_like() {
                    Some(QuoteLikeSyntax::Perl) => vec!["q", "m", "s", "y"],
                    Some(QuoteLikeSyntax::RubyPercent) => vec!["%"],
                    Some(QuoteLikeSyntax::ElixirSigil | QuoteLikeSyntax::ErlangSigil) => {
                        vec!["~"]
                    }
                    Some(QuoteLikeSyntax::M4 { open, .. }) => vec![open],
                    // Only files that turn quasi-quotes on search for their
                    // `|`, which `may_open_quasi_quote` does.
                    Some(QuoteLikeSyntax::HaskellQuasiQuote) | None => vec![],
                    // A make reference changes the class of no line but one
                    // that a splice continues, which `make::needs_full_parse`
                    // sends to the full parse.
                    Some(QuoteLikeSyntax::MakeReference) => vec![],
                }
                .into_iter()
                .map(str::to_owned),
            )
            .chain(language.end_markers().iter().map(|m| (*m).to_owned()))
            .chain(starts(language.text_sections()).map(str::to_owned))
            .chain(language.markup_expressions().map(|e| e.open.to_owned()))
            .chain(language.dollar_quotes().then(|| "$".to_owned()))
            .collect();
        let important_syntax = without_superstrings(important_syntax);

        Self {
            language,
            allows_nested: language.allows_nested(),
            doc_quotes: language.doc_quotes(),
            is_fortran: language.is_fortran(),
            is_literate: language.is_literate(),
            important_syntax: (!important_syntax.is_empty())
                .then(|| init_corasick(&important_syntax)),
            any_comments: language.any_comments(),
            line_comments: language.line_comments(),
            line_start_comments: language.line_start_comments(),
            line_comment_flags: language.line_comment_flags(),
            batch_remarks: language.batch_remarks(),
            rem_comments: language.rem_comments(),
            comment_literals: language.comment_literals(),
            postfix_primes: language.postfix_primes(),
            xml_literals: language.xml_literals(),
            line_start_closers: language.line_start_closers(),
            pod: language.pod(),
            code_tags: language.code_tags(),
            frontmatter: language.frontmatter(),
            line_splices: language.line_splices(),
            shebang: language.shebang(),
            run_closers: language.run_closers(),
            identifier_quote_prefix: language.identifier_quote_prefix(),
            line_directives: language.line_directives(),
            backslash_operator: language.backslash_operator(),
            string_gaps: language.string_gaps(),
            escape: language.escape_character(),
            quote_like: language.quote_like(),
            end_markers: language.end_markers(),
            multi_line_comments: language.multi_line_comments(),
            any_multi_line_comments: language.any_multi_line_comments(),
            nested_comments: language.nested_comments(),
            text_sections: language.text_sections(),
            markup_expressions: language.markup_expressions(),
            dollar_quotes: language.dollar_quotes(),
            tag_line_comments: language.tag_line_comments(),
            tag_multi_line_comments: language.tag_multi_line_comments(),
            string_literals: language.quotes(),
            character_literals: language.character_literals(),
            code_tokens: language.code_tokens(),
            code_line_markers: language.code_line_markers(),
            code_pragmas: language.code_pragmas(),
            blank_symbols,
            doubled_quote_escapes: language.doubled_quote_escapes(),
            verbatim_string_literals: language.verbatim_quotes(),
            raw_strings: language.raw_strings(),
            raw_comments: language.raw_comments(),
            interpolations: language.interpolations(),
            heredocs: language.heredocs(),
            regex_keywords: language.regex_keywords(),
            line_comments_start_words: language.line_comments_start_words(),
            regex_after_line_break: language.regex_after_line_break(),
            regex_spans_lines: language.regex_spans_lines(),
            regex_operator_symbols: language.regex_operator_symbols(),
            regex_command_arguments: language.regex_command_arguments(),
            prefix_characters: language.prefix_characters(),
            quotes_in_tags: language.quotes_in_tags(),
            template_expressions: language.template_expressions(),
            word_delimiters: language.word_delimiters().as_bytes(),
            quotes_after: language.quotes_after(),
            block_scalars: language.block_scalars(),
            jsx: language.jsx(),
            opener_bytes,
            opener_pairs,
            hole_bytes,
            quote_aliases: language.quote_aliases(),
            alias_bytes: std::array::from_fn(|b| {
                language
                    .quote_aliases()
                    .iter()
                    .any(|(_, alias)| alias.as_bytes()[0] == b as u8)
            }),
            interpolating_quote_bytes,
            inert_bytes,
            pair_inert_bytes,
        }
    }
}

/// For each first byte of an opener, the second bytes it can continue with,
/// 256 bits each. An opener byte whose openers are not all texts of two or
/// more bytes, such as a one-byte quote or a raw-string start, allows any,
/// so Python's 80 prefixed quotes are tried only where one can start.
#[derive(Clone, Debug)]
pub struct OpenerPairs(Box<[[u64; 4]; 256]>);

impl Default for OpenerPairs {
    fn default() -> Self {
        Self(Box::new([[0; 4]; 256]))
    }
}

impl OpenerPairs {
    fn any() -> Self {
        Self(Box::new([[u64::MAX; 4]; 256]))
    }

    fn allow(&mut self, first: u8, second: u8) {
        self.0[usize::from(first)][usize::from(second >> 6)] |= 1 << (second & 63);
    }

    fn allow_any(&mut self, first: u8) {
        self.0[usize::from(first)] = [u64::MAX; 4];
    }

    fn may_start(&self, first: u8) -> bool {
        self.0[usize::from(first)] != [0; 4]
    }

    fn allows(&self, first: u8, second: u8) -> bool {
        self.0[usize::from(first)][usize::from(second >> 6)] & (1 << (second & 63)) != 0
    }
}

/// The class a line is counted in.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum LineClass {
    /// A line holding code or text outside comments.
    Code,
    /// A line holding nothing but comments.
    Comment,
    /// A line holding only whitespace that starts outside every comment and
    /// literal; one inside them takes their class.
    Blank,
}

#[derive(Debug)]
pub(crate) enum AnalysisReport {
    /// No child languages were found, contains a boolean representing whether
    /// the line ended with comments or not.
    Normal(bool),
    ChildLanguage(FileContext),
}

impl SyntaxCounter {
    pub(crate) fn new(language: LanguageType) -> Self {
        Self {
            shared: SharedMatchers::new(language),
            quote_is_doc_quote: false,
            quote_is_verbatim: false,
            stack: Vec::with_capacity(1),
            lf_embedded_language: None,
            quote: None,
            quote_escape: None,
            quote_closes_at_line_start: false,
            quote_opener: "",
            holes: Vec::new(),
            literal_end: 0,
            text_end: 0,
            quasi_quoting: None,
            comment_end: 0,
            in_data_section: false,
            outside_code: language.code_tags().is_some(),
            flow_depth: 0,
            in_tag: false,
            dollar_bodies: Vec::new(),
            jsx: Vec::new(),
            make_defines: make::Defines::default(),
            line_has_code: false,
        }
    }

    /// The class of the line at `lines[start..end]` when a region of whole
    /// lines decides it before any lexing: a line that starts inside a
    /// literal, such as a heredoc body or a Rust frontmatter, is code; a line
    /// of a Perl POD block is a comment (perlpod); and a line after an end
    /// marker such as `__END__` is data, so code or blank (perldata, "Special
    /// Literals").
    pub(crate) fn whole_line_class(
        &mut self,
        lines: &[u8],
        start: usize,
        end: usize,
    ) -> Option<LineClass> {
        if start < self.literal_end {
            return Some(
                if start < self.text_end && lines[start..end].trim().is_empty() {
                    LineClass::Blank
                } else {
                    LineClass::Code
                },
            );
        }
        if start < self.comment_end {
            return Some(LineClass::Comment);
        }
        if !self.is_plain_mode() {
            return None;
        }
        if start == 0
            && self
                .shared
                .shebang
                .is_some_and(|syntax| shebang_len(lines, syntax).is_some())
        {
            return Some(LineClass::Comment);
        }
        if self.shared.frontmatter {
            if let Some(frontmatter_end) = frontmatter_end(lines, start) {
                trace!("Start frontmatter, ends at {}", frontmatter_end);
                self.literal_end = frontmatter_end;
                return Some(LineClass::Code);
            }
        }
        if let Some(syntax) = self.shared.pod {
            if let Some(pod_end) = pod_block_end(lines, start, syntax) {
                trace!("Start POD, ends at {}", pod_end);
                self.comment_end = pod_end;
                return Some(LineClass::Comment);
            }
        }
        let line = &lines[start..end];
        if self
            .shared
            .line_directives
            .iter()
            .any(|directive| line.starts_with(directive.as_bytes()))
        {
            trace!("Line directive");
            return Some(LineClass::Code);
        }
        if self.in_data_section {
            return Some(if line.trim().is_empty() {
                LineClass::Blank
            } else {
                LineClass::Code
            });
        }
        if self
            .shared
            .end_markers
            .iter()
            .any(|m| line.trim_end() == m.as_bytes())
        {
            trace!("Data section after {:?}", String::from_utf8_lossy(line));
            self.in_data_section = true;
            return Some(LineClass::Code);
        }
        None
    }

    /// Whether `text` opens with a shebang or a frontmatter, whole lines
    /// that only the full parse reads. They open a file only, so they are
    /// not important syntax, which every line is searched for: as such,
    /// they cost Tokio `6470b5e` 42M of 503M instructions (measured
    /// 2026-10-29).
    pub(crate) fn opens_with_whole_lines(&self, text: &[u8]) -> bool {
        let first_line = || {
            text.iter().position(|b| !b.is_ascii_whitespace()).map(|n| {
                text[..n]
                    .iter()
                    .rposition(|b| *b == b'\n')
                    .map_or(0, |m| m + 1)
            })
        };
        self.shared
            .shebang
            .is_some_and(|syntax| shebang_len(text, syntax).is_some())
            || (self.shared.frontmatter
                && first_line().is_some_and(|start| frontmatter_end(text, start).is_some()))
    }

    /// Returns whether the syntax is currently in plain mode, outside every
    /// literal and comment.
    pub(crate) fn is_plain_mode(&self) -> bool {
        self.quote.is_none() && self.stack.is_empty() && !self.outside_code
    }

    /// How many bytes from `i` on, short of `end`, leave the analysis as it
    /// is, so the per-byte checks are passed over: in a string, bytes that
    /// neither close nor escape it nor open a hole; in code outside every
    /// hole and JSX frame, bytes that start nothing, unless the file `embeds`
    /// other languages. Passing over them cut the instructions for RuboCop
    /// `28b550d` from 1,585M to 1,361M, and for Symfony `5c0e37bf` from
    /// 18,019M to 14,893M (measured 2026-10-29). Taking each run at once,
    /// rather than byte by byte, cut Symfony `92216211` from 1.887 s to
    /// 1.117 s of user time and pandoc's Haskell files from 2.9 to 1.9 times
    /// v15.0.0's time, whose Haskell has no quotes (`judges/speed/check.py`,
    /// measured 2026-10-30).
    fn passed_over_len(&self, lines: &[u8], i: usize, end: usize, embeds: bool) -> usize {
        if !self.stack.is_empty() {
            return 0;
        }
        let escape = self.shared.escape;
        let span = &lines[i..end];
        let stops = match self.quote.as_deref() {
            Some(quote) => {
                let quote = quote.as_bytes().first().copied();
                let quote_escape = self
                    .quote_escape
                    .as_deref()
                    .and_then(|text| text.as_bytes().first().copied());
                span.iter().position(|&b| {
                    b == escape
                        || Some(b) == quote
                        || Some(b) == quote_escape
                        || self.shared.hole_bytes[usize::from(b)]
                        || self.shared.alias_bytes[usize::from(b)]
                })
            }
            None => {
                if embeds
                    || self.quote_escape.is_some()
                    || !self.holes.is_empty()
                    || !self.jsx.is_empty()
                {
                    return 0;
                }
                span.iter().enumerate().position(|(at, &b)| {
                    !(self.shared.inert_bytes[usize::from(b)]
                        || self.shared.pair_inert_bytes[usize::from(b)]
                            && lines
                                .get(i + at + 1)
                                .is_some_and(|&next| !self.shared.opener_pairs.allows(b, next)))
                })
            }
        };
        stops.unwrap_or(span.len())
    }

    /// Returns whether the syntax is currently in string mode.
    pub(crate) fn _is_string_mode(&self) -> bool {
        self.quote.is_some()
    }

    /// Returns whether the syntax is currently in comment mode.
    pub(crate) fn _is_comment_mode(&self) -> bool {
        !self.stack.is_empty()
    }

    pub(crate) fn get_lf_target_language(&self) -> LanguageType {
        // in case the target declaration was not found, default it to that language
        const DEFAULT_LANG: LanguageType = LinguaFranca;
        self.lf_embedded_language.unwrap_or(DEFAULT_LANG)
    }

    #[inline]
    pub(crate) fn parse_line_comment(&self, lines: &[u8], i: usize) -> bool {
        let window = &lines[i..];
        if self.quote.is_some()
            || !self.stack.is_empty()
            || (self.shared.line_comments_start_words && !self.starts_word(lines, i))
        {
            false
        } else if let Some(comment) = self
            .shared
            .line_comments
            .iter()
            .find(|c| window.starts_with(c.as_bytes()))
        {
            trace!("Start {:?}", comment);
            true
        } else if self.shared.rem_comments && visual_basic::starts_rem(lines, i) {
            trace!("Start a REM comment");
            true
        // Only comments may come before it on the line, as before an HTML-like
        // `-->` (ECMA-262 Annex B.1.1, HTMLCloseComment).
        } else if !self.line_has_code && self.shared.starts_with_line_start_comment(window) {
            trace!("Start a comment at the start of a line");
            true
        } else if self.in_tag
            && self
                .shared
                .tag_line_comments
                .iter()
                .any(|c| window.starts_with(c.as_bytes()))
            && may_start_tag_comment(&lines[..i])
        {
            trace!("Start line comment in a tag");
            true
        } else {
            false
        }
    }

    /// Try to see if we can determine what a line is from examining the whole
    /// line at once. Returns `true` if successful.
    pub(crate) fn try_perform_single_line_analysis(
        &mut self,
        lines: &[u8],
        line: &[u8],
        stats: &mut crate::stats::CodeStats,
    ) -> bool {
        if self.outside_code
            && self
                .shared
                .code_tags
                .is_some_and(|tags| !line.contains_slice(tags.open().as_bytes()))
        {
            // Output text holds no comments or literals.
            if line.trim().is_empty() {
                stats.blanks += 1;
            } else {
                stats.code += 1;
            }
            true
        } else if !self.is_plain_mode() {
            false
        } else if self.shared.blank_symbols.trim_start(line).is_empty() {
            stats.blanks += 1;
            trace!("Blank No.{}", stats.blanks);
            true
        } else if !self.holes.is_empty()
            || !self.jsx.is_empty()
            || self.in_tag
            || self.flow_depth > 0
            || self
                .shared
                .important_syntax
                .as_ref()
                .is_some_and(|syntax| syntax.is_match(line))
            || (self.shared.line_splices == Some(LineSplices::Make) && make::needs_full_parse(line))
            || self.may_open_quasi_quote(lines, line)
            || self.opens_flow_collection(line)
            || (self.shared.block_scalars && may_start_block_scalar(line))
        {
            false
        } else {
            trace!("^ Skippable");
            stats.add_line(self.shared.plain_line_class(line));
            true
        }
    }

    pub(crate) fn perform_multi_line_analysis(
        &mut self,
        lines: &[u8],
        start: usize,
        end: usize,
        config: &Config,
    ) -> AnalysisReport {
        let mut ended_with_comments = false;
        let regex_cache = RegexCache::build(self.shared.language, lines, start, end);
        let embeds = regex_cache.family().is_some();

        // The byte last read, and whether a comment was open before it: it is
        // comment when one was open before or after it, and code otherwise.
        let mut pending: Option<(u8, bool)> = None;
        macro_rules! settle {
            () => {{
                if let Some((byte, in_comment)) = pending.take() {
                    if !byte.is_ascii_whitespace() && !in_comment && self.stack.is_empty() {
                        self.line_has_code = true;
                    }
                }
            }};
        }

        let mut positions = start..end;
        while let Some(i) = positions.next() {
            // Resumes the analysis after the `len` bytes from `i` on.
            macro_rules! skip {
                ($len:expr) => {{
                    positions.start = i + $len;
                }};
            }
            // Skips the literal of `len` bytes from `i` on, or, when it goes
            // on past this line, leaves the rest of the line to it; a text
            // literal's lines of whitespace are text rather than code.
            macro_rules! literal {
                ($len:expr, $what:literal) => {
                    literal!($len, $what, false)
                };
                ($len:expr, $what:literal, $text:expr) => {{
                    let len = $len;
                    if lines[i..i + len].contains(&b'\n') {
                        trace!(concat!("Start ", $what, ", ends at {}"), i + len);
                        self.literal_end = i + len;
                        if $text {
                            self.text_end = i + len;
                        }
                        break;
                    }
                    skip!(len);
                    continue;
                }};
            }
            settle!();

            let window = &lines[i..];

            if window[..1].trim().is_empty() && window.trim().is_empty() {
                break;
            }
            if let Some(symbol) = self
                .shared
                .blank_symbols
                .at(window)
                .filter(|_| self.is_plain_mode())
            {
                skip!(symbol.len());
                continue;
            }
            if let Some(tags) = self.shared.code_tags {
                if self.outside_code {
                    // Text outside the tags is output, whose bytes are code.
                    pending = Some((window[0], false));
                    if let Some(len) = tags.open_len(window) {
                        trace!("Open code tag");
                        self.outside_code = false;
                        skip!(len);
                    }
                    continue;
                }
                if window.starts_with(tags.close().as_bytes()) && self.is_plain_mode() {
                    trace!("Close code tag");
                    pending = Some((window[0], false));
                    self.outside_code = true;
                    skip!(tags.close().len());
                    continue;
                }
            }
            pending = Some((window[0], !self.stack.is_empty()));

            ended_with_comments = false;
            let passed = self.passed_over_len(lines, i, end, embeds);
            if passed != 0 {
                if lines[i..i + passed]
                    .iter()
                    .any(|b| !b.is_ascii_whitespace())
                {
                    self.line_has_code = true;
                }
                pending = None;
                skip!(passed);
                continue;
            }
            let is_end_of_quote_or_multi_line = self
                .parse_end_of_quote(lines, i)
                .or_else(|| self.parse_end_of_multi_line(window));

            if let Some(skip_amount) = is_end_of_quote_or_multi_line {
                ended_with_comments = true;
                skip!(skip_amount);
                continue;
            } else if self.quote.is_some() {
                continue;
            }

            if let Some(skip_amount) = self.parse_end_of_hole(window) {
                skip!(skip_amount);
                continue;
            }

            if let Some(skip_amount) = self.parse_jsx(lines, i) {
                skip!(skip_amount);
                continue;
            }

            let may_open = self.shared.opener_bytes[usize::from(window[0])]
                && window.get(1).map_or(true, |&next| {
                    self.shared.opener_pairs.allows(window[0], next)
                });
            if may_open {
                if let Some(len) = self.parse_markup_expression(window) {
                    skip!(len);
                    continue;
                }

                if let Some(len) = self.code_token(window) {
                    skip!(len);
                    continue;
                }

                if let Some(skip_amount) = self.parse_character_literal(lines, i) {
                    skip!(skip_amount);
                    continue;
                }

                if let Some(len) = self
                    .shared
                    .prefix_characters
                    .filter(|_| self.is_plain_mode())
                    .and_then(|syntax| syntax.character_len(lines, i))
                {
                    skip!(len);
                    continue;
                }

                if let Some(len) = self.parse_regex_literal(lines, i) {
                    literal!(len, "regular expression");
                }

                if let Some(len) = self.parse_quote_like(lines, i) {
                    literal!(len, "quote-like literal");
                }

                if let Some(skip_amount) = self.parse_heredoc(lines, i) {
                    skip!(skip_amount);
                    continue;
                }

                if self.parse_block_scalar(lines, i) {
                    continue;
                }
            }

            if let Some(child) = self.parse_context(lines, i, end, config, &regex_cache) {
                return AnalysisReport::ChildLanguage(child);
            }

            if !may_open {
                continue;
            }

            if let Some(len) = self.text_section_len(lines, i) {
                literal!(len, "text section", true);
            }

            if let Some(len) = self.xml_literal_len(lines, i) {
                literal!(len, "XML literal", true);
            }

            match self.dollar_quote(lines, i) {
                Some(DollarQuote::Code(len)) => {
                    skip!(len);
                    continue;
                }
                Some(DollarQuote::Literal(len)) => literal!(len, "dollar-quoted literal"),
                None => {}
            }

            if self.shared.quotes_in_tags && !self.in_tag {
                if let Some(skip_amount) = self.parse_multi_line_comment(window) {
                    skip!(skip_amount);
                } else if starts_tag(window) {
                    self.in_tag = true;
                }
                continue;
            }
            if self.in_tag {
                if let Some(len) = template_expression_len(window, self.shared.template_expressions)
                {
                    literal!(len, "template expression");
                }
                if window[0] == b'>' {
                    self.in_tag = false;
                    continue;
                }
                if let Some(len) = self.parse_tag_block_comment(lines, i) {
                    skip!(len);
                    continue;
                }
            }

            self.track_flow_collection(lines, i);
            let may_quote = self.may_open_quote(lines, i);
            let is_quote_or_multi_line = may_quote
                .then(|| self.parse_quote(window, i > 0 && is_word_byte(lines[i - 1])))
                .flatten()
                .or_else(|| self.parse_multi_line_comment(window));

            if let Some(skip_amount) = is_quote_or_multi_line {
                skip!(skip_amount);
                continue;
            }

            if self.parse_line_comment(lines, i) {
                if let Some(close) = self.shared.code_tags.map(CodeTags::close) {
                    let line_end = memchr::memchr(b'\n', window).unwrap_or(window.len());
                    if let Some(at) = memchr::memmem::find(&window[..line_end], close.as_bytes()) {
                        trace!("Close code tag in a line comment");
                        pending = Some((close.as_bytes()[0], false));
                        self.outside_code = true;
                        skip!(at + close.len());
                        continue;
                    }
                }
                if let Some(splices) = self.shared.line_splices {
                    let end = splices.comment_end(lines, i, &mut self.make_defines);
                    self.comment_end = self.comment_end.max(end);
                }
                pending = None;
                ended_with_comments = true;
                break;
            }
        }
        settle!();

        AnalysisReport::Normal(ended_with_comments)
    }

    /// Whether `text`, read from a position outside any comment or literal,
    /// holds nothing but comments: block comments, each closed before the
    /// next begins, optionally followed by a line comment or by a block
    /// comment left open, as in `/* a */ // b`.
    fn holds_only_comments(&self, mut text: &[u8]) -> bool {
        loop {
            text = self.shared.blank_symbols.trim_start(text);
            if text.is_empty()
                || self
                    .shared
                    .line_comments
                    .iter()
                    .any(|c| text.starts_with(c.as_bytes()))
                || self.shared.starts_with_line_start_comment(text)
            {
                return true;
            }
            let Some(&(start, end)) = self
                .shared
                .any_multi_line_comments
                .iter()
                .find(|(start, _)| text.starts_with(start.as_bytes()))
            else {
                return false;
            };
            let nests =
                self.shared.allows_nested || self.shared.nested_comments.contains(&(start, end));
            match block_comment_len(text, start, end, nests) {
                Some(len) => text = &text[len..],
                None => return true,
            }
        }
    }

    /// Performs a set of heuristics to determine whether a line is a comment or
    /// not. The procedure is as follows.
    ///
    /// - Yes/No: Counted as Comment
    ///
    /// 1. Check if we're in string mode
    ///  1. Check if string literal is a doc string and whether tokei has
    ///     been configured to treat them as comments.
    ///     - Yes: When the line starts with the doc string or when we are
    ///       continuing from a previous line.
    ///  - No: The string is a normal string literal or tokei isn't
    ///    configured to count them as comments.
    /// 2. If we're not in string mode, check if we left it this on this line.
    ///    - Yes: When we found a doc quote and we started in comments.
    /// 3. No: When the line starts inside a literal.
    /// 4. Yes: When the line holds nothing but comments, so that code between
    ///    or after comments makes it code; PlantUML, whose block comments are
    ///    whole lines, keeps Tokei's text rules for them.
    pub(crate) fn line_is_comment(
        &self,
        line: &[u8],
        config: &crate::Config,
        _ended_with_comments: bool,
        started_in_comments: bool,
        started_in_literal: bool,
    ) -> bool {
        let trimmed = line.trim();
        let whole_line_is_comment = || {
            // A line opening with a code token such as PHP's `#[` is not a
            // comment, whatever comment marker that token starts with.
            let opens_with_code = self
                .shared
                .code_tokens
                .iter()
                .any(|e| trimmed.starts_with(e.as_bytes()));
            !opens_with_code
                && (self
                    .shared
                    .any_multi_line_comments
                    .iter()
                    .any(|(start, end)| {
                        trimmed.starts_with(start.as_bytes()) && trimmed.ends_with(end.as_bytes())
                    })
                    || self.holds_only_comments(trimmed))
        };
        let starts_with_comment = || {
            let quote = match self.stack.last() {
                Some(q) => q,
                _ => return false,
            };

            self.shared
                .any_multi_line_comments
                .iter()
                .any(|(start, end)| *end == quote.as_ref() && trimmed.starts_with(start.as_bytes()))
        };

        // `Some(true)` in order to respect the current configuration.
        #[allow(clippy::if_same_then_else)]
        if self.quote.is_some() {
            if self.quote_is_doc_quote && config.treat_doc_strings_as_comments == Some(true) {
                self.quote
                    .as_deref()
                    .is_some_and(|q| line.starts_with(q.as_bytes()))
                    || (self.quote.is_some())
            } else {
                false
            }
        } else if self
            .shared
            .doc_quotes
            .iter()
            .any(|(_, e)| line.contains_slice(e.as_bytes()))
            && started_in_comments
        {
            true
        } else if started_in_literal && !started_in_comments {
            // The line's first characters belong to a literal, so a comment
            // marker they spell, as in a raw string's `// …` line, is text.
            false
        } else if self.shared.language == LanguageType::PlantUml {
            // PlantUML's block comments are whole lines, so a line opening
            // with `/'` and ending with `'/` is one, whatever lies between.
            (whole_line_is_comment)() || started_in_comments || (starts_with_comment)()
        } else {
            // A line is a comment only when it holds nothing but comments, as
            // the owner decided on 2026-10-29.
            !self.line_has_code
        }
    }

    #[inline]
    pub(crate) fn parse_context(
        &mut self,
        lines: &[u8],
        start: usize,
        end: usize,
        config: &Config,
        regex_cache: &RegexCache,
    ) -> Option<FileContext> {
        // static TYPE_REGEX: Lazy<Regex> = Lazy::new(|| Regex::new(r#"type="(.*)".*>"#).unwrap());
        if self.quote.is_some() || !self.stack.is_empty() {
            return None;
        }

        match regex_cache.family()? {
            RegexFamily::LinguaFranca(lf) => {
                let opening_fence = lf.starts_in_range(start, end)?;
                let start_of_code = opening_fence.end();
                let closing_fence = ENDING_LF_BLOCK_REGEX.find(&lines[start_of_code..]);
                let end_of_code = closing_fence
                    .map_or_else(|| lines.len(), |fence| start_of_code + fence.start());

                let block_contents = &lines[start_of_code..end_of_code];
                trace!("LF block: {:?}", String::from_utf8_lossy(block_contents));
                let stats = self.get_lf_target_language().parse_from_slice(
                    block_contents.trim_first_and_last_line_of_whitespace(),
                    config,
                );
                trace!("-> stats: {:?}", stats);

                Some(FileContext::new(
                    LanguageContext::LinguaFranca,
                    end_of_code,
                    stats,
                ))
            }
            RegexFamily::HtmlLike(html) => {
                if let Some(mut captures) = html.start_script_in_range(start, end) {
                    let start_of_code = captures.next().unwrap().end();
                    let closing_tag = END_SCRIPT.find(&lines[start_of_code..])?;
                    let end_of_code = start_of_code + closing_tag.start();
                    // A script of a type that is not JavaScript's and that no
                    // language claims is a data block, which the browser does
                    // not run (HTML Standard, "The script element").
                    let language = match captures.next().map(|m| m.as_bytes().trim()) {
                        None | Some(b"" | b"module") => Some(LanguageType::JavaScript),
                        Some(mime) => LanguageType::from_mime(&String::from_utf8_lossy(mime)),
                    };
                    let script_contents = &lines[start_of_code..end_of_code];
                    if script_contents.trim().is_empty() {
                        return None;
                    }

                    let stats = language
                        .map(|language| language.parse_embedded(script_contents, config))
                        .unwrap_or_default();
                    Some(FileContext::new(
                        LanguageContext::Html {
                            language,
                            start: start_of_code,
                        },
                        end_of_code,
                        stats,
                    ))
                } else if let Some(mut captures) = html.start_style_in_range(start, end) {
                    let start_of_code = captures.next().unwrap().end();
                    let closing_tag = END_STYLE.find(&lines[start_of_code..])?;
                    let end_of_code = start_of_code + closing_tag.start();
                    let language = captures
                        .next()
                        .and_then(|m| lang_language(m.as_bytes()))
                        .unwrap_or(LanguageType::Css);
                    let style_contents = &lines[start_of_code..end_of_code];
                    if style_contents.trim().is_empty() {
                        return None;
                    }

                    let stats = language.parse_embedded(style_contents, config);
                    Some(FileContext::new(
                        LanguageContext::Html {
                            language: Some(language),
                            start: start_of_code,
                        },
                        end_of_code,
                        stats,
                    ))
                } else if let Some(mut captures) = html.start_template_in_range(start, end) {
                    let start_of_code = captures.next().unwrap().end();
                    let closing_tag = END_TEMPLATE.find(&lines[start_of_code..])?;
                    let end_of_code = start_of_code + closing_tag.start();
                    let language = captures
                        .next()
                        .and_then(|m| lang_language(m.as_bytes()))
                        .unwrap_or(LanguageType::Html);

                    let template_contents = &lines[start_of_code..end_of_code];
                    if template_contents.trim().is_empty() {
                        return None;
                    }
                    let stats = language.parse_embedded(template_contents, config);
                    Some(FileContext::new(
                        LanguageContext::Html {
                            language: Some(language),
                            start: start_of_code,
                        },
                        end_of_code,
                        stats,
                    ))
                } else {
                    None
                }
            }
        }
    }

    #[inline]
    pub(crate) fn parse_quote(&mut self, window: &[u8], after_identifier: bool) -> Option<usize> {
        if !self.stack.is_empty() {
            return None;
        }

        if let Some((raw, opening)) = self
            .shared
            .raw_strings
            .iter()
            .find_map(|raw| raw.parse_opener(window).map(|opening| (raw, opening)))
        {
            trace!("Start raw, closed by {:?}", opening.closer);
            self.quote = Some(Cow::Owned(opening.closer));
            self.quote_escape = opening.escape;
            self.quote_closes_at_line_start = opening.line_start;
            self.quote_opener = raw.start;
            self.quote_is_verbatim = true;
            self.quote_is_doc_quote = false;
            return Some(opening.len);
        }

        if let Some((start, end, verbatim, len)) = after_identifier
            .then(|| self.identifier_prefixed_opener(window))
            .flatten()
        {
            trace!("Start {:?} after an identifier", start);
            self.quote = Some(Cow::Borrowed(end));
            self.quote_opener = start;
            self.quote_is_verbatim = verbatim;
            self.quote_is_doc_quote = false;
            return Some(len);
        }

        if let Some((start, end)) = self
            .shared
            .doc_quotes
            .iter()
            .find(|(s, _)| opens_with(window, s))
        {
            trace!("Start Doc {:?}", start);
            self.quote = Some(Cow::Borrowed(end));
            self.quote_opener = start;
            self.quote_is_verbatim = false;
            self.quote_is_doc_quote = true;
            return Some(start.len());
        }

        if let Some((start, end)) = self
            .shared
            .verbatim_string_literals
            .iter()
            .find(|(s, _)| opens_with(window, s))
        {
            trace!("Start verbatim {:?}", start);
            self.quote = Some(Cow::Borrowed(end));
            self.quote_opener = start;
            self.quote_is_verbatim = true;
            self.quote_is_doc_quote = false;
            return Some(start.len());
        }

        if let Some((start, end)) = self
            .shared
            .string_literals
            .iter()
            .find(|(s, _)| opens_with(window, s))
        {
            trace!("Start {:?}", start);
            self.quote = Some(Cow::Borrowed(end));
            self.quote_opener = start;
            self.quote_is_verbatim = false;
            self.quote_is_doc_quote = false;
            return Some(start.len());
        }

        None
    }

    /// The declared opener `prefix` + quote, with its closer, whether it is
    /// verbatim, and the length of the quote, for a quote at the start of
    /// `window` that directly follows an identifier, in a language whose
    /// quotes take the identifier as a prefix.
    fn identifier_prefixed_opener(
        &self,
        window: &[u8],
    ) -> Option<(&'static str, &'static str, bool, usize)> {
        let prefix = self.shared.identifier_quote_prefix?;
        let quote_len = |opener: &str| {
            opener
                .strip_prefix(prefix)
                .filter(|quote| window.starts_with(quote.as_bytes()))
                .map(str::len)
        };
        let verbatim = self
            .shared
            .verbatim_string_literals
            .iter()
            .map(|&(s, e)| (s, e, true));
        let plain = self
            .shared
            .string_literals
            .iter()
            .map(|&(s, e)| (s, e, false));
        verbatim
            .chain(plain)
            .find_map(|(start, end, verbatim)| Some((start, end, verbatim, quote_len(start)?)))
    }

    /// Length of the code token starting `window` that only looks like a
    /// comment or string opener, a declared token or a code pragma.
    pub(crate) fn code_token(&self, window: &[u8]) -> Option<usize> {
        if !self.is_plain_mode() {
            return None;
        }
        self.shared
            .code_pragmas
            .and_then(|pragmas| pragmas.code_len(window))
            .or_else(|| {
                self.shared
                    .code_tokens
                    .iter()
                    .find(|e| window.starts_with(e.as_bytes()))
                    .map(|e| e.len())
            })
            .or_else(|| {
                // psql reads a dollar-quoted body as a string, without
                // meta-commands (src/bin/psql/psqlscan.l, `xdolq`).
                (self.dollar_bodies.is_empty()
                    && self
                        .shared
                        .code_line_markers
                        .iter()
                        .any(|m| window.starts_with(m.as_bytes())))
                .then(|| memchr::memchr(b'\n', window).unwrap_or(window.len()))
            })
    }

    /// Length of the character literal starting `window`: an escape running to
    /// the closing delimiter on the same line, or exactly one character. Any
    /// other opener, such as a Rust lifetime `'a`, is not a literal and stays
    /// code, so a literal can never carry string state to the next line.
    pub(crate) fn parse_character_literal(&self, lines: &[u8], i: usize) -> Option<usize> {
        if !self.is_plain_mode() || (self.shared.postfix_primes && julia::follows_operand(lines, i))
        {
            return None;
        }
        let window = &lines[i..];
        let (start, end) = self
            .shared
            .character_literals
            .iter()
            .find(|(s, _)| opens_with(window, s))?;
        let body = &window[start.len()..];
        let width = utf8_len(*body.first()?);
        let close = end.as_bytes();
        let Some(&first_close) = close.first() else {
            // A literal without a closing delimiter, such as Lisp's `#\"`,
            // holds exactly one character.
            return Some(start.len() + width);
        };
        let len = if body[0] == b'\\' {
            2 + body
                .get(2..)?
                .iter()
                .take_while(|b| **b != b'\n')
                .position(|b| *b == first_close)?
        } else {
            width
        };
        body.get(len..)?
            .starts_with(close)
            .then_some(start.len() + len + close.len())
    }

    #[inline]
    pub(crate) fn parse_end_of_quote(&mut self, lines: &[u8], i: usize) -> Option<usize> {
        let window = &lines[i..];
        #[allow(clippy::if_same_then_else)]
        if let Some((quote, len)) = self
            .quote
            .as_deref()
            .and_then(|quote| Some((quote, self.closer_len(window, quote)?)))
        {
            // Elsewhere on its line, a heredoc's closer is text.
            if (self.quote_closes_at_line_start || self.shared.line_start_closers.contains(&quote))
                && !starts_line(lines, i)
            {
                return Some(1);
            }
            // A run of more quotes closes at its end, the ones before the
            // closer being text (Kotlin grammar, TRIPLE_QUOTE_CLOSE).
            if self.shared.run_closers.contains(&quote)
                && window.get(len) == quote.as_bytes().last()
            {
                return Some(1);
            }
            if self.quote_is_verbatim
                && self.shared.doubled_quote_escapes.contains(&quote)
                && window[len..].starts_with(quote.as_bytes())
            {
                return Some(len * 2);
            }
            trace!("End {:?}", quote);
            self.quote = None;
            self.quote_escape = None;
            self.quote_closes_at_line_start = false;
            self.quote_opener = "";
            self.quote_is_verbatim = false;
            Some(len)
        } else if let Some(len) = self.string_gap_len(lines, i) {
            Some(len)
        } else if let Some(escape) = self
            .quote_escape
            .as_deref()
            .filter(|escape| window.starts_with(escape.as_bytes()))
        {
            Some(escape.len() + 1)
        } else if let Some(skip_amount) = self.parse_start_of_hole(window) {
            Some(skip_amount)
        } else if !self.escapes_here() {
            None
        } else if !self.quote_is_verbatim && window.starts_with(&[self.shared.escape; 2]) {
            Some(2)
        } else if !self.quote_is_verbatim
            && window.first() == Some(&self.shared.escape)
            && (self
                .shared
                .string_literals
                .iter()
                .chain(self.shared.verbatim_string_literals)
                .any(|(start, _)| opens_with(&window[1..], start))
                || self
                    .quote
                    .as_deref()
                    .is_some_and(|quote| window[1..].starts_with(quote.as_bytes()))
                || (self.quote.is_none()
                    && self
                        .shared
                        .line_comments
                        .iter()
                        .any(|comment| window[1..].starts_with(comment.as_bytes()))))
        {
            // Tell the state machine to skip the next character because it
            // has been escaped if the string isn't a verbatim string. The
            // closer need not open a string itself, as with shell's `$'…'`.
            // Outside a literal, an escaped line comment marker is text, as
            // make's `\#` is (GNU make manual, 3.1 What Makefiles Contain).
            Some(2)
        } else {
            None
        }
    }

    /// Length of the `closer` that `window` starts with, its leading quote
    /// character perhaps written as one of the language's aliases.
    fn closer_len(&self, window: &[u8], closer: &str) -> Option<usize> {
        if window.starts_with(closer.as_bytes()) {
            return Some(closer.len());
        }
        self.shared
            .quote_aliases
            .iter()
            .find_map(|&(quote, alias)| {
                let rest = closer.strip_prefix(quote)?;
                (window.starts_with(alias.as_bytes())
                    && window[alias.len()..].starts_with(rest.as_bytes()))
                .then_some(alias.len() + rest.len())
            })
    }

    /// Whether the language's escape character at the current position
    /// escapes the next byte: inside a literal, and outside one unless the
    /// language makes it an operator. Outside a literal it is the shells'
    /// quoting of a single character (POSIX Shell Command Language, 2.2.1).
    fn escapes_here(&self) -> bool {
        self.quote.is_some() || !self.shared.backslash_operator
    }

    /// Inside an interpolating string, skips text that only looks like a hole
    /// or is escaped, or suspends the string at the start of a hole.
    fn parse_start_of_hole(&mut self, window: &[u8]) -> Option<usize> {
        let opener = self.quote_opener;
        if !self.shared.hole_bytes[usize::from(*window.first()?)]
            || !self.shared.interpolating_quote_bytes[usize::from(*opener.as_bytes().first()?)]
        {
            return None;
        }
        let mut kinds = self
            .shared
            .interpolations
            .iter()
            .filter(|kind| kind.quote == opener)
            .peekable();
        kinds.peek()?;
        let mut hole = None;
        for kind in kinds {
            if let Some(literal) = kind
                .literal
                .iter()
                .find(|l| window.starts_with(l.as_bytes()))
            {
                return Some(literal.len());
            }
            if hole.is_none() && window.starts_with(kind.open.as_bytes()) {
                hole = Some(kind);
            }
        }
        let Some(kind) = hole else {
            // An escaped character, such as the `$` of JavaScript's `\${`,
            // cannot open a hole.
            return (!self.quote_is_verbatim && window.first() == Some(&self.shared.escape))
                .then_some(2);
        };
        if kind.run_opens {
            let run = window
                .iter()
                .take_while(|&&b| b == kind.open.as_bytes()[0])
                .count();
            if run > kind.open.len() {
                return Some(run - kind.open.len());
            }
        }
        trace!("Start hole {:?}", kind.open);
        self.open_hole(kind.close, kind.nest, kind.language);
        Some(kind.open.len())
    }

    /// Suspends the string being read, if any, for a hole up to `close`,
    /// whose contents `language` reads, if given, instead of the host's.
    pub(crate) fn open_hole(
        &mut self,
        close: &'static str,
        nest: u8,
        language: Option<LanguageType>,
    ) {
        self.holes.push(OpenHole {
            quote: self.quote.take(),
            quote_opener: std::mem::take(&mut self.quote_opener),
            quote_escape: self.quote_escape.take(),
            quote_is_verbatim: self.quote_is_verbatim,
            close,
            nest,
            depth: 0,
            host: language.map(|_| self.shared.clone()),
            in_tag: self.in_tag,
        });
        self.quote_is_verbatim = false;
        if let Some(language) = language {
            self.in_tag = false;
            self.shared = SharedMatchers::new(language);
        }
    }

    /// Inside a hole, outside its strings and comments, tracks nested brackets
    /// and resumes the suspended string at the hole's closer.
    fn parse_end_of_hole(&mut self, window: &[u8]) -> Option<usize> {
        if !self.stack.is_empty() {
            return None;
        }
        let hole = self.holes.last_mut()?;
        if hole.depth == 0 && window.starts_with(hole.close.as_bytes()) {
            let hole = self.holes.pop()?;
            trace!("End hole {:?}", hole.close);
            self.quote = hole.quote;
            self.quote_opener = hole.quote_opener;
            self.quote_escape = hole.quote_escape;
            self.quote_is_verbatim = hole.quote_is_verbatim;
            if let Some(host) = hole.host {
                self.shared = host;
                self.in_tag = hole.in_tag;
            }
            return Some(hole.close.len());
        }
        if window[0] == hole.nest {
            hole.depth += 1;
        } else if Some(&window[0]) == hole.close.as_bytes().first() {
            hole.depth = hole.depth.saturating_sub(1);
        }
        None
    }

    /// Opens the here-document starting at `lines[i]`, whose body begins on
    /// the next line or after the bodies of here-documents opened before it
    /// on this line.
    fn parse_heredoc(&mut self, lines: &[u8], i: usize) -> Option<usize> {
        if self.shared.heredocs.is_empty() || !self.is_plain_mode() {
            return None;
        }
        self.shared.heredocs.iter().find_map(|heredoc| {
            let opening = heredoc.parse_opener(lines, i)?;
            let next_line = i + lines[i..].iter().position(|b| *b == b'\n')? + 1;
            let from = next_line.max(self.literal_end);
            self.literal_end = heredoc.closing_line_end(&opening, lines, from)?;
            trace!("Start heredoc, body ends at {}", self.literal_end);
            Some(opening.len)
        })
    }

    /// Whether a word can start at `lines[i]`: at the start of the text, or
    /// after whitespace or one of the language's word delimiters, such as a
    /// shell operator character (POSIX.1-2024, Shell Command Language §2.3,
    /// rules 6, 8 and 9) or a backquote, whose text is read as a script of
    /// its own (§2.6.3); a `#` right after a closing backquote, which goes on
    /// with the word, is taken for a comment too.
    fn starts_word(&self, lines: &[u8], i: usize) -> bool {
        match lines[..i].last() {
            None => true,
            Some(b) => b.is_ascii_whitespace() || self.shared.word_delimiters.contains(b),
        }
    }

    /// Length of the regular expression literal at `lines[i]`, in plain mode
    /// for languages that have them.
    fn parse_regex_literal(&self, lines: &[u8], i: usize) -> Option<usize> {
        if !self.is_plain_mode() {
            return None;
        }
        regex_literal_len(lines, i, &self.expression_context()?)
    }

    /// Length of the literal with chosen delimiters at `lines[i]`, such as
    /// Perl's `qw(…)` or Ruby's `%w[…]`, in plain mode.
    fn parse_quote_like(&mut self, lines: &[u8], i: usize) -> Option<usize> {
        if !self.is_plain_mode() {
            return None;
        }
        match self.shared.quote_like? {
            QuoteLikeSyntax::Perl => {
                let (open, parts) = perl::quote_operator(lines, i)?;
                delimited_len(lines, i, open, parts)
            }
            QuoteLikeSyntax::RubyPercent => {
                let (open, parts) = ruby::percent_literal(lines, i, &self.expression_context()?)?;
                delimited_len(lines, i, open, parts)
            }
            QuoteLikeSyntax::ElixirSigil => elixir::sigil_len(lines, i),
            QuoteLikeSyntax::ErlangSigil => erlang::sigil_len(lines, i),
            QuoteLikeSyntax::HaskellQuasiQuote => {
                haskell::quasi_quote_len(lines, i, || self.quasi_quoting(lines))
            }
            QuoteLikeSyntax::MakeReference => make::reference_len(lines, i),
            QuoteLikeSyntax::M4 { open, close } => m4::quote_len(lines, i, open, close),
        }
    }

    /// Length of the text section at `lines[i]`, outside tags and literals,
    /// up to its closer or the end of the text: only the closer is markup
    /// in it (XML 1.0, §2.7 "CDATA Sections").
    fn text_section_len(&self, lines: &[u8], i: usize) -> Option<usize> {
        if !self.is_plain_mode() || self.in_tag {
            return None;
        }
        let window = &lines[i..];
        let &(open, close) = self
            .shared
            .text_sections
            .iter()
            .find(|(open, _)| window.starts_with(open.as_bytes()))?;
        let text = &window[open.len()..];
        Some(
            open.len()
                + memchr::memmem::find(text, close.as_bytes())
                    .map_or(text.len(), |at| at + close.len()),
        )
    }

    /// Length of the XML literal at `lines[i]`, outside comments, whose text
    /// counts as text literals' does.
    fn xml_literal_len(&self, lines: &[u8], i: usize) -> Option<usize> {
        (self.shared.xml_literals
            && lines[i] == b'<'
            && self.stack.is_empty()
            && visual_basic::expression_may_start(lines, i))
        .then(|| visual_basic::xml_literal_len(lines, i))
        .flatten()
    }

    /// The syntax that tells where an expression may start, for languages
    /// with regular expression literals.
    pub(crate) fn expression_context(&self) -> Option<RegexContext<'static>> {
        Some(RegexContext {
            keywords: self.shared.regex_keywords?,
            after_line_break: self.shared.regex_after_line_break,
            spans_lines: self.shared.regex_spans_lines,
            operator_symbols: self.shared.regex_operator_symbols,
            command_arguments: self.shared.regex_command_arguments,
            line_comments: self.shared.line_comments,
            multi_line_comments: self.shared.any_multi_line_comments,
        })
    }

    #[inline]
    pub(crate) fn parse_multi_line_comment(&mut self, window: &[u8]) -> Option<usize> {
        if self.quote.is_some() {
            return None;
        }

        if !self.stack.is_empty() {
            // A code token is one token in a comment too, so F#'s `(*)`
            // opens no nested comment there (lex.fsl, rule `comment`).
            let text_len = self
                .shared
                .code_tokens
                .iter()
                .find(|token| window.starts_with(token.as_bytes()))
                .map(|token| token.len())
                .or_else(|| {
                    self.shared
                        .comment_literals
                        .and_then(|literals| literals.len(window))
                });
            if text_len.is_some() {
                return text_len;
            }
        }

        if self.stack.is_empty() || self.shared.allows_nested {
            if let Some(opening) = self
                .shared
                .raw_comments
                .iter()
                .find_map(|raw| raw.parse_opener(window))
            {
                trace!("Start raw comment, closed by {:?}", opening.closer);
                self.stack.push(Cow::Owned(opening.closer));
                return Some(opening.len);
            }
        }

        let iter = self
            .shared
            .multi_line_comments
            .iter()
            .chain(self.shared.nested_comments);
        for &(start, end) in iter {
            // Inside a comment that does not nest, an opener means nothing,
            // so it must not hide a closer it overlaps, as `<!-->` closes an
            // HTML comment and `/*/` a C one.
            if window.starts_with(start.as_bytes())
                && (self.stack.is_empty()
                    || self.shared.allows_nested
                    || self.shared.nested_comments.contains(&(start, end)))
            {
                self.stack.push(Cow::Borrowed(end));

                if log_enabled!(Trace) && self.shared.allows_nested {
                    trace!("Start nested {:?}", start);
                } else {
                    trace!("Start {:?}", start);
                }

                return Some(start.len());
            }
        }

        None
    }

    #[inline]
    pub(crate) fn parse_end_of_multi_line(&mut self, window: &[u8]) -> Option<usize> {
        if self
            .stack
            .last()
            .map_or(false, |l| window.starts_with(l.as_bytes()))
        {
            let last = self.stack.pop().unwrap();

            if log_enabled!(Trace) {
                if self.stack.is_empty() {
                    trace!("End {:?}", last);
                } else {
                    trace!("End {:?}. Still in comments.", last);
                }
            }

            Some(last.len())
        } else {
            None
        }
    }
}

/// The language a `lang` attribute names, by name or, as Vue's
/// `<style lang="scss">` does, by file extension (Vue SFC specification,
/// "Pre-Processors").
fn lang_language(lang: &[u8]) -> Option<LanguageType> {
    use std::str::FromStr;
    let lang = String::from_utf8_lossy(lang.trim()).to_lowercase();
    LanguageType::from_str(&lang)
        .ok()
        .or_else(|| LanguageType::from_file_extension(&lang))
}

/// The position of the newline that ends the line holding `lines[at]`, or
/// the end of `lines`; `at` may lie past it.
pub(crate) fn line_end(lines: &[u8], at: usize) -> usize {
    let at = at.min(lines.len());
    memchr::memchr(b'\n', &lines[at..]).map_or(lines.len(), |n| at + n)
}

/// The start of the line holding `lines[at]`.
pub(crate) fn line_start(lines: &[u8], at: usize) -> usize {
    memchr::memrchr(b'\n', &lines[..at]).map_or(0, |n| n + 1)
}

/// Whether nothing but spaces and tabs precede `lines[i]` on its line.
pub(crate) fn starts_line(lines: &[u8], i: usize) -> bool {
    lines[..i]
        .iter()
        .rev()
        .take_while(|b| **b != b'\n')
        .all(|b| matches!(b, b' ' | b'\t'))
}

/// Whether `window` starts with `text`, comparing the first byte before the
/// rest, as most of Python's 82 quote openers are compared at every quote
/// and escaped byte without sharing it.
fn opens_with(window: &[u8], text: &str) -> bool {
    text.as_bytes()
        .first()
        .map_or(true, |first| window.first() == Some(first))
        && window.starts_with(text.as_bytes())
}

/// Whether `window` starts a markup tag: `<` followed by a name, `/`, `!` or
/// `?`, as in `<p`, `</p`, `<!DOCTYPE` or `<?xml`, not by a space, as in
/// `a < b`.
fn starts_tag(window: &[u8]) -> bool {
    window.first() == Some(&b'<')
        && window
            .get(1)
            .is_some_and(|b| b.is_ascii_alphabetic() || b"/!?".contains(b))
}

/// The length of the template expression that starts `window`, such as
/// `{{ $a->b }}`, up to its closer, or `None` when none starts or closes there.
fn template_expression_len(window: &[u8], expressions: &[(&str, &str)]) -> Option<usize> {
    let (open, close) = expressions
        .iter()
        .find(|(open, _)| window.starts_with(open.as_bytes()))?;
    let body = &window[open.len()..];
    let end = body
        .windows(close.len())
        .position(|w| w == close.as_bytes())?;
    Some(open.len() + end + close.len())
}

/// The length of the block comment that opens at the start of `text`, up to
/// and including its closer, or `None` when it does not close in `text`.
pub(crate) fn block_comment_len(text: &[u8], start: &str, end: &str, nests: bool) -> Option<usize> {
    let mut depth = 0usize;
    let mut i = 0;
    while i < text.len() {
        let rest = &text[i..];
        if rest.starts_with(start.as_bytes()) && (depth == 0 || nests) {
            depth += 1;
            i += start.len();
        } else if rest.starts_with(end.as_bytes()) {
            depth -= 1;
            i += end.len();
            if depth == 0 {
                return Some(i);
            }
        } else {
            i += 1;
        }
    }
    None
}
