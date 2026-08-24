use super::*;

use std::fs;
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicU64, Ordering as AtomicOrdering};

use serde_json::json;
use tower_lsp::lsp_types::{CallHierarchyItem, Position, Range, SymbolKind};
use trust_hir::SourceDatabase;

static TEMP_SEQUENCE: AtomicU64 = AtomicU64::new(1);

#[test]
fn state_contract_new_and_default_start_empty_with_closed_capabilities() {
    for state in [ServerState::new(), ServerState::default()] {
        assert!(state.documents().is_empty());
        assert!(state.workspace_folders().is_empty());
        assert!(state.workspace_configs().is_empty());
        assert_eq!(
            state.primary_workspace_config().map(|config| config.root),
            None
        );
        assert_eq!(state.config(), Value::Null);
        assert!(!state.work_done_progress());
        assert!(!state.diagnostic_refresh_supported());
        assert!(!state.diagnostic_pull_supported());
        assert!(!state.use_pull_diagnostics());
        assert!(!state.semantic_tokens_refresh_supported());
        assert_eq!(state.activity_age_ms(), u64::MAX);
        assert_eq!(state.document_generation(), 1);
        assert!(state.semantic_request_cancelled(0));
        assert!(!state.semantic_request_cancelled(1));
    }
}

#[test]
fn state_contract_document_constructor_counts_utf8_bytes() {
    let uri = Url::parse("untitled:unicode").unwrap();
    let content = "é🙂".to_string();
    let document = Document::new(uri.clone(), -7, content.clone(), FileId(9), true, 11);

    assert_eq!(document.uri, uri);
    assert_eq!(document.version, -7);
    assert_eq!(document.content, content);
    assert_eq!(document.file_id, FileId(9));
    assert!(document.is_open);
    assert_eq!(document.last_access, 11);
    assert_eq!(document.content_bytes, "é🙂".len());
}

#[test]
fn state_contract_workspace_folders_are_owned_snapshots() {
    let state = ServerState::new();
    let first = Url::parse("file:///workspace/one").unwrap();
    let second = Url::parse("file:///workspace/two").unwrap();

    state.set_workspace_folders(vec![first.clone(), second.clone()]);
    let mut snapshot = state.workspace_folders();
    snapshot.clear();

    assert_eq!(state.workspace_folders(), vec![first, second]);
    state.set_workspace_folders(Vec::new());
    assert!(state.workspace_folders().is_empty());
}

#[test]
fn state_contract_client_capabilities_toggle_independently() {
    let state = ServerState::new();

    state.set_work_done_progress(true);
    state.set_diagnostic_pull_supported(true);
    assert!(state.work_done_progress());
    assert!(state.diagnostic_pull_supported());
    assert!(!state.diagnostic_refresh_supported());
    assert!(!state.use_pull_diagnostics());
    assert!(!state.semantic_tokens_refresh_supported());

    state.set_diagnostic_refresh_supported(true);
    assert!(state.use_pull_diagnostics());
    state.set_semantic_tokens_refresh_supported(true);
    assert!(state.semantic_tokens_refresh_supported());

    state.set_diagnostic_pull_supported(false);
    assert!(!state.use_pull_diagnostics());
    assert!(state.diagnostic_refresh_supported());
    assert!(state.semantic_tokens_refresh_supported());
}

#[test]
fn state_contract_configuration_is_an_owned_snapshot() {
    let state = ServerState::new();
    state.set_config(json!({"analysis": {"strict": true}, "items": [1, 2]}));

    let mut snapshot = state.config();
    snapshot["analysis"]["strict"] = Value::Bool(false);
    snapshot["items"] = json!([]);

    assert_eq!(
        state.config(),
        json!({"analysis": {"strict": true}, "items": [1, 2]})
    );
    state.set_config(Value::Null);
    assert_eq!(state.config(), Value::Null);
}

#[test]
fn state_contract_primary_workspace_uses_highest_priority_and_root_replacement() {
    let first = TempTree::new("trust-lsp-state-primary-one");
    let second = TempTree::new("trust-lsp-state-primary-two");
    let state = ServerState::new();
    let first_uri = file_uri(first.path());
    let second_uri = file_uri(second.path());
    let mut first_config = ProjectConfig::load(first.path());
    first_config.workspace.priority = -2;
    let mut second_config = ProjectConfig::load(second.path());
    second_config.workspace.priority = 7;

    state.set_workspace_config(first_uri.clone(), first_config);
    state.set_workspace_config(second_uri.clone(), second_config);
    assert_eq!(
        state.primary_workspace_config().unwrap().root,
        second.path()
    );
    assert_eq!(state.workspace_configs().len(), 2);

    let mut replacement = ProjectConfig::load(first.path());
    replacement.workspace.priority = 12;
    state.set_workspace_config(first_uri, replacement);
    assert_eq!(state.workspace_configs().len(), 2);
    assert_eq!(state.primary_workspace_config().unwrap().root, first.path());
    assert!(state
        .workspace_configs()
        .iter()
        .any(|(root, config)| root == &second_uri && config.workspace.priority == 7));
}

#[test]
fn state_contract_workspace_uri_match_uses_deepest_registered_root() {
    let root = TempTree::new("trust-lsp-state-match");
    let nested = root.path().join("nested");
    let outside = TempTree::new("trust-lsp-state-outside");
    fs::create_dir_all(&nested).unwrap();
    let state = ServerState::new();
    let mut root_config = ProjectConfig::load(root.path());
    root_config.workspace.priority = 50;
    let mut nested_config = ProjectConfig::load(&nested);
    nested_config.workspace.priority = -50;
    state.set_workspace_config(file_uri(root.path()), root_config);
    state.set_workspace_config(file_uri(&nested), nested_config);

    let nested_file = file_uri(&nested.join("src/Main.st"));
    assert_eq!(
        state.workspace_config_for_uri(&nested_file).unwrap().root,
        nested
    );
    assert_eq!(
        state
            .workspace_config_for_uri(&file_uri(&root.path().join("Other.st")))
            .unwrap()
            .root,
        root.path()
    );
    assert!(state
        .workspace_config_for_uri(&file_uri(&outside.path().join("None.st")))
        .is_none());
    assert!(state
        .library_docs_for_uri(&file_uri(outside.path()))
        .is_none());
}

#[test]
fn state_contract_unknown_update_is_a_complete_no_op() {
    let state = ServerState::new();
    let uri = Url::parse("untitled:unknown-update").unwrap();
    let generation = state.document_generation();
    let source_count = state.project.read().sources().iter().count();

    state.update_document(&uri, 3, "PROGRAM Ghost\nEND_PROGRAM\n".to_string());

    assert!(state.get_document(&uri).is_none());
    assert_eq!(state.project.read().sources().iter().count(), source_count);
    assert_eq!(state.document_generation(), generation);
}

#[test]
fn state_contract_update_does_not_promote_a_closed_indexed_document() {
    let state = ServerState::new();
    let uri = Url::parse("untitled:closed-update").unwrap();
    let original = "PROGRAM Indexed\nEND_PROGRAM\n".to_string();
    let file_id = state
        .index_document(uri.clone(), original.clone())
        .expect("index closed document");
    let generation = state.document_generation();

    state.update_document(&uri, 9, "PROGRAM Unsolicited\nEND_PROGRAM\n".to_string());

    let document = state.get_document(&uri).expect("closed document remains");
    assert!(!document.is_open);
    assert_eq!(document.version, 0);
    assert_eq!(document.content, original);
    assert_eq!(state.document_generation(), generation);
    state.with_database(|database| {
        assert_eq!(
            database.source_text(file_id).as_str(),
            "PROGRAM Indexed\nEND_PROGRAM\n"
        );
    });
}

#[test]
fn state_contract_indexing_does_not_replace_identical_closed_or_open_content() {
    let state = ServerState::new();
    let uri = Url::parse("untitled:index-guard").unwrap();
    let disk_content = "PROGRAM Indexed\nEND_PROGRAM\n".to_string();
    let indexed_id = state
        .index_document(uri.clone(), disk_content.clone())
        .expect("first index");
    let indexed_generation = state.document_generation();

    assert_eq!(
        state.index_document(uri.clone(), disk_content),
        None,
        "identical closed content is a no-op"
    );
    assert_eq!(state.document_generation(), indexed_generation);

    let editor_content = "PROGRAM Editor\nEND_PROGRAM\n".to_string();
    let opened_id = state.open_document(uri.clone(), 5, editor_content.clone());
    assert_eq!(opened_id, indexed_id);
    let open_generation = state.document_generation();
    assert_eq!(
        state.index_document(uri.clone(), "PROGRAM StaleDisk\nEND_PROGRAM\n".to_string()),
        None
    );
    let document = state.get_document(&uri).unwrap();
    assert!(document.is_open);
    assert_eq!(document.version, 5);
    assert_eq!(document.content, editor_content);
    assert_eq!(state.document_generation(), open_generation);
}

#[test]
fn state_contract_unknown_close_and_remove_preserve_generation_and_ticket() {
    let state = ServerState::new();
    let uri = Url::parse("untitled:missing-lifecycle").unwrap();
    let generation = state.document_generation();
    let ticket = state.begin_semantic_request();

    state.close_document(&uri);
    assert!(!state.semantic_request_cancelled(ticket));
    assert_eq!(state.remove_document(&uri), None);
    assert_eq!(state.document_generation(), generation);
}

#[test]
fn state_contract_document_and_file_id_lookups_are_bidirectional() {
    let state = ServerState::new();
    let first_uri = Url::parse("untitled:first").unwrap();
    let second_uri = Url::parse("untitled:second").unwrap();
    let first_id = state.open_document(first_uri.clone(), 1, "PROGRAM A END_PROGRAM".into());
    let second_id = state.open_document(second_uri.clone(), 2, "PROGRAM B END_PROGRAM".into());

    assert_ne!(first_id, second_id);
    assert_eq!(state.uri_for_file_id(first_id), Some(first_uri.clone()));
    assert_eq!(state.uri_for_file_id(second_id), Some(second_uri.clone()));
    assert_eq!(state.document_for_file_id(first_id).unwrap().uri, first_uri);
    assert_eq!(
        state.document_for_file_id(second_id).unwrap().uri,
        second_uri
    );
    assert!(state.document_for_file_id(FileId(u32::MAX)).is_none());
}

#[test]
fn state_contract_file_ids_for_config_are_scoped_to_indexing_roots() {
    let root = TempTree::new("trust-lsp-state-config-ids");
    let outside = TempTree::new("trust-lsp-state-config-ids-outside");
    let state = ServerState::new();
    let inside_path = root.path().join("src/Inside.st");
    let outside_path = outside.path().join("Outside.st");
    fs::create_dir_all(inside_path.parent().expect("inside source directory"))
        .expect("create inside source directory");
    fs::write(&inside_path, "PROGRAM Inside END_PROGRAM").expect("write inside source");
    fs::write(&outside_path, "PROGRAM Outside END_PROGRAM").expect("write outside source");
    let inside_uri = file_uri(&inside_path);
    let outside_uri = file_uri(&outside_path);
    let virtual_uri = Url::parse("untitled:virtual").unwrap();
    let inside_id = state
        .index_document_deferred_budget(inside_uri, "PROGRAM Inside END_PROGRAM".to_string())
        .unwrap();
    state
        .index_document_deferred_budget(outside_uri, "PROGRAM Outside END_PROGRAM".to_string())
        .unwrap();
    state.open_document(virtual_uri, 1, "PROGRAM Virtual END_PROGRAM".to_string());

    let ids = state.file_ids_for_config(&ProjectConfig::load(root.path()));
    assert_eq!(ids.len(), 1);
    assert!(ids.contains(&inside_id));
}

#[test]
fn state_contract_ensure_document_rejects_non_file_and_missing_file_uris() {
    let state = ServerState::new();
    let root = TempTree::new("trust-lsp-state-ensure");
    let missing = file_uri(&root.path().join("missing.st"));

    assert!(state
        .ensure_document(&Url::parse("untitled:not-on-disk").unwrap())
        .is_none());
    assert!(state.ensure_document(&missing).is_none());
    assert!(state.documents().is_empty());
}

#[test]
fn state_contract_generation_clears_both_call_hierarchy_caches() {
    let state = ServerState::new();
    state.store_call_hierarchy_incoming("symbol".to_string(), vec![incoming_call()]);
    state.store_call_hierarchy_outgoing("symbol".to_string(), vec![outgoing_call()]);
    let generation = state.document_generation();
    assert_eq!(
        state
            .cached_call_hierarchy_incoming("symbol")
            .unwrap()
            .len(),
        1
    );
    assert_eq!(
        state
            .cached_call_hierarchy_outgoing("symbol")
            .unwrap()
            .len(),
        1
    );

    state.open_document(
        Url::parse("untitled:generation").unwrap(),
        1,
        "PROGRAM Main END_PROGRAM".to_string(),
    );

    assert_eq!(state.document_generation(), generation + 1);
    assert!(state.cached_call_hierarchy_incoming("symbol").is_none());
    assert!(state.cached_call_hierarchy_outgoing("symbol").is_none());
}

#[test]
fn state_contract_call_hierarchy_reads_are_detached_snapshots() {
    let state = ServerState::new();
    state.store_call_hierarchy_incoming("in".to_string(), vec![incoming_call()]);
    state.store_call_hierarchy_outgoing("out".to_string(), vec![outgoing_call()]);

    let mut incoming = state.cached_call_hierarchy_incoming("in").unwrap();
    let mut outgoing = state.cached_call_hierarchy_outgoing("out").unwrap();
    incoming[0].from_ranges.clear();
    outgoing[0].from_ranges.clear();

    assert_eq!(
        state.cached_call_hierarchy_incoming("in").unwrap()[0]
            .from_ranges
            .len(),
        1
    );
    assert_eq!(
        state.cached_call_hierarchy_outgoing("out").unwrap()[0]
            .from_ranges
            .len(),
        1
    );
    assert!(state.cached_call_hierarchy_incoming("missing").is_none());
    assert!(state.cached_call_hierarchy_outgoing("missing").is_none());
}

#[test]
fn state_contract_semantic_request_tickets_cancel_every_older_generation() {
    let state = ServerState::new();
    let first = state.begin_semantic_request();
    assert_eq!(first, 2);
    assert!(!state.semantic_request_cancelled(first));

    let second = state.begin_semantic_request();
    assert_eq!(second, 3);
    assert!(state.semantic_request_cancelled(first));
    assert!(!state.semantic_request_cancelled(second));

    state.cancel_semantic_requests();
    assert!(state.semantic_request_cancelled(first));
    assert!(state.semantic_request_cancelled(second));
}

#[test]
fn state_contract_semantic_token_cache_replaces_only_with_current_ticket() {
    let state = ServerState::new();
    let uri = Url::parse("untitled:semantic-cache").unwrap();
    let first_ticket = state.begin_semantic_request();
    let first_tokens = vec![semantic_token(1, 2, 3, 4, 5)];
    let first_id = state
        .store_semantic_tokens(uri.clone(), first_tokens.clone(), first_ticket)
        .unwrap();
    assert_eq!(first_id, "1");
    let mut detached = state.semantic_tokens_cache(&uri).unwrap();
    detached.tokens.clear();
    assert_eq!(
        state.semantic_tokens_cache(&uri).unwrap().tokens,
        first_tokens
    );

    let second_ticket = state.begin_semantic_request();
    assert!(state
        .store_semantic_tokens(
            uri.clone(),
            vec![semantic_token(9, 9, 9, 9, 9)],
            first_ticket
        )
        .is_none());
    assert_eq!(
        state.semantic_tokens_cache(&uri).unwrap().result_id,
        first_id
    );

    let second_tokens = vec![semantic_token(2, 3, 4, 5, 6)];
    let second_id = state
        .store_semantic_tokens(uri.clone(), second_tokens.clone(), second_ticket)
        .unwrap();
    assert_eq!(second_id, "2");
    assert_eq!(
        state.semantic_tokens_cache(&uri).unwrap().tokens,
        second_tokens
    );
}

#[test]
fn state_contract_diagnostic_cache_keys_identity_by_both_hashes() {
    let state = ServerState::new();
    let uri = Url::parse("untitled:diagnostics").unwrap();

    let first = state
        .store_diagnostics(uri.clone(), 10, 20, state.begin_semantic_request())
        .unwrap();
    let same = state
        .store_diagnostics(uri.clone(), 10, 20, state.begin_semantic_request())
        .unwrap();
    let content_changed = state
        .store_diagnostics(uri.clone(), 11, 20, state.begin_semantic_request())
        .unwrap();
    let diagnostics_changed = state
        .store_diagnostics(uri.clone(), 11, 21, state.begin_semantic_request())
        .unwrap();

    assert_eq!(first, same);
    assert_ne!(same, content_changed);
    assert_ne!(content_changed, diagnostics_changed);
    assert_eq!(
        state.diagnostic_result_id(&uri),
        Some(diagnostics_changed.clone())
    );

    let current = state.begin_semantic_request();
    assert!(state
        .store_diagnostics(uri.clone(), 99, 99, current - 1)
        .is_none());
    assert_eq!(state.diagnostic_result_id(&uri), Some(diagnostics_changed));
}

#[test]
fn state_contract_activity_transitions_from_never_recorded_to_elapsed_age() {
    let state = ServerState::new();
    assert_eq!(state.activity_age_ms(), u64::MAX);

    state.record_activity();

    assert!(state.activity_age_ms() < 5_000);
}

#[test]
fn state_contract_database_view_tracks_document_sources() {
    let state = ServerState::new();
    let uri = Url::parse("untitled:database-view").unwrap();
    let file_id = state.open_document(uri.clone(), 1, "PROGRAM Main\nEND_PROGRAM\n".to_string());

    state.with_database(|database| {
        assert_eq!(
            database.source_text(file_id).as_str(),
            "PROGRAM Main\nEND_PROGRAM\n"
        );
    });
    state.remove_document(&uri).unwrap();
    assert!(state.project.read().key_for_file_id(file_id).is_none());
}

fn semantic_token(
    delta_line: u32,
    delta_start: u32,
    length: u32,
    token_type: u32,
    token_modifiers_bitset: u32,
) -> SemanticToken {
    SemanticToken {
        delta_line,
        delta_start,
        length,
        token_type,
        token_modifiers_bitset,
    }
}

fn call_item(name: &str) -> CallHierarchyItem {
    CallHierarchyItem {
        name: name.to_string(),
        kind: SymbolKind::FUNCTION,
        tags: None,
        detail: Some("detail".to_string()),
        uri: Url::parse("file:///workspace/Main.st").unwrap(),
        range: range(),
        selection_range: range(),
        data: None,
    }
}

fn incoming_call() -> CallHierarchyIncomingCall {
    CallHierarchyIncomingCall {
        from: call_item("Caller"),
        from_ranges: vec![range()],
    }
}

fn outgoing_call() -> CallHierarchyOutgoingCall {
    CallHierarchyOutgoingCall {
        to: call_item("Callee"),
        from_ranges: vec![range()],
    }
}

fn range() -> Range {
    Range::new(Position::new(1, 2), Position::new(1, 5))
}

fn file_uri(path: &Path) -> Url {
    Url::from_file_path(path).expect("absolute path URI")
}

struct TempTree {
    path: PathBuf,
}

impl TempTree {
    fn new(prefix: &str) -> Self {
        let sequence = TEMP_SEQUENCE.fetch_add(1, AtomicOrdering::Relaxed);
        let path = std::env::temp_dir().join(format!("{prefix}-{}-{sequence}", std::process::id()));
        fs::create_dir_all(&path).expect("create temp tree");
        Self { path }
    }

    fn path(&self) -> &Path {
        &self.path
    }
}

impl Drop for TempTree {
    fn drop(&mut self) {
        let _ = fs::remove_dir_all(&self.path);
    }
}
