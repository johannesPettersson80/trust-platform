use std::collections::BTreeSet;
use std::path::PathBuf;
use trust_ide::stdlib_docs::{standard_fb_names, standard_function_entries};

include!("../../../tests/support/repository_source_oracle.rs");

fn coverage_tokens() -> BTreeSet<String> {
    let repository_root = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../..");
    let coverage_path = repository_root.join("docs/specs/coverage/standard-functions-coverage.md");
    let contents = repository_source_tree_read_to_string!(
        (&coverage_path, &repository_root),
        roots = ["docs/specs/coverage"],
        extension = "md",
    )
    .unwrap_or_else(|err| panic!("failed to read coverage doc: {err}"));

    let mut tokens = BTreeSet::new();
    for line in contents.lines() {
        let line = line.trim();
        if !(line.starts_with("- [x]") || line.starts_with("- [~]")) {
            continue;
        }
        tokens.extend(extract_tokens(line));
    }
    tokens
}

fn extract_tokens(line: &str) -> Vec<String> {
    let mut tokens = Vec::new();
    let mut current = String::new();
    for ch in line.chars() {
        if ch.is_ascii_uppercase() || ch.is_ascii_digit() || ch == '_' {
            if current.is_empty() && ch.is_ascii_digit() {
                continue;
            }
            current.push(ch);
        } else if !current.is_empty() {
            tokens.push(std::mem::take(&mut current));
        }
    }
    if !current.is_empty() {
        tokens.push(current);
    }
    tokens
}

#[test]
fn coverage_doc_includes_all_stdlib_names() {
    let coverage = coverage_tokens();
    let mut missing = Vec::new();

    for entry in standard_function_entries() {
        let name = entry.name.as_str();
        if entry.doc.contains("Tables 22-27") {
            continue;
        }
        if !coverage.contains(name) {
            missing.push(name.to_string());
        }
    }

    for name in standard_fb_names() {
        if !coverage.contains(*name) {
            missing.push((*name).to_string());
        }
    }

    assert!(
        missing.is_empty(),
        "coverage doc missing stdlib entries: {missing:?}"
    );
}
