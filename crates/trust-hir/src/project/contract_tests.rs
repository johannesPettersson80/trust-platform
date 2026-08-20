use super::*;

use crate::db::{SemanticDatabase, SourceDatabase};
use rustc_hash::FxHashMap;

#[test]
fn ensured_key_has_one_stable_bidirectional_identity() {
    let mut registry = SourceRegistry::new();
    let key = SourceKey::from_virtual("main.st");

    let first = registry.ensure_file_id(key.clone());
    let second = registry.ensure_file_id(key.clone());

    assert_eq!(first, FileId(0));
    assert_eq!(second, first);
    assert_eq!(registry.file_id_for_key(&key), Some(first));
    assert_eq!(registry.key_for_file_id(first), Some(&key));
}

#[test]
fn distinct_automatic_identities_are_monotonic() {
    let mut registry = SourceRegistry::new();

    let first = registry.ensure_file_id(SourceKey::from_virtual("first.st"));
    let second = registry.ensure_file_id(SourceKey::from_virtual("second.st"));
    let third = registry.ensure_file_id(SourceKey::from_virtual("third.st"));

    assert_eq!((first, second, third), (FileId(0), FileId(1), FileId(2)));
}

#[test]
fn explicit_identity_advances_automatic_allocation_floor() {
    let mut registry = SourceRegistry::new();

    assert_eq!(
        registry.insert_with_id(SourceKey::from_virtual("fixed.st"), FileId(41)),
        FileId(41)
    );
    assert_eq!(
        registry.ensure_file_id(SourceKey::from_virtual("automatic.st")),
        FileId(42)
    );
}

#[test]
fn explicit_identity_below_current_floor_does_not_move_floor_backward() {
    let mut registry = SourceRegistry::new();
    assert_eq!(
        registry.insert_with_id(SourceKey::from_virtual("high.st"), FileId(20)),
        FileId(20)
    );
    assert_eq!(
        registry.insert_with_id(SourceKey::from_virtual("low.st"), FileId(3)),
        FileId(3)
    );

    assert_eq!(
        registry.ensure_file_id(SourceKey::from_virtual("next.st")),
        FileId(21)
    );
}

#[test]
fn removing_source_clears_both_lookup_directions_without_recycling_id() {
    let mut registry = SourceRegistry::new();
    let removed_key = SourceKey::from_virtual("removed.st");
    let removed_id = registry.ensure_file_id(removed_key.clone());

    assert_eq!(registry.remove(&removed_key), Some(removed_id));
    assert_eq!(registry.file_id_for_key(&removed_key), None);
    assert_eq!(registry.key_for_file_id(removed_id), None);
    assert_eq!(
        registry.ensure_file_id(SourceKey::from_virtual("replacement.st")),
        FileId(1)
    );
}

#[test]
fn removing_absent_key_does_not_advance_or_disturb_registry() {
    let mut registry = SourceRegistry::new();
    let live_key = SourceKey::from_virtual("live.st");
    let live_id = registry.ensure_file_id(live_key.clone());

    assert_eq!(
        registry.remove(&SourceKey::from_virtual("missing.st")),
        None
    );
    assert_eq!(registry.file_id_for_key(&live_key), Some(live_id));
    assert_eq!(
        registry.ensure_file_id(SourceKey::from_virtual("next.st")),
        FileId(1)
    );
}

#[test]
fn clear_removes_every_mapping_and_resets_empty_project_allocation() {
    let mut registry = SourceRegistry::new();
    let first_key = SourceKey::from_virtual("first.st");
    let second_key = SourceKey::from_virtual("second.st");
    let first_id = registry.ensure_file_id(first_key.clone());
    let second_id = registry.ensure_file_id(second_key.clone());

    registry.clear();

    assert_eq!(registry.file_id_for_key(&first_key), None);
    assert_eq!(registry.file_id_for_key(&second_key), None);
    assert_eq!(registry.key_for_file_id(first_id), None);
    assert_eq!(registry.key_for_file_id(second_id), None);
    assert_eq!(registry.iter().count(), 0);
    assert_eq!(
        registry.ensure_file_id(SourceKey::from_virtual("fresh.st")),
        FileId(0)
    );
}

#[test]
fn registry_iteration_contains_exactly_live_bijection() {
    let mut registry = SourceRegistry::new();
    let first = SourceKey::from_virtual("first.st");
    let second = SourceKey::from_virtual("second.st");
    let removed = SourceKey::from_virtual("removed.st");
    let first_id = registry.ensure_file_id(first.clone());
    let second_id = registry.ensure_file_id(second.clone());
    registry.ensure_file_id(removed.clone());
    registry.remove(&removed);

    let entries = registry
        .iter()
        .map(|(key, id)| (key.clone(), id))
        .collect::<FxHashMap<_, _>>();

    assert_eq!(entries.len(), 2);
    assert_eq!(entries.get(&first), Some(&first_id));
    assert_eq!(entries.get(&second), Some(&second_id));
}

#[test]
fn project_updates_same_key_without_changing_file_identity() {
    let mut project = Project::new();
    let key = SourceKey::from_virtual("main.st");
    let first = project.set_source_text(key.clone(), "PROGRAM First\nEND_PROGRAM\n".to_owned());
    let second = project.set_source_text(key.clone(), "PROGRAM Second\nEND_PROGRAM\n".to_owned());

    assert_eq!(first, second);
    assert_eq!(project.file_id_for_key(&key), Some(first));
    assert_eq!(project.key_for_file_id(first), Some(&key));
    assert_eq!(
        project.database().source_text(first).as_str(),
        "PROGRAM Second\nEND_PROGRAM\n"
    );
    assert!(project
        .database()
        .file_symbols(first)
        .lookup("Second")
        .is_some());
    assert!(project
        .database()
        .file_symbols(first)
        .lookup("First")
        .is_none());
}

#[test]
fn project_remove_source_clears_registry_and_database_products() {
    let mut project = Project::new();
    let key = SourceKey::from_virtual("main.st");
    let file = project.set_source_text(
        key.clone(),
        "PROGRAM Main\nVAR value : INT; END_VAR\nEND_PROGRAM\n".to_owned(),
    );
    assert!(project
        .database()
        .file_symbols(file)
        .lookup("Main")
        .is_some());

    assert_eq!(project.remove_source(&key), Some(file));

    assert_eq!(project.file_id_for_key(&key), None);
    assert_eq!(project.key_for_file_id(file), None);
    assert_eq!(project.database().source_text(file).as_str(), "");
    assert!(project
        .database()
        .file_symbols(file)
        .lookup("Main")
        .is_none());
    assert!(project.database().diagnostics(file).is_empty());
}

#[test]
fn project_remove_absent_source_preserves_live_sources() {
    let mut project = Project::new();
    let live_key = SourceKey::from_virtual("live.st");
    let live_file =
        project.set_source_text(live_key.clone(), "PROGRAM Live\nEND_PROGRAM\n".to_owned());

    assert_eq!(
        project.remove_source(&SourceKey::from_virtual("missing.st")),
        None
    );
    assert_eq!(project.file_id_for_key(&live_key), Some(live_file));
    assert_eq!(
        project.database().source_text(live_file).as_str(),
        "PROGRAM Live\nEND_PROGRAM\n"
    );
}

#[test]
fn virtual_and_path_keys_retain_distinct_display_and_identity() {
    let virtual_key = SourceKey::from_virtual("same.st");
    let path_key = SourceKey::from_path("same.st");
    let mut registry = SourceRegistry::new();

    let virtual_id = registry.ensure_file_id(virtual_key.clone());
    let path_id = registry.ensure_file_id(path_key.clone());

    assert_ne!(virtual_key, path_key);
    assert_ne!(virtual_id, path_id);
    assert_eq!(virtual_key.display(), "same.st");
    assert!(!path_key.display().is_empty());
}

#[test]
fn fallback_normalization_removes_current_directory_but_retains_parent() {
    let input = PathBuf::from("root")
        .join(".")
        .join("child")
        .join("..")
        .join("file.st");
    let normalized = normalize_path_lossy_without_canonicalize(&input);

    assert_eq!(
        normalized,
        PathBuf::from("root")
            .join("child")
            .join("..")
            .join("file.st")
    );
}
