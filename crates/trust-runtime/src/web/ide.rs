//! Web IDE scope/session model and document editing state.

#![allow(missing_docs)]

use std::collections::{BTreeMap, BTreeSet, HashMap};
use std::fmt;
use std::path::{Component, Path, PathBuf};
use std::sync::{Arc, Mutex};

use glob::Pattern;
use serde::{Deserialize, Serialize};
use text_size::{TextRange, TextSize};
use trust_hir::db::{FileId, SemanticDatabase, SourceDatabase};
use trust_wasm_analysis::{
    BrowserAnalysisEngine, CompletionItem, CompletionRequest, DiagnosticItem, DocumentInput,
    HoverItem, HoverRequest, Position,
};

mod analysis_cache;
mod utils;
mod workspace_analysis;
mod workspace_api;
mod workspace_edit_api;
mod workspace_health_api;
mod workspace_session_api;

use utils::*;

const SESSION_TTL_SECS: u64 = 15 * 60;
const MAX_SESSIONS: usize = 16;
const MAX_FILE_BYTES: usize = 256 * 1024;
const MAX_FS_AUDIT_EVENTS: usize = 1024;
const ANALYSIS_CACHE_REFRESH_INTERVAL_SECS: u64 = 2;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum IdeRole {
    Viewer,
    Editor,
}

impl IdeRole {
    #[must_use]
    pub fn parse(text: &str) -> Option<Self> {
        match text.trim().to_ascii_lowercase().as_str() {
            "viewer" | "read_only" | "readonly" => Some(Self::Viewer),
            "editor" | "authoring" => Some(Self::Editor),
            _ => None,
        }
    }

    #[must_use]
    pub fn as_str(self) -> &'static str {
        match self {
            Self::Viewer => "viewer",
            Self::Editor => "editor",
        }
    }
}

#[derive(Debug, Clone, Serialize)]
pub struct WebIdeCapabilities {
    pub enabled: bool,
    pub mode: String,
    pub diagnostics_source: String,
    pub deployment_boundaries: Vec<String>,
    pub security_model: Vec<String>,
    pub limits: WebIdeLimits,
}

#[derive(Debug, Clone, Serialize)]
pub struct WebIdeLimits {
    pub session_ttl_secs: u64,
    pub max_sessions: usize,
    pub max_file_bytes: usize,
}

#[derive(Debug, Clone, Serialize)]
pub struct IdeSession {
    pub token: String,
    pub role: String,
    pub expires_at: u64,
}

#[derive(Debug, Clone, Serialize)]
pub struct IdeFileSnapshot {
    pub path: String,
    pub content: String,
    pub version: u64,
    pub read_only: bool,
}

#[derive(Debug, Clone, Serialize)]
pub struct IdeWriteResult {
    pub path: String,
    pub version: u64,
}

#[derive(Debug, Clone, Serialize)]
pub struct IdeFormatResult {
    pub path: String,
    pub content: String,
    pub changed: bool,
}

#[derive(Debug, Clone, Serialize)]
pub struct IdeTreeNode {
    pub name: String,
    pub path: String,
    pub kind: String,
    pub children: Vec<IdeTreeNode>,
}

#[derive(Debug, Clone, Serialize)]
pub struct IdeFsResult {
    pub path: String,
    pub kind: String,
    pub version: Option<u64>,
}

#[derive(Debug, Clone, Serialize)]
pub struct IdePosition {
    pub line: u32,
    pub character: u32,
}

#[derive(Debug, Clone, Serialize)]
pub struct IdeRange {
    pub start: IdePosition,
    pub end: IdePosition,
}

#[derive(Debug, Clone, Serialize)]
pub struct IdeLocation {
    pub path: String,
    pub range: IdeRange,
}

#[derive(Debug, Clone, Serialize)]
pub struct IdeSearchHit {
    pub path: String,
    pub line: u32,
    pub character: u32,
    pub preview: String,
}

#[derive(Debug, Clone, Serialize)]
pub struct IdeSymbolHit {
    pub path: String,
    pub name: String,
    pub kind: String,
    pub line: u32,
    pub character: u32,
}

#[derive(Debug, Clone, Serialize)]
pub struct IdeRenameResult {
    pub edit_count: usize,
    pub changed_files: Vec<IdeWriteResult>,
}

#[derive(Debug, Clone, Serialize)]
pub struct IdeFsAuditRecord {
    pub ts_secs: u64,
    pub session: String,
    pub action: String,
    pub path: String,
}

#[derive(Debug, Clone, Serialize)]
pub struct WebIdeHealth {
    pub active_sessions: usize,
    pub editor_sessions: usize,
    pub tracked_documents: usize,
    pub open_document_handles: usize,
    pub fs_mutation_events: usize,
    pub limits: WebIdeLimits,
    pub frontend_telemetry: WebIdeFrontendTelemetry,
}

#[derive(Debug, Clone, Serialize)]
pub struct IdeProjectSelection {
    pub active_project: Option<String>,
    pub startup_project: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct WebIdeFrontendTelemetry {
    pub bootstrap_failures: u64,
    pub analysis_timeouts: u64,
    pub worker_restarts: u64,
    pub autosave_failures: u64,
}

#[derive(Debug, Clone, Serialize)]
pub struct IdeBrowseEntry {
    pub name: String,
    pub path: String,
    pub kind: String,
    pub size: u64,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub st_count: Option<u32>,
}

#[derive(Debug, Clone, Serialize)]
pub struct IdeBrowseResult {
    pub current_path: String,
    pub parent_path: Option<String>,
    pub entries: Vec<IdeBrowseEntry>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum IdeErrorKind {
    Unauthorized,
    Forbidden,
    NotFound,
    Conflict,
    InvalidInput,
    TooLarge,
    LimitExceeded,
    Internal,
}

#[derive(Debug, Clone)]
pub struct IdeError {
    kind: IdeErrorKind,
    message: String,
    current_version: Option<u64>,
}

impl IdeError {
    fn new(kind: IdeErrorKind, message: impl Into<String>) -> Self {
        Self {
            kind,
            message: message.into(),
            current_version: None,
        }
    }

    fn conflict(current_version: u64) -> Self {
        Self {
            kind: IdeErrorKind::Conflict,
            message: format!("edit conflict: current version is {current_version}"),
            current_version: Some(current_version),
        }
    }

    #[must_use]
    pub fn kind(&self) -> IdeErrorKind {
        self.kind
    }

    #[must_use]
    pub fn status_code(&self) -> u16 {
        match self.kind {
            IdeErrorKind::Unauthorized => 401,
            IdeErrorKind::Forbidden => 403,
            IdeErrorKind::NotFound => 404,
            IdeErrorKind::Conflict => 409,
            IdeErrorKind::InvalidInput => 400,
            IdeErrorKind::TooLarge => 413,
            IdeErrorKind::LimitExceeded => 429,
            IdeErrorKind::Internal => 500,
        }
    }

    #[must_use]
    pub fn current_version(&self) -> Option<u64> {
        self.current_version
    }
}

impl fmt::Display for IdeError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}", self.message)
    }
}

impl std::error::Error for IdeError {}

pub struct WebIdeState {
    startup_project_root: Option<PathBuf>,
    active_project_root: Mutex<Option<PathBuf>>,
    now: Arc<dyn Fn() -> u64 + Send + Sync>,
    limits: WebIdeLimits,
    inner: Mutex<IdeStateInner>,
}

impl fmt::Debug for WebIdeState {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("WebIdeState")
            .field("startup_project_root", &self.startup_project_root)
            .field("limits", &self.limits)
            .finish()
    }
}

#[derive(Debug, Default)]
struct IdeStateInner {
    sessions: HashMap<String, IdeSessionEntry>,
    documents: HashMap<String, IdeDocumentEntry>,
    frontend_telemetry_by_session: HashMap<String, WebIdeFrontendTelemetry>,
    analysis_cache: HashMap<String, IdeAnalysisCacheEntry>,
    fs_audit_log: Vec<IdeFsAuditEvent>,
}

#[derive(Debug, Clone)]
struct IdeSessionEntry {
    role: IdeRole,
    expires_at: u64,
    open_paths: BTreeSet<String>,
}

#[derive(Debug, Clone)]
struct IdeDocumentEntry {
    content: String,
    version: u64,
    opened_by: BTreeSet<String>,
}

#[derive(Debug)]
struct IdeAnalysisCacheEntry {
    engine: BrowserAnalysisEngine,
    docs: BTreeMap<String, String>,
    fingerprints: BTreeMap<String, SourceFingerprint>,
    initialized: bool,
    next_refresh_at_secs: u64,
    engine_applied: bool,
}

impl Default for IdeAnalysisCacheEntry {
    fn default() -> Self {
        Self {
            engine: BrowserAnalysisEngine::new(),
            docs: BTreeMap::new(),
            fingerprints: BTreeMap::new(),
            initialized: false,
            next_refresh_at_secs: 0,
            engine_applied: false,
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
struct SourceFingerprint {
    size_bytes: u64,
    modified_ms: u128,
}

#[derive(Debug, Clone)]
struct IdeFsAuditEvent {
    ts_secs: u64,
    session: String,
    action: String,
    path: String,
}

impl WebIdeState {
    #[must_use]
    pub fn new(project_root: Option<PathBuf>) -> Self {
        Self {
            startup_project_root: project_root.clone(),
            active_project_root: Mutex::new(project_root),
            now: Arc::new(now_secs),
            limits: WebIdeLimits {
                session_ttl_secs: SESSION_TTL_SECS,
                max_sessions: MAX_SESSIONS,
                max_file_bytes: MAX_FILE_BYTES,
            },
            inner: Mutex::new(IdeStateInner::default()),
        }
    }

    #[cfg(test)]
    fn with_clock(project_root: Option<PathBuf>, now: Arc<dyn Fn() -> u64 + Send + Sync>) -> Self {
        Self {
            startup_project_root: project_root.clone(),
            active_project_root: Mutex::new(project_root),
            now,
            limits: WebIdeLimits {
                session_ttl_secs: SESSION_TTL_SECS,
                max_sessions: MAX_SESSIONS,
                max_file_bytes: MAX_FILE_BYTES,
            },
            inner: Mutex::new(IdeStateInner::default()),
        }
    }

    fn ensure_session<'a>(
        &self,
        guard: &'a mut IdeStateInner,
        session_token: &str,
        now: u64,
    ) -> Result<&'a mut IdeSessionEntry, IdeError> {
        prune_expired(guard, now);
        let session = guard.sessions.get_mut(session_token).ok_or_else(|| {
            IdeError::new(IdeErrorKind::Unauthorized, "invalid or expired session")
        })?;
        // Sliding renewal keeps active sessions alive while preserving TTL for idle sessions.
        session.expires_at = now.saturating_add(self.limits.session_ttl_secs);
        Ok(session)
    }

    fn ensure_editor_session<'a>(
        &self,
        guard: &'a mut IdeStateInner,
        session_token: &str,
        now: u64,
    ) -> Result<&'a mut IdeSessionEntry, IdeError> {
        let session = self.ensure_session(guard, session_token, now)?;
        if !matches!(session.role, IdeRole::Editor) {
            return Err(IdeError::new(
                IdeErrorKind::Forbidden,
                "session role does not allow edits",
            ));
        }
        Ok(session)
    }

    fn record_fs_audit_event(
        &self,
        guard: &mut IdeStateInner,
        session_token: &str,
        action: &str,
        path: &str,
        ts_secs: u64,
    ) {
        let session = session_token.chars().take(8).collect::<String>();
        guard.fs_audit_log.push(IdeFsAuditEvent {
            ts_secs,
            session,
            action: action.to_string(),
            path: path.to_string(),
        });
        if guard.fs_audit_log.len() > MAX_FS_AUDIT_EVENTS {
            let drain = guard.fs_audit_log.len() - MAX_FS_AUDIT_EVENTS;
            guard.fs_audit_log.drain(0..drain);
        }
    }

    fn resolve_source_path(&self, normalized: &str) -> Result<PathBuf, IdeError> {
        self.resolve_workspace_path(normalized)
    }

    fn resolve_workspace_path(&self, normalized: &str) -> Result<PathBuf, IdeError> {
        let root = self.workspace_root()?;
        let joined = root.join(normalized);
        if std::fs::symlink_metadata(&joined)
            .is_ok_and(|metadata| metadata.file_type().is_symlink())
        {
            return Err(IdeError::new(
                IdeErrorKind::Forbidden,
                "symbolic-link workspace files are not allowed",
            ));
        }
        let canonical_root = root.canonicalize().unwrap_or(root.clone());
        let canonical_parent = closest_existing_parent(joined.parent(), &canonical_root)?;
        if !canonical_parent.starts_with(&canonical_root) {
            return Err(IdeError::new(
                IdeErrorKind::Forbidden,
                "workspace path escapes project root",
            ));
        }
        Ok(joined)
    }

    fn workspace_root(&self) -> Result<PathBuf, IdeError> {
        let Some(root) = self.active_project_root() else {
            return Err(IdeError::new(
                IdeErrorKind::NotFound,
                "project root unavailable for web IDE",
            ));
        };
        if !root.is_dir() {
            return Err(IdeError::new(
                IdeErrorKind::NotFound,
                "project root directory is missing",
            ));
        }
        Ok(root)
    }
}

#[derive(Debug)]
struct AnalysisContext {
    db: trust_hir::Database,
    file_id_by_path: BTreeMap<String, FileId>,
    path_by_file_id: HashMap<FileId, String>,
    text_by_file: HashMap<FileId, String>,
}

#[cfg(test)]
mod tests;
