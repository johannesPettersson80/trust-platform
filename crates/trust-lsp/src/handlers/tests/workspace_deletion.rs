use super::*;
use tower_lsp::lsp_types::{DidChangeWatchedFilesParams, FileChangeType, FileEvent, Url};

const MAIN: &str = "PROGRAM Main\nEND_PROGRAM\n";

#[test]
fn deleting_a_project_directory_removes_its_imports_before_the_next_scaffold() {
    let root = temp_dir("trust-lsp-directory-deletion");
    let old = root.join("old-project");
    let sibling = root.join("old-project-other");
    std::fs::create_dir_all(&old).unwrap();
    std::fs::create_dir_all(&sibling).unwrap();
    let old_file = old.join("Main.st");
    let sibling_file = sibling.join("Keep.st");
    std::fs::write(&old_file, MAIN).unwrap();
    std::fs::write(&sibling_file, "PROGRAM Keep\nEND_PROGRAM\n").unwrap();
    let old_uri = Url::from_file_path(&old_file).unwrap();
    let sibling_uri = Url::from_file_path(&sibling_file).unwrap();
    let state = Arc::new(ServerState::new());
    let old_id = state.open_document(old_uri.clone(), 1, MAIN.to_string());
    state.index_document(
        sibling_uri.clone(),
        "PROGRAM Keep\nEND_PROGRAM\n".to_string(),
    );
    std::fs::remove_dir_all(&old).unwrap();

    let runtime = tokio::runtime::Builder::new_current_thread()
        .enable_all()
        .build()
        .unwrap();
    runtime.block_on(async {
        did_change_watched_files(
            &test_client(),
            &state,
            DidChangeWatchedFilesParams {
                changes: vec![FileEvent {
                    uri: Url::from_file_path(&old).unwrap(),
                    typ: FileChangeType::DELETED,
                }],
            },
        )
        .await;
    });

    assert!(state.get_document(&old_uri).is_none());
    assert!(state.uri_for_file_id(old_id).is_none());
    assert!(state.get_document(&sibling_uri).is_some());
    let next_uri = Url::from_file_path(root.join("new-project/Main.st")).unwrap();
    let next_id = state.open_document(next_uri, 1, MAIN.to_string());
    state.with_database(|database| {
        assert!(database
            .analyze(next_id)
            .diagnostics
            .iter()
            .all(|item| !item.is_error()));
    });
    std::fs::remove_dir_all(root).unwrap();
}

#[test]
fn reindex_removes_missing_sources_without_dropping_budget_skipped_live_files() {
    let root = temp_dir("trust-lsp-reindex-deletion");
    std::fs::write(root.join("trust-lsp.toml"), "[indexing]\nmax_files = 1\n").unwrap();
    let old_file = root.join("Old.st");
    let live_file = root.join("Live.st");
    std::fs::write(&old_file, MAIN).unwrap();
    std::fs::write(&live_file, "PROGRAM Live\nEND_PROGRAM\n").unwrap();
    let first_file = root.join("AAA.st");
    std::fs::write(&first_file, "PROGRAM First\nEND_PROGRAM\n").unwrap();
    let old_uri = Url::from_file_path(&old_file).unwrap();
    let live_uri = Url::from_file_path(&live_file).unwrap();
    let unsaved_uri = Url::from_file_path(root.join("Unsaved.st")).unwrap();
    let state = ServerState::new();
    state.set_workspace_folders(vec![Url::from_file_path(&root).unwrap()]);
    let old_id = state
        .index_document(old_uri.clone(), MAIN.to_string())
        .unwrap();
    state.index_document(live_uri.clone(), "PROGRAM Live\nEND_PROGRAM\n".to_string());
    state.open_document(
        unsaved_uri.clone(),
        1,
        "PROGRAM Unsaved\nEND_PROGRAM\n".to_string(),
    );
    std::fs::remove_file(old_file).unwrap();
    let next_uri = Url::from_file_path(root.join("New.st")).unwrap();
    let next_id = state.open_document(next_uri, 1, MAIN.to_string());

    let runtime = tokio::runtime::Builder::new_current_thread()
        .enable_all()
        .build()
        .unwrap();
    runtime.block_on(index_workspace(&test_client(), &state));

    assert!(state.get_document(&old_uri).is_none());
    assert!(state.uri_for_file_id(old_id).is_none());
    assert!(state
        .get_document(&Url::from_file_path(first_file).unwrap())
        .is_some());
    assert!(state.get_document(&live_uri).is_some());
    assert!(state.get_document(&unsaved_uri).unwrap().is_open);
    state.with_database(|database| {
        assert!(database
            .analyze(next_id)
            .diagnostics
            .iter()
            .all(|item| !item.is_error()));
    });
    std::fs::remove_dir_all(root).unwrap();
}
