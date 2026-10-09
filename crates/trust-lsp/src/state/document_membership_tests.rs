use super::state_contract_tests::TempTree;
use super::*;
use std::sync::{Arc, Barrier};
use trust_hir::SourceDatabase;

const MAIN: &str = "PROGRAM Main\nEND_PROGRAM\n";

#[test]
fn deletion_removes_a_source_after_its_document_text_was_evicted() {
    let root = TempTree::new("trust-lsp-source-membership");
    std::fs::write(
        root.path().join("trust-lsp.toml"),
        "[indexing]\nmemory_budget_mb = 1\nevict_to_percent = 75\n",
    )
    .unwrap();
    let state = ServerState::new();
    state.set_workspace_config(
        Url::from_file_path(root.path()).unwrap(),
        ProjectConfig::load(root.path()),
    );
    let first = root.path().join("Old.st");
    let second = root.path().join("Keep.st");
    let payload = format!("{MAIN}(*{}*)", " ".repeat(600_000));
    std::fs::write(&first, &payload).unwrap();
    std::fs::write(&second, payload.replace("Main", "Keep")).unwrap();
    let first_uri = Url::from_file_path(&first).unwrap();
    let first_id = state
        .index_document(first_uri.clone(), payload.clone())
        .unwrap();
    state.index_document(
        Url::from_file_path(second).unwrap(),
        payload.replace("Main", "Keep"),
    );
    assert!(
        state.get_document(&first_uri).is_none(),
        "closed text was evicted"
    );
    assert!(
        state.uri_for_file_id(first_id).is_some(),
        "semantic source remains"
    );
    std::fs::remove_file(first).unwrap();
    assert_eq!(state.remove_document(&first_uri), Some(first_id));
    assert!(state.uri_for_file_id(first_id).is_none());
}

#[test]
fn delayed_disk_indexing_cannot_restore_a_deleted_source_or_overwrite_an_editor() {
    let root = TempTree::new("trust-lsp-source-membership");
    let path = root.path().join("Main.st");
    std::fs::write(&path, MAIN).unwrap();
    let uri = Url::from_file_path(&path).unwrap();
    let state = ServerState::new();
    state.index_document(uri.clone(), MAIN.to_string());
    let cached = std::fs::read_to_string(&path).unwrap();
    std::fs::remove_file(&path).unwrap();
    state.remove_document(&uri);
    assert_eq!(
        state.index_document_deferred_budget(uri.clone(), cached.clone()),
        None
    );
    assert!(state.get_document(&uri).is_none());
    assert_eq!(state.project.read().sources().iter().count(), 0);

    let editor = "PROGRAM Edited\nEND_PROGRAM\n";
    let id = state.open_document(uri.clone(), 2, editor.to_string());
    assert_eq!(
        state.index_document_deferred_budget(uri.clone(), cached),
        None
    );
    assert_eq!(state.get_document(&uri).unwrap().content, editor);
    state.with_database(|database| assert_eq!(database.source_text(id).as_str(), editor));
}

#[test]
fn concurrent_close_and_removal_leave_no_orphan_semantic_source() {
    let root = TempTree::new("trust-lsp-source-membership");
    let path = root.path().join("Main.st");
    std::fs::write(&path, MAIN).unwrap();
    let uri = Url::from_file_path(path).unwrap();
    for _ in 0..64 {
        let state = Arc::new(ServerState::new());
        state.open_document(uri.clone(), 1, MAIN.to_string());
        let start = Arc::new(Barrier::new(3));
        std::thread::scope(|threads| {
            let close_state = Arc::clone(&state);
            let close_start = Arc::clone(&start);
            let close_uri = uri.clone();
            threads.spawn(move || {
                close_start.wait();
                close_state.close_document(&close_uri);
            });
            let remove_state = Arc::clone(&state);
            let remove_start = Arc::clone(&start);
            let remove_uri = uri.clone();
            threads.spawn(move || {
                remove_start.wait();
                remove_state.remove_document(&remove_uri);
            });
            start.wait();
        });
        assert!(state.get_document(&uri).is_none());
        assert_eq!(state.project.read().sources().iter().count(), 0);
    }
}

#[cfg(unix)]
#[test]
fn directory_deletion_keeps_identity_through_a_surviving_symlinked_parent() {
    let root = TempTree::new("trust-lsp-deleted-alias");
    let real = root.path().join("real");
    let alias = root.path().join("alias");
    std::fs::create_dir_all(real.join("project")).unwrap();
    std::os::unix::fs::symlink(&real, &alias).unwrap();
    let path = alias.join("project/Main.st");
    std::fs::write(&path, MAIN).unwrap();
    let uri = Url::from_file_path(&path).unwrap();
    let state = ServerState::new();
    let id = state.index_document(uri.clone(), MAIN.to_string()).unwrap();
    std::fs::remove_dir_all(real.join("project")).unwrap();
    let deleted = Url::from_file_path(alias.join("project")).unwrap();
    assert_eq!(state.remove_document_tree(&deleted), 1);
    assert!(state.get_document(&uri).is_none());
    assert!(state.uri_for_file_id(id).is_none());
}
