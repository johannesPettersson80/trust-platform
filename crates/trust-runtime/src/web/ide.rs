//! Web IDE scope/session model and document editing state.

#![allow(missing_docs)]

use std::collections::{BTreeSet, HashMap};
use std::fmt;
use std::path::{Component, Path, PathBuf};
use std::sync::{Arc, Mutex};
use std::time::{SystemTime, UNIX_EPOCH};

use base64::engine::general_purpose::URL_SAFE_NO_PAD;
use base64::Engine;
use rand::rngs::OsRng;
use rand::RngCore;
use serde::Serialize;

const SESSION_TTL_SECS: u64 = 15 * 60;
const MAX_SESSIONS: usize = 16;
const MAX_FILE_BYTES: usize = 256 * 1024;

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
    project_root: Option<PathBuf>,
    now: Arc<dyn Fn() -> u64 + Send + Sync>,
    limits: WebIdeLimits,
    inner: Mutex<IdeStateInner>,
}

impl fmt::Debug for WebIdeState {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("WebIdeState")
            .field("project_root", &self.project_root)
            .field("limits", &self.limits)
            .finish()
    }
}

#[derive(Debug, Default)]
struct IdeStateInner {
    sessions: HashMap<String, IdeSessionEntry>,
    documents: HashMap<String, IdeDocumentEntry>,
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

impl WebIdeState {
    #[must_use]
    pub fn new(project_root: Option<PathBuf>) -> Self {
        Self {
            project_root,
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
            project_root,
            now,
            limits: WebIdeLimits {
                session_ttl_secs: SESSION_TTL_SECS,
                max_sessions: MAX_SESSIONS,
                max_file_bytes: MAX_FILE_BYTES,
            },
            inner: Mutex::new(IdeStateInner::default()),
        }
    }

    #[must_use]
    pub fn capabilities(&self, write_enabled: bool) -> WebIdeCapabilities {
        WebIdeCapabilities {
            enabled: self.project_root.is_some(),
            mode: if write_enabled {
                "authoring".to_string()
            } else {
                "read_only".to_string()
            },
            diagnostics_source: "trust-lsp/trust-hir through project sources (out-of-process)"
                .to_string(),
            deployment_boundaries: vec![
                "Allowed file scope: <project>/src/**/*.st".to_string(),
                "Runtime/deploy configs are excluded from IDE write surface".to_string(),
                "Authoring mode requires runtime control mode=debug".to_string(),
            ],
            security_model: vec![
                "Session bootstrap requires web auth (local or X-Trust-Token)".to_string(),
                "Per-session token required for IDE API calls (X-Trust-Ide-Session)".to_string(),
                "Optimistic concurrency via expected_version prevents blind overwrite".to_string(),
            ],
            limits: self.limits.clone(),
        }
    }

    pub fn create_session(&self, role: IdeRole) -> Result<IdeSession, IdeError> {
        let now = (self.now)();
        let mut guard = self
            .inner
            .lock()
            .map_err(|_| IdeError::new(IdeErrorKind::Internal, "session state lock poisoned"))?;
        prune_expired(&mut guard, now);
        if guard.sessions.len() >= self.limits.max_sessions {
            return Err(IdeError::new(
                IdeErrorKind::LimitExceeded,
                "too many active IDE sessions",
            ));
        }

        let token = generate_token();
        let expires_at = now.saturating_add(self.limits.session_ttl_secs);
        guard.sessions.insert(
            token.clone(),
            IdeSessionEntry {
                role,
                expires_at,
                open_paths: BTreeSet::new(),
            },
        );

        Ok(IdeSession {
            token,
            role: role.as_str().to_string(),
            expires_at,
        })
    }

    pub fn list_sources(&self, session_token: &str) -> Result<Vec<String>, IdeError> {
        let now = (self.now)();
        let mut guard = self
            .inner
            .lock()
            .map_err(|_| IdeError::new(IdeErrorKind::Internal, "session state lock poisoned"))?;
        self.ensure_session(&mut guard, session_token, now)?;
        drop(guard);

        let root = self.sources_root()?;
        let mut list = Vec::new();
        collect_source_files(&root, &PathBuf::new(), &mut list)?;
        list.sort();
        Ok(list)
    }

    pub fn open_source(
        &self,
        session_token: &str,
        path: &str,
    ) -> Result<IdeFileSnapshot, IdeError> {
        let normalized = normalize_source_path(path)?;
        let source_path = self.resolve_source_path(&normalized)?;
        let disk_content = std::fs::read_to_string(&source_path)
            .map_err(|_| IdeError::new(IdeErrorKind::NotFound, "source file not found"))?;
        if disk_content.len() > self.limits.max_file_bytes {
            return Err(IdeError::new(
                IdeErrorKind::TooLarge,
                format!(
                    "source file exceeds limit ({} > {} bytes)",
                    disk_content.len(),
                    self.limits.max_file_bytes
                ),
            ));
        }

        let now = (self.now)();
        let mut guard = self
            .inner
            .lock()
            .map_err(|_| IdeError::new(IdeErrorKind::Internal, "session state lock poisoned"))?;
        let role = {
            let session = self.ensure_session(&mut guard, session_token, now)?;
            session.open_paths.insert(normalized.clone());
            session.role
        };

        let entry = guard
            .documents
            .entry(normalized.clone())
            .or_insert_with(|| IdeDocumentEntry {
                content: disk_content.clone(),
                version: 1,
                opened_by: BTreeSet::new(),
            });
        if entry.content != disk_content {
            entry.content = disk_content.clone();
            entry.version = entry.version.saturating_add(1);
        }
        entry.opened_by.insert(session_token.to_string());

        Ok(IdeFileSnapshot {
            path: normalized,
            content: disk_content,
            version: entry.version,
            read_only: !matches!(role, IdeRole::Editor),
        })
    }

    pub fn apply_source(
        &self,
        session_token: &str,
        path: &str,
        expected_version: u64,
        content: String,
        write_enabled: bool,
    ) -> Result<IdeWriteResult, IdeError> {
        if !write_enabled {
            return Err(IdeError::new(
                IdeErrorKind::Forbidden,
                "web IDE authoring is disabled in current runtime mode",
            ));
        }
        if content.len() > self.limits.max_file_bytes {
            return Err(IdeError::new(
                IdeErrorKind::TooLarge,
                format!(
                    "payload exceeds limit ({} > {} bytes)",
                    content.len(),
                    self.limits.max_file_bytes
                ),
            ));
        }

        let normalized = normalize_source_path(path)?;
        let source_path = self.resolve_source_path(&normalized)?;
        let disk_content = std::fs::read_to_string(&source_path)
            .map_err(|_| IdeError::new(IdeErrorKind::NotFound, "source file not found"))?;

        let now = (self.now)();
        let mut guard = self
            .inner
            .lock()
            .map_err(|_| IdeError::new(IdeErrorKind::Internal, "session state lock poisoned"))?;
        let role = {
            let session = self.ensure_session(&mut guard, session_token, now)?;
            session.open_paths.insert(normalized.clone());
            session.role
        };
        if !matches!(role, IdeRole::Editor) {
            return Err(IdeError::new(
                IdeErrorKind::Forbidden,
                "session role does not allow edits",
            ));
        }

        let entry = guard
            .documents
            .entry(normalized.clone())
            .or_insert_with(|| IdeDocumentEntry {
                content: disk_content.clone(),
                version: 1,
                opened_by: BTreeSet::new(),
            });
        if entry.content != disk_content {
            entry.content = disk_content;
            entry.version = entry.version.saturating_add(1);
        }
        if entry.version != expected_version {
            return Err(IdeError::conflict(entry.version));
        }

        std::fs::write(&source_path, &content)
            .map_err(|err| IdeError::new(IdeErrorKind::Internal, format!("write failed: {err}")))?;

        entry.content = content;
        entry.version = entry.version.saturating_add(1);
        entry.opened_by.insert(session_token.to_string());

        Ok(IdeWriteResult {
            path: normalized,
            version: entry.version,
        })
    }

    fn ensure_session<'a>(
        &self,
        guard: &'a mut IdeStateInner,
        session_token: &str,
        now: u64,
    ) -> Result<&'a mut IdeSessionEntry, IdeError> {
        prune_expired(guard, now);
        guard
            .sessions
            .get_mut(session_token)
            .ok_or_else(|| IdeError::new(IdeErrorKind::Unauthorized, "invalid or expired session"))
    }

    fn resolve_source_path(&self, normalized: &str) -> Result<PathBuf, IdeError> {
        let root = self.sources_root()?;
        let joined = root.join(normalized);
        let canonical_root = root.canonicalize().unwrap_or(root.clone());
        let canonical_parent = joined
            .parent()
            .ok_or_else(|| IdeError::new(IdeErrorKind::InvalidInput, "invalid source path"))?
            .canonicalize()
            .map_err(|_| IdeError::new(IdeErrorKind::NotFound, "source folder not found"))?;
        if !canonical_parent.starts_with(&canonical_root) {
            return Err(IdeError::new(
                IdeErrorKind::Forbidden,
                "source path escapes project source root",
            ));
        }
        Ok(joined)
    }

    fn sources_root(&self) -> Result<PathBuf, IdeError> {
        let Some(root) = self.project_root.as_ref() else {
            return Err(IdeError::new(
                IdeErrorKind::NotFound,
                "project root unavailable for web IDE",
            ));
        };
        let sources = root.join("src");
        if !sources.is_dir() {
            return Err(IdeError::new(
                IdeErrorKind::NotFound,
                "project src directory is missing",
            ));
        }
        Ok(sources)
    }
}

fn normalize_source_path(path: &str) -> Result<String, IdeError> {
    let trimmed = path.trim();
    if trimmed.is_empty() {
        return Err(IdeError::new(
            IdeErrorKind::InvalidInput,
            "source path is required",
        ));
    }

    let raw = Path::new(trimmed);
    if raw.is_absolute() {
        return Err(IdeError::new(
            IdeErrorKind::Forbidden,
            "absolute source paths are not allowed",
        ));
    }

    let mut parts = Vec::new();
    for component in raw.components() {
        match component {
            Component::Normal(value) => {
                let text = value.to_string_lossy();
                if text.starts_with('.') {
                    return Err(IdeError::new(
                        IdeErrorKind::Forbidden,
                        "hidden source paths are not allowed",
                    ));
                }
                parts.push(text.to_string());
            }
            Component::CurDir => {}
            Component::ParentDir | Component::RootDir | Component::Prefix(_) => {
                return Err(IdeError::new(
                    IdeErrorKind::Forbidden,
                    "source path escapes project source root",
                ));
            }
        }
    }

    if parts.is_empty() {
        return Err(IdeError::new(
            IdeErrorKind::InvalidInput,
            "source path is required",
        ));
    }

    let normalized = parts.join("/");
    if !normalized.to_ascii_lowercase().ends_with(".st") {
        return Err(IdeError::new(
            IdeErrorKind::InvalidInput,
            "only .st files are allowed",
        ));
    }
    Ok(normalized)
}

fn collect_source_files(
    root: &Path,
    relative: &Path,
    out: &mut Vec<String>,
) -> Result<(), IdeError> {
    let dir = root.join(relative);
    let entries = std::fs::read_dir(&dir)
        .map_err(|err| IdeError::new(IdeErrorKind::Internal, format!("read_dir failed: {err}")))?;
    for entry in entries.flatten() {
        let path = entry.path();
        let file_name = entry.file_name();
        let file_name = file_name.to_string_lossy();
        if file_name.starts_with('.') {
            continue;
        }
        let next_relative = if relative.as_os_str().is_empty() {
            PathBuf::from(file_name.as_ref())
        } else {
            relative.join(file_name.as_ref())
        };
        if path.is_dir() {
            collect_source_files(root, &next_relative, out)?;
            continue;
        }
        if path
            .extension()
            .and_then(|ext| ext.to_str())
            .is_some_and(|ext| ext.eq_ignore_ascii_case("st"))
        {
            out.push(next_relative.to_string_lossy().replace('\\', "/"));
        }
    }
    Ok(())
}

fn prune_expired(state: &mut IdeStateInner, now: u64) {
    let expired = state
        .sessions
        .iter()
        .filter_map(|(token, session)| {
            if session.expires_at <= now {
                Some(token.clone())
            } else {
                None
            }
        })
        .collect::<Vec<_>>();
    if expired.is_empty() {
        return;
    }

    for token in &expired {
        state.sessions.remove(token);
    }
    for doc in state.documents.values_mut() {
        for token in &expired {
            doc.opened_by.remove(token);
        }
    }
}

fn generate_token() -> String {
    let mut bytes = [0_u8; 32];
    OsRng.fill_bytes(&mut bytes);
    URL_SAFE_NO_PAD.encode(bytes)
}

fn now_secs() -> u64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap_or_default()
        .as_secs()
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::sync::atomic::{AtomicU64, Ordering};
    use std::time::{Duration, Instant};

    fn project_dir(name: &str) -> PathBuf {
        let path = std::env::temp_dir().join(format!(
            "trust-runtime-web-ide-{name}-{}",
            std::process::id()
        ));
        let _ = std::fs::remove_dir_all(&path);
        std::fs::create_dir_all(path.join("src")).expect("create src");
        path
    }

    fn write_source(project: &Path, rel: &str, content: &str) {
        let path = project.join("src").join(rel);
        if let Some(parent) = path.parent() {
            std::fs::create_dir_all(parent).expect("create source parent");
        }
        std::fs::write(path, content).expect("write source");
    }

    #[test]
    fn auth_and_session_lifecycle_contract() {
        let project = project_dir("session");
        write_source(&project, "main.st", "PROGRAM Main\nEND_PROGRAM\n");

        let clock = Arc::new(AtomicU64::new(10_000));
        let state = WebIdeState::with_clock(
            Some(project.clone()),
            Arc::new({
                let clock = clock.clone();
                move || clock.load(Ordering::SeqCst)
            }),
        );

        let err = state.list_sources("missing").expect_err("missing session");
        assert_eq!(err.kind(), IdeErrorKind::Unauthorized);

        let session = state
            .create_session(IdeRole::Viewer)
            .expect("create viewer session");
        let files = state
            .list_sources(&session.token)
            .expect("list files with session");
        assert_eq!(files, vec!["main.st".to_string()]);

        clock.store(10_000 + SESSION_TTL_SECS + 1, Ordering::SeqCst);
        let expired_err = state
            .open_source(&session.token, "main.st")
            .expect_err("session should be expired");
        assert_eq!(expired_err.kind(), IdeErrorKind::Unauthorized);

        let _ = std::fs::remove_dir_all(project);
    }

    #[test]
    fn collaborative_conflict_detected_with_expected_version() {
        let project = project_dir("conflict");
        write_source(&project, "main.st", "PROGRAM Main\nEND_PROGRAM\n");

        let state = WebIdeState::new(Some(project.clone()));
        let s1 = state
            .create_session(IdeRole::Editor)
            .expect("create editor session 1");
        let s2 = state
            .create_session(IdeRole::Editor)
            .expect("create editor session 2");

        let doc1 = state
            .open_source(&s1.token, "main.st")
            .expect("open from s1");
        let doc2 = state
            .open_source(&s2.token, "main.st")
            .expect("open from s2");
        assert_eq!(doc1.version, doc2.version);

        let write1 = state
            .apply_source(
                &s1.token,
                "main.st",
                doc1.version,
                "PROGRAM Main\nVAR\nA : INT;\nEND_VAR\nEND_PROGRAM\n".to_string(),
                true,
            )
            .expect("apply first edit");
        assert!(write1.version > doc1.version);

        let conflict = state
            .apply_source(
                &s2.token,
                "main.st",
                doc2.version,
                "PROGRAM Main\nVAR\nB : INT;\nEND_VAR\nEND_PROGRAM\n".to_string(),
                true,
            )
            .expect_err("stale write must conflict");
        assert_eq!(conflict.kind(), IdeErrorKind::Conflict);
        assert_eq!(conflict.current_version(), Some(write1.version));

        let _ = std::fs::remove_dir_all(project);
    }

    #[test]
    fn latency_and_resource_budgets_are_enforced() {
        let project = project_dir("budget");
        write_source(&project, "main.st", "PROGRAM Main\nEND_PROGRAM\n");

        let state = WebIdeState::new(Some(project.clone()));
        let session = state
            .create_session(IdeRole::Editor)
            .expect("create editor session");
        let mut snapshot = state
            .open_source(&session.token, "main.st")
            .expect("open source");

        let runs = 80_u32;
        let mut total = Duration::ZERO;
        let mut max = Duration::ZERO;

        for idx in 0..runs {
            let started = Instant::now();
            let result = state
                .apply_source(
                    &session.token,
                    "main.st",
                    snapshot.version,
                    format!(
                        "PROGRAM Main\\nVAR\\nA : INT := {};\\nEND_VAR\\nEND_PROGRAM\\n",
                        idx
                    ),
                    true,
                )
                .expect("apply edit within budget");
            let elapsed = started.elapsed();
            total += elapsed;
            max = max.max(elapsed);
            snapshot.version = result.version;
        }

        let avg = total / runs;
        assert!(
            max < Duration::from_millis(250),
            "max apply latency {:?} exceeded budget",
            max
        );
        assert!(
            avg < Duration::from_millis(40),
            "avg apply latency {:?} exceeded budget",
            avg
        );

        let too_large = "X".repeat(MAX_FILE_BYTES + 1);
        let err = state
            .apply_source(&session.token, "main.st", snapshot.version, too_large, true)
            .expect_err("oversized payload should fail");
        assert_eq!(err.kind(), IdeErrorKind::TooLarge);

        let _ = std::fs::remove_dir_all(project);
    }
}
