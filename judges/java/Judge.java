import com.sun.tools.javac.api.BasicJavacTask;
import com.sun.tools.javac.parser.Scanner;
import com.sun.tools.javac.parser.ScannerFactory;
import com.sun.tools.javac.parser.Tokens.Comment;
import com.sun.tools.javac.parser.Tokens.Token;
import com.sun.tools.javac.parser.Tokens.TokenKind;
import com.sun.tools.javac.util.Context;
import com.sun.tools.javac.util.Log;
import java.io.*;
import java.nio.charset.*;
import java.nio.file.*;
import java.util.*;
import javax.tools.*;

/**
 * Judges Java files with javac's own scanner: reads paths from stdin and
 * prints, per file, the comment and literal ranges in UTF-16 offsets and the
 * number of lexical errors javac reported, as judgecmp's stream expects.
 */
public class Judge {
    static int errors;
    static final java.lang.reflect.Field OFFSET = field("offset"), LENGTH = field("length");

    static java.lang.reflect.Field field(String name) {
        try {
            var f = Class.forName("com.sun.tools.javac.parser.UnicodeReader").getDeclaredField(name);
            f.setAccessible(true);
            return f;
        } catch (ReflectiveOperationException e) {
            throw new IllegalStateException(e);
        }
    }

    public static void main(String[] args) throws Exception {
        JavaCompiler compiler = ToolProvider.getSystemJavaCompiler();
        DiagnosticListener<JavaFileObject> listener = d -> {
            if (d.getKind() == Diagnostic.Kind.ERROR) errors++;
        };
        BasicJavacTask task = (BasicJavacTask) compiler.getTask(null, null, listener, List.of(), null, List.of());
        Context context = task.getContext();
        ScannerFactory factory = ScannerFactory.instance(context);
        Log log = Log.instance(context);
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
            errors = 0;
            StringBuilder comments = new StringBuilder(), literals = new StringBuilder();
            JavaFileObject source = new SimpleJavaFileObject(Path.of(path).toUri(), JavaFileObject.Kind.SOURCE) {};
            JavaFileObject previous = log.useSource(source);
            try {
                Scanner scanner = factory.newScanner(text, false);
                Set<Integer> seen = new HashSet<>();
                int lastEnd = 0;
                int[] maxCommentEnd = {0};
                while (true) {
                    scanner.nextToken();
                    Token token = scanner.token();
                    for (Comment c : token.comments == null ? List.<Comment>of() : token.comments) {
                        // javac's comment reader spans the comment's source:
                        // `offset` is its start and `length` its length.
                        int start = OFFSET.getInt(c), end = start + LENGTH.getInt(c);
                        if (!seen.add(start)) continue;
                        maxCommentEnd[0] = Math.max(maxCommentEnd[0], end);
                        range(comments, start, end);
                    }
                    if (token.kind == TokenKind.STRINGLITERAL || token.kind == TokenKind.CHARLITERAL) {
                        range(literals, token.pos, token.endPos);
                    }
                    if (token.kind == TokenKind.EOF) {
                        // javac skips a line comment that ends the input without
                        // a line terminator but records it only `if
                        // (isAvailable())` (JavaTokenizer.readToken); it is still
                        // an EndOfLineComment (JLS 3.7).
                        int last = Math.max(lastEnd, maxCommentEnd[0]);
                        String rest = text.substring(last).stripLeading();
                        if (rest.startsWith("//")) range(comments, text.length() - rest.length(), text.length());
                        break;
                    }
                    lastEnd = token.endPos;
                }
            } catch (Throwable t) {
                if (System.getenv("JUDGE_DEBUG") != null) t.printStackTrace();
                errors++;
            } finally {
                log.useSource(previous);
            }
            out.println("{\"path\":" + quote(path) + ",\"units\":\"utf16\",\"errors\":" + errors
                + ",\"comments\":[" + comments + "],\"literals\":[" + literals + "]}");
        }
    }

    static void range(StringBuilder ranges, int start, int end) {
        if (ranges.length() > 0) ranges.append(',');
        ranges.append('[').append(start).append(',').append(end).append(']');
    }

    static String quote(String s) {
        return "\"" + s.replace("\\", "\\\\").replace("\"", "\\\"") + "\"";
    }
}
