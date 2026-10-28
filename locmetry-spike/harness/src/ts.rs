//! Comment and literal ranges from tree-sitter syntax trees.

use crate::lines::{Class, Region};
use tree_sitter::{Language, Parser};

pub fn regions(text: &str, language: &Language) -> (Vec<Region>, bool) {
    let mut parser = Parser::new();
    parser.set_language(language).unwrap();
    let tree = parser.parse(text, None).unwrap();
    let bytes = text.as_bytes();
    let mut out = Vec::new();
    let mut cursor = tree.walk();
    'walk: loop {
        let node = cursor.node();
        let kind = node.kind();
        // Haskell pragmas are written as comments and Tokei counts them so.
        let class = if kind.contains("comment") || matches!(kind, "haddock" | "xml_doc" | "pragma") {
            Some(Class::Comment)
        } else if is_literal(kind) {
            Some(Class::Literal)
        } else {
            None
        };
        match class {
            Some(class) => {
                let mut end = node.end_byte();
                // Line comment tokens of some grammars include their newline.
                while class == Class::Comment && end > node.start_byte() && matches!(bytes[end - 1], b'\n' | b'\r') {
                    end -= 1;
                }
                out.push(Region { start: node.start_byte(), end, class });
            }
            None if cursor.goto_first_child() => continue,
            None => {}
        }
        loop {
            if cursor.goto_next_sibling() {
                continue 'walk;
            }
            if !cursor.goto_parent() {
                break 'walk;
            }
        }
    }
    (out, tree.root_node().has_error())
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
