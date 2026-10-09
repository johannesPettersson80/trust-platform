//! The Control library's manual (`docs/guides/CONTROL_LIBRARY_GUIDE.md`, specifications 32 and
//! 37): every Structured Text example in it compiles with the library and runs, and it names every
//! type, function block, function and public method the library declares.

use std::path::{Path, PathBuf};

use trust_runtime::harness::TestHarness;
use trust_runtime::value::Duration;

fn repo() -> &'static Path {
    Path::new(concat!(env!("CARGO_MANIFEST_DIR"), "/../.."))
}

fn guide() -> String {
    std::fs::read_to_string(repo().join("docs/guides/CONTROL_LIBRARY_GUIDE.md"))
        .expect("the manual exists")
}

/// The library's sources, in a stable order.
fn library() -> Vec<String> {
    let mut paths: Vec<PathBuf> = std::fs::read_dir(repo().join("libraries/control/src"))
        .expect("the sources")
        .map(|entry| entry.expect("an entry").path())
        .filter(|path| path.extension().is_some_and(|extension| extension == "st"))
        .collect();
    paths.sort();
    paths
        .iter()
        .map(|path| std::fs::read_to_string(path).expect("readable"))
        .collect()
}

/// The identifier at the start of `text`.
fn identifier(text: &str) -> &str {
    text.split(|c: char| !(c.is_alphanumeric() || c == '_'))
        .next()
        .unwrap_or_default()
}

#[test]
fn every_example_of_the_manual_compiles_and_runs_with_the_library() {
    let guide = guide();
    let library = library();
    let examples: Vec<&str> = guide
        .split("```iecst")
        .skip(1)
        .filter_map(|rest| rest.split("```").next())
        .collect();
    assert!(
        examples.len() >= 5,
        "the manual shows the library at work: {} examples",
        examples.len()
    );
    for (index, example) in examples.iter().enumerate() {
        let sources: Vec<&str> = library
            .iter()
            .map(String::as_str)
            .chain(std::iter::once(*example))
            .collect();
        let mut harness = TestHarness::from_sources(&sources)
            .unwrap_or_else(|error| panic!("example {} compiles: {error:?}\n{example}", index + 1));
        for _ in 0..3 {
            harness.advance_time(Duration::from_millis(100));
            let result = harness.cycle();
            assert!(
                result.errors.is_empty(),
                "example {} runs: {:?}",
                index + 1,
                result.errors
            );
        }
    }
}

#[test]
fn the_manual_names_everything_the_library_declares() {
    let guide = guide();
    let mut missing = Vec::new();
    for source in library() {
        let mut block: Option<String> = None;
        for line in source.lines().map(str::trim) {
            if line.starts_with("END_FUNCTION_BLOCK") {
                block = None;
            } else if let Some(rest) = ["TYPE ", "FUNCTION_BLOCK ", "FUNCTION ", "INTERFACE "]
                .iter()
                .find_map(|keyword| line.strip_prefix(keyword))
            {
                let name = identifier(rest);
                if line.starts_with("FUNCTION_BLOCK ") {
                    block = Some(name.to_string());
                }
                if !guide.contains(&format!("`{name}`")) {
                    missing.push(name.to_string());
                }
            } else if let (Some(block), Some(rest)) = (&block, line.strip_prefix("METHOD PUBLIC "))
            {
                let method = format!("{block}.{}", identifier(rest));
                if !guide.contains(&method) {
                    missing.push(method);
                }
            }
        }
    }
    assert_eq!(
        missing,
        Vec::<String>::new(),
        "declarations the manual does not name"
    );
}
