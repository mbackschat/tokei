//! Comment and literal ranges from tree-sitter syntax trees, with the
//! interpolation holes of literals as code.

use crate::lines::{Class, Region};
use tree_sitter::{Language, Node, Parser};

pub fn regions(text: &str, language: &Language) -> (Vec<Region>, bool) {
    let mut parser = Parser::new();
    parser.set_language(language).unwrap();
    let tree = parser.parse(text, None).unwrap();
    let mut out = Vec::new();
    walk(tree.root_node(), text.as_bytes(), &mut out);
    (out, tree.root_node().has_error())
}

/// HTML regions with the bodies of `<script>` and `<style>` elements parsed
/// by their own grammars, which tree-sitter-html leaves as raw text. A script
/// is JavaScript only for the types the HTML standard runs as one (HTML
/// §4.12.1.1, "prepare the script element"); other types are data blocks,
/// whose text is not code of any grammar here and stays as raw text.
pub fn html_regions(text: &str) -> (Vec<Region>, bool) {
    let mut parser = Parser::new();
    parser.set_language(&tree_sitter_html::LANGUAGE.into()).unwrap();
    let tree = parser.parse(text, None).unwrap();
    let bytes = text.as_bytes();
    let mut out = Vec::new();
    walk(tree.root_node(), bytes, &mut out);
    let mut error = tree.root_node().has_error();
    let mut bodies = Vec::new();
    embedded(tree.root_node(), bytes, &mut bodies);
    for (body, language) in bodies {
        let (inner, inner_error) = regions(&text[body.start_byte()..body.end_byte()], &language);
        error |= inner_error;
        let offset = body.start_byte();
        out.extend(inner.into_iter().map(|r| Region { start: r.start + offset, end: r.end + offset, class: r.class }));
    }
    (out, error)
}

fn embedded<'t>(node: Node<'t>, bytes: &[u8], out: &mut Vec<(Node<'t>, Language)>) {
    let mut cursor = node.walk();
    for child in node.children(&mut cursor) {
        let language: Option<Language> = match child.kind() {
            "script_element" if runs_as_javascript(child, bytes) => Some(tree_sitter_javascript::LANGUAGE.into()),
            "style_element" => Some(tree_sitter_css::LANGUAGE.into()),
            "script_element" => None,
            _ => {
                embedded(child, bytes, out);
                continue;
            }
        };
        let mut inner = child.walk();
        if let (Some(language), Some(body)) = (language, child.children(&mut inner).find(|n| n.kind() == "raw_text")) {
            out.push((body, language));
        }
    }
}

fn runs_as_javascript(script: Node, bytes: &[u8]) -> bool {
    let attribute = |name: &str| -> Option<String> {
        let start = script.child(0)?;
        let mut cursor = start.walk();
        let found = start.children(&mut cursor).filter(|a| a.kind() == "attribute").find(|a| {
            a.child(0).is_some_and(|n| n.utf8_text(bytes).is_ok_and(|t| t.eq_ignore_ascii_case(name)))
        });
        let value = found.map(|a| {
            let mut c = a.walk();
            a.children(&mut c)
                .filter(|n| matches!(n.kind(), "attribute_value" | "quoted_attribute_value"))
                .last()
                .map_or(String::new(), |n| n.utf8_text(bytes).unwrap_or("").trim_matches(|c| c == '"' || c == '\'').to_string())
        });
        value
    };
    let essence = match attribute("type") {
        Some(t) => t.split(';').next().unwrap_or("").trim().to_ascii_lowercase(),
        None => match attribute("language") {
            Some(l) if !l.is_empty() => format!("text/{}", l.to_ascii_lowercase()),
            _ => String::new(),
        },
    };
    // Babel standalone transpiles `text/babel` and `text/jsx` scripts as
    // JavaScript (babeljs.io/docs/babel-standalone).
    essence.is_empty()
        || matches!(essence.as_str(), "module" | "text/babel" | "text/jsx")
        || matches!(
            essence.as_str(),
            "application/ecmascript" | "application/javascript" | "application/x-ecmascript" | "application/x-javascript"
                | "text/ecmascript" | "text/javascript" | "text/javascript1.0" | "text/javascript1.1" | "text/javascript1.2"
                | "text/javascript1.3" | "text/javascript1.4" | "text/javascript1.5" | "text/jscript" | "text/livescript"
                | "text/x-ecmascript" | "text/x-javascript"
        )
}

/// Emits the regions of `root`'s subtree in pre-order, without recursion, so
/// deeply nested code cannot overflow the stack.
fn walk(root: Node, bytes: &[u8], out: &mut Vec<Region>) {
    let mut cursor = root.walk();
    'walk: loop {
        let node = cursor.node();
        let kind = node.kind();
        // Haskell pragmas are written as comments and Tokei counts them so.
        // Perl ignores POD (perlsyn, "PODs: Embedded Documentation"), so it
        // is a comment, as Tokei's `=pod` declaration intends.
        if kind.contains("comment") || matches!(kind, "Comment" | "haddock" | "xml_doc" | "pragma" | "pod") {
            let mut end = node.end_byte();
            // Line comment tokens of some grammars include their newline.
            while end > node.start_byte() && matches!(bytes[end - 1], b'\n' | b'\r') {
                end -= 1;
            }
            // POD without `=cut` runs to the end of the file (perlpod), past
            // the blank lines that tree-sitter leaves outside the node.
            if kind == "pod" && !bytes[node.start_byte()..end].trim_ascii_end().ends_with(b"=cut")
                && bytes[end..].iter().all(u8::is_ascii_whitespace)
            {
                end = bytes.len();
            }
            out.push(Region { start: node.start_byte(), end, class: Class::Comment });
        } else if is_literal(kind) {
            out.push(Region { start: literal_start(node, bytes), end: literal_end(node, bytes), class: Class::Literal });
            holes(node, bytes, out);
        } else if cursor.goto_first_child() {
            continue;
        }
        loop {
            if cursor.node() == root {
                break 'walk;
            }
            if cursor.goto_next_sibling() {
                continue 'walk;
            }
            if !cursor.goto_parent() {
                break 'walk;
            }
        }
    }
}

/// The start of a literal. Heredoc grammars start the body at its first
/// non-blank line, but the body is every line after the redirection (Bash
/// Reference Manual §3.6.6), so it starts at the newline ending the line
/// before its leading blank lines.
fn literal_start(node: Node, bytes: &[u8]) -> usize {
    let start = node.start_byte();
    if !matches!(node.kind(), "heredoc_body" | "heredoc_content") {
        return start;
    }
    let mut s = start;
    while s > 0 && bytes[s - 1].is_ascii_whitespace() {
        s -= 1;
    }
    match bytes[s..start].iter().position(|b| *b == b'\n') {
        Some(n) => s + n,
        None => start,
    }
}

/// The end of a literal. A YAML block scalar's node runs over the blank lines
/// after its content, which clip and strip chomping remove from the value
/// (YAML 1.2.2 §8.1.1.2), so they end at the last content character; keep
/// chomping, `+`, retains them.
fn literal_end(node: Node, bytes: &[u8]) -> usize {
    let end = node.end_byte();
    if node.kind() != "block_scalar" {
        return end;
    }
    let text = &bytes[node.start_byte()..end];
    let header_end = text.iter().position(|b| *b == b'\n').unwrap_or(text.len());
    if text[..header_end].split(|b| *b == b'#').next().is_some_and(|h| h.contains(&b'+')) {
        // Keep chomping makes the empty lines after the content part of it,
        // which tree-sitter-yaml leaves outside the node.
        let next_text = bytes[end..].iter().position(|b| !b.is_ascii_whitespace()).map_or(bytes.len(), |n| end + n);
        return bytes[..next_text].iter().rposition(|b| *b == b'\n').map_or(end, |n| n.max(end));
    }
    node.start_byte() + text.iter().rposition(|b| !b.is_ascii_whitespace()).map_or(0, |n| n + 1)
}

/// Emits the interpolation holes inside a literal as code, with their contents.
fn holes(literal: Node, bytes: &[u8], out: &mut Vec<Region>) {
    let mut cursor = literal.walk();
    for child in literal.children(&mut cursor) {
        if is_hole(child.kind()) {
            out.push(Region { start: child.start_byte(), end: child.end_byte(), class: Class::Code });
            let mut inner = child.walk();
            for part in child.children(&mut inner) {
                walk(part, bytes, out);
            }
        } else {
            holes(child, bytes, out);
        }
    }
}

fn is_hole(kind: &str) -> bool {
    kind.contains("interpolation") && !kind.contains("string") || matches!(kind, "template_substitution" | "interpolated_expression" | "command_substitution")
}

fn is_literal(kind: &str) -> bool {
    (kind.contains("string")
        && !kind.contains("expansion")
        && !kind.contains("concatenated")
        && !kind.contains("get_expression"))
        || kind.contains("char_literal")
        || kind.contains("character_literal")
        || matches!(kind, "char" | "character" | "rune_literal" | "heredoc" | "heredoc_body" | "heredoc_content" | "nowdoc" | "regex" | "block_scalar" | "double_quote_scalar" | "single_quote_scalar")
        // HTML attribute values in quotes (HTML §13.2.5.36 and §13.2.5.37).
        || kind == "quoted_attribute_value"
        // Perl's quote-like operators (perlop, "Quote and Quote-like Operators").
        || matches!(kind, "quoted_word_list" | "match_regexp" | "quoted_regexp" | "substitution_regexp" | "transliteration_expression")
}
