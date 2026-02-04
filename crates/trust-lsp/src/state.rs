//! Server state management.
//!
//! This module manages the state of the language server, including
//! open documents and the semantic database.

use parking_lot::RwLock;
use percent_encoding::percent_decode_str;
use rustc_hash::{FxHashMap, FxHashSet};
use serde_json::Value;
use std::future::Future;
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicBool, AtomicU64, Ordering};
use std::sync::Arc;
use std::time::Duration;
use tokio::sync::Semaphore;
use tower_lsp::lsp_types::{SemanticToken, Url};

use crate::config::ProjectConfig;
use crate::telemetry::{TelemetryCollector, TelemetryEvent};
use trust_hir::{db::FileId, Database, Project, SourceKey};

const BACKGROUND_REQUEST_LIMIT: usize = 1;

/// A document managed by the server.
#[derive(Debug, Clone)]
pub struct Document {
    /// The document URI.
    pub uri: Url,
    /// The document version.
    pub version: i32,
    /// The document content.
    pub content: String,
    /// The file ID in the database.
    pub file_id: FileId,
    /// Whether the document is currently open in the editor.
    pub is_open: bool,
    /// Last access counter for LRU eviction.
    pub last_access: u64,
    /// Cached content size in bytes.
    pub content_bytes: usize,
}

impl Document {
    /// Creates a new document.
    pub fn new(
        uri: Url,
        version: i32,
        content: String,
        file_id: FileId,
        is_open: bool,
        last_access: u64,
    ) -> Self {
        let content_bytes = content.len();
        Self {
            uri,
            version,
            content,
            file_id,
            is_open,
            last_access,
            content_bytes,
        }
    }
}

#[derive(Debug, Clone)]
pub struct SemanticTokensCache {
    pub result_id: String,
    pub tokens: Vec<SemanticToken>,
}

#[derive(Debug, Clone)]
pub struct DiagnosticCache {
    pub result_id: String,
    pub content_hash: u64,
    pub diagnostic_hash: u64,
}

#[derive(Debug, Clone)]
struct RequestLimiter {
    background: Arc<Semaphore>,
}

impl RequestLimiter {
    fn new(limit: usize) -> Self {
        Self {
            background: Arc::new(Semaphore::new(limit)),
        }
    }

    async fn run_background<F, T>(&self, fut: F) -> T
    where
        F: Future<Output = T>,
    {
        let _permit = self
            .background
            .clone()
            .acquire_owned()
            .await
            .expect("background request permit");
        fut.await
    }
}

/// The server state.
pub struct ServerState {
    /// Known documents (open + indexed).
    documents: RwLock<FxHashMap<Url, Document>>,
    /// Cached semantic tokens for delta responses.
    semantic_tokens: RwLock<FxHashMap<Url, SemanticTokensCache>>,
    /// Cached diagnostics for pull requests.
    diagnostics: RwLock<FxHashMap<Url, DiagnosticCache>>,
    /// Monotonic ID for semantic token result IDs.
    semantic_tokens_id: AtomicU64,
    /// Monotonic ID for diagnostic result IDs.
    diagnostic_id: AtomicU64,
    /// Monotonic counter for document access.
    doc_access_counter: AtomicU64,
    /// Last activity time (epoch ms) for adaptive throttling.
    last_activity_ms: AtomicU64,
    /// Whether work-done progress is supported by the client.
    work_done_progress: AtomicBool,
    /// Whether diagnostic refresh requests are supported by the client.
    diagnostic_refresh_supported: AtomicBool,
    /// Whether pull diagnostics are supported by the client.
    diagnostic_pull_supported: AtomicBool,
    /// Whether semantic token refresh requests are supported by the client.
    semantic_tokens_refresh_supported: AtomicBool,
    /// Current client configuration settings.
    config: RwLock<Value>,
    /// The semantic project state.
    project: RwLock<Project>,
    /// Workspace folders.
    workspace_folders: RwLock<Vec<Url>>,
    /// Workspace configuration per root.
    workspace_configs: RwLock<FxHashMap<Url, ProjectConfig>>,
    /// Telemetry collector (opt-in).
    telemetry: TelemetryCollector,
    /// Limits concurrency for background workspace scans.
    request_limiter: RequestLimiter,
}

impl ServerState {
    /// Creates a new server state.
    pub fn new() -> Self {
        Self {
            documents: RwLock::new(FxHashMap::default()),
            semantic_tokens: RwLock::new(FxHashMap::default()),
            diagnostics: RwLock::new(FxHashMap::default()),
            semantic_tokens_id: AtomicU64::new(1),
            diagnostic_id: AtomicU64::new(1),
            doc_access_counter: AtomicU64::new(1),
            last_activity_ms: AtomicU64::new(0),
            work_done_progress: AtomicBool::new(false),
            diagnostic_refresh_supported: AtomicBool::new(false),
            diagnostic_pull_supported: AtomicBool::new(false),
            semantic_tokens_refresh_supported: AtomicBool::new(false),
            config: RwLock::new(Value::Null),
            project: RwLock::new(Project::new()),
            workspace_folders: RwLock::new(Vec::new()),
            workspace_configs: RwLock::new(FxHashMap::default()),
            telemetry: TelemetryCollector::new(),
            request_limiter: RequestLimiter::new(BACKGROUND_REQUEST_LIMIT),
        }
    }

    /// Stores the workspace folders.
    pub fn set_workspace_folders(&self, folders: Vec<Url>) {
        *self.workspace_folders.write() = folders;
    }

    /// Returns the current workspace folders.
    pub fn workspace_folders(&self) -> Vec<Url> {
        self.workspace_folders.read().clone()
    }

    /// Records whether the client supports work-done progress.
    pub fn set_work_done_progress(&self, supported: bool) {
        self.work_done_progress.store(supported, Ordering::Relaxed);
    }

    /// Returns true if the client supports work-done progress.
    pub fn work_done_progress(&self) -> bool {
        self.work_done_progress.load(Ordering::Relaxed)
    }

    /// Records whether diagnostic refresh is supported by the client.
    pub fn set_diagnostic_refresh_supported(&self, supported: bool) {
        self.diagnostic_refresh_supported
            .store(supported, Ordering::Relaxed);
    }

    /// Records whether pull diagnostics are supported by the client.
    pub fn set_diagnostic_pull_supported(&self, supported: bool) {
        self.diagnostic_pull_supported
            .store(supported, Ordering::Relaxed);
    }

    /// Returns true if diagnostic refresh is supported by the client.
    pub fn diagnostic_refresh_supported(&self) -> bool {
        self.diagnostic_refresh_supported.load(Ordering::Relaxed)
    }

    /// Returns true if the client supports pull diagnostics.
    pub fn diagnostic_pull_supported(&self) -> bool {
        self.diagnostic_pull_supported.load(Ordering::Relaxed)
    }

    /// Returns true if the server should use pull diagnostics for this client.
    pub fn use_pull_diagnostics(&self) -> bool {
        self.diagnostic_pull_supported() && self.diagnostic_refresh_supported()
    }

    /// Records whether semantic token refresh is supported by the client.
    pub fn set_semantic_tokens_refresh_supported(&self, supported: bool) {
        self.semantic_tokens_refresh_supported
            .store(supported, Ordering::Relaxed);
    }

    /// Returns true if semantic token refresh is supported by the client.
    pub fn semantic_tokens_refresh_supported(&self) -> bool {
        self.semantic_tokens_refresh_supported
            .load(Ordering::Relaxed)
    }

    /// Stores configuration for a workspace root.
    pub fn set_workspace_config(&self, root: Url, config: ProjectConfig) {
        self.workspace_configs.write().insert(root, config);
    }

    /// Returns configuration for a workspace root.
    #[allow(dead_code)]
    pub fn workspace_config_for_root(&self, root: &Url) -> Option<ProjectConfig> {
        self.workspace_configs.read().get(root).cloned()
    }

    /// Returns all workspace configurations with their roots.
    pub fn workspace_configs(&self) -> Vec<(Url, ProjectConfig)> {
        self.workspace_configs
            .read()
            .iter()
            .map(|(root, config)| (root.clone(), config.clone()))
            .collect()
    }

    /// Returns the highest-priority workspace configuration, if any.
    pub fn primary_workspace_config(&self) -> Option<ProjectConfig> {
        self.workspace_configs
            .read()
            .values()
            .cloned()
            .max_by_key(|config| config.workspace.priority)
    }

    /// Returns the best-matching workspace configuration for a document URI.
    pub fn workspace_config_for_uri(&self, uri: &Url) -> Option<ProjectConfig> {
        let configs = self.workspace_configs.read();
        if let Some(path) = uri_to_path(uri) {
            let path = normalize_match_path(path);
            let mut best: Option<(usize, ProjectConfig)> = None;
            for (root_url, config) in configs.iter() {
                let Some(root_path) = uri_to_path(root_url) else {
                    continue;
                };
                let root_path = normalize_match_path(root_path);
                if path.starts_with(&root_path) {
                    let depth = root_path.components().count();
                    if best
                        .as_ref()
                        .is_none_or(|(best_depth, _)| depth > *best_depth)
                    {
                        best = Some((depth, config.clone()));
                    }
                }
            }
            if best.is_some() {
                return best.map(|(_, config)| config);
            }
        }

        let uri_segments: Vec<_> = uri
            .path_segments()
            .map(|segments| segments.filter(|segment| !segment.is_empty()).collect())
            .unwrap_or_default();
        let mut best: Option<(usize, ProjectConfig)> = None;
        for (root_url, config) in configs.iter() {
            let root_segments: Vec<_> = root_url
                .path_segments()
                .map(|segments| segments.filter(|segment| !segment.is_empty()).collect())
                .unwrap_or_default();
            if root_segments.is_empty() {
                continue;
            }
            if uri_segments.len() < root_segments.len() {
                continue;
            }
            if uri_segments[..root_segments.len()] == root_segments[..] {
                let depth = root_segments.len();
                if best
                    .as_ref()
                    .is_none_or(|(best_depth, _)| depth > *best_depth)
                {
                    best = Some((depth, config.clone()));
                }
            }
        }
        best.map(|(_, config)| config)
    }

    pub fn record_telemetry(&self, event: TelemetryEvent, duration: Duration, uri: Option<&Url>) {
        let config = uri
            .and_then(|uri| self.workspace_config_for_uri(uri))
            .or_else(|| self.primary_workspace_config());
        self.telemetry.record(
            config.as_ref().map(|config| &config.telemetry),
            event,
            duration,
        );
    }

    pub fn flush_telemetry(&self) {
        self.telemetry.flush();
    }

    /// Returns true if the document is already tracked.
    #[allow(dead_code)]
    pub fn has_document(&self, uri: &Url) -> bool {
        self.documents.read().contains_key(uri)
    }

    /// Opens a document.
    pub fn open_document(&self, uri: Url, version: i32, content: String) -> FileId {
        let key = source_key_for_uri(&uri);
        let file_id = {
            let mut project = self.project.write();
            project.set_source_text(key, content.clone())
        };

        let access = self.next_document_access();
        let mut docs = self.documents.write();
        if let Some(doc) = docs.get_mut(&uri) {
            doc.version = version;
            doc.content = content;
            doc.is_open = true;
            doc.file_id = file_id;
            self.touch_document(doc, access);
        } else {
            let doc = Document::new(uri.clone(), version, content, file_id, true, access);
            docs.insert(uri, doc);
        }

        file_id
    }

    /// Indexes a document that is not currently open.
    pub fn index_document(&self, uri: Url, content: String) -> Option<FileId> {
        if let Some(doc) = self.documents.read().get(&uri) {
            if !doc.is_open && doc.content == content {
                return None;
            }
        }
        if self
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
            let mut project = self.project.write();
            project.set_source_text(key.clone(), content.clone())
        };

        let access = self.next_document_access();
        {
            let mut docs = self.documents.write();
            if let Some(doc) = docs.get_mut(&uri) {
                if doc.is_open {
                    return None;
                }
                doc.version = 0;
                doc.content = content;
                doc.is_open = false;
                doc.file_id = file_id;
                self.touch_document(doc, access);
            } else {
                let doc = Document::new(uri.clone(), 0, content, file_id, false, access);
                docs.insert(uri, doc);
            }
        }

        self.enforce_memory_budget();
        Some(file_id)
    }

    /// Updates a document.
    pub fn update_document(&self, uri: &Url, version: i32, content: String) {
        let key = source_key_for_uri(uri);
        let file_id = {
            let mut project = self.project.write();
            project.set_source_text(key, content.clone())
        };

        let access = self.next_document_access();
        let mut docs = self.documents.write();
        if let Some(doc) = docs.get_mut(uri) {
            doc.version = version;
            doc.content = content;
            doc.is_open = true;
            doc.file_id = file_id;
            self.touch_document(doc, access);
        }
    }

    /// Closes a document.
    pub fn close_document(&self, uri: &Url) {
        if let Some(doc) = self.documents.write().get_mut(uri) {
            doc.is_open = false;
        }
        self.enforce_memory_budget();
    }

    /// Removes a document and its source from the project.
    pub fn remove_document(&self, uri: &Url) -> Option<FileId> {
        let key = source_key_for_uri(uri);
        let doc = self.documents.write().remove(uri)?;
        self.semantic_tokens.write().remove(uri);
        self.diagnostics.write().remove(uri);
        let mut project = self.project.write();
        project.remove_source(&key);
        Some(doc.file_id)
    }

    /// Renames a document while preserving its open state and content.
    pub fn rename_document(&self, old_uri: &Url, new_uri: &Url) -> Option<FileId> {
        let mut docs = self.documents.write();
        let mut doc = docs.remove(old_uri)?;
        self.semantic_tokens.write().remove(old_uri);
        self.diagnostics.write().remove(old_uri);

        let old_key = source_key_for_uri(old_uri);
        let new_key = source_key_for_uri(new_uri);
        let mut project = self.project.write();
        project.remove_source(&old_key);
        project.remove_source(&new_key);
        let file_id = project.set_source_text(new_key, doc.content.clone());

        doc.uri = new_uri.clone();
        doc.file_id = file_id;
        docs.insert(new_uri.clone(), doc);
        Some(file_id)
    }

    /// Gets a document by URI.
    pub fn get_document(&self, uri: &Url) -> Option<Document> {
        self.documents.read().get(uri).cloned()
    }

    /// Returns all tracked documents.
    pub fn documents(&self) -> Vec<Document> {
        self.documents.read().values().cloned().collect()
    }

    /// Ensures a document is tracked, loading from disk if needed.
    pub fn ensure_document(&self, uri: &Url) -> Option<Document> {
        if let Some(doc) = self.get_document(uri) {
            return Some(doc);
        }
        let path = uri_to_path(uri)?;
        let content = std::fs::read_to_string(&path).ok()?;
        self.index_document(uri.clone(), content);
        self.get_document(uri)
    }

    /// Records recent editor activity for adaptive throttling.
    pub fn record_activity(&self) {
        self.last_activity_ms.store(now_millis(), Ordering::Relaxed);
    }

    /// Returns ms since last activity (or u64::MAX if none recorded).
    pub fn activity_age_ms(&self) -> u64 {
        let last = self.last_activity_ms.load(Ordering::Relaxed);
        if last == 0 {
            return u64::MAX;
        }
        now_millis().saturating_sub(last)
    }

    /// Finds a document URI by file ID.
    // Not wired yet; reserved for cross-file LSP features.
    #[allow(dead_code)]
    pub fn uri_for_file_id(&self, file_id: FileId) -> Option<Url> {
        if let Some(doc) = self.document_for_file_id(file_id) {
            return Some(doc.uri);
        }
        let project = self.project.read();
        let key = project.key_for_file_id(file_id)?;
        match key {
            SourceKey::Path(path) => path_to_uri(path),
            SourceKey::Virtual(name) => Url::parse(name).ok(),
        }
    }

    /// Finds a document by file ID.
    pub fn document_for_file_id(&self, file_id: FileId) -> Option<Document> {
        self.documents
            .read()
            .values()
            .find(|doc| doc.file_id == file_id)
            .cloned()
    }

    /// Returns file IDs that belong to the given workspace configuration.
    pub fn file_ids_for_config(&self, config: &ProjectConfig) -> FxHashSet<FileId> {
        let roots: Vec<PathBuf> = config
            .indexing_roots()
            .into_iter()
            .map(canonicalize_path)
            .collect();
        let project = self.project.read();
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

    /// Stores updated client configuration settings.
    pub fn set_config(&self, config: Value) {
        *self.config.write() = config;
    }

    /// Returns the current configuration snapshot.
    pub fn config(&self) -> Value {
        self.config.read().clone()
    }

    /// Runs background work with a concurrency cap to keep interactive requests responsive.
    pub async fn run_background<F, T>(&self, fut: F) -> T
    where
        F: Future<Output = T>,
    {
        self.request_limiter.run_background(fut).await
    }

    pub fn semantic_tokens_cache(&self, uri: &Url) -> Option<SemanticTokensCache> {
        self.semantic_tokens.read().get(uri).cloned()
    }

    pub fn store_semantic_tokens(&self, uri: Url, tokens: Vec<SemanticToken>) -> String {
        let result_id = self.next_semantic_tokens_id();
        let cache = SemanticTokensCache {
            result_id: result_id.clone(),
            tokens,
        };
        self.semantic_tokens.write().insert(uri, cache);
        result_id
    }

    /// Stores diagnostics in the cache and returns the result ID.
    pub fn store_diagnostics(&self, uri: Url, content_hash: u64, diagnostic_hash: u64) -> String {
        let mut cache = self.diagnostics.write();
        if let Some(existing) = cache.get(&uri) {
            if existing.content_hash == content_hash && existing.diagnostic_hash == diagnostic_hash
            {
                return existing.result_id.clone();
            }
        }
        let result_id = self.next_diagnostic_id();
        cache.insert(
            uri,
            DiagnosticCache {
                result_id: result_id.clone(),
                content_hash,
                diagnostic_hash,
            },
        );
        result_id
    }

    fn next_semantic_tokens_id(&self) -> String {
        self.semantic_tokens_id
            .fetch_add(1, Ordering::Relaxed)
            .to_string()
    }

    fn next_diagnostic_id(&self) -> String {
        format!(
            "diag-{}",
            self.diagnostic_id.fetch_add(1, Ordering::Relaxed)
        )
    }

    fn next_document_access(&self) -> u64 {
        self.doc_access_counter.fetch_add(1, Ordering::Relaxed)
    }

    fn touch_document(&self, doc: &mut Document, access: u64) {
        doc.last_access = access;
        doc.content_bytes = doc.content.len();
    }

    fn enforce_memory_budget(&self) {
        let Some(config) = self.primary_workspace_config() else {
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
            let docs = self.documents.read();
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
            let _ = self.remove_document(&uri);
        }
    }

    /// Enforces the configured memory budget for closed documents.
    pub fn apply_memory_budget(&self) {
        self.enforce_memory_budget();
    }

    /// Gets the database for reading.
    // Not used directly yet; kept for future handlers.
    #[allow(dead_code)]
    pub fn database(&self) -> parking_lot::MappedRwLockReadGuard<'_, Database> {
        let project = self.project.read();
        parking_lot::RwLockReadGuard::map(project, Project::database)
    }

    /// Gets the database for writing.
    // Not used directly yet; kept for future handlers.
    #[allow(dead_code)]
    pub fn database_mut(&self) -> parking_lot::MappedRwLockWriteGuard<'_, Database> {
        let project = self.project.write();
        parking_lot::RwLockWriteGuard::map(project, Project::database_mut)
    }

    /// Executes a function with a database snapshot.
    pub fn with_database<F, R>(&self, f: F) -> R
    where
        F: FnOnce(&Database) -> R,
    {
        let snapshot = {
            let project = self.project.read();
            project.database().clone()
        };
        f(&snapshot)
    }
}

impl Default for ServerState {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(windows)]
fn normalize_windows_path_for_url(path: &Path) -> Option<PathBuf> {
    let raw = path.to_str()?;
    if let Some(rest) = raw
        .strip_prefix("\\\\?\\UNC\\")
        .or_else(|| raw.strip_prefix("\\?\\UNC\\"))
        .or_else(|| raw.strip_prefix("\\\\.\\UNC\\"))
        .or_else(|| raw.strip_prefix("\\??\\UNC\\"))
    {
        let mut unc = String::from("\\\\");
        unc.push_str(rest);
        return Some(PathBuf::from(unc));
    }
    if let Some(rest) = raw
        .strip_prefix("\\\\?\\")
        .or_else(|| raw.strip_prefix("\\?\\"))
        .or_else(|| raw.strip_prefix("\\\\.\\"))
        .or_else(|| raw.strip_prefix("\\??\\"))
    {
        return Some(PathBuf::from(rest));
    }
    None
}

fn path_debug_enabled() -> bool {
    std::env::var_os("TRUST_LSP_PATH_DEBUG").is_some()
}

pub(crate) fn path_to_uri(path: &Path) -> Option<Url> {
    if path_debug_enabled() {
        tracing::info!(target: "trust_lsp::path", "path_to_uri input: {}", path.display());
    }

    #[cfg(windows)]
    {
        if let Some(normalized) = normalize_windows_path_for_url(path) {
            if let Ok(url) = Url::from_file_path(&normalized) {
                if path_debug_enabled() {
                    tracing::info!(target: "trust_lsp::path", "path_to_uri output: {}", url);
                }
                return Some(url);
            }
            return path_to_uri(&normalized);
        }
    }

    if let Ok(url) = Url::from_file_path(path) {
        if path_debug_enabled() {
            tracing::info!(target: "trust_lsp::path", "path_to_uri output: {}", url);
        }
        return Some(url);
    }

    let raw_str = path.to_string_lossy();
    if !path.is_absolute() && !raw_str.starts_with('/') {
        return None;
    }
    let mut raw = raw_str.replace('\\', "/");

    #[cfg(windows)]
    {
        if raw.starts_with("//?/UNC/")
            || raw.starts_with("/?/UNC/")
            || raw.starts_with("//./UNC/")
            || raw.starts_with("/./UNC/")
            || raw.starts_with("//??/UNC/")
            || raw.starts_with("/??/UNC/")
        {
            raw = format!(
                "//{}",
                raw.trim_start_matches("//?/UNC/")
                    .trim_start_matches("/?/UNC/")
                    .trim_start_matches("//./UNC/")
                    .trim_start_matches("/./UNC/")
                    .trim_start_matches("//??/UNC/")
                    .trim_start_matches("/??/UNC/")
            );
        } else if raw.starts_with("//?/")
            || raw.starts_with("/?/")
            || raw.starts_with("//./")
            || raw.starts_with("/./")
            || raw.starts_with("//??/")
            || raw.starts_with("/??/")
        {
            raw = raw
                .trim_start_matches("//?/")
                .trim_start_matches("/?/")
                .trim_start_matches("//./")
                .trim_start_matches("/./")
                .trim_start_matches("//??/")
                .trim_start_matches("/??/")
                .to_string();
        }
    }

    if !raw.starts_with('/') {
        raw = format!("/{raw}");
    }
    if path_debug_enabled() {
        tracing::info!(target: "trust_lsp::path", "path_to_uri raw: {}", raw);
    }
    let url = Url::parse(&format!("file://{raw}")).ok();
    if let Some(ref url) = url {
        if path_debug_enabled() {
            tracing::info!(target: "trust_lsp::path", "path_to_uri output: {}", url);
        }
    }
    url
}

pub(crate) fn uri_to_path(uri: &Url) -> Option<PathBuf> {
    if path_debug_enabled() {
        tracing::info!(target: "trust_lsp::path", "uri_to_path input: {}", uri);
    }
    if let Ok(path) = uri.to_file_path() {
        if path_debug_enabled() {
            tracing::info!(target: "trust_lsp::path", "uri_to_path output: {}", path.display());
        }
        return Some(path);
    }
    if uri.scheme() == "file" {
        let raw_path = uri.path();
        if raw_path.is_empty() {
            return None;
        }
        let decoded = percent_decode_str(raw_path).decode_utf8_lossy();
        if path_debug_enabled() {
            tracing::info!(
                target: "trust_lsp::path",
                "uri_to_path decoded path: {}",
                decoded
            );
        }

        #[cfg(windows)]
        {
            if let Some(stripped) = decoded.strip_prefix('/') {
                if stripped.len() >= 2 && stripped.as_bytes()[1] == b':' {
                    return Some(PathBuf::from(stripped));
                }
            }
            if let Some(host) = uri.host_str() {
                let host = percent_decode_str(host).decode_utf8_lossy();
                let mut unc = String::from("\\\\");
                unc.push_str(&host);
                if let Some(path) = decoded.strip_prefix('/') {
                    unc.push('\\');
                    unc.push_str(&path.replace('/', "\\"));
                }
                return Some(PathBuf::from(unc));
            }
        }

        return Some(PathBuf::from(decoded.as_ref()));
    }
    None
}

fn source_key_for_uri(uri: &Url) -> SourceKey {
    if let Some(path) = uri_to_path(uri) {
        SourceKey::from_path(path)
    } else {
        SourceKey::from_virtual(uri.to_string())
    }
}

fn now_millis() -> u64 {
    use std::time::{SystemTime, UNIX_EPOCH};
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap_or_default()
        .as_millis() as u64
}

fn normalize_match_path(path: PathBuf) -> PathBuf {
    let mut normalized = PathBuf::new();
    for component in path.components() {
        match component {
            std::path::Component::CurDir => {}
            std::path::Component::ParentDir => {
                normalized.pop();
            }
            _ => normalized.push(component.as_os_str()),
        }
    }
    normalized
}

#[cfg(windows)]
fn strip_windows_device_prefix(path: PathBuf) -> PathBuf {
    let raw = match path.to_str() {
        Some(raw) => raw,
        None => return path,
    };

    if let Some(rest) = raw
        .strip_prefix("\\\\?\\UNC\\")
        .or_else(|| raw.strip_prefix("\\?\\UNC\\"))
        .or_else(|| raw.strip_prefix("\\\\.\\UNC\\"))
        .or_else(|| raw.strip_prefix("\\??\\UNC\\"))
    {
        let mut unc = String::from("\\\\");
        unc.push_str(rest);
        return PathBuf::from(unc);
    }

    if let Some(rest) = raw
        .strip_prefix("\\\\?\\")
        .or_else(|| raw.strip_prefix("\\?\\"))
        .or_else(|| raw.strip_prefix("\\\\.\\"))
        .or_else(|| raw.strip_prefix("\\??\\"))
    {
        return PathBuf::from(rest);
    }

    path
}

#[cfg(not(windows))]
fn strip_windows_device_prefix(path: PathBuf) -> PathBuf {
    path
}

fn canonicalize_path(path: PathBuf) -> PathBuf {
    if path_debug_enabled() {
        tracing::info!(target: "trust_lsp::path", "canonicalize_path input: {}", path.display());
    }
    if let Ok(canon) = path.canonicalize() {
        let canon = strip_windows_device_prefix(canon);
        if path_debug_enabled() {
            tracing::info!(target: "trust_lsp::path", "canonicalize_path output: {}", canon.display());
        }
        return canon;
    }
    path
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::fs;
    use std::sync::Arc;
    use std::time::{SystemTime, UNIX_EPOCH};
    use tokio::sync::mpsc;
    use tokio::time::{timeout, Duration};

    fn temp_dir(prefix: &str) -> std::path::PathBuf {
        let stamp = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap()
            .as_millis();
        let dir = std::env::temp_dir().join(format!("{prefix}-{stamp}"));
        fs::create_dir_all(&dir).expect("create temp dir");
        dir
    }

    #[tokio::test(flavor = "current_thread")]
    async fn background_requests_serialize() {
        let state = Arc::new(ServerState::new());
        let (first_started_tx, mut first_started_rx) = mpsc::channel(1);
        let (release_tx, mut release_rx) = mpsc::channel(1);
        let (second_started_tx, mut second_started_rx) = mpsc::channel(1);

        let first = tokio::spawn({
            let state = Arc::clone(&state);
            async move {
                state
                    .run_background(async move {
                        let _ = first_started_tx.send(()).await;
                        let _ = release_rx.recv().await;
                    })
                    .await;
            }
        });

        let _ = first_started_rx.recv().await;

        let second = tokio::spawn({
            let state = Arc::clone(&state);
            async move {
                state
                    .run_background(async move {
                        let _ = second_started_tx.send(()).await;
                    })
                    .await;
            }
        });

        assert!(
            timeout(Duration::from_millis(50), second_started_rx.recv())
                .await
                .is_err(),
            "second background request should wait for permit"
        );

        let _ = release_tx.send(()).await;
        let _ = timeout(Duration::from_millis(200), second_started_rx.recv()).await;

        let _ = first.await;
        let _ = second.await;
    }

    #[test]
    fn evicts_closed_documents_over_budget() {
        let root = temp_dir("trustlsp-budget");
        let config_path = root.join("trust-lsp.toml");
        fs::write(
            &config_path,
            r#"
[indexing]
memory_budget_mb = 1
evict_to_percent = 75
"#,
        )
        .expect("write config");
        let config = ProjectConfig::load(&root);

        let state = ServerState::new();
        let root_uri = Url::from_file_path(&root).expect("root uri");
        state.set_workspace_config(root_uri, config);

        let payload = "A".repeat(600_000);
        let file_a = root.join("a.st");
        let file_b = root.join("b.st");
        fs::write(&file_a, &payload).expect("write a");
        fs::write(&file_b, &payload).expect("write b");

        let uri_a = Url::from_file_path(&file_a).expect("uri a");
        let uri_b = Url::from_file_path(&file_b).expect("uri b");

        state.index_document(uri_a.clone(), payload.clone());
        state.index_document(uri_b.clone(), payload.clone());

        assert!(
            state.get_document(&uri_a).is_none(),
            "expected least-recent document to be evicted"
        );
        assert!(
            state.get_document(&uri_b).is_some(),
            "expected newest document to remain"
        );

        let reloaded = state.ensure_document(&uri_a);
        assert!(reloaded.is_some(), "expected evicted document to reload");
        assert!(
            state.get_document(&uri_a).is_some(),
            "expected evicted document to be re-indexed"
        );

        fs::remove_dir_all(root).ok();
    }

    #[test]
    #[cfg(windows)]
    fn uri_to_path_decodes_drive_letter() {
        let uri = Url::parse("file:///c%3A/1.Work/projects/test.st").unwrap();
        let path = uri_to_path(&uri).expect("decoded path");
        let lower = path.to_string_lossy().to_lowercase();
        assert!(lower.starts_with("c:"));
        assert!(lower.contains("1.work"));
    }

    #[test]
    #[cfg(windows)]
    fn path_to_uri_strips_extended_length_prefix() {
        let path = PathBuf::from(r"\\?\C:\1.Work\projects\test.st");
        let uri = path_to_uri(&path).expect("uri");
        let uri_str = uri.as_str();
        assert!(!uri_str.contains("%3F"), "uri should not include ? host");
        assert!(uri_str.contains("c:/") || uri_str.contains("C:/"));
    }
}
