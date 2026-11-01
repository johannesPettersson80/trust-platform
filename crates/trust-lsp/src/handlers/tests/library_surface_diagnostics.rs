//! The library-surface rule for the unused-symbol warnings (specifications 14 §7.6, 28 §1): a file
//! inside a `[[libraries]]` folder or a resolved `[dependencies]` package never receives W001,
//! W002 or W009 — the editor judges a library in its own project, and the application is only one
//! of its users — while every other diagnostic stays on, and a library folder that swallows the
//! project root is refused with `L008` and exempts nothing.

use super::*;

/// A library's unused declarations; the vendor binary would own the bodies. Assigning a
/// function's own name is part of its result semantics, not a use (specification 28 §1).
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

/// The same kinds of declaration in the project's own source, where they do warn. The unused
/// constant lives in the program body: a function block's members are their own exemption
/// (specification 28 §1).
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

/// Writes a project and indexes it the way the language server loads a workspace folder; the
/// returned state answers diagnostics for every indexed file.
async fn indexed_project(name: &str, config: &str) -> (ServerState, PathBuf) {
    let root = temp_dir(name);
    for (path, text) in [
        ("trust-lsp.toml", config),
        ("libs/vendor/vendor.st", VENDOR),
        ("src/main.st", MAIN),
    ] {
        let file = root.join(path);
        std::fs::create_dir_all(file.parent().expect("a folder")).expect("folders");
        std::fs::write(&file, text).expect("write source");
    }

    let client = test_client();
    let state = ServerState::new();
    state.set_workspace_folders(vec![
        tower_lsp::lsp_types::Url::from_directory_path(&root).expect("root uri")
    ]);
    index_workspace(&client, &state).await;
    state.mark_index_first_pass_done();
    (state, root)
}

/// The diagnostic codes the server reports for one indexed file, by file name.
fn codes_for_file(state: &ServerState, file_name: &str) -> Vec<String> {
    let mut codes = Vec::new();
    let mut found = false;
    for doc in state.documents() {
        if !doc
            .uri
            .path_segments()
            .map(|mut segments| segments.any(|segment| segment == file_name))
            .unwrap_or(false)
        {
            continue;
        }
        found = true;
        for diagnostic in super::diagnostics::collect_diagnostics_with_ticket(
            state,
            &doc.uri,
            &doc.content,
            doc.file_id,
            None,
        ) {
            if let Some(tower_lsp::lsp_types::NumberOrString::String(code)) = &diagnostic.code {
                codes.push(code.clone());
            }
        }
    }
    assert!(found, "no indexed document for {file_name}");
    codes
}

#[tokio::test]
async fn a_library_inside_the_project_folder_is_spared_unused_warnings() {
    let (state, root) = indexed_project(
        "trustlsp-lib-surface-inside",
        r#"
[project]
include_paths = ["src"]

[[libraries]]
name = "Vendor"
path = "libs/vendor"
"#,
    )
    .await;

    let vendor = codes_for_file(&state, "vendor.st");
    for code in UNUSED_CODES {
        assert!(
            !vendor.contains(&code.to_string()),
            "library file must not carry {code}: {vendor:?}"
        );
    }

    let main = codes_for_file(&state, "main.st");
    for code in UNUSED_CODES {
        assert!(
            main.contains(&code.to_string()),
            "project file must keep {code}: {main:?}"
        );
    }

    std::fs::remove_dir_all(root).ok();
}

#[tokio::test]
async fn a_library_is_spared_unused_warnings_without_include_paths() {
    // Without `include_paths` the indexer walks the project root, which contains the library
    // folder; the rule must not depend on how the files entered the index.
    let (state, root) = indexed_project(
        "trustlsp-lib-surface-root-walk",
        r#"
[[libraries]]
name = "Vendor"
path = "libs/vendor"
"#,
    )
    .await;

    let vendor = codes_for_file(&state, "vendor.st");
    for code in UNUSED_CODES {
        assert!(
            !vendor.contains(&code.to_string()),
            "library file must not carry {code}: {vendor:?}"
        );
    }

    let main = codes_for_file(&state, "main.st");
    for code in UNUSED_CODES {
        assert!(
            main.contains(&code.to_string()),
            "project file must keep {code}: {main:?}"
        );
    }

    std::fs::remove_dir_all(root).ok();
}

#[tokio::test]
async fn a_resolved_dependency_package_is_library_surface() {
    let (state, root) = indexed_project(
        "trustlsp-lib-surface-dependency",
        r#"
[dependencies]
Vendor = "libs/vendor"
"#,
    )
    .await;

    assert!(
        state
            .workspace_config_for_uri(
                &tower_lsp::lsp_types::Url::from_directory_path(&root).expect("root uri")
            )
            .is_some_and(|config| config.libraries.iter().any(|lib| lib.name == "Vendor")),
        "the dependency must resolve to a library"
    );

    let vendor = codes_for_file(&state, "vendor.st");
    for code in UNUSED_CODES {
        assert!(
            !vendor.contains(&code.to_string()),
            "dependency file must not carry {code}: {vendor:?}"
        );
    }

    let main = codes_for_file(&state, "main.st");
    for code in UNUSED_CODES {
        assert!(
            main.contains(&code.to_string()),
            "project file must keep {code}: {main:?}"
        );
    }

    std::fs::remove_dir_all(root).ok();
}

#[tokio::test]
async fn a_library_folder_that_is_the_project_root_exempts_nothing_and_reports_l008() {
    let (state, root) = indexed_project(
        "trustlsp-lib-surface-root",
        r#"
[[libraries]]
name = "Whole"
path = "."
"#,
    )
    .await;

    // The refused entry shields nothing: every file of the project keeps its warnings.
    let main = codes_for_file(&state, "main.st");
    for code in UNUSED_CODES {
        assert!(
            main.contains(&code.to_string()),
            "project file must keep {code}: {main:?}"
        );
    }
    let vendor = codes_for_file(&state, "vendor.st");
    for code in UNUSED_CODES {
        assert!(
            vendor.contains(&code.to_string()),
            "library file must keep {code} when the entry is refused: {vendor:?}"
        );
    }

    // The configuration diagnostic anchors on the config file, which the editor holds as a
    // document of its own.
    let config_uri =
        tower_lsp::lsp_types::Url::from_file_path(root.join("trust-lsp.toml")).expect("config uri");
    state.open_document(
        config_uri.clone(),
        1,
        "[[libraries]]\nname = \"Whole\"\npath = \".\"\n".to_string(),
    );
    let config_doc = state.get_document(&config_uri).expect("config document");
    let diagnostics = super::diagnostics::collect_diagnostics_with_ticket(
        &state,
        &config_uri,
        &config_doc.content,
        config_doc.file_id,
        None,
    );
    assert!(diagnostics.iter().any(|diagnostic| {
        matches!(
            &diagnostic.code,
            Some(tower_lsp::lsp_types::NumberOrString::String(code)) if code == "L008"
        )
    }));

    std::fs::remove_dir_all(root).ok();
}
