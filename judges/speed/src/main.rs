// Parses every file under the given roots whose language Tokei recognizes,
// from memory, single threaded and three times, so the time measures the
// parser alone, not reading or walking.
use tokei::{Config, LanguageType};

fn main() {
    let config = Config::default();
    let mut files = Vec::new();
    for root in std::env::args().skip(1) {
        for entry in walkdir::WalkDir::new(root).into_iter().filter_map(Result::ok) {
            if !entry.file_type().is_file() {
                continue;
            }
            if let Some(language) = LanguageType::from_path(entry.path(), &config) {
                if let Ok(bytes) = std::fs::read(entry.path()) {
                    files.push((language, bytes));
                }
            }
        }
    }
    let mut code = 0;
    for _ in 0..3 {
        for (language, bytes) in &files {
            code += language.parse_from_slice(bytes, &config).code;
        }
    }
    println!("{code}");
}
