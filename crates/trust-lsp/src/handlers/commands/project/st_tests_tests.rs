//! The Testing view's tests (specification 25 §11.1; specification 22, "Test POU discovery"). Each
//! test writes its folders; the expected names, lines, projects and keys follow from them.

use super::*;
use std::sync::atomic::{AtomicUsize, Ordering};

static NEXT: AtomicUsize = AtomicUsize::new(0);

/// A folder under the system temporary folder, removed when the test ends.
struct Scratch(PathBuf);

impl Scratch {
    fn new() -> Self {
        let path = std::env::temp_dir().join(format!(
            "trust-lsp-st-tests-{}-{}",
            std::process::id(),
            NEXT.fetch_add(1, Ordering::Relaxed)
        ));
        let _ = std::fs::remove_dir_all(&path);
        std::fs::create_dir_all(&path).expect("scratch");
        // The canonical path, so keys and roots compare with the paths written below.
        Self(std::fs::canonicalize(&path).expect("canonical"))
    }

    fn write(&self, relative: &str, text: &str) -> PathBuf {
        let path = self.0.join(relative);
        std::fs::create_dir_all(path.parent().expect("parent")).expect("folder");
        std::fs::write(&path, text).expect("file");
        path
    }
}

impl Drop for Scratch {
    fn drop(&mut self) {
        let _ = std::fs::remove_dir_all(&self.0);
    }
}

const TESTS: &str = "NAMESPACE Plant\nTEST_PROGRAM Pass_Case\nEND_TEST_PROGRAM\nEND_NAMESPACE\n\nTEST_FUNCTION_BLOCK Pass_Fb\nEND_TEST_FUNCTION_BLOCK\n";

fn discover(state: &ServerState, argument: Value) -> Vec<Value> {
    let answer = answer(state, argument).expect("answer");
    assert_eq!(answer["ok"], json!(true));
    answer["tests"].as_array().expect("tests").clone()
}

fn summary(tests: &[Value]) -> Vec<(String, String, u64)> {
    tests
        .iter()
        .map(|test| {
            (
                test["kind"].as_str().expect("kind").to_owned(),
                test["name"].as_str().expect("name").to_owned(),
                test["line"].as_u64().expect("line"),
            )
        })
        .collect()
}

#[test]
fn discovery_names_tests_with_their_namespace_and_one_based_line() {
    // Line 2 declares Plant.Pass_Case, line 6 Pass_Fb. Catches: the extension's former pattern, which
    // gave the bare name `Pass_Case` that `trust-dev test` never reports, or zero-based lines.
    let project = Scratch::new();
    let file = project.write("src/tests.st", TESTS);
    let state = ServerState::new();
    let tests = discover(
        &state,
        json!({ "action": "discover", "files": [{ "path": file }], "folders": [project.0] }),
    );
    assert_eq!(
        summary(&tests),
        [
            ("TEST_PROGRAM".to_owned(), "Plant.Pass_Case".to_owned(), 2),
            ("TEST_FUNCTION_BLOCK".to_owned(), "Pass_Fb".to_owned(), 6),
        ]
    );
    let first = &tests[0];
    let file_key = path_key(&file);
    assert_eq!(first["projectRoot"], json!(project.0.display().to_string()));
    assert_eq!(
        first["key"],
        json!(format!("{file_key}::2::TEST_PROGRAM::plant.pass_case"))
    );
    assert_eq!(first["nameKey"], json!("TEST_PROGRAM::plant.pass_case"));
    let uri = Url::from_file_path(&file).expect("uri");
    assert_eq!(
        first["id"],
        json!(format!("{uri}::2::TEST_PROGRAM::Plant.Pass_Case"))
    );
}

#[test]
fn an_open_editor_text_counts_instead_of_the_file() {
    // The editor renamed the test without saving. Catches: discovery from the disk while the code
    // lens shows the editor's text.
    let project = Scratch::new();
    let file = project.write("src/tests.st", TESTS);
    let state = ServerState::new();
    let uri = Url::from_file_path(&file).expect("uri");
    state.open_document(
        uri.clone(),
        2,
        "TEST_PROGRAM Edited\nEND_TEST_PROGRAM\n".to_owned(),
    );
    let tests = discover(
        &state,
        json!({ "action": "discover", "files": [{ "path": file, "uri": uri.to_string() }], "folders": [project.0] }),
    );
    assert_eq!(
        summary(&tests),
        [("TEST_PROGRAM".to_owned(), "Edited".to_owned(), 1)]
    );
}

#[test]
fn a_file_changed_on_disk_counts_unless_its_editor_holds_unsaved_changes() {
    // The file on disk lost its test while its saved editor still shows the old text. Catches:
    // stale tests, and their results, kept after a refresh because the editor had not yet taken
    // the change.
    let project = Scratch::new();
    let file = project.write("src/tests.st", "PROGRAM Main\nEND_PROGRAM\n");
    let state = ServerState::new();
    let uri = Url::from_file_path(&file).expect("uri");
    state.open_document(uri, 1, "TEST_PROGRAM Old\nEND_TEST_PROGRAM\n".to_owned());
    let refreshed = discover(
        &state,
        json!({ "action": "discover", "folders": [project.0] }),
    );
    assert!(refreshed.is_empty(), "{refreshed:?}");
    let named = discover(
        &state,
        json!({ "action": "discover", "files": [{ "path": file }], "folders": [project.0] }),
    );
    assert!(named.is_empty(), "{named:?}");
    let unsaved = discover(
        &state,
        json!({ "action": "discover", "folders": [project.0], "unsaved": [file] }),
    );
    assert_eq!(
        summary(&unsaved),
        [("TEST_PROGRAM".to_owned(), "Old".to_owned(), 1)]
    );
}

#[test]
fn without_named_files_every_source_below_the_folders_counts_except_skipped_folders() {
    // `*.st` and `*.POU` count; `*.St` and the files below node_modules, target and .git do not.
    // Catches: a walk that discovers vendored or built copies of the tests.
    let project = Scratch::new();
    project.write("src/a.st", "TEST_PROGRAM A\nEND_TEST_PROGRAM\n");
    project.write("src/c.POU", "TEST_PROGRAM C\nEND_TEST_PROGRAM\n");
    project.write("src/b.St", "TEST_PROGRAM B\nEND_TEST_PROGRAM\n");
    project.write("node_modules/x/x.st", "TEST_PROGRAM X\nEND_TEST_PROGRAM\n");
    project.write("target/y.st", "TEST_PROGRAM Y\nEND_TEST_PROGRAM\n");
    project.write(".git/z.st", "TEST_PROGRAM Z\nEND_TEST_PROGRAM\n");
    project.write("src/plain.st", "PROGRAM Main\nEND_PROGRAM\n");
    let state = ServerState::new();
    let tests = discover(
        &state,
        json!({ "action": "discover", "folders": [project.0] }),
    );
    let names: Vec<_> = tests
        .iter()
        .map(|test| test["name"].as_str().expect("name").to_owned())
        .collect();
    assert_eq!(names, ["A", "C"]);
}

#[test]
fn a_test_belongs_to_its_nearest_source_root_else_its_workspace_folder() {
    // `app/src/t.st` belongs to `app`, which holds `src`; `lib/sources/u.st` to `lib`, which holds
    // `sources`; `scripts/v.st` has neither above it inside the workspace, so it belongs to the
    // workspace folder. Catches: every test run in the workspace folder, or a test without project.
    let workspace = Scratch::new();
    workspace.write("app/src/t.st", "TEST_PROGRAM T\nEND_TEST_PROGRAM\n");
    workspace.write("lib/sources/u.st", "TEST_PROGRAM U\nEND_TEST_PROGRAM\n");
    workspace.write("scripts/v.st", "TEST_PROGRAM V\nEND_TEST_PROGRAM\n");
    let state = ServerState::new();
    let tests = discover(
        &state,
        json!({ "action": "discover", "folders": [workspace.0] }),
    );
    let roots: Vec<_> = tests
        .iter()
        .map(|test| {
            (
                test["name"].as_str().expect("name").to_owned(),
                test["projectRoot"].as_str().expect("root").to_owned(),
            )
        })
        .collect();
    let at = |relative: &str| workspace.0.join(relative).display().to_string();
    assert_eq!(
        roots,
        [
            ("T".to_owned(), at("app")),
            ("U".to_owned(), at("lib")),
            ("V".to_owned(), workspace.0.display().to_string()),
        ]
    );
}

#[test]
fn a_file_outside_every_project_and_folder_has_no_tests() {
    let outside = Scratch::new();
    let file = outside.write("loose.st", "TEST_PROGRAM Loose\nEND_TEST_PROGRAM\n");
    let state = ServerState::new();
    assert!(discover(
        &state,
        json!({ "action": "discover", "files": [{ "path": file }], "folders": [] })
    )
    .is_empty());
}

#[test]
fn the_run_project_follows_the_named_project_file_active_editor_then_first_folder() {
    let workspace = Scratch::new();
    let app_test = workspace.write("app/src/t.st", "");
    let other_test = workspace.write("other/src/o.st", "");
    workspace.write("plain/readme.md", "");
    let state = ServerState::new();
    let folders = json!([workspace.0, workspace.0.join("second")]);
    let root = |argument: Value| {
        answer(&state, argument).expect("answer")["root"]
            .as_str()
            .map(str::to_owned)
    };
    let at = |relative: &str| Some(workspace.0.join(relative).display().to_string());
    assert_eq!(
        root(
            json!({ "action": "project", "projectPath": workspace.0.join("app"), "folders": folders })
        ),
        at("app")
    );
    // A named project without `src` or `sources` is passed over (catches: running a folder that is
    // no project), then the named file decides.
    assert_eq!(
        root(
            json!({ "action": "project", "projectPath": workspace.0.join("plain"), "path": other_test, "folders": folders })
        ),
        at("other")
    );
    assert_eq!(
        root(json!({ "action": "project", "activePath": app_test, "folders": folders })),
        at("app")
    );
    assert_eq!(
        root(json!({ "action": "project", "folders": folders })),
        Some(workspace.0.display().to_string())
    );
    assert_eq!(root(json!({ "action": "project", "folders": [] })), None);
}

#[test]
fn a_run_result_has_the_key_of_the_test_it_belongs_to() {
    // trust-dev reports `src/tests.st` relative to the project and kinds in any case; the result keys
    // equal the discovered keys. A result without a test kind or line has no key, and a status other
    // than passed, failed or error no outcome. Catches: relative files not resolved against the
    // project, or kinds compared by letter case.
    let project = Scratch::new();
    let file = project.write("src/tests.st", TESTS);
    let state = ServerState::new();
    let tests = discover(
        &state,
        json!({ "action": "discover", "files": [{ "path": file }], "folders": [project.0] }),
    );
    let results = answer(
        &state,
        json!({ "action": "results", "projectRoot": project.0, "tests": [
            { "name": "Plant.Pass_Case", "kind": "test_program", "status": "passed", "file": "src/tests.st", "line": 2 },
            { "name": "Pass_Fb", "kind": "TEST_FUNCTION_BLOCK", "status": "failed", "file": file, "line": 6, "message": "expected 1" },
            { "name": "Odd", "kind": "PROGRAM", "status": "passed", "file": "src/tests.st", "line": 9 },
            { "name": "NoLine", "kind": "TEST_PROGRAM", "status": "skipped", "file": "src/tests.st", "line": 0 },
        ] }),
    )
    .expect("answer");
    let cases = results["cases"].as_array().expect("cases");
    assert_eq!(cases[0]["key"], tests[0]["key"]);
    assert_eq!(cases[0]["kind"], json!("TEST_PROGRAM"));
    assert_eq!(cases[0]["outcome"], json!("passed"));
    assert_eq!(cases[0]["fileKey"], tests[0]["fileKey"]);
    assert_eq!(cases[1]["key"], tests[1]["key"]);
    assert_eq!(cases[1]["message"], json!("expected 1"));
    assert_eq!(cases[2]["key"], Value::Null);
    assert_eq!(cases[3]["key"], Value::Null);
    assert_eq!(cases[3]["outcome"], Value::Null);
    assert_eq!(cases[1]["nameKey"], json!("TEST_FUNCTION_BLOCK::pass_fb"));
    assert_eq!(results["projectKey"], tests[0]["projectKey"]);
}

#[test]
fn a_named_test_is_identified_by_the_same_key() {
    // `trust-lsp.test.runOne` names a test by file, line, kind and name; its key is the discovered
    // one whatever the letter case. A kind that is no test kind names nothing.
    let project = Scratch::new();
    let file = project.write("src/tests.st", TESTS);
    let state = ServerState::new();
    let tests = discover(
        &state,
        json!({ "action": "discover", "files": [{ "path": file }], "folders": [project.0] }),
    );
    let identify = |kind: &str, name: &str| {
        answer(
            &state,
            json!({ "action": "identify", "path": file, "line": 2, "kind": kind, "name": name, "projectRoot": project.0 }),
        )
        .expect("answer")["test"]
            .clone()
    };
    assert_eq!(
        identify("test_program", "PLANT.pass_case")["key"],
        tests[0]["key"]
    );
    assert_eq!(identify("PROGRAM", "Plant.Pass_Case"), Value::Null);
}

#[test]
fn a_missing_path_keys_without_dot_segments() {
    // A file that does not exist (yet) keys by its absolute path with `.` and `..` resolved.
    let base = std::env::temp_dir().join(format!("trust-lsp-missing-{}", std::process::id()));
    let key = path_key(&base.join("a/../b/./c.st"));
    let expected = lexical_absolute(&base.join("b/c.st")).display().to_string();
    let expected = if cfg!(windows) {
        expected.to_lowercase()
    } else {
        expected
    };
    assert_eq!(key, expected);
}
