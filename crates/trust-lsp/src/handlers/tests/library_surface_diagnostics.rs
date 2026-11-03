//! The library-surface rule through the editor's diagnostics (specification 14 §7.6,
//! specification 28 §1): for each file, the deepest folder that holds it among the project's own
//! source folders and its library folders decides. A library file never receives W001, W002 or
//! W009 — the editor judges a library in its own project, and the application is only one of its
//! users — while every other diagnostic stays on.

use super::*;
use tower_lsp::lsp_types::{DiagnosticSeverity, NumberOrString, Url};

/// A library's unused declarations; the vendor binary would own the bodies. Assigning a
/// function's own name is part of its result semantics, not a use (specification 28 §1). Outside
/// the exemption it carries W001 (`limit`), W002 (`a`, `b`) and W009 (both POUs).
const VENDOR: &str = r#"
FUNCTION VendConcat : STRING
VAR_INPUT
    a : STRING;
    b : STRING;
END_VAR
VAR CONSTANT limit : INT := 10;
END_VAR
VendConcat := '';
END_FUNCTION

FUNCTION_BLOCK VendBuffer
VAR_INPUT
    size : INT;
END_VAR
END_FUNCTION_BLOCK
"#;

/// The same kinds of declaration in the project's own source, where they do warn: W001 (`cap`),
/// W002 (`value`) and W009 (all three POUs). The unused constant lives in the program body: a
/// function block's members are their own exemption (specification 28 §1).
const MAIN: &str = r#"
PROGRAM Main
VAR CONSTANT cap : INT := 100;
END_VAR
END_PROGRAM

FUNCTION_BLOCK ProjTank
VAR_INPUT
    level : INT;
END_VAR
END_FUNCTION_BLOCK

FUNCTION ProjScale : INT
VAR_INPUT
    value : INT;
END_VAR
ProjScale := 0;
END_FUNCTION
"#;

const UNUSED_CODES: [&str; 3] = ["W001", "W002", "W009"];

/// Writes `files` under a fresh folder and indexes the project folder `project` below it (the
/// fresh folder itself when empty) the way the language server loads a workspace folder. Returns
/// the state, which answers diagnostics for every indexed file, and the fresh folder.
async fn indexed(name: &str, project: &str, files: &[(&str, &str)]) -> (ServerState, PathBuf) {
    let base = temp_dir(name);
    for (path, text) in files {
        let file = base.join(path);
        std::fs::create_dir_all(file.parent().expect("a folder")).expect("folders");
        std::fs::write(&file, text).expect("write source");
    }
    let root = if project.is_empty() {
        base.clone()
    } else {
        base.join(project)
    };
    let state = ServerState::new();
    state.set_workspace_folders(vec![Url::from_directory_path(&root).expect("root uri")]);
    index_workspace(&test_client(), &state).await;
    state.mark_index_first_pass_done();
    (state, base)
}

/// The diagnostic codes of every indexed document named `file_name`, one list per document: a
/// file the indexer reached under two spellings (through a link, or through a canonicalized
/// dependency path) is two documents, and each of them must follow the rule.
fn codes_per_document(state: &ServerState, file_name: &str) -> Vec<(Url, Vec<String>)> {
    let found: Vec<(Url, Vec<String>)> = state
        .documents()
        .into_iter()
        .filter(|doc| {
            doc.uri
                .path_segments()
                .and_then(|mut segments| segments.next_back())
                .is_some_and(|last| last == file_name)
        })
        .map(|doc| {
            let codes = super::diagnostics::collect_diagnostics_with_ticket(
                state,
                &doc.uri,
                &doc.content,
                doc.file_id,
                None,
            )
            .into_iter()
            .filter_map(|diagnostic| match diagnostic.code {
                Some(NumberOrString::String(code)) => Some(code),
                _ => None,
            })
            .collect();
            (doc.uri, codes)
        })
        .collect();
    assert!(!found.is_empty(), "no indexed document named {file_name}");
    found
}

/// Every document named `file_name` is library surface: none of W001, W002 and W009.
fn assert_spared(state: &ServerState, file_name: &str) {
    for (uri, codes) in codes_per_document(state, file_name) {
        for code in UNUSED_CODES {
            assert!(
                !codes.iter().any(|found| found == code),
                "library file {uri} must not carry {code}: {codes:?}"
            );
        }
    }
}

/// Every document named `file_name` is the project's own: W001, W002 and W009 all reported.
fn assert_warned(state: &ServerState, file_name: &str) {
    for (uri, codes) in codes_per_document(state, file_name) {
        for code in UNUSED_CODES {
            assert!(
                codes.iter().any(|found| found == code),
                "project file {uri} must keep {code}: {codes:?}"
            );
        }
    }
}

/// The diagnostics of the project's configuration file `config`, which the editor holds as a
/// document of its own.
fn config_diagnostics(
    state: &ServerState,
    root: &std::path::Path,
    config: &str,
) -> Vec<tower_lsp::lsp_types::Diagnostic> {
    let uri = Url::from_file_path(root.join("trust-lsp.toml")).expect("config uri");
    state.open_document(uri.clone(), 1, config.to_string());
    let document = state.get_document(&uri).expect("config document");
    super::diagnostics::collect_diagnostics_with_ticket(
        state,
        &uri,
        &document.content,
        document.file_id,
        None,
    )
}

/// `include_paths = ["src"]` with a `[[libraries]]` folder `libs/vendor` beside it: no own
/// folder holds the library's file, so it is library surface, and `src/main.st` is the
/// project's own. Catches an exemption that never reaches the editor's diagnostics, and one that
/// silences the project's own file.
#[tokio::test]
async fn a_library_inside_the_project_folder_is_spared_unused_warnings() {
    let (state, base) = indexed(
        "trustlsp-lib-surface-inside",
        "",
        &[
            (
                "trust-lsp.toml",
                r#"
[project]
include_paths = ["src"]

[[libraries]]
name = "Vendor"
path = "libs/vendor"
"#,
            ),
            ("libs/vendor/vendor.st", VENDOR),
            ("src/main.st", MAIN),
        ],
    )
    .await;

    assert_spared(&state, "vendor.st");
    assert_warned(&state, "main.st");
    std::fs::remove_dir_all(base).ok();
}

/// Without `include_paths` the root is the own source folder and the indexer walks it, library
/// folder included; `libs/vendor` is two components deeper than the root, so its file is library
/// surface. Catches a rule that depends on how the file entered the index.
#[tokio::test]
async fn a_library_is_spared_unused_warnings_without_include_paths() {
    let (state, base) = indexed(
        "trustlsp-lib-surface-root-walk",
        "",
        &[
            (
                "trust-lsp.toml",
                "[[libraries]]\nname = \"Vendor\"\npath = \"libs/vendor\"\n",
            ),
            ("libs/vendor/vendor.st", VENDOR),
            ("src/main.st", MAIN),
        ],
    )
    .await;

    assert_spared(&state, "vendor.st");
    assert_warned(&state, "main.st");
    std::fs::remove_dir_all(base).ok();
}

/// A `[dependencies]` package is a library folder like a `[[libraries]]` entry. Its folder is
/// stored canonicalized, so where the temporary folder is reached through a link (macOS) the
/// indexer finds the file under two spellings, and both are library surface. Catches a
/// dependency left out of the library folders, and a dependency matched by its spelling.
#[tokio::test]
async fn a_resolved_dependency_package_is_library_surface() {
    let (state, base) = indexed(
        "trustlsp-lib-surface-dependency",
        "",
        &[
            (
                "trust-lsp.toml",
                "[dependencies]\nVendor = \"libs/vendor\"\n",
            ),
            ("libs/vendor/vendor.st", VENDOR),
            ("src/main.st", MAIN),
        ],
    )
    .await;

    assert!(
        state
            .workspace_config_for_uri(&Url::from_directory_path(&base).expect("root uri"))
            .is_some_and(|config| config.libraries.iter().any(|lib| lib.name == "Vendor")),
        "the dependency must resolve to a library"
    );
    assert_spared(&state, "vendor.st");
    assert_warned(&state, "main.st");
    std::fs::remove_dir_all(base).ok();
}

/// `[[libraries]] path = "."` names the project folder itself, as deep as the project's own
/// folder, which wins: every file keeps its warnings, and the entry is valid configuration with
/// no library code and no error. Catches the former refusal with an `L008` error, and a rule
/// where a library entry at the root silences the whole project.
#[tokio::test]
async fn a_library_folder_that_is_the_project_root_exempts_nothing() {
    let config = "[[libraries]]\nname = \"Whole\"\npath = \".\"\n";
    let (state, base) = indexed(
        "trustlsp-lib-surface-root",
        "",
        &[
            ("trust-lsp.toml", config),
            ("libs/vendor/vendor.st", VENDOR),
            ("src/main.st", MAIN),
        ],
    )
    .await;

    assert_warned(&state, "main.st");
    assert_warned(&state, "vendor.st");
    let diagnostics = config_diagnostics(&state, &base, config);
    assert!(
        diagnostics.iter().all(|diagnostic| {
            diagnostic.severity != Some(DiagnosticSeverity::ERROR)
                && !matches!(&diagnostic.code, Some(NumberOrString::String(code)) if code.starts_with('L'))
        }),
        "the configuration must carry no library code and no error: {diagnostics:?}"
    );
    std::fs::remove_dir_all(base).ok();
}

/// `include_paths = ["src"]` with a library at `src/vendor`: the library folder is one component
/// deeper than the include path, so only `src/vendor` is library surface. Catches a rule where
/// any own source folder that holds a file makes it the project's own.
#[tokio::test]
async fn a_library_inside_an_include_path_is_library_surface() {
    let (state, base) = indexed(
        "trustlsp-lib-surface-in-include",
        "",
        &[
            (
                "trust-lsp.toml",
                r#"
[project]
include_paths = ["src"]

[[libraries]]
name = "Vendor"
path = "src/vendor"
"#,
            ),
            ("src/vendor/vendor.st", VENDOR),
            ("src/main.st", MAIN),
        ],
    )
    .await;

    assert_spared(&state, "vendor.st");
    assert_warned(&state, "main.st");
    std::fs::remove_dir_all(base).ok();
}

/// A library at the include path `src` is exactly as deep as the project's own folder, which
/// wins, so it exempts nothing, not even `src/vendor`. Catches a rule where any library folder
/// that holds a file makes it library surface, which silenced the whole application.
#[tokio::test]
async fn a_library_at_an_include_path_exempts_nothing() {
    let (state, base) = indexed(
        "trustlsp-lib-surface-at-include",
        "",
        &[
            (
                "trust-lsp.toml",
                r#"
[project]
include_paths = ["src"]

[[libraries]]
name = "Sources"
path = "src"
"#,
            ),
            ("src/vendor/vendor.st", VENDOR),
            ("src/main.st", MAIN),
        ],
    )
    .await;

    assert_warned(&state, "vendor.st");
    assert_warned(&state, "main.st");
    std::fs::remove_dir_all(base).ok();
}

/// The project reached through a different spelling of its own folder — a symlink on unix, where
/// `/tmp` and macOS `/var/folders` are symlinks and a canonicalized `[dependencies]` folder spells
/// its files differently than the folder the client opened.
#[tokio::test]
async fn a_project_opened_through_a_symlink_keeps_the_dependency_rule() {
    #[cfg(not(unix))]
    {
        // Symlinked project folders are a unix shape; the canonical-form identity itself is
        // pinned by the canonicalized-dependency tests on every platform.
        eprintln!("a project through a symlink is pinned on unix");
        return;
    }

    #[cfg(unix)]
    {
        let (state, root) = indexed(
            "trustlsp-lib-surface-symlink",
            "",
            &[
                (
                    "trust-lsp.toml",
                    "[dependencies]\nVendor = \"libs/vendor\"\n",
                ),
                ("libs/vendor/vendor.st", VENDOR),
                ("src/main.st", MAIN),
            ],
        )
        .await;
        let link = root.parent().expect("temp parent").join(format!(
            "{}-link",
            root.file_name().expect("temp name").to_string_lossy()
        ));
        std::os::unix::fs::symlink(&root, &link).expect("symlink the project folder");

        state.set_workspace_folders(vec![Url::from_directory_path(&link).expect("link uri")]);
        index_workspace(&test_client(), &state).await;
        state.mark_index_first_pass_done();

        assert_spared(&state, "vendor.st");
        assert_warned(&state, "main.st");
        std::fs::remove_file(&link).ok();
        std::fs::remove_dir_all(root).ok();
    }
}
