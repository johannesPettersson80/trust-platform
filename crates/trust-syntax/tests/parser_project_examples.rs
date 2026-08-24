mod common;
use common::*;

include!("../../../tests/support/repository_source_oracle.rs");

// Example Project Parsing Tests
/// Test that all example files parse without errors
#[test]
fn test_examples_parse() {
    use std::fs;
    use std::path::Path;

    fn test_dir(dir: &Path, repository_root: &Path) {
        if !dir.exists() {
            return;
        }
        for entry in fs::read_dir(dir).unwrap() {
            let entry = entry.unwrap();
            let path = entry.path();
            if path.is_dir() {
                test_dir(&path, repository_root);
            } else if path.extension().map(|e| e == "st").unwrap_or(false) {
                let content = repository_source_tree_read_to_string!(
                    (&path, repository_root),
                    roots = ["examples"],
                    extension = "st",
                )
                .unwrap();
                let parsed = parse(&content);

                if !parsed.ok() {
                    let errors: Vec<_> = parsed.errors().iter().map(|e| e.to_string()).collect();
                    panic!("Parse errors in {}:\n{}", path.display(), errors.join("\n"));
                }
            }
        }
    }

    let repository_root = Path::new(env!("CARGO_MANIFEST_DIR"))
        .parent()
        .and_then(Path::parent)
        .expect("workspace root");
    let examples = repository_root.join("examples");
    assert!(
        examples.is_dir(),
        "parser example corpus must exist at {}",
        examples.display()
    );
    test_dir(&examples, repository_root);
}

#[test]
fn test_pragmas_parse_and_preserve_tokens() {
    let source = r#"
{VERSION 2.0}
PROGRAM P
VAR
    x : INT;
END_VAR
    x := 1;
END_PROGRAM
"#;

    let parsed = parse(source);
    assert!(
        parsed.ok(),
        "Parse errors: {:?}",
        parsed
            .errors()
            .iter()
            .map(|e| e.to_string())
            .collect::<Vec<_>>()
    );

    let pragma_count = parsed
        .syntax()
        .descendants_with_tokens()
        .filter_map(|it| it.into_token())
        .filter(|t| t.kind() == SyntaxKind::Pragma)
        .count();

    assert_eq!(pragma_count, 1);
}
