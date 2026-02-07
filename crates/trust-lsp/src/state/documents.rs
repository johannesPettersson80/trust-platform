use rustc_hash::FxHashSet;
use tower_lsp::lsp_types::Url;

use crate::config::ProjectConfig;
use trust_hir::{db::FileId, SourceKey};

use super::path::{canonicalize_path, path_to_uri, source_key_for_uri, uri_to_path};
use super::{Document, ServerState};

pub(super) fn open_document(
    state: &ServerState,
    uri: Url,
    version: i32,
    content: String,
) -> FileId {
    let key = source_key_for_uri(&uri);
    let file_id = {
        let mut project = state.project.write();
        project.set_source_text(key, content.clone())
    };

    let access = next_document_access(state);
    let mut docs = state.documents.write();
    if let Some(doc) = docs.get_mut(&uri) {
        doc.version = version;
        doc.content = content;
        doc.is_open = true;
        doc.file_id = file_id;
        touch_document(doc, access);
    } else {
        let doc = Document::new(uri.clone(), version, content, file_id, true, access);
        docs.insert(uri, doc);
    }

    file_id
}

pub(super) fn index_document(state: &ServerState, uri: Url, content: String) -> Option<FileId> {
    if let Some(doc) = state.documents.read().get(&uri) {
        if !doc.is_open && doc.content == content {
            return None;
        }
    }
    if state
        .documents
        .read()
        .get(&uri)
        .map(|doc| doc.is_open)
        .unwrap_or(false)
    {
        return None;
    }

    let key = source_key_for_uri(&uri);
    let file_id = {
        let mut project = state.project.write();
        project.set_source_text(key.clone(), content.clone())
    };

    let access = next_document_access(state);
    {
        let mut docs = state.documents.write();
        if let Some(doc) = docs.get_mut(&uri) {
            if doc.is_open {
                return None;
            }
            doc.version = 0;
            doc.content = content;
            doc.is_open = false;
            doc.file_id = file_id;
            touch_document(doc, access);
        } else {
            let doc = Document::new(uri.clone(), 0, content, file_id, false, access);
            docs.insert(uri, doc);
        }
    }

    enforce_memory_budget(state);
    Some(file_id)
}

pub(super) fn update_document(state: &ServerState, uri: &Url, version: i32, content: String) {
    let key = source_key_for_uri(uri);
    let file_id = {
        let mut project = state.project.write();
        project.set_source_text(key, content.clone())
    };

    let access = next_document_access(state);
    let mut docs = state.documents.write();
    if let Some(doc) = docs.get_mut(uri) {
        doc.version = version;
        doc.content = content;
        doc.is_open = true;
        doc.file_id = file_id;
        touch_document(doc, access);
    }
}

pub(super) fn close_document(state: &ServerState, uri: &Url) {
    if let Some(doc) = state.documents.write().get_mut(uri) {
        doc.is_open = false;
    }
    enforce_memory_budget(state);
}

pub(super) fn remove_document(state: &ServerState, uri: &Url) -> Option<FileId> {
    let key = source_key_for_uri(uri);
    let doc = state.documents.write().remove(uri)?;
    state.semantic_tokens.write().remove(uri);
    state.diagnostics.write().remove(uri);
    let mut project = state.project.write();
    project.remove_source(&key);
    Some(doc.file_id)
}

pub(super) fn rename_document(state: &ServerState, old_uri: &Url, new_uri: &Url) -> Option<FileId> {
    let mut docs = state.documents.write();
    let mut doc = docs.remove(old_uri)?;
    state.semantic_tokens.write().remove(old_uri);
    state.diagnostics.write().remove(old_uri);

    let old_key = source_key_for_uri(old_uri);
    let new_key = source_key_for_uri(new_uri);
    let mut project = state.project.write();
    project.remove_source(&old_key);
    project.remove_source(&new_key);
    let file_id = project.set_source_text(new_key, doc.content.clone());

    doc.uri = new_uri.clone();
    doc.file_id = file_id;
    docs.insert(new_uri.clone(), doc);
    Some(file_id)
}

pub(super) fn get_document(state: &ServerState, uri: &Url) -> Option<Document> {
    state.documents.read().get(uri).cloned()
}

pub(super) fn documents(state: &ServerState) -> Vec<Document> {
    state.documents.read().values().cloned().collect()
}

pub(super) fn ensure_document(state: &ServerState, uri: &Url) -> Option<Document> {
    if let Some(doc) = get_document(state, uri) {
        return Some(doc);
    }
    let path = uri_to_path(uri)?;
    let content = std::fs::read_to_string(&path).ok()?;
    index_document(state, uri.clone(), content);
    get_document(state, uri)
}

pub(super) fn uri_for_file_id(state: &ServerState, file_id: FileId) -> Option<Url> {
    if let Some(doc) = document_for_file_id(state, file_id) {
        return Some(doc.uri);
    }
    let project = state.project.read();
    let key = project.key_for_file_id(file_id)?;
    match key {
        SourceKey::Path(path) => path_to_uri(path),
        SourceKey::Virtual(name) => Url::parse(name).ok(),
    }
}

pub(super) fn document_for_file_id(state: &ServerState, file_id: FileId) -> Option<Document> {
    state
        .documents
        .read()
        .values()
        .find(|doc| doc.file_id == file_id)
        .cloned()
}

pub(super) fn file_ids_for_config(
    state: &ServerState,
    config: &ProjectConfig,
) -> FxHashSet<FileId> {
    let roots = config
        .indexing_roots()
        .into_iter()
        .map(canonicalize_path)
        .collect::<Vec<_>>();
    let project = state.project.read();
    let mut ids = FxHashSet::default();
    for (key, file_id) in project.sources().iter() {
        let SourceKey::Path(path) = key else {
            continue;
        };
        if roots.iter().any(|root| path.starts_with(root)) {
            ids.insert(file_id);
        }
    }
    ids
}

pub(super) fn apply_memory_budget(state: &ServerState) {
    enforce_memory_budget(state);
}

fn next_document_access(state: &ServerState) -> u64 {
    state
        .doc_access_counter
        .fetch_add(1, std::sync::atomic::Ordering::Relaxed)
}

fn touch_document(doc: &mut Document, access: u64) {
    doc.last_access = access;
    doc.content_bytes = doc.content.len();
}

fn enforce_memory_budget(state: &ServerState) {
    let Some(config) = state.primary_workspace_config() else {
        return;
    };
    let Some(budget_mb) = config.indexing.memory_budget_mb else {
        return;
    };
    let budget_bytes = budget_mb.saturating_mul(1024 * 1024);
    if budget_bytes == 0 {
        return;
    }
    let evict_target = {
        let percent = config.indexing.evict_to_percent.clamp(1, 100) as usize;
        budget_bytes.saturating_mul(percent) / 100
    };

    let mut total_bytes = 0usize;
    let mut candidates = Vec::new();
    {
        let docs = state.documents.read();
        for (uri, doc) in docs.iter() {
            if doc.is_open {
                continue;
            }
            total_bytes = total_bytes.saturating_add(doc.content_bytes);
            candidates.push((doc.last_access, uri.clone(), doc.content_bytes));
        }
    }
    if total_bytes <= budget_bytes {
        return;
    }

    candidates.sort_by_key(|(access, _, _)| *access);
    let mut remaining = total_bytes;
    let mut to_evict = Vec::new();
    for (_, uri, size) in candidates {
        if remaining <= evict_target {
            break;
        }
        to_evict.push(uri);
        remaining = remaining.saturating_sub(size);
    }
    for uri in to_evict {
        let _ = remove_document(state, &uri);
    }
}
