//! A changed project configuration takes effect at once (specification 14 §7.6, §7.2.1): when the
//! server indexes again after `trust-lsp.toml` changed, what the configuration no longer names
//! leaves the project, and an open document stays while it is open.

use super::*;
use tower_lsp::lsp_types::{
    DidChangeWatchedFilesParams, FileChangeType, FileEvent, NumberOrString, Url,
};

const WITH_LIBRARY: &str =
    "[project]\ninclude_paths = [\"src\"]\n\n[[libraries]]\nname = \"vendor\"\npath = \"vendor\"\n";
const WITHOUT_LIBRARY: &str = "[project]\ninclude_paths = [\"src\"]\n";
const MAIN: &str =
    "PROGRAM Main\nVAR\n    Valve : VendorValve;\nEND_VAR\nValve(Open := TRUE);\nEND_PROGRAM\n";
const VALVE: &str =
    "FUNCTION_BLOCK VendorValve\nVAR_INPUT\n    Open : BOOL;\nEND_VAR\nEND_FUNCTION_BLOCK\n";
/// Main.st's errors when no library declares `VendorValve`: the declaration's type and the call.
const UNRESOLVED: [&str; 2] = [
    "E102: cannot resolve type 'VendorValve'",
    "E103: 'Valve' is not callable",
];

/// A project folder whose program uses the block of its `vendor` library, indexed as the server
/// loads a workspace folder, with `src/Main.st` open; the state, the folder and Main.st's URI.
async fn opened_project(name: &str) -> (Arc<ServerState>, PathBuf, Url) {
    let root = temp_dir(name);
    for (path, text) in [
        ("trust-lsp.toml", WITH_LIBRARY),
        ("src/Main.st", MAIN),
        ("vendor/valve.st", VALVE),
    ] {
        let file = root.join(path);
        std::fs::create_dir_all(file.parent().expect("a folder")).expect("folders");
        std::fs::write(&file, text).expect("a file");
    }
    let state = Arc::new(ServerState::new());
    state.set_workspace_folders(vec![Url::from_directory_path(&root).expect("root uri")]);
    index_workspace(&test_client(), &state).await;
    state.mark_index_first_pass_done();
    let main = indexed(&state, "src/Main.st");
    state.open_document(main.uri.clone(), 1, main.content.clone());
    (state, root, main.uri)
}

/// The indexed document whose path ends with `suffix`.
fn indexed(state: &ServerState, suffix: &str) -> crate::state::Document {
    state
        .documents()
        .into_iter()
        .find(|doc| doc.uri.path().ends_with(suffix))
        .unwrap_or_else(|| panic!("no indexed document ends with {suffix}"))
}

/// The errors of the document at `uri` as `code: message`, sorted.
fn errors(state: &ServerState, uri: &Url) -> Vec<String> {
    let doc = state.get_document(uri).expect("the document");
    let mut lines: Vec<String> = super::diagnostics::collect_diagnostics_with_ticket(
        state,
        &doc.uri,
        &doc.content,
        doc.file_id,
        None,
    )
    .into_iter()
    .filter_map(|diagnostic| match diagnostic.code {
        Some(NumberOrString::String(code)) if code.starts_with('E') => {
            Some(format!("{code}: {}", diagnostic.message))
        }
        _ => None,
    })
    .collect();
    lines.sort();
    lines
}

/// Writes `config` to the project's `trust-lsp.toml` and reports the change as VS Code's file
/// watcher does; the server then indexes again in the background.
async fn change_config(state: &Arc<ServerState>, root: &std::path::Path, config: &str) {
    let file = root.join("trust-lsp.toml");
    std::fs::write(&file, config).expect("trust-lsp.toml");
    did_change_watched_files(
        &test_client(),
        state,
        DidChangeWatchedFilesParams {
            changes: vec![FileEvent {
                uri: Url::from_file_path(&file).expect("trust-lsp.toml uri"),
                typ: FileChangeType::CHANGED,
            }],
        },
    )
    .await;
}

/// Waits, while the background indexing runs, until Main.st's errors are `expected`.
async fn wait_for_errors(state: &ServerState, uri: &Url, expected: &[&str]) {
    let deadline = std::time::Instant::now() + std::time::Duration::from_secs(10);
    loop {
        let now = errors(state, uri);
        if now == expected {
            return;
        }
        assert!(
            std::time::Instant::now() < deadline,
            "Main.st's errors stayed {now:?}, not {expected:?}"
        );
        tokio::time::sleep(std::time::Duration::from_millis(20)).await;
    }
}

/// Catches a removed `[[libraries]]` entry whose files keep their declarations after the
/// configuration changed (G331: until the server restarted, so a name only the library declared
/// stayed resolved), an edit of one of its files that the watcher brings back, and a restored entry
/// whose files do not come back.
#[tokio::test]
async fn a_library_removed_from_the_configuration_leaves_the_project_and_returns_with_it() {
    let (state, root, uri) = opened_project("trust-lsp-config-change-library").await;
    assert_eq!(errors(&state, &uri), Vec::<String>::new());

    change_config(&state, &root, WITHOUT_LIBRARY).await;
    wait_for_errors(&state, &uri, &UNRESOLVED).await;

    // An edit of the removed library's file, which the watcher reports, keeps it out (Codex's
    // review of #843): no project folder holds it any more.
    let valve = root.join("vendor").join("valve.st");
    std::fs::write(&valve, format!("{VALVE}\n")).expect("valve.st");
    did_change_watched_files(
        &test_client(),
        &state,
        DidChangeWatchedFilesParams {
            changes: vec![FileEvent {
                uri: Url::from_file_path(&valve).expect("valve.st uri"),
                typ: FileChangeType::CHANGED,
            }],
        },
    )
    .await;
    assert_eq!(
        errors(&state, &uri),
        UNRESOLVED,
        "the edited file stays out"
    );

    change_config(&state, &root, WITH_LIBRARY).await;
    wait_for_errors(&state, &uri, &[]).await;
}

/// Catches an open document dropped by the configuration change: it stays while it is open, as a
/// file opened from another folder does (specification 14 §7.2.1), and leaves when it closes.
#[tokio::test]
async fn an_open_document_of_a_removed_library_stays_while_it_is_open() {
    let (state, root, uri) = opened_project("trust-lsp-config-change-open-library").await;
    let valve = indexed(&state, "vendor/valve.st");
    state.open_document(valve.uri.clone(), 1, valve.content.clone());

    std::fs::write(root.join("trust-lsp.toml"), WITHOUT_LIBRARY).expect("trust-lsp.toml");
    index_workspace(&test_client(), &state).await;
    assert_eq!(errors(&state, &uri), Vec::<String>::new(), "open, it stays");

    state.close_document(&valve.uri);
    assert_eq!(errors(&state, &uri), UNRESOLVED, "closed, it leaves");
}

/// Specification 14 §7.6: a file that the watcher reports created is indexed when a project folder
/// holds it, here the `vendor` library folder, and not when none does, here a `scratch` folder
/// beside `src` that the configuration does not name, as the workspace indexing leaves it out.
/// Catches a watcher that indexes every reported file (the edited removed library came back that
/// way), and one that indexes none.
#[tokio::test]
async fn the_watcher_indexes_a_created_file_only_where_a_project_folder_holds_it() {
    let (state, root, _) = opened_project("trust-lsp-config-change-watcher").await;
    let held = root.join("vendor").join("pump.st");
    let loose = root.join("scratch").join("other.st");
    for file in [&held, &loose] {
        std::fs::create_dir_all(file.parent().expect("a folder")).expect("folders");
        std::fs::write(file, "FUNCTION_BLOCK VendorPump\nEND_FUNCTION_BLOCK\n").expect("a file");
    }
    did_change_watched_files(
        &test_client(),
        &state,
        DidChangeWatchedFilesParams {
            changes: [&held, &loose]
                .into_iter()
                .map(|file| FileEvent {
                    uri: Url::from_file_path(file).expect("a file uri"),
                    typ: FileChangeType::CREATED,
                })
                .collect(),
        },
    )
    .await;
    let indexed_paths: Vec<String> = state
        .documents()
        .into_iter()
        .map(|doc| doc.uri.path().to_string())
        .collect();
    assert!(
        indexed_paths
            .iter()
            .any(|path| path.ends_with("vendor/pump.st")),
        "the library's new file is indexed: {indexed_paths:?}"
    );
    assert!(
        !indexed_paths
            .iter()
            .any(|path| path.ends_with("scratch/other.st")),
        "a file no project folder holds is not: {indexed_paths:?}"
    );
}
