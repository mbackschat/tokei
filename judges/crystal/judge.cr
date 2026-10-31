# Judges Crystal files with the compiler's own parser: reads paths on stdin
# and prints, per file, the comment and literal ranges in byte offsets after a
# byte order mark and whether the parser rejected the file, as judgecmp's
# stream expects.
#
# The parser is the lexer, which it drives: string pieces, heredoc bodies
# included, come from `next_string_token` as the parser asks for them, so
# the lexer is reopened to record each piece, and each comment where
# `skip_comment` passes over it. Inside a macro body `next_macro_token`
# reads a `#` outside a string as a comment to the end of its line, as the
# expanded code will; an interpolation in it is expanded into that comment.
# The parser leaves a heredoc's indentation and its blank lines out of the
# pieces, so each string's whole span, from its first piece to its closing
# delimiter, is a literal too. A lookahead lexes text twice, which records
# the same range twice.
require "compiler/crystal/syntax"
require "json"

COMMENTS = [] of {Int32, Int32}
LITERALS = [] of {Int32, Int32}
LAST_PIECE_START = [0]

class Crystal::Lexer
  # The lexer reaches this past the `#`, having looked for a `#<loc:`
  # pragma, so the comment starts at the last `#` before.
  def skip_comment
    start = current_pos - 1
    while @reader.string.byte_at(start) != '#'.ord
      start -= 1
    end
    previous_def
    COMMENTS << {start, current_pos}
  end

  def next_string_token(delimiter_state)
    start = LAST_PIECE_START[0] = current_pos
    token = previous_def
    LITERALS << {start, current_pos} unless token.type.interpolation_start?
    token
  end

  # Called at the start of each line of a heredoc body, also in a macro
  # body, whose strings make no pieces; the line's break belongs to it, so
  # that an empty line lies in the literal.
  def check_heredoc_end(delimiter_state)
    text = @reader.string
    LITERALS << {current_pos, (text.byte_index('\n', current_pos) || text.bytesize - 1) + 1}
    previous_def
  end

  def next_macro_token(macro_state, skip_whitespace)
    token = previous_def
    value = token.value
    return token unless token.type.macro_literal? && value.is_a?(String)
    start = current_pos - value.bytesize
    before = macro_state.delimiter_state
    after = token.macro_state.delimiter_state
    if !macro_state.comment && before.nil? && value.starts_with?('#')
      COMMENTS << {start, @reader.string.byte_index('\n', start) || @reader.string.bytesize}
    elsif span = macro_string_span(value, before, after)
      LITERALS << {start + span[0], start + span[1]}
    end
    token
  end

  # The part of a macro literal that lies in a string, from the delimiter
  # states the lexer gives before and after it, where at most one change
  # of state can lie in the text; nothing when another could.
  private def macro_string_span(value, before, after)
    state = before || after
    finish, nest = state.try(&.end), state.try(&.nest)
    # A heredoc's lines come from check_heredoc_end.
    return unless finish.is_a?(Char) && nest.is_a?(Char)
    stops = {'\\', '}', finish, nest}
    plain = ->(text : String) { text.each_char.none?(&.in?(stops)) && !text.includes?("\#{") }
    if before && after
      {0, value.bytesize} if plain.call(value)
    elsif after
      opener = value.to_slice.rindex(nest.ord.to_u8)
      {opener, value.bytesize} if opener && plain.call(value.byte_slice(opener + 1))
    else
      closer = value.to_slice.index(finish.ord.to_u8)
      {0, closer + 1} if closer && plain.call(value.byte_slice(0, closer))
    end
  end
end

class Crystal::Parser
  def consume_delimiter(pieces, delimiter_state)
    start = LAST_PIECE_START[0]
    recorded = LITERALS.size
    result = previous_def
    ends = LITERALS[recorded..].map(&.[1])
    LITERALS << {start, ends.max} unless ends.empty?
    result
  end
end

def judge(path)
  bytes = File.read(path).to_slice rescue return
  bytes = bytes[3..] if bytes.size >= 3 && bytes[0] == 0xEF && bytes[1] == 0xBB && bytes[2] == 0xBF
  text = String.new(bytes)
  return unless text.valid_encoding?
  COMMENTS.clear
  LITERALS.clear
  errors = 0
  begin
    Crystal::Parser.new(text).parse
  rescue Crystal::SyntaxException
    errors = 1
  end
  puts({path: path, units: "bytes", errors: errors,
        comments: COMMENTS.uniq.sort.map(&.to_a), literals: LITERALS.uniq.sort.map(&.to_a)}.to_json)
end

STDIN.each_line { |path| judge(path) unless path.empty? }
