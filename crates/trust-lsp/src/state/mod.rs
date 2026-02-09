//! Server state management.
//!
//! This module manages the state of the language server, including
//! open documents and the semantic database.
#![cfg_attr(not(test), deny(clippy::unwrap_used))]

use parking_lot::RwLock;
use rustc_hash::{FxHashMap, FxHashSet};
use serde_json::Value;
use std::future::Future;
use std::sync::atomic::{AtomicBool, AtomicU64, Ordering};
use std::sync::Arc;
use std::time::Duration;
use tokio::sync::Semaphore;
use tower_lsp::lsp_types::{SemanticToken, Url};

use crate::config::ProjectConfig;
use crate::telemetry::{TelemetryCollector, TelemetryEvent};
use trust_hir::{db::FileId, Database, Project};

const BACKGROUND_REQUEST_LIMIT: usize = 1;

mod cache;
mod documents;
mod path;

pub(crate) use path::{path_to_uri, uri_to_path};

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
        let _permit = self.background.clone().acquire_owned().await.ok();
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
        path::workspace_config_for_uri(self, uri)
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

    /// Opens a document.
    pub fn open_document(&self, uri: Url, version: i32, content: String) -> FileId {
        documents::open_document(self, uri, version, content)
    }

    /// Indexes a document that is not currently open.
    pub fn index_document(&self, uri: Url, content: String) -> Option<FileId> {
        documents::index_document(self, uri, content)
    }

    /// Indexes a document while deferring memory-budget enforcement to the caller.
    pub fn index_document_deferred_budget(&self, uri: Url, content: String) -> Option<FileId> {
        documents::index_document_deferred_budget(self, uri, content)
    }

    /// Updates a document.
    pub fn update_document(&self, uri: &Url, version: i32, content: String) {
        documents::update_document(self, uri, version, content);
    }

    /// Closes a document.
    pub fn close_document(&self, uri: &Url) {
        documents::close_document(self, uri);
    }

    /// Removes a document and its source from the project.
    pub fn remove_document(&self, uri: &Url) -> Option<FileId> {
        documents::remove_document(self, uri)
    }

    /// Renames a document while preserving its open state and content.
    pub fn rename_document(&self, old_uri: &Url, new_uri: &Url) -> Option<FileId> {
        documents::rename_document(self, old_uri, new_uri)
    }

    /// Gets a document by URI.
    pub fn get_document(&self, uri: &Url) -> Option<Document> {
        documents::get_document(self, uri)
    }

    /// Returns all tracked documents.
    pub fn documents(&self) -> Vec<Document> {
        documents::documents(self)
    }

    /// Ensures a document is tracked, loading from disk if needed.
    pub fn ensure_document(&self, uri: &Url) -> Option<Document> {
        documents::ensure_document(self, uri)
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
    pub fn uri_for_file_id(&self, file_id: FileId) -> Option<Url> {
        documents::uri_for_file_id(self, file_id)
    }

    /// Finds a document by file ID.
    pub fn document_for_file_id(&self, file_id: FileId) -> Option<Document> {
        documents::document_for_file_id(self, file_id)
    }

    /// Returns file IDs that belong to the given workspace configuration.
    pub fn file_ids_for_config(&self, config: &ProjectConfig) -> FxHashSet<FileId> {
        documents::file_ids_for_config(self, config)
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
        cache::semantic_tokens_cache(self, uri)
    }

    pub fn store_semantic_tokens(&self, uri: Url, tokens: Vec<SemanticToken>) -> String {
        cache::store_semantic_tokens(self, uri, tokens)
    }

    /// Stores diagnostics in the cache and returns the result ID.
    pub fn store_diagnostics(&self, uri: Url, content_hash: u64, diagnostic_hash: u64) -> String {
        cache::store_diagnostics(self, uri, content_hash, diagnostic_hash)
    }

    /// Enforces the configured memory budget for closed documents.
    pub fn apply_memory_budget(&self) {
        documents::apply_memory_budget(self);
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

fn now_millis() -> u64 {
    use std::time::{SystemTime, UNIX_EPOCH};
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap_or_default()
        .as_millis() as u64
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::fs;
    #[cfg(windows)]
    use std::path::PathBuf;
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

    #[tokio::test(flavor = "current_thread")]
    async fn background_requests_run_when_limiter_is_closed() {
        let state = ServerState::new();
        state.request_limiter.background.close();

        let result = state.run_background(async { 42_u8 }).await;
        assert_eq!(result, 42);
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
    fn document_lifecycle_open_update_close_rename_remove() {
        let root = temp_dir("trustlsp-doc-lifecycle");
        let file_a = root.join("main.st");
        let file_b = root.join("renamed.st");
        fs::write(&file_a, "PROGRAM Main\nEND_PROGRAM\n").expect("write source");

        let uri_a = Url::from_file_path(&file_a).expect("uri a");
        let uri_b = Url::from_file_path(&file_b).expect("uri b");

        let state = ServerState::new();
        let file_id = state.open_document(uri_a.clone(), 1, "PROGRAM Main\nEND_PROGRAM\n".into());
        let opened = state.get_document(&uri_a).expect("opened doc");
        assert_eq!(opened.file_id, file_id);
        assert_eq!(opened.version, 1);
        assert!(opened.is_open);

        state.update_document(
            &uri_a,
            2,
            "PROGRAM Main\nVAR x : INT;\nEND_VAR\nEND_PROGRAM\n".into(),
        );
        let updated = state.get_document(&uri_a).expect("updated doc");
        assert_eq!(updated.version, 2);
        assert!(updated.content.contains("x : INT"));

        state.close_document(&uri_a);
        let closed = state.get_document(&uri_a).expect("closed doc");
        assert!(!closed.is_open);

        let renamed_id = state
            .rename_document(&uri_a, &uri_b)
            .expect("rename document");
        assert!(state.get_document(&uri_a).is_none());
        let renamed = state.get_document(&uri_b).expect("renamed doc");
        assert_eq!(renamed.file_id, renamed_id);

        let removed_id = state.remove_document(&uri_b).expect("remove document");
        assert_eq!(removed_id, renamed_id);
        assert!(state.get_document(&uri_b).is_none());

        fs::remove_dir_all(root).ok();
    }

    #[test]
    fn diagnostic_cache_reuses_result_id_for_identical_hashes() {
        let state = ServerState::new();
        let uri = Url::parse("file:///tmp/trust-lsp-cache.st").expect("uri");

        let first = state.store_diagnostics(uri.clone(), 11, 22);
        let second = state.store_diagnostics(uri.clone(), 11, 22);
        let third = state.store_diagnostics(uri.clone(), 11, 23);

        assert_eq!(
            first, second,
            "unchanged diagnostics should reuse result id"
        );
        assert_ne!(
            first, third,
            "changed diagnostics should emit a new result id"
        );
    }

    #[test]
    fn path_uri_roundtrip_handles_spaces_and_fragments() {
        let root = temp_dir("trustlsp-path-roundtrip");
        let nested = root.join("dir with space");
        fs::create_dir_all(&nested).expect("create nested");
        let file = nested.join("file #1.st");
        fs::write(&file, "PROGRAM Main\nEND_PROGRAM\n").expect("write source");

        let uri = path_to_uri(&file).expect("path -> uri");
        let roundtrip = uri_to_path(&uri).expect("uri -> path");
        assert_eq!(roundtrip, file);

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
