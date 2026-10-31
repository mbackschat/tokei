// Judges FreeMarker templates with FreeMarker 2.3.34's own parser: reads
// paths from stdin and prints, per file, the comment ranges in UTF-16
// offsets after a byte order mark and the number of parse errors, as
// judgecmp's stream expects. Comments are the comment elements of the parsed
// template: `<#-- … -->` and `[#-- … --]`, whose tag syntax is detected as
// FreeMarker detects it, from the first FreeMarker tag, and in the legacy
// syntax of `<#ftl strict_syntax=false>` also `<comment> … </comment>`;
// `<#noparse>` text holds none. Comments inside `${…}` and inside tags are skipped tokens
// the parser keeps no element for, so they are not reported. A template that
// does not parse as plain text output, such as one using `?no_esc`, is parsed
// again with HTML auto-escaping, as Keycloak configures it; neither setting
// changes where a comment is. String literals in expressions are not
// reported, so a template's literals are empty. A template that does not
// parse counts as one error.
import freemarker.core.HTMLOutputFormat;
import freemarker.core.TemplateElement;
import freemarker.template.Configuration;
import freemarker.template.Template;
import java.io.BufferedReader;
import java.io.InputStreamReader;
import java.nio.charset.StandardCharsets;
import java.nio.file.Files;
import java.nio.file.Path;
import java.util.ArrayList;
import java.util.List;

public class Judge {
    public static void main(String[] args) throws Exception {
        Configuration plain = configuration();
        Configuration html = configuration();
        html.setOutputFormat(HTMLOutputFormat.INSTANCE);
        BufferedReader in = new BufferedReader(new InputStreamReader(System.in, StandardCharsets.UTF_8));
        StringBuilder out = new StringBuilder();
        for (String path; (path = in.readLine()) != null; ) {
            String text = new String(Files.readAllBytes(Path.of(path)), StandardCharsets.UTF_8);
            if (text.startsWith("﻿")) text = text.substring(1);
            List<int[]> comments = new ArrayList<>();
            int errors = 0;
            Template t = parse(path, text, plain);
            if (t == null) t = parse(path, text, html);
            if (t == null) errors = 1;
            else collect(t.getRootTreeNode(), lineStarts(text), comments);
            out.setLength(0);
            out.append("{\"path\":").append(quote(path)).append(",\"units\":\"utf16\",\"errors\":").append(errors)
                .append(",\"comments\":[");
            for (int i = 0; i < comments.size(); i++) {
                if (i > 0) out.append(',');
                out.append('[').append(comments.get(i)[0]).append(',').append(comments.get(i)[1]).append(']');
            }
            out.append("],\"literals\":[]}");
            System.out.println(out);
        }
    }

    static Configuration configuration() {
        Configuration cfg = new Configuration(Configuration.VERSION_2_3_34);
        cfg.setTagSyntax(Configuration.AUTO_DETECT_TAG_SYNTAX);
        // Columns then count characters, not tab stops.
        cfg.setTabSize(1);
        return cfg;
    }

    static Template parse(String path, String text, Configuration cfg) {
        try {
            return new Template(path, text, cfg);
        } catch (Exception e) {
            return null;
        }
    }

    // The offset each line starts at, ending lines at `\r\n`, `\r` or `\n`
    // as FreeMarker counts them.
    static List<Integer> lineStarts(String text) {
        List<Integer> starts = new ArrayList<>(List.of(0));
        for (int i = 0; i < text.length(); i++) {
            char c = text.charAt(i);
            if (c == '\n' || (c == '\r' && (i + 1 == text.length() || text.charAt(i + 1) != '\n'))) {
                starts.add(i + 1);
            }
        }
        return starts;
    }

    @SuppressWarnings("deprecation")
    static void collect(TemplateElement element, List<Integer> starts, List<int[]> comments) {
        if (element == null) return;
        if (element.getClass().getName().equals("freemarker.core.Comment")) {
            int begin = starts.get(element.getBeginLine() - 1) + element.getBeginColumn() - 1;
            int end = starts.get(element.getEndLine() - 1) + element.getEndColumn();
            comments.add(new int[] {begin, end});
        }
        for (int i = 0; i < element.getChildCount(); i++) {
            collect((TemplateElement) element.getChildAt(i), starts, comments);
        }
    }

    static String quote(String s) {
        return "\"" + s.replace("\\", "\\\\").replace("\"", "\\\"") + "\"";
    }
}
