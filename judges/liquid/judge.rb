# Judges Liquid templates with Liquid 5.14.0's own tokenizer and tag
# patterns: reads paths from stdin and prints, per file, the comment and
# literal ranges in byte offsets after a byte order mark, as judgecmp's stream
# expects.
#
# `Liquid::Tokenizer` splits a template into text, `{% … %}` tags and
# `{{ … }}` variables. A `comment` block, with the nesting and `raw` bodies
# `Liquid::Comment#parse_body` reads, a `doc` block (`Liquid::Doc#parse`), an
# inline `{% # … %}` tag, and inside a `{% liquid %}` tag a `#` line or the
# lines from `comment` to its `endcomment`, are comments; `QuotedString`s in
# tags and variables are literals. Liquid reads text as text, but the HTML it
# usually holds has comments, so a `<!--` in text opens a comment that the next
# `-->` closes, as an HTML parser reads the rendered page. A template whose
# tag or variable does not end, or whose block never closes, is invalid.
require "json"
require "liquid"
require "strscan"

FULL_TOKEN = Liquid::BlockBody::FullToken
POSSIBLY_INVALID = Liquid::BlockBody::FullTokenPossiblyInvalid
LIQUID_TAG_TOKEN = Liquid::BlockBody::LiquidTagToken
QUOTED_STRING = Liquid::QuotedString
BOM = "\xEF\xBB\xBF".b

class Invalid < StandardError; end

def tag_name(token)
  token =~ FULL_TOKEN ? Regexp.last_match(2) : nil
end

# The tokens with their byte offsets.
def tokens(source)
  tokenizer = Liquid::Tokenizer.new(source: source, string_scanner: StringScanner.new(""))
  offset = 0
  list = []
  while (token = tokenizer.send(:shift))
    token = token.b
    raise Invalid if token == "{{" || token == "{%"
    list << [offset, token]
    offset += token.bytesize
  end
  raise Invalid unless offset == source.bytesize
  list
end

# Returns the index of the first token after `from` that the block accepts
# as its end, raising when none does.
def block_end(list, from)
  index = (from + 1...list.size).find { |i| yield list[i][1] }
  index or raise Invalid
end

def comment_end(list, from)
  depth = 1
  i = from + 1
  while i < list.size
    case tag_name(list[i][1])
    when "raw" then i = block_end(list, i) { |t| t =~ POSSIBLY_INVALID && Regexp.last_match(2) == "endraw" }
    when "comment" then depth += 1
    when "endcomment" then depth -= 1
    end
    return i if depth.zero?
    i += 1
  end
  raise Invalid
end

def literals_in(text, base, literals)
  text.scan(QUOTED_STRING) { literals << [base + Regexp.last_match.begin(0), base + Regexp.last_match.end(0)] }
end

# The comments and literals of a `{% liquid %}` tag's markup, read line by
# line as `BlockBody#parse_for_liquid_tag` reads it.
def liquid_tag(markup, base, comments, literals)
  depth = 0
  start = nil
  offset = base
  markup.split("\n", -1).each do |line|
    name = line =~ LIQUID_TAG_TOKEN ? Regexp.last_match(1) : nil
    line_end = offset + line.bytesize
    if depth.positive?
      depth += 1 if name == "comment"
      depth -= 1 if name == "endcomment"
      comments << [start, line_end] if depth.zero?
    elsif name == "comment"
      depth = 1
      start = offset + line.index("comment")
    elsif name == "#"
      comments << [offset + line.index("#"), line_end]
    else
      literals_in(line, offset, literals)
    end
    offset = line_end + 1
  end
  raise Invalid if depth.positive?
end

def judge(source)
  comments = []
  literals = []
  list = tokens(source)
  # The end of the last HTML comment; tokens that start before it are inside it.
  covered = 0
  i = 0
  while i < list.size
    offset, token = list[i]
    if offset < covered
      covered = html_comments(source, covered, offset + token.bytesize, comments) unless token.start_with?("{%", "{{")
    elsif token.start_with?("{%")
      raise Invalid unless token =~ FULL_TOKEN
      name = Regexp.last_match(2)
      markup_at = offset + Regexp.last_match.begin(4)
      markup = Regexp.last_match(4)
      case name
      when "comment", "doc"
        last = name == "comment" ? comment_end(list, i) : block_end(list, i) { |t| t =~ POSSIBLY_INVALID && Regexp.last_match(2) == "enddoc" }
        comments << [offset, list[last][0] + list[last][1].bytesize]
        i = last
      when "raw"
        i = block_end(list, i) { |t| t =~ POSSIBLY_INVALID && Regexp.last_match(2) == "endraw" }
      when "#"
        # `Liquid::InlineComment` rejects a later line that is not a comment.
        raise Invalid if markup.match?(/\n\s*[^#\s]/)
        comments << [offset, offset + token.bytesize]
      when "liquid"
        liquid_tag(markup, markup_at, comments, literals)
      else
        literals_in(markup, markup_at, literals)
      end
    elsif token.start_with?("{{")
      literals_in(token, offset, literals)
    else
      covered = html_comments(source, offset, offset + token.bytesize, comments)
    end
    i += 1
  end
  [comments, literals]
end

# Appends the HTML comments that open in text from `from` to `to`, each
# running to the next `-->` in the source whatever tokens it crosses, and
# returns where the last one ends.
def html_comments(source, from, to, comments)
  covered = from
  while (open = source.index("<!--", covered)) && open < to
    close = source.index("-->", open + 4)
    covered = close ? close + 3 : source.bytesize
    comments << [open, covered]
  end
  covered
end

$stdin.each_line do |line|
  path = line.chomp
  next if path.empty?
  source = File.binread(path)
  source = source.byteslice(3..) if source.start_with?(BOM)
  begin
    comments, literals = judge(source)
    errors = 0
  rescue Invalid, Liquid::SyntaxError
    comments = []
    literals = []
    errors = 1
  end
  puts JSON.generate({ path: path, units: "bytes", errors: errors, comments: comments, literals: literals })
end
