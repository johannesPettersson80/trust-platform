use rustc_hash::{FxHashMap, FxHashSet};
use tower_lsp::lsp_types::Url;

use crate::config::ProjectConfig;
use trust_hir::{db::FileId, Project, SourceDatabase, SourceKey};

use super::path::{canonicalize_path, path_to_uri, source_key_for_uri, uri_to_path};
use super::{Document, ServerState};

/// Text is evictable; the URI-to-source registration survives until deletion.
pub(super) enum DocumentEntry {
    Resident(Document),
    Evicted(FileId),
}

impl DocumentEntry {
    fn file_id(&self) -> FileId {
        match self {
            Self::Resident(doc) => doc.file_id,
            Self::Evicted(id) => *id,
        }
    }

    fn resident(&self) -> Option<&Document> {
        match self {
            Self::Resident(doc) => Some(doc),
            Self::Evicted(_) => None,
        }
    }

    fn resident_mut(&mut self) -> Option<&mut Document> {
        match self {
            Self::Resident(doc) => Some(doc),
            Self::Evicted(_) => None,
        }
    }
}

pub(super) fn open_document(
    state: &ServerState,
    uri: Url,
    version: i32,
    content: String,
) -> FileId {
    // Membership transitions always lock Project before documents.
    let mut project = state.project.write();
    let mut docs = state.documents.write();
    let key = docs
        .get(&uri)
        .and_then(|entry| project.key_for_file_id(entry.file_id()).cloned())
        .unwrap_or_else(|| source_key_for_uri(&uri));
    let file_id = project.set_source_text(key, content.clone());
    let access = next_document_access(state);
    docs.insert(
        uri.clone(),
        DocumentEntry::Resident(Document::new(uri, version, content, file_id, true, access)),
    );

    drop(docs);
    drop(project);
    invalidate_project_caches(state);
    file_id
}

pub(super) fn index_document(state: &ServerState, uri: Url, content: String) -> Option<FileId> {
    index_document_impl(state, uri, content, true)
}

pub(super) fn index_document_deferred_budget(
    state: &ServerState,
    uri: Url,
    content: String,
) -> Option<FileId> {
    index_document_impl(state, uri, content, false)
}

fn index_document_impl(
    state: &ServerState,
    uri: Url,
    content: String,
    enforce_budget_after_index: bool,
) -> Option<FileId> {
    let mut project = state.project.write();
    let mut docs = state.documents.write();
    if docs
        .get(&uri)
        .and_then(DocumentEntry::resident)
        .is_some_and(|doc| doc.is_open || doc.content == content)
    {
        return None;
    }
    // The content may have been read before a delete notification acquired these
    // locks. Do not resurrect a disk source after that deletion has committed.
    if uri_to_path(&uri).is_some_and(|path| path_is_missing(&path)) {
        return None;
    }
    let key = docs
        .get(&uri)
        .and_then(|entry| project.key_for_file_id(entry.file_id()).cloned())
        .unwrap_or_else(|| source_key_for_uri(&uri));
    let file_id = project.set_source_text(key, content.clone());
    let access = next_document_access(state);
    docs.insert(
        uri.clone(),
        DocumentEntry::Resident(Document::new(uri, 0, content, file_id, false, access)),
    );
    drop(docs);
    drop(project);
    if enforce_budget_after_index {
        enforce_memory_budget(state);
    }
    invalidate_project_caches(state);
    Some(file_id)
}

pub(super) fn update_document(state: &ServerState, uri: &Url, version: i32, content: String) {
    let mut project = state.project.write();
    let mut docs = state.documents.write();
    let Some(doc) = docs
        .get_mut(uri)
        .and_then(DocumentEntry::resident_mut)
        .filter(|doc| doc.is_open)
    else {
        return;
    };
    let key = project
        .key_for_file_id(doc.file_id)
        .cloned()
        .unwrap_or_else(|| source_key_for_uri(uri));
    let file_id = project.set_source_text(key, content.clone());
    let access = next_document_access(state);
    doc.version = version;
    doc.content = content;
    doc.file_id = file_id;
    touch_document(doc, access);
    drop(docs);
    drop(project);
    invalidate_project_caches(state);
}

pub(super) fn close_document(state: &ServerState, uri: &Url) {
    let mut project = state.project.write();
    let mut docs = state.documents.write();
    let Some(doc) = docs.get_mut(uri).and_then(DocumentEntry::resident_mut) else {
        return;
    };
    state.cancel_semantic_requests();
    let key = project
        .key_for_file_id(doc.file_id)
        .cloned()
        .unwrap_or_else(|| source_key_for_uri(uri));
    // Read and commit while holding the membership locks, so deletion cannot
    // interleave between restoring disk text and closing the document entry.
    if let Some(content) = uri_to_path(uri).and_then(|path| std::fs::read_to_string(path).ok()) {
        doc.file_id = project.set_source_text(key, content.clone());
        doc.version = 0;
        doc.content = content;
        doc.is_open = false;
        touch_document(doc, next_document_access(state));
    } else {
        docs.remove(uri);
        project.remove_source(&key);
    }
    drop(docs);
    drop(project);
    state.semantic_tokens.write().remove(uri);
    state.diagnostics.write().remove(uri);
    invalidate_project_caches(state);
    enforce_memory_budget(state);
}

pub(super) fn remove_document(state: &ServerState, uri: &Url) -> Option<FileId> {
    let mut project = state.project.write();
    let mut docs = state.documents.write();
    let key = docs
        .get(uri)
        .and_then(|entry| project.key_for_file_id(entry.file_id()).cloned())
        .unwrap_or_else(|| source_key_for_uri(uri));
    // Eviction can remove the document text while retaining the semantic source.
    let file_id = project.remove_source(&key)?;
    let uris = docs
        .iter()
        .filter(|(_, entry)| entry.file_id() == file_id)
        .map(|(uri, _)| uri.clone())
        .collect::<Vec<_>>();
    docs.retain(|_, entry| entry.file_id() != file_id);
    drop(docs);
    drop(project);
    state.cancel_semantic_requests();
    for uri in uris {
        state.semantic_tokens.write().remove(&uri);
        state.diagnostics.write().remove(&uri);
    }
    invalidate_project_caches(state);
    Some(file_id)
}

pub(super) fn rename_document(state: &ServerState, old_uri: &Url, new_uri: &Url) -> Option<FileId> {
    let mut project = state.project.write();
    let mut docs = state.documents.write();
    let entry = docs.remove(old_uri)?;
    let old_id = entry.file_id();
    let old_key = project
        .key_for_file_id(old_id)
        .cloned()
        .unwrap_or_else(|| source_key_for_uri(old_uri));
    let content = project.database().source_text(old_id).to_string();
    let new_key = source_key_for_uri(new_uri);
    project.remove_source(&old_key);
    project.remove_source(&new_key);
    let file_id = project.set_source_text(new_key, content);
    let entry = match entry {
        DocumentEntry::Resident(mut doc) => {
            doc.uri = new_uri.clone();
            doc.file_id = file_id;
            DocumentEntry::Resident(doc)
        }
        DocumentEntry::Evicted(_) => DocumentEntry::Evicted(file_id),
    };
    docs.insert(new_uri.clone(), entry);
    drop(docs);
    drop(project);
    state.semantic_tokens.write().remove(old_uri);
    state.diagnostics.write().remove(old_uri);
    invalidate_project_caches(state);
    Some(file_id)
}

fn path_is_missing(path: &std::path::Path) -> bool {
    std::fs::metadata(path).is_err_and(|error| {
        matches!(
            error.kind(),
            std::io::ErrorKind::NotFound | std::io::ErrorKind::NotADirectory
        )
    })
}

pub(super) fn remove_document_tree(state: &ServerState, uri: &Url) -> usize {
    let Some(path) = uri_to_path(uri) else {
        return usize::from(remove_document(state, uri).is_some());
    };
    // Registered URI membership remains authoritative if an alias itself vanished.
    let canonical = path
        .ancestors()
        .find_map(|ancestor| {
            ancestor.canonicalize().ok().and_then(|_| {
                path.strip_prefix(ancestor)
                    .ok()
                    .map(|tail| canonicalize_path(ancestor.to_path_buf()).join(tail))
            })
        })
        .unwrap_or_else(|| path.clone());
    let deleted_path_is_missing = path_is_missing(&path);
    remove_sources(state, |project, docs| {
        let mut ids = docs
            .iter()
            .filter(|(uri, _)| uri_to_path(uri).is_some_and(|source| source.starts_with(&path)))
            .map(|(_, entry)| entry.file_id())
            .collect::<FxHashSet<_>>();
        // A live replacement alias must not redirect the old deletion event to
        // unrelated sources, including open unsaved files under its new target.
        if deleted_path_is_missing {
            ids.extend(project.sources().iter().filter_map(|(key, id)| {
                matches!(key, SourceKey::Path(source) if source.starts_with(&canonical))
                    .then_some(id)
            }));
        }
        ids
    })
}

pub(super) fn reconcile_missing_documents(state: &ServerState, config: &ProjectConfig) -> usize {
    let registered_roots = config.indexing_roots();
    let canonical_roots = registered_roots
        .iter()
        .cloned()
        .map(canonicalize_path)
        .collect::<Vec<_>>();
    remove_sources(state, |project, docs| {
        let open_ids = docs
            .values()
            .filter_map(DocumentEntry::resident)
            .filter(|doc| doc.is_open)
            .map(|doc| doc.file_id)
            .collect::<FxHashSet<_>>();
        let mut ids = docs.iter().filter_map(|(uri, entry)| {
            let path = uri_to_path(uri)?;
            let in_scope = registered_roots.iter().any(|root| path.starts_with(root))
                || matches!(project.key_for_file_id(entry.file_id()), Some(SourceKey::Path(source))
                    if canonical_roots.iter().any(|root| source.starts_with(root)));
            (in_scope && path_is_missing(&path)).then_some(entry.file_id())
        }).collect::<FxHashSet<_>>();
        ids.extend(project.sources().iter().filter_map(|(key, id)| {
            let SourceKey::Path(path) = key else {
                return None;
            };
            (canonical_roots.iter().any(|root| path.starts_with(root)) && path_is_missing(path))
                .then_some(id)
        }));
        ids.retain(|id| !open_ids.contains(id));
        ids
    })
}

fn remove_sources(
    state: &ServerState,
    select: impl FnOnce(&Project, &FxHashMap<Url, DocumentEntry>) -> FxHashSet<FileId>,
) -> usize {
    let mut project = state.project.write();
    let mut docs = state.documents.write();
    let ids = select(&project, &docs);
    let removed = project
        .sources()
        .iter()
        .filter(|(_, file_id)| ids.contains(file_id))
        .map(|(key, file_id)| (key.clone(), file_id))
        .collect::<Vec<_>>();
    if removed.is_empty() {
        return 0;
    }
    let uris = docs
        .iter()
        .filter(|(_, entry)| ids.contains(&entry.file_id()))
        .map(|(uri, _)| uri.clone())
        .collect::<Vec<_>>();
    docs.retain(|_, entry| !ids.contains(&entry.file_id()));
    for (key, _) in &removed {
        project.remove_source(key);
    }
    drop(docs);
    drop(project);
    state.cancel_semantic_requests();
    for uri in uris {
        state.semantic_tokens.write().remove(&uri);
        state.diagnostics.write().remove(&uri);
    }
    invalidate_project_caches(state);
    removed.len()
}

pub(super) fn get_document(state: &ServerState, uri: &Url) -> Option<Document> {
    state
        .documents
        .read()
        .get(uri)
        .and_then(DocumentEntry::resident)
        .cloned()
}

pub(super) fn documents(state: &ServerState) -> Vec<Document> {
    state
        .documents
        .read()
        .values()
        .filter_map(DocumentEntry::resident)
        .cloned()
        .collect()
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
        .filter_map(DocumentEntry::resident)
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

fn invalidate_project_caches(state: &ServerState) {
    state.bump_document_generation();
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
        for (uri, entry) in docs.iter() {
            let Some(doc) = entry.resident() else {
                continue;
            };
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
        evict_document_text(state, &uri);
    }
}

fn evict_document_text(state: &ServerState, uri: &Url) {
    let mut docs = state.documents.write();
    if let Some(entry) = docs.get_mut(uri) {
        if entry.resident().is_none_or(|doc| doc.is_open) {
            return;
        }
        *entry = DocumentEntry::Evicted(entry.file_id());
        drop(docs);
        state.semantic_tokens.write().remove(uri);
        state.diagnostics.write().remove(uri);
        // Keep the URI and FileId alongside the semantic source. Filesystem
        // canonicalization cannot recover that identity after an alias disappears.
    }
}
