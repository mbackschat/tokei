{% macro raw_delimiters(items) -%}
    {%- for raw in items -%}
        crate::language::raw_delimiter::RawDelimiter {
            start: "{{ raw.start | default(value="") }}",
            middle: {% if raw.repeat is defined -%}
                crate::language::raw_delimiter::RawMiddle::Repeat { byte: b'{{ raw.repeat }}', min: {{ raw.min_repeat | default(value=0) }} }
            {%- else -%}
                crate::language::raw_delimiter::RawMiddle::Label { chars: "{{ raw.label }}", max: {% if raw.max_label is defined %}{{ raw.max_label }}{% else %}usize::MAX{% endif %} }
            {%- endif %},
            open: "{{ raw.open | default(value="") }}",
            close: "{{ raw.close }}",
            end: "{{ raw.end | default(value="") }}",
            escape: "{{ raw.escape | default(value="") }}",
            single_line: {{ raw.single_line | default(value=false) }},
        },
    {%- endfor -%}
{%- endmacro raw_delimiters %}
{% macro heredocs(items) -%}
    {%- for heredoc in items -%}
        crate::language::heredoc::Heredoc {
            start: "{{ heredoc.start }}",
            indent_flags: "{{ heredoc.indent_flags | default(value="") }}",
            spacing: crate::language::heredoc::HeredocSpacing::
                {%- if heredoc.spacing == "adjacent" %}Adjacent{% elif heredoc.spacing == "quoted_only" %}QuotedOnly{% else %}Any{% endif %},
            word: crate::language::heredoc::HeredocWord::
                {%- if heredoc.word == "shell" %}Shell{% else %}Identifier{% endif %},
            closer: crate::language::heredoc::HeredocCloser::
                {%- if heredoc.closer == "flexible" %}Flexible{% else %}Line{% endif %},
        },
    {%- endfor -%}
{%- endmacro heredocs %}
use arbitrary::Arbitrary;

/// Represents a individual programming language. Can be used to provide
/// information about the language, such as multi line comments, single line
/// comments, string literal syntax, whether a given language allows nesting
/// comments.
#[derive(Deserialize)]
#[derive(Arbitrary, Clone, Copy, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
#[non_exhaustive]
#[allow(clippy::upper_case_acronyms)]
pub enum LanguageType {
    {% for key, value in languages -%}
        #[allow(missing_docs)] {% if value.name is defined %} #[serde(alias = "{{value.name}}")] {% else %} #[serde(alias = "{{key}}")] {% endif %} {{key}},
    {% endfor %}
}

impl LanguageType {

    /// Returns the display name of a language.
    ///
    /// ```
    /// # use tokei::*;
    /// let bash = LanguageType::Bash;
    ///
    /// assert_eq!(bash.name(), "BASH");
    /// ```
    pub fn name(self) -> &'static str {
        match self {
            {% for key, value in languages -%}
                {{key}} => {% if value.name %}"{{value.name}}"{% else %}"{{key}}"{% endif %},
            {% endfor %}
        }
    }

    pub(crate) fn _is_blank(self) -> bool {
        match self {
            {% for key, v in languages -%}
                {{key}} => {{ v.blank | default(value=false) }},
            {% endfor %}
        }
    }

    pub(crate) fn is_fortran(self) -> bool {
        self == LanguageType::FortranModern ||
        self == LanguageType::FortranLegacy
    }

    /// Returns whether the language is "literate", meaning that it considered
    /// to primarily be documentation and is counted primarily as comments
    /// rather than procedural code.
    pub fn is_literate(self) -> bool {
        match self {
            {% for key, v in languages -%}
                {{key}} => {{ v.literate | default(value=false) }},
            {% endfor %}
        }
    }

    /// Provides every variant in a Vec
    pub fn list() -> &'static [(Self, &'static [&'static str])] {
        &[{% for key, val in languages -%}
            ({{key}},
            {% if val.extensions %} &[{% for extension in val.extensions %}"{{extension}}", {% endfor %}],
            {% else %} &[],
            {% endif %}),
        {% endfor %}]
    }

    /// Returns the single line comments of a language.
    /// ```
    /// use tokei::LanguageType;
    /// let lang = LanguageType::Rust;
    /// assert_eq!(lang.line_comments(), &["//"]);
    /// ```
    pub fn line_comments(self) -> &'static [&'static str] {
        match self {
            {% for key, value in languages -%}
                {{key}} => &[{% for item in value.line_comment | default(value=[]) %}"{{item}}",{% endfor %}],
            {% endfor %}
        }
    }

    /// Returns the single line comments of a language.
    /// ```
    /// use tokei::LanguageType;
    /// let lang = LanguageType::Rust;
    /// assert_eq!(lang.multi_line_comments(), &[("/*", "*/")]);
    /// ```
    pub fn multi_line_comments(self) -> &'static [(&'static str, &'static str)]
    {
        match self {
            {% for key, value in languages -%}
                {{key}} => &[
                    {%- for items in value.multi_line_comments | default(value=[]) -%}
                        ({% for item in items %}"{{item}}",{% endfor %}),
                    {%- endfor -%}
                ],
            {% endfor %}
        }
    }


    /// Returns whether the language allows nested multi line comments.
    /// ```
    /// use tokei::LanguageType;
    /// let lang = LanguageType::Rust;
    /// assert!(lang.allows_nested());
    /// ```
    pub fn allows_nested(self) -> bool {
        match self {
            {% for key, v in languages -%}
                {{key}} => {{ v.nested | default(value=false) }},
            {% endfor %}
        }
    }

    /// Returns what nested comments the language has. (Currently only D has
    /// any of this type.)
    /// ```
    /// use tokei::LanguageType;
    /// let lang = LanguageType::D;
    /// assert_eq!(lang.nested_comments(), &[("/+", "+/")]);
    /// ```
    pub fn nested_comments(self) -> &'static [(&'static str, &'static str)]
    {
        match self {
            {% for key, value in languages -%}
                {{key}} => &[
                    {%- for items in value.nested_comments | default(value=[]) -%}
                        ({% for item in items %}"{{item}}",{% endfor %}),
                    {%- endfor -%}
                ],
            {% endfor %}
        }
    }

    /// Returns the quotes of a language.
    /// ```
    /// use tokei::LanguageType;
    /// let lang = LanguageType::C;
    /// assert_eq!(lang.quotes(), &[("\"", "\"")]);
    /// ```
    pub fn quotes(self) -> &'static [(&'static str, &'static str)] {
        match self {
            {% for key, value in languages -%}
                {{key}} => &[
                    {%- for items in value.quotes | default(value=[]) -%}
                        ({% for item in items %}"{{item}}",{% endfor %}),
                    {%- endfor -%}
                ],
            {% endfor %}
        }
    }

    /// Returns the closing delimiters of a language's verbatim quotes that stand
    /// for themselves when written twice, as in C#'s `@"say ""hi"""`.
    /// ```
    /// use tokei::LanguageType;
    /// let lang = LanguageType::CSharp;
    /// assert_eq!(lang.doubled_quote_escapes(), &["\""]);
    /// ```
    pub fn doubled_quote_escapes(self) -> &'static [&'static str] {
        match self {
            {% for key, value in languages -%}
                {{key}} => &[
                    {%- for item in value.doubled_quote_escapes | default(value=[]) -%}
                        "{{item}}",
                    {%- endfor -%}
                ],
            {% endfor %}
        }
    }

    /// Returns the literals of a language whose closer repeats a variable part
    /// of the opener, such as Rust's `r#"…"#`; the first matching one wins.
    pub(crate) fn raw_strings(self) -> &'static [crate::language::raw_delimiter::RawDelimiter] {
        match self {
            {% for key, value in languages -%}
                {{key}} => &[{{ self::raw_delimiters(items=value.raw_strings | default(value=[])) }}],
            {% endfor %}
        }
    }

    /// Returns the here-documents of a language, such as shell's `<<EOF`; the
    /// first matching one wins.
    pub(crate) fn heredocs(self) -> &'static [crate::language::heredoc::Heredoc] {
        match self {
            {% for key, value in languages -%}
                {{key}} => &[{{ self::heredocs(items=value.heredocs | default(value=[])) }}],
            {% endfor %}
        }
    }

    /// Returns the comments of a language whose closer repeats a variable part
    /// of the opener, such as Lua's `--[==[…]==]`; the first matching one wins.
    pub(crate) fn raw_comments(self) -> &'static [crate::language::raw_delimiter::RawDelimiter] {
        match self {
            {% for key, value in languages -%}
                {{key}} => &[{{ self::raw_delimiters(items=value.raw_comments | default(value=[])) }}],
            {% endfor %}
        }
    }

    /// Returns the holes of a language's strings whose contents are code, such
    /// as `${…}` in JavaScript template literals.
    pub(crate) fn interpolations(self) -> &'static [crate::language::interpolation::Interpolation] {
        match self {
            {% for key, value in languages -%}
                {{key}} => &[
                    {%- for hole in value.interpolations | default(value=[]) -%}
                        crate::language::interpolation::Interpolation {
                            quote: "{{ hole.quote }}",
                            open: "{{ hole.open }}",
                            close: "{{ hole.close }}",
                            nest: b'{{ hole.nest }}',
                            literal: &[{% for text in hole.literal | default(value=[]) %}"{{ text }}",{% endfor %}],
                        },
                    {%- endfor -%}
                ],
            {% endfor %}
        }
    }

    /// Returns the keywords after which `/` starts a regular expression, or
    /// `None` for a language without regular expression literals.
    pub(crate) fn regex_keywords(self) -> Option<&'static [&'static str]> {
        match self {
            {% for key, value in languages -%}
                {{key}} => {% if value.regex_literals is defined -%}
                    Some(&[{% for keyword in value.regex_literals.after_keywords | default(value=[]) %}"{{ keyword }}",{% endfor %}])
                {%- else -%}
                    None
                {%- endif %},
            {% endfor %}
        }
    }

    /// Returns whether a line comment marker opens a comment only where a word
    /// could start, as `#` does in POSIX shells, so that `$#` is code.
    pub(crate) fn line_comments_start_words(self) -> bool {
        match self {
            {% for key, value in languages -%}
                {{key}} => {{ value.line_comment_starts_word | default(value=false) }},
            {% endfor %}
        }
    }

    /// Returns whether a line beginning with `=` and a word starts a Perl POD
    /// block (perlpod).
    pub(crate) fn pod(self) -> bool {
        match self {
            {% for key, value in languages -%}
                {{key}} => {{ value.pod | default(value=false) }},
            {% endfor %}
        }
    }

    /// Returns the lines, such as Perl's `__END__`, after which the text is
    /// data rather than code.
    pub(crate) fn end_markers(self) -> &'static [&'static str] {
        match self {
            {% for key, value in languages -%}
                {{key}} => &[{% for item in value.end_markers | default(value=[]) %}"{{ item }}",{% endfor %}],
            {% endfor %}
        }
    }

    /// Returns the line comment markers that open a comment only at the start
    /// of a line, as JavaScript's `-->` does (ECMA-262 Annex B.1.1).
    pub(crate) fn line_start_comments(self) -> &'static [&'static str] {
        match self {
            {% for key, value in languages -%}
                {{key}} => &[{% for item in value.line_start_comments | default(value=[]) %}"{{ item }}",{% endfor %}],
            {% endfor %}
        }
    }

    /// Returns whether a `/` that starts a line opens a regular expression,
    /// as in Ruby, where a line break ends the statement before it.
    pub(crate) fn regex_after_line_break(self) -> bool {
        match self {
            {% for key, value in languages -%}
                {{key}} => {% if value.regex_literals is defined %}{{ value.regex_literals.after_line_break | default(value=false) }}{% else %}false{% endif %},
            {% endfor %}
        }
    }

    /// Returns the characters besides whitespace after which a word can
    /// start, for languages whose comments or quotes must start a word.
    pub(crate) fn word_delimiters(self) -> &'static str {
        match self {
            {% for key, value in languages -%}
                {{key}} => "{{ value.word_delimiters | default(value="") }}",
            {% endfor %}
        }
    }

    /// Returns the texts after which a quote can open a string, for
    /// languages where a quote opens one only at the start of a value, as in
    /// YAML, where a quote inside a plain scalar is text. A text ending in a
    /// space needs whitespace before the quote; a quote at the start of a line
    /// always can open a string.
    pub(crate) fn quotes_after(self) -> Option<&'static [&'static str]> {
        match self {
            {% for key, value in languages -%}
                {{key}} => {% if value.quotes_after is defined %}Some(&[{% for text in value.quotes_after %}"{{ text }}",{% endfor %}]){% else %}None{% endif %},
            {% endfor %}
        }
    }

    /// Returns whether the language has YAML block scalars, such as `run: |`.
    pub(crate) fn block_scalars(self) -> bool {
        match self {
            {% for key, value in languages -%}
                {{key}} => {{ value.block_scalars | default(value=false) }},
            {% endfor %}
        }
    }

    /// Returns whether the language has JSX elements, whose text between tags
    /// is not code.
    pub(crate) fn jsx(self) -> bool {
        match self {
            {% for key, value in languages -%}
                {{key}} => {{ value.jsx | default(value=false) }},
            {% endfor %}
        }
    }

    /// Returns whether quotes open strings only inside markup tags, such as
    /// HTML's `<p class='a'>`, so that an apostrophe in text is text.
    pub(crate) fn quotes_in_tags(self) -> bool {
        match self {
            {% for key, value in languages -%}
                {{key}} => {{ value.quotes_in_tags | default(value=false) }},
            {% endfor %}
        }
    }

    /// Returns the tokens of a language that start like a comment or a string
    /// but are code, such as PHP's `#[` attributes, F#'s `(*)` operator or
    /// Ruby's `$'` global.
    /// ```
    /// use tokei::LanguageType;
    /// let lang = LanguageType::Php;
    /// assert_eq!(lang.code_tokens(), &["#["]);
    /// ```
    pub fn code_tokens(self) -> &'static [&'static str] {
        match self {
            {% for key, value in languages -%}
                {{key}} => &[
                    {%- for item in value.code_tokens | default(value=[]) -%}
                        "{{item}}",
                    {%- endfor -%}
                ],
            {% endfor %}
        }
    }

    /// Returns the character literals of a language, as opening and closing
    /// delimiters. An empty closing delimiter marks a literal that is the
    /// opener followed by exactly one character, such as Lisp's `#\a`.
    /// ```
    /// use tokei::LanguageType;
    /// let lang = LanguageType::C;
    /// assert_eq!(lang.character_literals(), &[("'", "'")]);
    /// ```
    pub fn character_literals(self) -> &'static [(&'static str, &'static str)] {
        match self {
            {% for key, value in languages -%}
                {{key}} => &[
                    {%- for items in value.character_literals | default(value=[]) -%}
                        ({% for item in items %}"{{item}}",{% endfor %}),
                    {%- endfor -%}
                ],
            {% endfor %}
        }
    }

    /// Returns the verbatim quotes of a language.
    /// ```
    /// use tokei::LanguageType;
    /// let lang = LanguageType::CSharp;
    /// assert!(lang.verbatim_quotes().contains(&("@\"", "\"")));
    /// ```
    pub fn verbatim_quotes(self) -> &'static [(&'static str, &'static str)] {
        match self {
            {% for key, value in languages -%}
                {{key}} => &[
                    {%- for items in value.verbatim_quotes | default(value=[]) -%}
                        ({% for item in items %}"{{item}}",{% endfor %}),
                    {%- endfor -%}
                ],
            {% endfor %}
        }
    }

    /// Returns the doc quotes of a language.
    /// ```
    /// use tokei::LanguageType;
    /// let lang = LanguageType::Python;
    /// assert_eq!(lang.doc_quotes(), &[("\"\"\"", "\"\"\""), ("'''", "'''")]);
    /// ```
    pub fn doc_quotes(self) -> &'static [(&'static str, &'static str)] {
        match self {
            {% for key, value in languages -%}
                {{key}} => &[
                    {% for items in value.doc_quotes | default(value=[])-%}
                        ({% for item in items %}"{{item}}",{% endfor %}),
                    {%- endfor %}
                ],
            {%- endfor %}
        }
    }

    /// Returns the shebang of a language.
    /// ```
    /// use tokei::LanguageType;
    /// let lang = LanguageType::Bash;
    /// assert_eq!(lang.shebangs(), &["#!/bin/bash"]);
    /// ```
    pub fn shebangs(self) -> &'static [&'static str] {
        match self {
            {% for key, lang in languages -%}
                {{key}} => &[{% for item in lang.shebangs | default(value=[]) %}"{{item}}",{% endfor %}],
            {% endfor %}
        }
    }

    pub(crate) fn any_multi_line_comments(self) -> &'static [(&'static str, &'static str)] {
        match self {
            {% for key, value in languages -%}
                {{key}} => &[
                {%- set starting_multi_line_comments = value.multi_line_comments | default(value=[]) -%}
                {%- set starting_nested_comments = value.nested_comments | default(value=[]) -%}
                    {%- for item in starting_multi_line_comments | concat(with=starting_nested_comments) -%}
                        ("{{item.0}}", "{{item.1}}"),
                    {%- endfor -%}
                ],
            {% endfor %}
        }
    }

    pub(crate) fn any_comments(self) -> &'static [&'static str] {
        match self {
            {% for key, value in languages -%}
                {{key}} => &[
                {%- set starting_multi_line_comments = value.multi_line_comments | default(value=[]) -%}
                {%- set starting_nested_comments = value.nested_comments | default(value=[]) -%}

                    {%- for item in starting_multi_line_comments | concat(with=starting_nested_comments) -%}
                        "{{item.0}}",
                        "{{item.1}}",
                    {%- endfor -%}
                    {%- for item in value.line_comment | default(value=[]) -%}
                        "{{item}}",
                    {%- endfor -%}
                ],
            {% endfor %}
        }
    }

    /// Returns the parts of syntax that determines whether tokei can skip large
    /// parts of analysis.
    pub fn important_syntax(self) -> &'static [&'static str] {
        match self {
            {% for key, value in languages -%}
                {%- set starting_quotes = value.quotes | default(value=[]) | map(attribute="0") -%}
                {%- set starting_doc_quotes = value.doc_quotes | default(value=[]) | map(attribute="0") -%}
                {#- A line that only opens a verbatim quote, such as a Go backtick, must not skip analysis. -#}
                {%- set starting_verbatim_quotes = value.verbatim_quotes | default(value=[]) | map(attribute="0") -%}
                {%- set code_tokens = value.code_tokens | default(value=[]) -%}
                {%- set starting_multi_line_comments = value.multi_line_comments | default(value=[]) | map(attribute="0") -%}
                {%- set starting_nested_comments = value.nested_comments | default(value=[]) | map(attribute="0") -%}
                {%- set important_syntax = value.important_syntax | default(value=[]) -%}

                {{key}} => &[
                    {%- for item in starting_quotes |
                                   concat(with=starting_doc_quotes) |
                                   concat(with=starting_verbatim_quotes) |
                                   concat(with=code_tokens) |
                                   concat(with=starting_multi_line_comments) |
                                   concat(with=starting_nested_comments) |
                                   concat(with=important_syntax) -%}
                        "{{item}}",
                    {%- endfor -%}
                    {%- for context in value.contexts | default(value=[]) -%}
                        {% if value.kind == "html" %}
                            "<{{context.tag}}",
                        {% endif %}
                    {%- endfor -%}
                ],
            {% endfor %}
        }
    }

    /// Get language from a file path. May open and read the file.
    ///
    /// ```no_run
    /// use tokei::{Config, LanguageType};
    ///
    /// let rust = LanguageType::from_path("./main.rs", &Config::default());
    ///
    /// assert_eq!(rust, Some(LanguageType::Rust));
    /// ```
    pub fn from_path<P: AsRef<Path>>(entry: P, _config: &Config)
        -> Option<Self>
    {
        let entry = entry.as_ref();

        if let Some(filename) = fsutils::get_filename(entry) {
            match &*filename {
                {% for key, value in languages -%}
                    {%- if value.filenames -%}
                        {%- for item in value.filenames -%}
                            | "{{item}}"
                        {%- endfor -%}
                            => return Some({{key}}),
                    {% endif -%}
                {%- endfor %}
                _ => ()
            }

            {% for key, value in languages -%}
                {%- if value.path_suffixes -%}
                    {%- for suffix in value.path_suffixes %}
            if filename.ends_with("{{suffix}}") {
                return Some({{key}});
            }
                    {%- endfor %}
                {% endif -%}
            {%- endfor %}
        }

        match fsutils::get_extension(entry) {
            Some(extension) => LanguageType::from_file_extension(extension.as_str()),
            None => LanguageType::from_shebang(entry),
        }
    }

    /// Get language from a file extension.
    ///
    /// ```no_run
    /// use tokei::LanguageType;
    ///
    /// let rust = LanguageType::from_file_extension("rs");
    ///
    /// assert_eq!(rust, Some(LanguageType::Rust));
    /// ```
    #[must_use]
    pub fn from_file_extension(extension: &str) -> Option<Self> {
        match extension {
            {% for key, value in languages -%}
                {%- if value.extensions -%}
                    {%- for item in value.extensions  %}| "{{item}}" {% endfor %}=> Some({{key}}),
                {% endif -%}
            {%- endfor %}
            extension => {
                warn!("Unknown extension: {}", extension);
                None
            },
        }
    }

    /// Get language from its name.
    ///
    /// ```no_run
    /// use tokei::LanguageType;
    ///
    /// let rust = LanguageType::from_name("Rust");
    ///
    /// assert_eq!(rust, Some(LanguageType::Rust));
    /// ```
    #[must_use]
    pub fn from_name(name: &str) -> Option<Self> {
        match name {
            {% for key, value in languages -%}
                {% if value.name and value.name != key -%}
                    | "{{value.name}}"
                {% endif -%}
                    | "{{key}}" => Some({{key}}),
            {% endfor %}
            unknown => {
                warn!("Unknown language name: {}", unknown);
                None
            },
        }
    }

    /// Get language from its MIME type if available.
    ///
    /// ```no_run
    /// use tokei::LanguageType;
    ///
    /// let lang = LanguageType::from_mime("application/javascript");
    ///
    /// assert_eq!(lang, Some(LanguageType::JavaScript));
    /// ```
    #[must_use]
    pub fn from_mime(mime: &str) -> Option<Self> {
        match mime {
            {% for key, value in languages -%}
                {%- if value.mime -%}
                    {%- for item in value.mime  %}| "{{item}}" {% endfor %}=> Some({{key}}),
                {% endif -%}
            {%- endfor %}
            _ => {
                warn!("Unknown MIME: {}", mime);
                None
            },
        }
    }

    /// Get language from a shebang. May open and read the file.
    ///
    /// ```no_run
    /// use tokei::LanguageType;
    ///
    /// let rust = LanguageType::from_shebang("./main.rs");
    ///
    /// assert_eq!(rust, Some(LanguageType::Rust));
    /// ```
    pub fn from_shebang<P: AsRef<Path>>(entry: P) -> Option<Self> {
        // Read at max `READ_LIMIT` bytes from the given file.
        // A typical shebang line has a length less than 32 characters;
        // e.g. '#!/bin/bash' - 11B / `#!/usr/bin/env python3` - 22B
        // It is *very* unlikely the file contains a valid shebang syntax
        // if we don't find a newline character after searching the first 128B.
        const READ_LIMIT: usize = 128;

        let mut file = File::open(entry).ok()?;
        let mut buf = [0; READ_LIMIT];

        let len = file.read(&mut buf).ok()?;
        let buf = &buf[..len];

        let first_line = buf.split(|b| *b == b'\n').next()?;
        let first_line = std::str::from_utf8(first_line).ok()?;

        let mut words = first_line.split_whitespace();
        match words.next() {
            {# First match against any shebang paths, and then check if the
               language matches any found in the environment shebang path. #}
            {% for key, value in languages -%}
                {%- if value.shebangs %}
                    {%- for item in value.shebangs  %}| Some("{{item}}") {% endfor %}=> Some({{key}}),
                {% endif -%}
            {%- endfor %}

            Some("#!/usr/bin/env") => {
                if let Some(word) = words.next() {
                    match word {
                        {% for key, value in languages -%}
                            {%- if value.env -%}
                                {%- for item in value.env  %}
                                    {% if loop.index == 1 %}
                                        _ if word.starts_with("{{item}}")
                                    {% else %}
                                        || word.starts_with("{{item}}")
                                    {% endif %}
                                {% endfor %}=> Some({{key}}),
                            {% endif -%}
                        {%- endfor %}
                        env => {
                            warn!("Unknown environment: {:?}", env);
                            None
                        }
                    }
                } else {
                    None
                }
            }
            _ => None,
        }
    }
}

impl FromStr for LanguageType {
    type Err = &'static str;

    fn from_str(from: &str) -> Result<Self, Self::Err> {
        match &*from.to_lowercase() {
            {% for key, value in languages %}
                {% if value.name %}"{{value.name | lower}}"{% else %}"{{key | lower}}"{% endif %}
                => Ok({{key}}),
            {% endfor %}
            _ => Err("Language not found, please use `-l` to see all available\
                     languages."),
        }
    }
}

impl fmt::Display for LanguageType {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        write!(f, "{}", self.name())
    }
}


impl<'a> From<LanguageType> for Cow<'a, LanguageType> {
    fn from(from: LanguageType) -> Self {
        Cow::Owned(from)
    }
}

impl<'a> From<&'a LanguageType> for Cow<'a, LanguageType> {
    fn from(from: &'a LanguageType) -> Self {
        Cow::Borrowed(from)
    }
}
