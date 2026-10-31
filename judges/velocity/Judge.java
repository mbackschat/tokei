// Judges Velocity templates with Velocity 2.4.1's own parser: reads paths
// from stdin and prints, per file, the comment and literal ranges in UTF-16
// offsets after a byte order mark and the number of parse errors, as
// judgecmp's stream expects.
//
// The ranges come from the parser's token chain, every token the template
// lexed into (Parser.jjt). Inside a comment the grammar still matches `#`
// and `$` as MORE, so a comment's closing token holds its text from the last
// such run on, and its body before that is SKIPped, `#*` prefix included.
// The text a comment's first token holds before its `##` or `#*`, such as
// the `$` of `$##`, is template text, which ASTComment renders. A `##`
// comment therefore runs from the `##` in its first token to the end of the
// newline token that closes it, newline excluded, or to the end of the file;
// a `#* … *#` or `#** … *#` comment runs from the first `#*` after the token
// before it to the end of its closing token. String literals are
// STRING_LITERAL tokens, which only directives and reference arguments lex,
// so a quote in template text starts none.
//
// XWiki's templates use its `#try` block directive, which XWiki registers
// with Velocity (TryCatchDirective), so a stand-in for it is registered too.
// A template that does not parse is parsed again with hyphens allowed in
// identifiers, as in `${form-id}`, which Velocity 1.x read and Sakai's
// templates use; that setting changes no comment. A template that parses
// neither way counts as one error.
import java.io.BufferedReader;
import java.io.InputStreamReader;
import java.io.StringReader;
import java.io.Writer;
import java.nio.charset.StandardCharsets;
import java.nio.file.Files;
import java.nio.file.Path;
import java.util.ArrayList;
import java.util.List;
import org.apache.velocity.Template;
import org.apache.velocity.context.InternalContextAdapter;
import org.apache.velocity.runtime.RuntimeConstants;
import org.apache.velocity.runtime.RuntimeInstance;
import org.apache.velocity.runtime.directive.Directive;
import org.apache.velocity.runtime.directive.DirectiveConstants;
import org.apache.velocity.runtime.parser.StandardParserConstants;
import org.apache.velocity.runtime.parser.Token;
import org.apache.velocity.runtime.parser.node.Node;
import org.apache.velocity.runtime.parser.node.SimpleNode;

public class Judge {
    public static void main(String[] args) throws Exception {
        RuntimeInstance strict = runtime(false);
        RuntimeInstance hyphens = runtime(true);
        BufferedReader in = new BufferedReader(new InputStreamReader(System.in, StandardCharsets.UTF_8));
        StringBuilder out = new StringBuilder();
        for (String path; (path = in.readLine()) != null; ) {
            if (path.isEmpty()) continue;
            String source = new String(Files.readAllBytes(Path.of(path)), StandardCharsets.UTF_8);
            int bom = source.startsWith("\uFEFF") ? 1 : 0;
            String text = source.substring(bom);
            List<int[]> comments = new ArrayList<>();
            List<int[]> literals = new ArrayList<>();
            int errors = 0;
            String diagnosis = null;
            try {
                SimpleNode root;
                try {
                    root = parse(strict, path, text);
                } catch (Exception | Error e) {
                    root = parse(hyphens, path, text);
                }
                collect(root.getFirstToken(), text, comments, literals);
            } catch (Exception | Error e) {
                comments.clear();
                literals.clear();
                errors = 1;
                diagnosis = String.valueOf(e.getMessage()).lines().findFirst().orElse("");
            }
            out.setLength(0);
            out.append("{\"path\":").append(quote(path)).append(",\"units\":\"utf16\",\"errors\":").append(errors);
            out.append(",\"comments\":").append(ranges(comments, bom)).append(",\"literals\":").append(ranges(literals, bom));
            if (diagnosis != null) out.append(",\"diagnosis\":").append(quote(diagnosis));
            out.append('}');
            System.out.println(out);
        }
    }

    static RuntimeInstance runtime(boolean hyphens) {
        RuntimeInstance runtime = new RuntimeInstance();
        runtime.setProperty(RuntimeConstants.PARSER_HYPHEN_ALLOWED, hyphens);
        runtime.init();
        runtime.addDirective(new Try());
        return runtime;
    }

    static SimpleNode parse(RuntimeInstance runtime, String path, String text) throws Exception {
        Template template = new Template();
        template.setName(path);
        return runtime.parse(new StringReader(text), template);
    }

    /** A stand-in for XWiki's `#try … #end` block directive, which is only parsed here. */
    public static class Try extends Directive {
        public String getName() {
            return "try";
        }

        public int getType() {
            return DirectiveConstants.BLOCK;
        }

        public boolean render(InternalContextAdapter context, Writer writer, Node node) {
            return false;
        }
    }

    static void collect(Token first, String text, List<int[]> comments, List<int[]> literals) {
        int[] lineStarts = lineStarts(text);
        int previousEnd = 0;
        for (Token t = first; t != null && t.kind != StandardParserConstants.EOF; t = t.next) {
            int begin = offset(text, lineStarts, t.beginLine, t.beginColumn);
            int end = offset(text, lineStarts, t.endLine, t.endColumn) + 1;
            switch (t.kind) {
                case StandardParserConstants.SINGLE_LINE_COMMENT_START: {
                    int start = text.indexOf("##", begin);
                    Token newline = t.next;
                    if (newline == null || newline.kind != StandardParserConstants.SINGLE_LINE_COMMENT) {
                        comments.add(new int[] {start, text.length()});
                        break;
                    }
                    int newlineEnd = offset(text, lineStarts, newline.endLine, newline.endColumn) + 1;
                    comments.add(new int[] {start, newlineEnd - (newline.image.endsWith("\r\n") ? 2 : 1)});
                    break;
                }
                case StandardParserConstants.MULTI_LINE_COMMENT:
                case StandardParserConstants.FORMAL_COMMENT:
                    comments.add(new int[] {text.indexOf("#*", previousEnd), end});
                    break;
                case StandardParserConstants.STRING_LITERAL:
                    literals.add(new int[] {begin, end});
                    break;
                default:
                    break;
            }
            previousEnd = Math.max(previousEnd, end);
        }
    }

    // Line starts after "\r\n", "\r" or "\n", the line ends VelocityCharStream counts.
    static int[] lineStarts(String text) {
        List<Integer> starts = new ArrayList<>(List.of(0));
        for (int i = 0; i < text.length(); i++) {
            char c = text.charAt(i);
            if (c == '\n' || (c == '\r' && (i + 1 == text.length() || text.charAt(i + 1) != '\n'))) starts.add(i + 1);
        }
        return starts.stream().mapToInt(Integer::intValue).toArray();
    }

    // VelocityCharStream counts a tab to the next multiple of 8, as JavaCC's
    // generated streams do, so a column is found by replaying the line.
    static int offset(String text, int[] lineStarts, int line, int column) {
        int i = lineStarts[line - 1];
        for (int col = 0; i < text.length(); i++) {
            col = text.charAt(i) == '\t' ? col + 8 - col % 8 : col + 1;
            if (col >= column) return i;
        }
        return i;
    }

    static String ranges(List<int[]> ranges, int bom) {
        StringBuilder s = new StringBuilder("[");
        for (int[] r : ranges) {
            if (s.length() > 1) s.append(',');
            s.append('[').append(r[0] + bom).append(',').append(r[1] + bom).append(']');
        }
        return s.append(']').toString();
    }

    static String quote(String s) {
        StringBuilder q = new StringBuilder("\"");
        for (char c : s.toCharArray()) {
            if (c == '"' || c == '\\') q.append('\\').append(c);
            else if (c < 0x20) q.append(String.format("\\u%04x", (int) c));
            else q.append(c);
        }
        return q.append('"').toString();
    }
}
