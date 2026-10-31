# Judges Haml templates with Haml 7.5.1's own parser, `Haml::Parser`: reads
# paths from stdin and prints, per file, the comment ranges in byte offsets
# after a byte order mark, as judgecmp's stream expects.
#
# The parser turns every line it reads on its own into a node and folds the
# rest into the node before them: the lines nested under a `-#` comment or a
# filter such as `:javascript`, whose text it reads raw, the ` |` lines of a
# multiline sequence, the lines a script continues after a trailing comma and
# the lines of an attribute hash. A `-#` comment, and a `/` HTML comment but
# for a revealed `/![…]` one, is a comment from its marker to the end of the
# last line before the next node outside it that holds text. Ruby code and
# filter bodies are not lexed, so the judge reports no literals; a template
# the parser rejects is invalid.
require "haml"
require "json"

BOM = "\xEF\xBB\xBF".b

# The nodes of the tree in document order.
def nodes(node, out = [])
  node.children.each do |child|
    out << child
    nodes(child, out)
  end
  out
end

def descendant?(node, ancestor)
  node = node.parent while node && !node.equal?(ancestor)
  !node.nil?
end

def judge(source)
  lines = source.split("\n", -1)
  starts = lines.each_with_object([0]) { |line, acc| acc << acc.last + line.bytesize + 1 }
  root = Haml::Parser.new({}).call(source.dup.force_encoding("UTF-8"))
  list = nodes(root).reject { |n| n.line.nil? || n.line > lines.size }
  comments = []
  list.each_with_index do |node, i|
    next unless node.type == :haml_comment || (node.type == :comment && !node.value[:revealed])
    following = list[(i + 1)..].find { |n| !descendant?(n, node) }
    last = (following ? following.line - 1 : lines.size) - 1
    last -= 1 while last >= node.line && lines[last].strip.empty?
    first = node.line - 1
    comments << [starts[first] + lines[first][/\A[ \t]*/].bytesize, starts[last] + lines[last].bytesize]
  end
  comments
end

$stdin.each_line do |line|
  path = line.chomp
  next if path.empty?
  source = File.binread(path)
  source = source.byteslice(3..) if source.start_with?(BOM)
  begin
    comments = judge(source)
    errors = 0
  rescue Haml::Error, ArgumentError, EncodingError
    comments = []
    errors = 1
  end
  puts JSON.generate({ path: path, units: "bytes", errors: errors, comments: comments, literals: [] })
end
