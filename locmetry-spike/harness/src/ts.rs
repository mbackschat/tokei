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

/// Emits the regions of `root`'s subtree in pre-order, without recursion, so
/// deeply nested code cannot overflow the stack.
fn walk(root: Node, bytes: &[u8], out: &mut Vec<Region>) {
    let mut cursor = root.walk();
    'walk: loop {
        let node = cursor.node();
        let kind = node.kind();
        // Haskell pragmas are written as comments and Tokei counts them so.
        if kind.contains("comment") || matches!(kind, "haddock" | "xml_doc" | "pragma") {
            let mut end = node.end_byte();
            // Line comment tokens of some grammars include their newline.
            while end > node.start_byte() && matches!(bytes[end - 1], b'\n' | b'\r') {
                end -= 1;
            }
            out.push(Region { start: node.start_byte(), end, class: Class::Comment });
        } else if is_literal(kind) {
            out.push(Region { start: node.start_byte(), end: node.end_byte(), class: Class::Literal });
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
        || matches!(kind, "char" | "character" | "rune_literal" | "heredoc" | "heredoc_body" | "nowdoc" | "regex")
}
