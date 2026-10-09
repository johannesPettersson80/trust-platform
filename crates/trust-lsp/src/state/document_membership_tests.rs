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

fn indexed_evicted_file(
    root: &std::path::Path,
    first: &std::path::Path,
) -> (ServerState, Url, FileId) {
    std::fs::write(
        root.join("trust-lsp.toml"),
        "[indexing]\nmemory_budget_mb = 1\nevict_to_percent = 75\n",
    )
    .unwrap();
    let state = ServerState::new();
    state.set_workspace_config(
        Url::from_file_path(root).unwrap(),
        ProjectConfig::load(root),
    );
    let payload = format!("{MAIN}(*{}*)", " ".repeat(600_000));
    std::fs::write(first, &payload).unwrap();
    let uri = Url::from_file_path(first).unwrap();
    let id = state.index_document(uri.clone(), payload.clone()).unwrap();
    let keep = root.join("Keep.st");
    let keep_text = payload.replace("Main", "Keep");
    std::fs::write(&keep, &keep_text).unwrap();
    state
        .index_document(Url::from_file_path(keep).unwrap(), keep_text)
        .unwrap();
    assert!(
        state.get_document(&uri).is_none(),
        "text must actually be evicted"
    );
    assert!(
        state.uri_for_file_id(id).is_some(),
        "identity remains registered"
    );
    (state, uri, id)
}

#[test]
fn rename_preserves_an_evicted_sources_identity_without_reloading_its_text() {
    let root = TempTree::new("trust-lsp-evicted-rename");
    let old = root.path().join("Old.st");
    let new = root.path().join("New.st");
    let (state, old_uri, old_id) = indexed_evicted_file(root.path(), &old);
    std::fs::rename(old, &new).unwrap();
    let new_uri = Url::from_file_path(new).unwrap();
    let new_id = state.rename_document(&old_uri, &new_uri).unwrap();
    assert!(state.uri_for_file_id(old_id).is_none());
    assert!(
        state.get_document(&new_uri).is_none(),
        "rename does not reload evicted text"
    );
    assert_eq!(state.ensure_document(&new_uri).unwrap().file_id, new_id);
    state.with_database(|db| assert!(db.source_text(new_id).contains("PROGRAM Main")));
}

#[cfg(unix)]
#[test]
fn evicted_file_deletion_through_a_symlink_parent_removes_the_registered_source() {
    let root = TempTree::new("trust-lsp-evicted-alias-file");
    let real = root.path().join("real");
    let alias = root.path().join("alias");
    std::fs::create_dir(&real).unwrap();
    std::os::unix::fs::symlink(&real, &alias).unwrap();
    let file = alias.join("Main.st");
    let (state, uri, id) = indexed_evicted_file(root.path(), &file);
    std::fs::remove_file(file).unwrap();
    assert_eq!(state.remove_document(&uri), Some(id));
    assert!(state.uri_for_file_id(id).is_none());
    assert_eq!(state.project.read().sources().iter().count(), 1);
}

#[cfg(unix)]
#[test]
fn deleted_symlink_directory_removes_evicted_membership_but_preserves_siblings() {
    let root = TempTree::new("trust-lsp-evicted-alias-directory");
    let real = root.path().join("real");
    let alias = root.path().join("alias");
    std::fs::create_dir(&real).unwrap();
    std::os::unix::fs::symlink(&real, &alias).unwrap();
    let (state, _, id) = indexed_evicted_file(root.path(), &alias.join("Main.st"));
    std::fs::remove_file(&alias).unwrap();
    assert!(
        real.join("Main.st").is_file(),
        "the target itself still exists"
    );
    assert_eq!(
        state.remove_document_tree(&Url::from_file_path(alias).unwrap()),
        1
    );
    assert!(state.uri_for_file_id(id).is_none());
    assert_eq!(state.project.read().sources().iter().count(), 1);
}

#[cfg(unix)]
#[test]
fn reindex_reconciles_a_missing_alias_after_its_text_was_evicted() {
    let root = TempTree::new("trust-lsp-evicted-alias-reindex");
    let real = root.path().join("real");
    let alias = root.path().join("alias");
    std::fs::create_dir(&real).unwrap();
    std::os::unix::fs::symlink(&real, &alias).unwrap();
    let (state, _, id) = indexed_evicted_file(root.path(), &alias.join("Main.st"));
    std::fs::remove_file(alias).unwrap();
    assert_eq!(
        state.reconcile_missing_documents(&ProjectConfig::load(
            &root.path().canonicalize().unwrap()
        )),
        1
    );
    assert!(state.uri_for_file_id(id).is_none());
    assert_eq!(state.project.read().sources().iter().count(), 1);
}

#[cfg(unix)]
#[test]
fn retargeting_an_evicted_alias_does_not_change_the_source_deleted_by_its_notification() {
    let root = TempTree::new("trust-lsp-evicted-alias-retarget");
    let original = root.path().join("original");
    let replacement = root.path().join("replacement");
    let alias = root.path().join("alias");
    std::fs::create_dir(&original).unwrap();
    std::fs::create_dir(&replacement).unwrap();
    std::os::unix::fs::symlink(&original, &alias).unwrap();
    let (state, uri, old_id) = indexed_evicted_file(root.path(), &alias.join("Main.st"));
    let replacement_file = replacement.join("Main.st");
    let replacement_text = "PROGRAM Replacement\nEND_PROGRAM\n";
    std::fs::write(&replacement_file, replacement_text).unwrap();
    let replacement_id = state
        .index_document(
            Url::from_file_path(replacement_file).unwrap(),
            replacement_text.to_string(),
        )
        .unwrap();
    std::fs::remove_file(&alias).unwrap();
    std::os::unix::fs::symlink(replacement, alias).unwrap();
    assert_eq!(state.remove_document(&uri), Some(old_id));
    assert!(state.uri_for_file_id(old_id).is_none());
    assert!(state.uri_for_file_id(replacement_id).is_some());
    assert_eq!(state.project.read().sources().iter().count(), 2);
}

#[cfg(unix)]
#[test]
fn retargeting_an_evicted_directory_alias_preserves_the_replacement_sources() {
    let root = TempTree::new("trust-lsp-evicted-alias-retarget");
    let original = root.path().join("original");
    let replacement = root.path().join("replacement");
    let alias = root.path().join("alias");
    std::fs::create_dir(&original).unwrap();
    std::fs::create_dir(&replacement).unwrap();
    std::os::unix::fs::symlink(&original, &alias).unwrap();
    let (state, _, old_id) = indexed_evicted_file(root.path(), &alias.join("Main.st"));
    let replacement_file = replacement.join("Main.st");
    let replacement_text = "PROGRAM Replacement\nEND_PROGRAM\n";
    std::fs::write(&replacement_file, replacement_text).unwrap();
    let replacement_id = state
        .index_document(
            Url::from_file_path(replacement_file).unwrap(),
            replacement_text.to_string(),
        )
        .unwrap();
    let unsaved_uri = Url::from_file_path(replacement.join("Unsaved.st")).unwrap();
    let unsaved_id = state.open_document(
        unsaved_uri.clone(),
        1,
        "PROGRAM Unsaved\nEND_PROGRAM\n".to_string(),
    );
    std::fs::remove_file(&alias).unwrap();
    std::os::unix::fs::symlink(replacement, &alias).unwrap();
    assert_eq!(
        state.remove_document_tree(&Url::from_file_path(alias).unwrap()),
        1
    );
    assert!(state.uri_for_file_id(old_id).is_none());
    assert!(state.uri_for_file_id(replacement_id).is_some());
    assert!(state.uri_for_file_id(unsaved_id).is_some());
    assert!(state.get_document(&unsaved_uri).unwrap().is_open);
    assert_eq!(state.project.read().sources().iter().count(), 3);
}

#[cfg(unix)]
#[test]
fn deleting_a_project_with_an_interior_file_symlink_preserves_external_siblings() {
    let root = TempTree::new("trust-lsp-interior-alias-deletion");
    let project = root.path().join("project");
    let external = root.path().join("external");
    std::fs::create_dir(&project).unwrap();
    std::fs::create_dir(&external).unwrap();
    let target = external.join("Main.st");
    std::fs::write(&target, MAIN).unwrap();
    let alias = project.join("Alias.st");
    std::os::unix::fs::symlink(target, &alias).unwrap();
    let (state, _, old_id) = indexed_evicted_file(root.path(), &alias);
    let sibling = external.join("Sibling.st");
    std::fs::write(&sibling, "PROGRAM Sibling\nEND_PROGRAM\n").unwrap();
    let sibling_id = state
        .index_document(
            Url::from_file_path(&sibling).unwrap(),
            std::fs::read_to_string(&sibling).unwrap(),
        )
        .unwrap();
    std::fs::remove_dir_all(&project).unwrap();
    assert_eq!(
        state.remove_document_tree(&Url::from_file_path(project).unwrap()),
        1
    );
    assert!(state.uri_for_file_id(old_id).is_none());
    assert!(state.uri_for_file_id(sibling_id).is_some());
    assert_eq!(state.project.read().sources().iter().count(), 2);
}

#[cfg(unix)]
#[test]
fn physical_directory_deletion_removes_sources_registered_through_other_aliases() {
    let root = TempTree::new("trust-lsp-physical-directory-deletion");
    let project = root.path().join("project");
    let alias = root.path().join("alias");
    std::fs::create_dir(&project).unwrap();
    std::os::unix::fs::symlink(&project, &alias).unwrap();
    let (state, _, old_id) = indexed_evicted_file(root.path(), &alias.join("Main.st"));
    let direct = project.join("Direct.st");
    std::fs::write(&direct, "PROGRAM Direct\nEND_PROGRAM\n").unwrap();
    let direct_id = state
        .index_document(
            Url::from_file_path(&direct).unwrap(),
            std::fs::read_to_string(direct).unwrap(),
        )
        .unwrap();
    std::fs::remove_dir_all(&project).unwrap();
    assert_eq!(
        state.remove_document_tree(&Url::from_file_path(project).unwrap()),
        2
    );
    assert!(state.uri_for_file_id(old_id).is_none());
    assert!(state.uri_for_file_id(direct_id).is_none());
    assert_eq!(state.project.read().sources().iter().count(), 1);
}
