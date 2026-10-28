//! A project manifest is readable when the language server loads it (specification 25 §6.2).

use super::*;

fn issue(text: &str) -> Option<String> {
    let answer = answer(json!({ "text": text })).expect("answer");
    assert_eq!(answer["ok"], json!(true));
    answer["issue"].as_str().map(str::to_owned)
}

#[test]
fn the_manifest_of_a_new_project_is_readable() {
    // `trust-runtime new` writes `include_paths = ["src"]` at the top. Catches: a new project shown
    // as needing repair.
    assert_eq!(issue("include_paths = [\"src\"]\n"), None);
    assert_eq!(
        issue(""),
        None,
        "an empty manifest is a project with the defaults"
    );
}

#[test]
fn table_headers_that_toml_allows_are_readable() {
    // `[ project ]` and a quoted key are valid TOML, which the language server reads. Catches: the
    // extension's former header pattern, which called both incomplete.
    assert_eq!(issue("[ project ]\ninclude_paths = [\"src\"]\n"), None);
    assert_eq!(
        issue("[dependencies]\n\"My Lib\" = { path = \"libs/my\" }\n"),
        None
    );
}

#[test]
fn an_incomplete_header_names_its_line() {
    // Line 3 opens a table header and never closes it.
    let found = issue("# settings\ninclude_paths = [\"src\"]\n[dependencies\n").expect("an issue");
    assert!(
        found.starts_with("Project settings on line 3 cannot be read: "),
        "{found}"
    );
}

#[test]
fn a_value_of_the_wrong_kind_is_not_readable() {
    // `include_paths` of `[project]` is a list of folders; a text on line 2 makes the language server
    // drop the whole file. Catches: a header-only check that calls such a project open.
    let found = issue("[project]\ninclude_paths = \"src\"\n").expect("an issue");
    assert!(
        found.starts_with("Project settings on line 2 cannot be read: "),
        "{found}"
    );
}

#[test]
fn a_dependency_without_a_value_names_its_line() {
    // Line 4 gives `path` no value. Catches: a check that reads only the headers.
    let found = issue("include_paths = [\"src\"]\n\n[dependencies]\nOSCAT = { path = }\n")
        .expect("an issue");
    assert!(
        found.starts_with("Project settings on line 4 cannot be read: "),
        "{found}"
    );
}
