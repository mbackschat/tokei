use {super::*, CompileErrorKind::*, TokenKind::*};

static DEDENT_RE: LazyLock<Regex> =
  LazyLock::new(|| Regex::new(r"^[ \t\r\n]*\n[^ \t\r\n]").unwrap());

/// Just language lexer
///
/// The lexer proceeds character-by-character, as opposed to using regular
/// expressions to lex tokens or semi-tokens at a time. As a result, it is
/// verbose and straightforward. Just used to have a regex-based lexer, which
/// was slower and generally godawful.  However, this should not be taken as a
/// slight against regular expressions, the lexer was just idiosyncratically
/// bad.
pub(crate) struct Lexer<'src> {
  /// Char iterator
  chars: Chars<'src>,
  /// Indentation stack
  indentation: Vec<&'src str>,
  /// Interpolation token start stack
  interpolation_stack: Vec<Token<'src>>,
  /// Source text line count
  lines: usize,
  /// Next character to be lexed
  next: Option<char>,
  /// Current open delimiters
  open_delimiters: Vec<(Delimiter, usize)>,
  /// Path to source file
  path: &'src Utf8Path,
  /// Inside recipe body
  recipe_body: bool,
  /// Next indent will start a recipe body
  recipe_body_pending: bool,
  /// Source text
  src: &'src str,
  /// Current token end
  token_end: Position,
  /// Current token start
  token_start: Position,
  /// Tokens
  tokens: Vec<Token<'src>>,
}

impl<'src> Lexer<'src> {
  pub(crate) const INTERPOLATION_END: &'static str = "}}";
  pub(crate) const INTERPOLATION_ESCAPE: &'static str = "{{{{";
  pub(crate) const INTERPOLATION_START: &'static str = "{{";

  /// Lex `src`
  pub(crate) fn lex(path: &'src Utf8Path, src: &'src str) -> CompileResult<'src, Vec<Token<'src>>> {
    Self::new(path, src).tokenize()
  }

  #[cfg(test)]
  pub(crate) fn test_lex(src: &'src str) -> CompileResult<'src, Vec<Token<'src>>> {
    Self::new("justfile".as_ref(), src).tokenize()
  }

  /// Create a new Lexer to lex `src`
  fn new(path: &'src Utf8Path, src: &'src str) -> Self {
    let mut chars = src.chars();
    let next = chars.next();

    let start = Position {
      offset: 0,
      column: 0,
      line: 0,
    };

    Self {
      chars,
      indentation: vec![""],
      interpolation_stack: Vec::new(),
      lines: src.lines().count(),
      next,
      open_delimiters: Vec::new(),
      path,
      recipe_body: false,
      recipe_body_pending: false,
      src,
      token_end: start,
      token_start: start,
      tokens: Vec::new(),
    }
  }

  /// Advance over the character in `self.next`, updating `self.token_end`
  /// accordingly.
  fn advance(&mut self) -> CompileResult<'src> {
    match self.next {
      Some(c) => {
        let len_utf8 = c.len_utf8();

        self.token_end.offset += len_utf8;
        self.token_end.column += len_utf8;

        if c == '\n' && self.token_end.line + 1 < self.lines {
          self.token_end.column = 0;
          self.token_end.line += 1;
        }

        self.next = self.chars.next();

        Ok(())
      }
      None => Err(self.internal_error("Lexer advanced past end of text")),
    }
  }

  /// Lexeme of in-progress token
  fn lexeme(&self) -> &'src str {
    &self.src[self.token_start.offset..self.token_end.offset]
  }

  /// Length of current token
  fn current_token_length(&self) -> usize {
    self.token_end.offset - self.token_start.offset
  }

  fn accepted(&mut self, c: char) -> CompileResult<'src, bool> {
    if self.next_is(c) {
      self.advance()?;
      Ok(true)
    } else {
      Ok(false)
    }
  }

  fn presume(&mut self, c: char) -> CompileResult<'src> {
    if !self.next_is(c) {
      return Err(self.internal_error(format!("Lexer presumed character `{c}`")));
    }

    self.advance()?;

    Ok(())
  }

  fn presume_str(&mut self, s: &str) -> CompileResult<'src> {
    for c in s.chars() {
      self.presume(c)?;
    }

    Ok(())
  }

  /// Is next character c?
  fn next_is(&self, c: char) -> bool {
    self.next == Some(c)
  }

  /// Is next character ' ' or '\t'?
  fn next_is_whitespace(&self) -> bool {
    self.next_is(' ') || self.next_is('\t')
  }

  /// Un-lexed text
  fn rest(&self) -> &'src str {
    &self.src[self.token_end.offset..]
  }

  /// Check if unlexed text begins with prefix
  fn rest_starts_with(&self, prefix: &str) -> bool {
    self.rest().starts_with(prefix)
  }

  /// Does rest start with "\n" or "\r\n"?
  fn at_eol(&self) -> bool {
    self.next_is('\n') || self.rest_starts_with("\r\n")
  }

  /// Are we at end-of-file?
  fn at_eof(&self) -> bool {
    self.rest().is_empty()
  }

  /// Are we at end-of-line or end-of-file?
  fn at_eol_or_eof(&self) -> bool {
    self.at_eol() || self.at_eof()
  }

  /// Get current indentation
  fn indentation(&self) -> &'src str {
    self.indentation.last().unwrap()
  }

  /// Are we currently indented
  fn indented(&self) -> bool {
    !self.indentation().is_empty()
  }

  /// Create a new token with `kind` whose lexeme is between `self.token_start`
  /// and `self.token_end`
  fn token(&mut self, kind: TokenKind) {
    self.tokens.push(Token {
      column: self.token_start.column,
      kind,
      length: self.token_end.offset - self.token_start.offset,
      line: self.token_start.line,
      offset: self.token_start.offset,
      path: self.path,
      src: self.src,
    });

    // Set `token_start` to point after the lexed token
    self.token_start = self.token_end;
  }

  /// Create an internal error with `message`
  fn internal_error(&self, message: impl Into<String>) -> CompileError<'src> {
    // Use `self.token_end` as the location of the error
    let token = Token {
      src: self.src,
      offset: self.token_end.offset,
      line: self.token_end.line,
      column: self.token_end.column,
      length: 0,
      kind: Unspecified,
      path: self.path,
    };
    CompileError::new(
      token,
      Internal {
        message: message.into(),
      },
    )
  }

  /// Create a compilation error with `kind`
  fn error(&self, kind: CompileErrorKind<'src>) -> CompileError<'src> {
    // Use the in-progress token span as the location of the error.

    // The width of the error site to highlight depends on the kind of error:
    let length = match kind {
      UnterminatedString | UnterminatedBacktick => {
        if self.lexeme().starts_with(Self::INTERPOLATION_END) {
          Self::INTERPOLATION_END.len()
        } else {
          let Some(kind) = StringKind::from_token_start(self.lexeme()) else {
            return self.internal_error("Lexer::error: expected string or backtick token start");
          };
          kind.delimiter().len()
        }
      }
      // highlight the full token
      _ => self.lexeme().len(),
    };

    let token = Token {
      kind: Unspecified,
      src: self.src,
      offset: self.token_start.offset,
      line: self.token_start.line,
      column: self.token_start.column,
      length,
      path: self.path,
    };

    CompileError::new(token, kind)
  }

  fn unterminated_interpolation_error(interpolation_start: Token<'src>) -> CompileError<'src> {
    CompileError::new(interpolation_start, UnterminatedInterpolation)
  }

  /// True if `c` can be the first character of an identifier
  pub(crate) fn is_identifier_start(c: char) -> bool {
    matches!(c, 'a'..='z' | 'A'..='Z' | '_')
  }

  /// True if `c` can be a continuation character of an identifier
  pub(crate) fn is_identifier_continue(c: char) -> bool {
    Self::is_identifier_start(c) || matches!(c, '0'..='9' | '-')
  }

  /// Consume the text and produce a series of tokens
  fn tokenize(mut self) -> CompileResult<'src, Vec<Token<'src>>> {
    loop {
      if self.token_start.column == 0 {
        self.lex_line_start()?;
      }

      match self.next {
        Some(first) => {
          if let Some(&interpolation_start) = self.interpolation_stack.last() {
            self.lex_interpolation(interpolation_start, first)?;
          } else if self.recipe_body {
            self.lex_body()?;
          } else {
            self.lex_normal(first)?;
          }
        }
        None => break,
      }
    }

    if let Some(&interpolation_start) = self.interpolation_stack.last() {
      return Err(Self::unterminated_interpolation_error(interpolation_start));
    }

    while self.indented() {
      self.lex_dedent();
    }

    self.token(Eof);

    assert_eq!(self.token_start.offset, self.token_end.offset);
    assert_eq!(self.token_start.offset, self.src.len());
    assert_eq!(self.indentation.len(), 1);

    Ok(self.tokens)
  }

  /// Handle blank lines and indentation
  fn lex_line_start(&mut self) -> CompileResult<'src> {
    enum Indentation<'src> {
      // Line only contains whitespace
      Blank,
      // Indentation continues
      Continue,
      // Indentation decreases
      Decrease,
      // Indentation isn't consistent
      Inconsistent,
      // Indentation increases
      Increase,
      // Indentation mixes spaces and tabs
      Mixed { whitespace: &'src str },
    }

    use Indentation::*;

    let nonblank_index = self
      .rest()
      .find(|c| !matches!(c, ' ' | '\t'))
      .unwrap_or(self.rest().len());

    let rest = &self.rest()[nonblank_index..];

    let whitespace = &self.rest()[..nonblank_index];

    if self.open_delimiters_or_interpolation() {
      if !whitespace.is_empty() {
        while self.next_is_whitespace() {
          self.advance()?;
        }

        self.token(Whitespace);
      }

      return Ok(());
    }

    let indentation = whitespace
      .char_indices()
      .nth(self.indentation().chars().count())
      .map_or(whitespace.len(), |(i, _c)| i);

    let body_whitespace = &whitespace[..indentation];

    let spaces = whitespace.chars().any(|c| c == ' ');
    let tabs = whitespace.chars().any(|c| c == '\t');

    let body_spaces = body_whitespace.chars().any(|c| c == ' ');
    let body_tabs = body_whitespace.chars().any(|c| c == '\t');

    #[allow(clippy::if_same_then_else)]
    let indentation = if rest.starts_with('\n') || rest.starts_with("\r\n") || rest.is_empty() {
      Blank
    } else if whitespace == self.indentation() {
      Continue
    } else if self.indentation.contains(&whitespace) {
      Decrease
    } else if self.recipe_body && whitespace.starts_with(self.indentation()) {
      Continue
    } else if self.recipe_body && body_spaces && body_tabs {
      Mixed {
        whitespace: body_whitespace,
      }
    } else if !self.recipe_body && spaces && tabs {
      Mixed { whitespace }
    } else if whitespace.len() < self.indentation().len() {
      Inconsistent
    } else if self.recipe_body
      && body_whitespace.len() >= self.indentation().len()
      && !body_whitespace.starts_with(self.indentation())
    {
      Inconsistent
    } else if whitespace.len() >= self.indentation().len()
      && !whitespace.starts_with(self.indentation())
    {
      Inconsistent
    } else {
      Increase
    };

    match indentation {
      Blank => {
        if DEDENT_RE.is_match(self.rest()) {
          while self.indented() {
            self.lex_dedent();
          }
        }

        if !whitespace.is_empty() {
          while self.next_is_whitespace() {
            self.advance()?;
          }

          self.token(Whitespace);
        }

        Ok(())
      }
      Continue => {
        if !self.indentation().is_empty() {
          for _ in self.indentation().chars() {
            self.advance()?;
          }

          self.token(Whitespace);
        }

        Ok(())
      }
      Decrease => {
        while self.indentation() != whitespace {
          self.lex_dedent();
        }

        if !whitespace.is_empty() {
          while self.next_is_whitespace() {
            self.advance()?;
          }

          self.token(Whitespace);
        }

        Ok(())
      }
      Mixed { whitespace } => {
        for _ in whitespace.chars() {
          self.advance()?;
        }

        Err(self.error(MixedLeadingWhitespace { whitespace }))
      }
      Inconsistent => {
        for _ in whitespace.chars() {
          self.advance()?;
        }

        Err(self.error(InconsistentLeadingWhitespace {
          expected: self.indentation(),
          found: whitespace,
        }))
      }
      Increase => {
        while self.next_is_whitespace() {
          self.advance()?;
        }

        let indentation = self.lexeme();
        self.indentation.push(indentation);
        self.token(Indent);
        if self.recipe_body_pending {
          self.recipe_body = true;
        }

        Ok(())
      }
    }
  }

  /// Lex token beginning with `start` outside of a recipe body
  fn lex_normal(&mut self, start: char) -> CompileResult<'src> {
    match start {
      ' ' | '\t' => self.lex_whitespace(),
      '!' => self.lex_choices('!', &[('=', BangEquals), ('~', BangTilde)], Bang),
      '#' => self.lex_comment(),
      '$' => self.lex_single(Dollar),
      '&' => self.lex_digraph('&', '&', AmpersandAmpersand),
      '(' => self.lex_delimiter(ParenL),
      ')' => self.lex_delimiter(ParenR),
      '*' => self.lex_single(Asterisk),
      '+' => self.lex_choices('+', &[('+', PlusPlus)], Plus),
      ',' => self.lex_single(Comma),
      '/' => self.lex_single(Slash),
      ':' => self.lex_colon(),
      '=' => self.lex_choices('=', &[('=', EqualsEquals), ('~', EqualsTilde)], Equals),
      '?' => self.lex_single(QuestionMark),
      '@' => self.lex_single(At),
      '[' => self.lex_delimiter(BracketL),
      '\\' => self.lex_escape(),
      '\n' | '\r' => self.lex_eol(),
      '\u{feff}' => self.lex_single(ByteOrderMark),
      ']' => self.lex_delimiter(BracketR),
      '`' | '"' | '\'' => self.lex_string(None),
      '{' => self.lex_delimiter(BraceL),
      '|' => self.lex_digraph('|', '|', BarBar),
      '}' => {
        let format_string_kind = self.open_delimiters.last().and_then(|(delimiter, _line)| {
          if !self.rest().starts_with(Self::INTERPOLATION_END) {
            None
          } else if let Delimiter::FormatString(kind) = delimiter {
            Some(kind)
          } else {
            None
          }
        });

        if let Some(format_string_kind) = format_string_kind {
          self.lex_string(Some(*format_string_kind))
        } else {
          self.lex_delimiter(BraceR)
        }
      }
      _ if Self::is_identifier_start(start) => self.lex_identifier(),
      _ => {
        self.advance()?;
        Err(self.error(UnknownStartOfToken { start }))
      }
    }
  }

  /// Lex token beginning with `start` inside an interpolation
  fn lex_interpolation(
    &mut self,
    interpolation_start: Token<'src>,
    start: char,
  ) -> CompileResult<'src> {
    if self.rest_starts_with(Self::INTERPOLATION_END) && self.open_delimiters.is_empty() {
      // end current interpolation
      if self.interpolation_stack.pop().is_none() {
        self.presume_str(Self::INTERPOLATION_END)?;
        return Err(self.internal_error(
          "Lexer::lex_interpolation found `}}` but was called with empty interpolation stack.",
        ));
      }
      // Emit interpolation end token
      self.lex_double(InterpolationEnd)
    } else if self.at_eof() && self.open_delimiters.is_empty() {
      // Return unterminated interpolation error that highlights the opening
      // {{
      Err(Self::unterminated_interpolation_error(interpolation_start))
    } else {
      // Otherwise lex as per normal
      self.lex_normal(start)
    }
  }

  /// Lex token while in recipe body
  fn lex_body(&mut self) -> CompileResult<'src> {
    enum Terminator {
      EndOfFile,
      Interpolation,
      Newline,
      NewlineCarriageReturn,
    }

    use Terminator::*;

    let terminator = loop {
      if self.rest_starts_with(Self::INTERPOLATION_ESCAPE) {
        self.presume_str(Self::INTERPOLATION_ESCAPE)?;
        continue;
      }

      if self.rest_starts_with("\n") {
        break Newline;
      }

      if self.rest_starts_with("\r\n") {
        break NewlineCarriageReturn;
      }

      if self.rest_starts_with(Self::INTERPOLATION_START) {
        break Interpolation;
      }

      if self.at_eof() {
        break EndOfFile;
      }

      self.advance()?;
    };

    // emit text token containing text so far
    if self.current_token_length() > 0 {
      self.token(Text);
    }

    match terminator {
      Newline => self.lex_single(Eol),
      NewlineCarriageReturn => self.lex_double(Eol),
      Interpolation => {
        self.lex_double(InterpolationStart)?;
        self
          .interpolation_stack
          .push(self.tokens[self.tokens.len() - 1]);
        Ok(())
      }
      EndOfFile => Ok(()),
    }
  }

  fn lex_dedent(&mut self) {
    assert_eq!(self.current_token_length(), 0);
    self.token(Dedent);
    self.indentation.pop();
    self.recipe_body_pending = false;
    self.recipe_body = false;
  }

  /// Lex a single-character token
  fn lex_single(&mut self, kind: TokenKind) -> CompileResult<'src> {
    self.advance()?;
    self.token(kind);
    Ok(())
  }

  /// Lex a double-character token
  fn lex_double(&mut self, kind: TokenKind) -> CompileResult<'src> {
    self.advance()?;
    self.advance()?;
    self.token(kind);
    Ok(())
  }

  /// Lex a double-character token of kind `then` if the second character of
  /// that token would be `second`, otherwise lex a single-character token of
  /// kind `otherwise`
  fn lex_choices(
    &mut self,
    first: char,
    choices: &[(char, TokenKind)],
    otherwise: TokenKind,
  ) -> CompileResult<'src> {
    self.presume(first)?;

    for (second, then) in choices {
      if self.accepted(*second)? {
        self.token(*then);
        return Ok(());
      }
    }

    self.token(otherwise);

    Ok(())
  }

  /// Lex an opening or closing delimiter
  fn lex_delimiter(&mut self, kind: TokenKind) -> CompileResult<'src> {
    match kind {
      BraceL => self.open_delimiter(Delimiter::Brace),
      BraceR => self.close_delimiter(Delimiter::Brace)?,
      BracketL => self.open_delimiter(Delimiter::Bracket),
      BracketR => self.close_delimiter(Delimiter::Bracket)?,
      ParenL => self.open_delimiter(Delimiter::Paren),
      ParenR => self.close_delimiter(Delimiter::Paren)?,
      _ => {
        return Err(self.internal_error(format!(
          "Lexer::lex_delimiter called with non-delimiter token: `{kind}`",
        )));
      }
    }

    // Emit the delimiter token
    self.lex_single(kind)?;

    Ok(())
  }

  /// Push a delimiter onto the open delimiter stack
  fn open_delimiter(&mut self, delimiter: Delimiter) {
    self
      .open_delimiters
      .push((delimiter, self.token_start.line));
  }

  /// Pop a delimiter from the open delimiter stack and error if incorrect type
  fn close_delimiter(&mut self, close: Delimiter) -> CompileResult<'src> {
    match self.open_delimiters.pop() {
      Some((open, _)) if open == close => Ok(()),
      Some((open, open_line)) => Err(self.error(MismatchedClosingDelimiter {
        open,
        close,
        open_line,
      })),
      None => Err(self.error(UnexpectedClosingDelimiter { close })),
    }
  }

  /// Return true if there are any unclosed delimiters
  fn open_delimiters_or_interpolation(&self) -> bool {
    !self.open_delimiters.is_empty() || !self.interpolation_stack.is_empty()
  }

  /// Lex a two-character digraph
  fn lex_digraph(&mut self, left: char, right: char, token: TokenKind) -> CompileResult<'src> {
    self.presume(left)?;

    if self.accepted(right)? {
      self.token(token);
      Ok(())
    } else {
      // Emit an unspecified token to consume the current character,
      self.token(Unspecified);

      if self.at_eof() {
        return Err(self.error(UnexpectedEndOfToken {
          expected: vec![right],
        }));
      }

      // …and advance past another character,
      self.advance()?;

      // …so that the error we produce highlights the unexpected character.
      Err(self.error(UnexpectedCharacter {
        expected: vec![right],
      }))
    }
  }

  /// Lex a token starting with ':'
  fn lex_colon(&mut self) -> CompileResult<'src> {
    self.presume(':')?;

    if self.accepted('=')? {
      self.token(ColonEquals);
    } else if self.accepted(':')? {
      self.token(ColonColon);
    } else {
      self.token(Colon);
      self.recipe_body_pending = true;
    }

    Ok(())
  }

  /// Lex an token starting with '\' escape
  fn lex_escape(&mut self) -> CompileResult<'src> {
    self.presume('\\')?;

    // Treat newline escaped with \ as whitespace
    if self.accepted('\n')? {
      while self.next_is_whitespace() {
        self.advance()?;
      }
      self.token(Whitespace);
    } else if self.accepted('\r')? {
      if !self.accepted('\n')? {
        return Err(self.error(UnpairedCarriageReturn));
      }
      while self.next_is_whitespace() {
        self.advance()?;
      }
      self.token(Whitespace);
    } else if let Some(character) = self.next {
      return Err(self.error(InvalidEscapeSequence { character }));
    } else {
      return Err(self.error(EscapeEndOfFile));
    }

    Ok(())
  }

  /// Lex a carriage return and line feed
  fn lex_eol(&mut self) -> CompileResult<'src> {
    if self.accepted('\r')? {
      if !self.accepted('\n')? {
        return Err(self.error(UnpairedCarriageReturn));
      }
    } else {
      self.presume('\n')?;
    }

    // Emit eol if there are no open delimiters, otherwise emit whitespace.
    if self.open_delimiters_or_interpolation() {
      self.token(Whitespace);
    } else {
      self.token(Eol);
    }

    Ok(())
  }

  /// Lex name: [a-zA-Z_][a-zA-Z0-9_]*
  fn lex_identifier(&mut self) -> CompileResult<'src> {
    self.advance()?;

    while let Some(c) = self.next {
      if !Self::is_identifier_continue(c) {
        break;
      }

      self.advance()?;
    }

    self.token(Identifier);

    Ok(())
  }

  /// Lex comment: #[^\r\n]
  fn lex_comment(&mut self) -> CompileResult<'src> {
    self.presume('#')?;

    while !self.at_eol_or_eof() {
      self.advance()?;
    }

    self.token(Comment);

    Ok(())
  }

  /// Lex whitespace: [ \t]+
  fn lex_whitespace(&mut self) -> CompileResult<'src> {
    while self.next_is_whitespace() {
      self.advance()?;
    }

    self.token(Whitespace);

    Ok(())
  }

  /// Lex a backtick, cooked string, or raw string.
  ///
  /// Backtick:      ``[^`]*``
  /// Cooked string: "[^"]*" # also processes escape sequences
  /// Raw string:    '[^']*'
  fn lex_string(&mut self, format_string_kind: Option<StringKind>) -> CompileResult<'src> {
    let kind = if let Some(kind) = format_string_kind {
      self.presume_str(Self::INTERPOLATION_END)?;
      kind
    } else {
      let Some(kind) = StringKind::from_token_start(self.rest()) else {
        self.advance()?;
        return Err(self.internal_error("Lexer::lex_string: invalid string start"));
      };
      self.presume_str(kind.delimiter())?;
      kind
    };

    let format = format_string_kind.is_some()
      || kind.delimiter != StringDelimiter::Backtick
        && self.tokens.last().is_some_and(|token| {
          token.kind == TokenKind::Identifier && token.lexeme() == Keyword::F.lexeme()
        });

    let mut escape = false;

    loop {
      if self.next.is_none() {
        return Err(self.error(kind.unterminated_error_kind()));
      } else if !escape && kind.processes_escape_sequences() && self.next_is('\\') {
        escape = true;
      } else if escape && kind.processes_escape_sequences() && self.next_is('u') {
        escape = false;
      } else if format && self.rest_starts_with(Self::INTERPOLATION_ESCAPE) {
        escape = false;
        self.advance()?;
        self.advance()?;
        self.advance()?;
      } else if !escape
        && (self.rest_starts_with(kind.delimiter())
          || format && self.rest_starts_with(Self::INTERPOLATION_START))
      {
        break;
      } else {
        escape = false;
      }

      self.advance()?;
    }

    if format && self.rest_starts_with(Self::INTERPOLATION_START) {
      self.presume_str(Self::INTERPOLATION_START)?;
      if format_string_kind.is_some() {
        self.token(FormatStringContinue);
      } else {
        self.token(FormatStringStart);
        self.open_delimiter(Delimiter::FormatString(kind));
      }
    } else {
      self.presume_str(kind.delimiter())?;

      if let Some(format_string_kind) = format_string_kind {
        self.close_delimiter(Delimiter::FormatString(format_string_kind))?;
        self.token(FormatStringEnd);
      } else {
        self.token(kind.token_kind());
      }
    }

    Ok(())
  }
}

