extern crate ignore;
extern crate regex;
extern crate tokei;

use std::fs;

use once_cell::sync::Lazy;
use regex::Regex;
use tokei::{Config, Languages};

static LINES: Lazy<Regex> = Lazy::new(|| Regex::new(r"\d+ lines").unwrap());
static CODE: Lazy<Regex> = Lazy::new(|| Regex::new(r"\d+ code").unwrap());
static COMMENTS: Lazy<Regex> = Lazy::new(|| Regex::new(r"\d+ comments").unwrap());
static BLANKS: Lazy<Regex> = Lazy::new(|| Regex::new(r"\d+ blanks").unwrap());

macro_rules! get_digit {
    ($regex:expr, $text:expr) => {{
        let matched = $regex.find(&$text).expect("Couldn't find category");
        matched
            .as_str()
            .split_whitespace()
            .next()
            .unwrap()
            .parse::<usize>()
            .unwrap()
    }};
}

mod config {
    use tokei::*;

    /*
    #[test]
    fn extension_change() {
        use std::collections::HashMap;
        let mut languages = Languages::new();
        let config = Config {
            languages: {
                let mut map = HashMap::new();
                let mut config = LanguageConfig::new();
                config.extensions(vec![String::from("cpp")]);
                map.insert(LanguageType::C, config);

                Some(map)
            },
            ..Config::default()
        };

        languages.get_statistics(&["tests/data/cpp.cpp"], &[], &config);

        if languages.len() != 1 {
            panic!("wrong languages detected: expected just C, found {:?}",
                   languages.into_iter().collect::<Vec<_>>());
        }

        let (name, _) = languages.into_iter().next().unwrap();

        assert_eq!(LanguageType::C, name);
    }
    */

    #[test]
    fn treating_comments_as_code() {
        let mut languages = Languages::new();
        let config = Config {
            treat_doc_strings_as_comments: Some(true),
            ..Config::default()
        };

        languages.get_statistics(&["tests/data/python.py"], &[], &config);

        if languages.len() != 1 {
            panic!(
                "wrong languages detected: expected just Python, found {:?}",
                languages.into_iter().collect::<Vec<_>>()
            );
        }

        let (_, language) = languages.into_iter().next().unwrap();

        assert_eq!(language.lines(), 15);
        assert_eq!(language.blanks, 3);
        assert_eq!(language.comments, 7);
        assert_eq!(language.code, 5);
    }
}

mod line_classes {
    use tokei::{Config, LanguageType, LineClass};

    #[test]
    fn classes_each_line_in_order() {
        let text =
            "fn main() {\n    // c\n\n    /* a */ let b = \"/*\"; /* c */\n    /* d */ // e\n}\n";
        let classes = LanguageType::Rust.line_classes(text.as_bytes(), &Config::default());
        use LineClass::*;
        assert_eq!(
            classes,
            Some(vec![Code, Comment, Blank, Code, Comment, Code])
        );
    }

    #[test]
    fn classes_lines_of_embedded_elements_as_their_language_does() {
        let text =
            "<p>a</p>\n<script>\n// c\nlet a = 1;\n\n</script>\n<style>\n/* s */\n</style>\n";
        let classes = LanguageType::Html.line_classes(text.as_bytes(), &Config::default());
        use LineClass::*;
        assert_eq!(
            classes,
            Some(vec![
                Code, Code, Comment, Code, Blank, Code, Code, Comment, Code
            ])
        );
    }

    #[test]
    fn classes_an_embedded_element_that_opens_and_closes_on_one_line() {
        let text = "<p>a</p>\n<script type=\"text/markdown\">## Hi</script>\n<p>b</p>\n";
        let classes = LanguageType::Html.line_classes(text.as_bytes(), &Config::default());
        use LineClass::*;
        assert_eq!(classes, Some(vec![Code, Code, Code]));
    }

    /// The fork's own counts are the oracle: wherever `line_classes` answers,
    /// its classes must tally to them on every fixture.
    #[test]
    fn classes_tally_to_the_counts_of_every_fixture() {
        let config = Config::default();
        let mut checked = 0;
        for entry in std::fs::read_dir("tests/data").unwrap() {
            let path = entry.unwrap().path();
            let Some(language) = LanguageType::from_path(&path, &config) else {
                continue;
            };
            let text = std::fs::read(&path).unwrap();
            let Some(classes) = language.line_classes(&text, &config) else {
                continue;
            };
            let stats = language.parse_from_slice(&text, &config).summarise();
            let tally = |class| classes.iter().filter(|c| **c == class).count();
            assert_eq!(
                (
                    tally(LineClass::Code),
                    tally(LineClass::Comment),
                    tally(LineClass::Blank)
                ),
                (stats.code, stats.comments, stats.blanks),
                "{}",
                path.display()
            );
            checked += 1;
        }
        assert!(checked > 250, "only {checked} fixtures classified");
    }
}

include!(concat!(env!("OUT_DIR"), "/tests.rs"));
