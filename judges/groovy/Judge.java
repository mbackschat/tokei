import groovyjarjarantlr4.v4.runtime.ANTLRErrorListener;
import groovyjarjarantlr4.v4.runtime.CharStreams;
import groovyjarjarantlr4.v4.runtime.RecognitionException;
import groovyjarjarantlr4.v4.runtime.Recognizer;
import groovyjarjarantlr4.v4.runtime.Token;
import java.io.*;
import java.nio.charset.*;
import java.nio.file.*;
import java.util.*;
import org.apache.groovy.parser.antlr4.GroovyLangLexer;
import org.apache.groovy.parser.antlr4.GroovyLexer;
import org.apache.groovy.parser.antlr4.GroovySyntaxError;

/**
 * Judges Groovy files with Groovy's own lexer, the Parrot parser's
 * `GroovyLangLexer`: reads paths from stdin and prints, per file, the comment
 * and literal ranges in UTF-16 offsets and the number of lexical errors, as
 * judgecmp's stream expects. String literals and the text parts of GStrings
 * are literals; `$name` and `${…}` interpolations are code. The lexer turns a
 * comment into a newline token, or drops it inside parentheses and after a
 * shebang, so comments are read from those tokens' text and from the text
 * between tokens.
 */
public class Judge {
    static final Set<Integer> LITERALS = Set.of(
        GroovyLexer.StringLiteral, GroovyLexer.GStringBegin, GroovyLexer.GStringPart, GroovyLexer.GStringEnd);

    public static void main(String[] args) throws Exception {
        BufferedReader in = new BufferedReader(new InputStreamReader(System.in, StandardCharsets.UTF_8));
        PrintStream out = new PrintStream(new FileOutputStream(FileDescriptor.out), true, StandardCharsets.UTF_8);
        CharsetDecoder decoder = StandardCharsets.UTF_8.newDecoder();
        for (String path; (path = in.readLine()) != null; ) {
            String text;
            try {
                text = decoder.decode(java.nio.ByteBuffer.wrap(Files.readAllBytes(Path.of(path)))).toString();
            } catch (CharacterCodingException e) {
                continue;
            }
            if (text.startsWith("﻿")) text = text.substring(1);
            out.println(judge(path, text));
        }
    }

    static String judge(String path, String text) {
        // The lexer's indices count code points; the ranges count UTF-16 units.
        int[] utf16 = new int[text.codePointCount(0, text.length()) + 1];
        for (int cp = 0, i = 0; i <= text.length(); cp++) {
            utf16[cp] = i;
            if (i == text.length()) break;
            i += Character.charCount(text.codePointAt(i));
        }
        List<int[]> comments = new ArrayList<>(), literals = new ArrayList<>();
        int[] errors = {0};
        GroovyLangLexer lexer = new GroovyLangLexer(CharStreams.fromString(text));
        lexer.removeErrorListeners();
        lexer.addErrorListener(new ANTLRErrorListener<Integer>() {
            @Override
            public <T extends Integer> void syntaxError(Recognizer<T, ?> recognizer, T symbol, int line,
                                                        int column, String message, RecognitionException e) {
                errors[0]++;
            }
        });
        int end = 0;
        try {
            for (Token token = lexer.nextToken(); token.getType() != Token.EOF; token = lexer.nextToken()) {
                int start = utf16[token.getStartIndex()], stop = utf16[token.getStopIndex() + 1];
                gapComments(text, end, start, comments);
                String content = text.substring(start, stop);
                if (LITERALS.contains(token.getType())) {
                    literals.add(new int[] {start, stop});
                } else if (content.startsWith("//") || content.startsWith("/*")) {
                    comments.add(new int[] {start, stop});
                }
                end = Math.max(end, stop);
            }
        } catch (RuntimeException | GroovySyntaxError e) {
            errors[0]++;
        }
        if (errors[0] == 0) gapComments(text, end, text.length(), comments);
        return String.format("{\"path\":%s,\"units\":\"utf16\",\"errors\":%d,\"comments\":%s,\"literals\":%s}",
            quote(path), errors[0] > 0 ? 1 : 0, ranges(comments), ranges(literals));
    }

    /** Adds the comments in the text between tokens, `from` to `to`, which holds only whitespace and comments. */
    static void gapComments(String text, int from, int to, List<int[]> comments) {
        int i = from;
        while (i < to) {
            if (text.startsWith("/*", i)) {
                int close = text.indexOf("*/", i + 2);
                int stop = close < 0 ? to : Math.min(to, close + 2);
                comments.add(new int[] {i, stop});
                i = stop;
            } else if (text.startsWith("//", i) || (i == 0 && text.startsWith("#!"))) {
                int stop = i;
                while (stop < to && text.charAt(stop) != '\n' && text.charAt(stop) != '\r') stop++;
                comments.add(new int[] {i, stop});
                i = stop;
            } else {
                i++;
            }
        }
    }

    static String ranges(List<int[]> list) {
        StringBuilder b = new StringBuilder("[");
        for (int[] r : list) b.append(b.length() > 1 ? "," : "").append('[').append(r[0]).append(',').append(r[1]).append(']');
        return b.append(']').toString();
    }

    static String quote(String s) {
        StringBuilder b = new StringBuilder("\"");
        for (char c : s.toCharArray()) {
            if (c == '"' || c == '\\') b.append('\\').append(c);
            else if (c < 0x20) b.append(String.format("\\u%04x", (int) c));
            else b.append(c);
        }
        return b.append('"').toString();
    }
}
