extern crate ignore;
extern crate json5;
extern crate serde_json;

use std::ffi::OsStr;
use std::fs;
use std::path::Path;
use std::{cmp, env, error};

use ignore::Walk;
use serde_json::Value;

fn main() -> Result<(), Box<dyn error::Error>> {
    let out_dir = env::var_os("OUT_DIR").expect("No OUT_DIR variable.");
    generate_languages(&out_dir)?;
    generate_tests(&out_dir)?;

    Ok(())
}

fn generate_languages(out_dir: &OsStr) -> Result<(), Box<dyn error::Error>> {
    let mut tera = tera::Tera::default();

    let json_string: String = fs::read_to_string("languages.json")?.parse()?;
    let mut json: Value = json5::from_str(&json_string)?;
    inherit_syntax(json.get_mut("languages").unwrap().as_object_mut().unwrap());

    for (_key, ref mut item) in json
        .get_mut("languages")
        .unwrap()
        .as_object_mut()
        .unwrap()
        .iter_mut()
    {
        macro_rules! sort_prop {
            ($prop:expr) => {{
                if let Some(ref mut prop) = item.get_mut($prop) {
                    prop.as_array_mut()
                        .unwrap()
                        .sort_unstable_by(compare_json_str_len)
                }
            }};
        }

        sort_prop!("quotes");
        sort_prop!("verbatim_quotes");
        sort_prop!("multi_line");
    }

    let output_path = Path::new(&out_dir).join("language_type.rs");
    let rust_code = tera.render_str(
        &std::fs::read_to_string("src/language/language_type.tera.rs")?,
        &tera::Context::from_value(json)?,
    )?;
    std::fs::write(output_path, rust_code)?;

    Ok(())
}

/// Fields that say which files a language claims and how it is named, as
/// opposed to how its text is lexed.
const IDENTITY_FIELDS: &[&str] = &[
    "name",
    "extensions",
    "filenames",
    "exact_filenames",
    "content_checked_extensions",
    "path_suffixes",
    "env",
    "mime",
    "shebangs",
    "syntax_of",
];

/// Gives each language declaring `"syntax_of": "Other"` every lexical field
/// of `Other`, for languages read by another's lexer, as Nextflow scripts are
/// Groovy and Snakefiles are tokenized as Python.
fn inherit_syntax(languages: &mut serde_json::Map<String, Value>) {
    let inheriting: Vec<(String, String)> = languages
        .iter()
        .filter_map(|(key, value)| {
            let source = value.get("syntax_of")?.as_str()?;
            Some((key.clone(), source.to_owned()))
        })
        .collect();
    for (key, source) in inheriting {
        let lexical: Vec<(String, Value)> = languages
            .get(&source)
            .and_then(Value::as_object)
            .unwrap_or_else(|| panic!("{key} takes the syntax of unknown {source}"))
            .iter()
            .filter(|(field, _)| !IDENTITY_FIELDS.contains(&field.as_str()))
            .map(|(field, value)| (field.clone(), value.clone()))
            .collect();
        let target = languages.get_mut(&key).unwrap().as_object_mut().unwrap();
        for (field, value) in lexical {
            assert!(
                !target.contains_key(&field),
                "{key} declares {field} although it takes the syntax of {source}"
            );
            target.insert(field, value);
        }
    }
}

fn compare_json_str_len(a: &Value, b: &Value) -> cmp::Ordering {
    let a = a.as_array().expect("a as array");
    let b = b.as_array().expect("b as array");

    let max_a_size = a.iter().map(|e| e.as_str().unwrap().len()).max().unwrap();
    let max_b_size = b.iter().map(|e| e.as_str().unwrap().len()).max().unwrap();

    max_b_size.cmp(&max_a_size)
}

fn generate_tests(out_dir: &OsStr) -> Result<(), Box<dyn error::Error>> {
    // Length of string literal below by number of languages
    const INITIAL_BUFFER_SIZE: usize = 989 * 130;
    let mut string = String::with_capacity(INITIAL_BUFFER_SIZE);

    generate_tests_batch("./tests/data", None, &mut string)?;
    generate_tests_batch("./tests/embedding", Some("embedding"), &mut string)?;

    Ok(fs::write(Path::new(&out_dir).join("tests.rs"), string)?)
}

fn generate_tests_batch(
    src_dir: &str,
    test_module: Option<&str>,
    string: &mut String,
) -> Result<(), Box<dyn error::Error>> {
    let walker = Walk::new(src_dir).filter(|p| match p {
        Ok(ref p) => {
            if let Ok(ref p) = p.metadata() {
                p.is_file()
            } else {
                false
            }
        }
        _ => false,
    });

    if let Some(test_module) = test_module {
        string.push_str(&format!(
            r####"
#[cfg(test)]
mod {0} {{
use super::*;
        "####,
            test_module
        ));
    }

    for path in walker {
        let path = path?;
        let path = path.path();
        let root = std::path::PathBuf::from(std::env::var("CARGO_MANIFEST_DIR").unwrap());

        let name = path
            .file_stem()
            .unwrap()
            .to_str()
            .unwrap()
            .to_lowercase()
            .replace('.', "_");

        if name == "jupyter" {
            continue;
        }

        string.push_str(&format!(
            r####"
        #[test]
        fn {0}() {{
            const _: &str = include_str!(r###"{2}"###);
            let mut languages = Languages::new();
            languages.get_statistics(&["{1}"], &[], &Config::default());

            if languages.len() != 1 {{
                panic!("wrong languages detected: expected just {0}, found {{:?}}",
                       languages.into_iter().collect::<Vec<_>>());
            }}

            let (name, language) = languages.into_iter().next().unwrap();
            let mut language = language.summarise();

            let contents = fs::read_to_string("{1}").unwrap();

            println!("{{}} {1}", name);
            assert_eq!(get_digit!(LINES, contents), language.lines());
            println!("{{}} LINES MATCH", name);
            assert_eq!(get_digit!(CODE, contents), language.code);
            println!("{{}} CODE MATCH", name);
            assert_eq!(get_digit!(COMMENTS, contents), language.comments);
            println!("{{}} COMMENTS MATCH", name);
            assert_eq!(get_digit!(BLANKS, contents), language.blanks);
            println!("{{}} BLANKS MATCH", name);

            let report = language.reports.pop().unwrap();
            let stats = report.stats.summarise();

            assert_eq!(language.lines(), stats.lines());
            assert_eq!(language.code, stats.code);
            assert_eq!(language.comments, stats.comments);
            assert_eq!(language.blanks, stats.blanks);
        }}
        "####,
            name,
            path.to_string_lossy().replace('\\', "/"),
            std::fs::canonicalize(root.join(path)).unwrap().display(),
        ));
    }

    if test_module.is_some() {
        string.push_str("\n}");
    }

    Ok(())
}
