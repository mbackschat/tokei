# Judges Ruby files with Prism, Ruby's default parser since 3.4: reads paths
# on stdin and prints, per file, the comment and literal ranges in byte
# offsets after a byte order mark and the number of syntax errors, as
# judgecmp's stream expects. Strings, heredoc bodies, regular expressions,
# character literals, symbols' quotes and percent literals are literals;
# `#{…}` holes are code with their own strings and comments. Data after
# `__END__` is in no range, so its lines are code or blank.
require "json"
require "prism"

LITERALS = %i[
  STRING_BEGIN STRING_CONTENT STRING_END HEREDOC_START HEREDOC_END REGEXP_BEGIN REGEXP_END
  CHARACTER_LITERAL PERCENT_LOWER_W PERCENT_UPPER_W PERCENT_LOWER_I PERCENT_UPPER_I PERCENT_LOWER_X
  WORDS_SEP SYMBOL_BEGIN LABEL_END BACKTICK
].freeze
COMMENTS = %i[COMMENT EMBDOC_BEGIN EMBDOC_LINE EMBDOC_END].freeze

$stdin.each_line do |line|
  path = line.chomp
  source = File.binread(path)
  next unless source.dup.force_encoding("UTF-8").valid_encoding?
  source = source.byteslice(3..) if source.start_with?("\xEF\xBB\xBF".b)
  result = Prism.lex(source.force_encoding("UTF-8"))
  comments = []
  literals = []
  result.value.each do |token, _state|
    range = [token.location.start_offset, token.location.end_offset]
    if COMMENTS.include?(token.type)
      comments << range
    elsif LITERALS.include?(token.type)
      literals << range
    end
  end
  puts JSON.generate({ path: path, units: "bytes", errors: result.errors.size, comments: comments, literals: literals })
end
