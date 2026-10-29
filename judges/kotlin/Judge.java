import java.io.*;
import java.nio.charset.*;
import java.nio.file.*;
import java.util.*;
import org.jetbrains.kotlin.com.intellij.psi.TokenType;
import org.jetbrains.kotlin.com.intellij.psi.tree.IElementType;
import org.jetbrains.kotlin.lexer.KotlinLexer;
import org.jetbrains.kotlin.lexer.KtTokens;

/**
 * Judges Kotlin files with the Kotlin compiler's own lexer: reads paths from
 * stdin and prints, per file, the comment and literal ranges in UTF-16
 * offsets and the number of lexical errors, as judgecmp's stream expects.
 * String parts are literals; `${…}` template entries are code with their own
 * strings and comments.
 */
public class Judge {
    static final Set<IElementType> COMMENTS = Set.of(
        KtTokens.EOL_COMMENT, KtTokens.BLOCK_COMMENT, KtTokens.DOC_COMMENT, KtTokens.SHEBANG_COMMENT);
    static final Set<IElementType> LITERALS = Set.of(
        KtTokens.OPEN_QUOTE, KtTokens.CLOSING_QUOTE, KtTokens.REGULAR_STRING_PART, KtTokens.ESCAPE_SEQUENCE,
        KtTokens.CHARACTER_LITERAL, KtTokens.INTERPOLATION_PREFIX, KtTokens.SHORT_TEMPLATE_ENTRY_START);

    public static void main(String[] args) throws Exception {
        BufferedReader in = new BufferedReader(new InputStreamReader(System.in, StandardCharsets.UTF_8));
        PrintStream out = new PrintStream(new FileOutputStream(FileDescriptor.out), true, StandardCharsets.UTF_8);
        CharsetDecoder decoder = StandardCharsets.UTF_8.newDecoder();
        KotlinLexer lexer = new KotlinLexer();
        for (String path; (path = in.readLine()) != null; ) {
            String text;
            try {
                text = decoder.decode(java.nio.ByteBuffer.wrap(Files.readAllBytes(Path.of(path)))).toString();
            } catch (CharacterCodingException e) {
                continue;
            }
            if (text.startsWith("﻿")) text = text.substring(1);
            int errors = 0, openStrings = 0;
            StringBuilder comments = new StringBuilder(), literals = new StringBuilder();
            lexer.start(text);
            for (IElementType type; (type = lexer.getTokenType()) != null; lexer.advance()) {
                int start = lexer.getTokenStart(), end = lexer.getTokenEnd();
                if (COMMENTS.contains(type)) {
                    range(comments, start, end);
                    if (type != KtTokens.EOL_COMMENT && type != KtTokens.SHEBANG_COMMENT
                        && (end - start < 4 || !text.startsWith("*/", end - 2))) errors++;
                } else if (LITERALS.contains(type)) {
                    range(literals, start, end);
                }
                if (type == KtTokens.OPEN_QUOTE) openStrings++;
                if (type == KtTokens.CLOSING_QUOTE) openStrings--;
                if (type == TokenType.BAD_CHARACTER || type == KtTokens.DANGLING_NEWLINE) errors++;
            }
            errors += Math.max(openStrings, 0);
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
