//! Config domain models for trust-lsp.

use std::collections::HashMap;
use std::path::PathBuf;
use tower_lsp::lsp_types::DiagnosticSeverity;

/// Project configuration loaded from `trust-lsp.toml`.
#[derive(Debug, Clone)]
pub struct ProjectConfig {
    /// Root directory for the workspace.
    pub root: PathBuf,
    /// Config file path (if found).
    pub config_path: Option<PathBuf>,
    /// Extra include paths to index.
    pub include_paths: Vec<PathBuf>,
    /// Vendor profile hint (e.g., codesys, twincat, mitsubishi/gxworks3).
    pub vendor_profile: Option<String>,
    /// Standard library selection settings.
    pub stdlib: StdlibSettings,
    /// External libraries to index.
    pub libraries: Vec<LibrarySpec>,
    /// Local package dependencies declared in `[dependencies]`.
    pub dependencies: Vec<ProjectDependency>,
    /// Resolver issues produced while expanding local dependencies.
    pub dependency_resolution_issues: Vec<DependencyResolutionIssue>,
    /// External diagnostics sources (custom linters).
    pub diagnostic_external_paths: Vec<PathBuf>,
    /// Build configuration (compile flags, target profile).
    pub build: BuildConfig,
    /// Target profiles for build configuration.
    pub targets: Vec<TargetProfile>,
    /// Indexing budget options.
    pub indexing: IndexingConfig,
    /// Diagnostics configuration.
    pub diagnostics: DiagnosticSettings,
    /// Runtime control configuration for debug-assisted features.
    pub runtime: RuntimeConfig,
    /// Workspace federation settings.
    pub workspace: WorkspaceSettings,
    /// Telemetry configuration (opt-in).
    pub telemetry: TelemetryConfig,
}
impl ProjectConfig {
    pub fn indexing_roots(&self) -> Vec<PathBuf> {
        let mut roots = Vec::new();
        // `include_paths` is meant to scope what the indexer walks — when set,
        // it replaces the workspace root rather than extending it. Otherwise a
        // user with `include_paths = ["src"]` still gets the entire workspace
        // root scanned, which on a multi-project tree (e.g. trust-platform/
        // hosting many examples/) leaks cross-project symbols and produces
        // spurious E104 duplicate-import diagnostics. Falling back to the root
        // when `include_paths` is empty preserves the unconfigured-project case.
        if self.include_paths.is_empty() {
            roots.push(self.root.clone());
        } else {
            roots.extend(self.include_paths.iter().cloned());
        }
        for lib in &self.libraries {
            roots.push(lib.path.clone());
        }
        roots
    }

    /// Returns the resolved index cache directory (if enabled).
    pub fn index_cache_dir(&self) -> Option<PathBuf> {
        if !self.indexing.cache_enabled {
            return None;
        }
        let dir = self
            .indexing
            .cache_dir
            .clone()
            .unwrap_or_else(|| PathBuf::from(".trust-lsp/index-cache"));
        Some(super::resolve_path(
            &self.root,
            dir.to_string_lossy().as_ref(),
        ))
    }
}

/// Standard library selection settings.
#[derive(Debug, Clone, Default)]
pub struct StdlibSettings {
    /// Named profile (e.g., "iec", "full", "none").
    pub profile: Option<String>,
    /// Allow list of function/FB names (case-insensitive).
    pub allow: Option<Vec<String>>,
}

#[derive(Debug, Clone)]
pub struct IndexingConfig {
    /// Optional maximum number of files to index.
    pub max_files: Option<usize>,
    /// Optional maximum duration (ms) for indexing.
    pub max_ms: Option<u64>,
    /// Whether persistent index caching is enabled.
    pub cache_enabled: bool,
    /// Optional cache directory override.
    pub cache_dir: Option<PathBuf>,
    /// Optional memory budget for indexed (closed) documents, in MB.
    pub memory_budget_mb: Option<usize>,
    /// Target percent of the budget to evict down to (0-100).
    pub evict_to_percent: u8,
    /// Throttle delay (ms) when idle.
    pub throttle_idle_ms: u64,
    /// Throttle delay (ms) when recent editor activity is detected.
    pub throttle_active_ms: u64,
    /// Maximum throttle delay (ms).
    pub throttle_max_ms: u64,
    /// Activity window (ms) that triggers active throttling.
    pub throttle_active_window_ms: u64,
}

impl Default for IndexingConfig {
    fn default() -> Self {
        Self {
            max_files: None,
            max_ms: None,
            cache_enabled: true,
            cache_dir: None,
            memory_budget_mb: None,
            evict_to_percent: 80,
            throttle_idle_ms: 0,
            throttle_active_ms: 8,
            throttle_max_ms: 50,
            throttle_active_window_ms: 250,
        }
    }
}

#[derive(Debug, Clone)]
pub struct DiagnosticSettings {
    /// Toggle unused variable/parameter warnings (W001/W002).
    pub warn_unused: bool,
    /// Toggle unreachable code warnings (W003).
    pub warn_unreachable: bool,
    /// Toggle missing ELSE warnings for CASE (W004).
    pub warn_missing_else: bool,
    /// Toggle implicit conversion warnings (W005).
    pub warn_implicit_conversion: bool,
    /// Toggle shadowed variable warnings (W006).
    pub warn_shadowed: bool,
    /// Toggle deprecated feature warnings (W007).
    pub warn_deprecated: bool,
    /// Toggle cyclomatic complexity warnings (W008).
    pub warn_complexity: bool,
    /// Toggle non-determinism warnings (W010/W011).
    pub warn_nondeterminism: bool,
    /// Toggle numeric hazard warnings (W013/W014).
    pub warn_numeric_hazards: bool,
    /// Per-code severity overrides (e.g., W010 -> error).
    pub severity_overrides: HashMap<String, DiagnosticSeverity>,
}

impl Default for DiagnosticSettings {
    fn default() -> Self {
        Self {
            warn_unused: true,
            warn_unreachable: true,
            warn_missing_else: true,
            warn_implicit_conversion: true,
            warn_shadowed: true,
            warn_deprecated: true,
            warn_complexity: true,
            warn_nondeterminism: true,
            warn_numeric_hazards: true,
            severity_overrides: HashMap::new(),
        }
    }
}
impl DiagnosticSettings {
    pub(crate) fn enable_all_warnings(&mut self) {
        self.warn_unused = true;
        self.warn_unreachable = true;
        self.warn_missing_else = true;
        self.warn_implicit_conversion = true;
        self.warn_shadowed = true;
        self.warn_deprecated = true;
        self.warn_complexity = true;
        self.warn_nondeterminism = true;
        self.warn_numeric_hazards = true;
    }
}
/// Runtime control settings for inline values/debug integration.
#[derive(Debug, Clone, Default)]
pub struct RuntimeConfig {
    /// Control endpoint (e.g., unix:///tmp/trust-runtime.sock, tcp://127.0.0.1:9000).
    pub control_endpoint: Option<String>,
    /// Optional control auth token.
    pub control_auth_token: Option<String>,
}

/// Build configuration for project compilation.
#[derive(Debug, Clone)]
pub struct BuildConfig {
    /// Optional target name to select.
    pub target: Option<String>,
    /// Optional profile (e.g., debug/release).
    pub profile: Option<String>,
    /// Additional compile flags.
    pub flags: Vec<String>,
    /// Preprocessor/define flags.
    pub defines: Vec<String>,
    /// Dependency resolver runs in offline mode (no fetch/clone).
    pub dependencies_offline: bool,
    /// Dependency resolver requires locked/pinned revisions.
    pub dependencies_locked: bool,
    /// Lock file path used for dependency pinning snapshots.
    pub dependency_lockfile: PathBuf,
}

impl Default for BuildConfig {
    fn default() -> Self {
        Self {
            target: None,
            profile: None,
            flags: Vec::new(),
            defines: Vec::new(),
            dependencies_offline: false,
            dependencies_locked: false,
            dependency_lockfile: PathBuf::from("trust-lsp.lock"),
        }
    }
}

/// Target-specific build configuration.
#[derive(Debug, Clone)]
pub struct TargetProfile {
    pub name: String,
    pub profile: Option<String>,
    pub flags: Vec<String>,
    pub defines: Vec<String>,
}

#[derive(Debug, Clone)]
pub struct LibrarySpec {
    pub name: String,
    pub path: PathBuf,
    pub version: Option<String>,
    pub dependencies: Vec<LibraryDependency>,
    pub docs: Vec<PathBuf>,
}

/// Workspace visibility for multi-root symbol federation.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum WorkspaceVisibility {
    #[default]
    Public,
    Private,
    Hidden,
}

impl WorkspaceVisibility {
    pub(crate) fn from_str(value: &str) -> Self {
        match value.trim().to_ascii_lowercase().as_str() {
            "private" => WorkspaceVisibility::Private,
            "hidden" => WorkspaceVisibility::Hidden,
            "public" => WorkspaceVisibility::Public,
            _ => WorkspaceVisibility::Public,
        }
    }

    pub fn allows_query(self, query_empty: bool) -> bool {
        match self {
            WorkspaceVisibility::Public => true,
            WorkspaceVisibility::Private => !query_empty,
            WorkspaceVisibility::Hidden => false,
        }
    }
}

/// Workspace federation settings.
#[derive(Debug, Clone)]
pub struct WorkspaceSettings {
    pub priority: i32,
    pub visibility: WorkspaceVisibility,
}

impl Default for WorkspaceSettings {
    fn default() -> Self {
        Self {
            priority: 0,
            visibility: WorkspaceVisibility::Public,
        }
    }
}
/// Telemetry configuration (opt-in).
#[derive(Debug, Clone)]
pub struct TelemetryConfig {
    pub enabled: bool,
    pub path: Option<PathBuf>,
    pub flush_every: usize,
}

impl Default for TelemetryConfig {
    fn default() -> Self {
        Self {
            enabled: false,
            path: None,
            flush_every: 25,
        }
    }
}
#[derive(Debug, Clone)]
pub struct LibraryDependency {
    pub name: String,
    pub version: Option<String>,
}

#[derive(Debug, Clone)]
pub struct GitDependency {
    pub url: String,
    pub rev: Option<String>,
    pub tag: Option<String>,
    pub branch: Option<String>,
}

/// A local package dependency declared in `[dependencies]`.
#[derive(Debug, Clone)]
pub struct ProjectDependency {
    pub name: String,
    pub path: Option<PathBuf>,
    pub git: Option<GitDependency>,
    pub version: Option<String>,
}

/// Dependency resolver issue surfaced as a config diagnostic.
#[derive(Debug, Clone)]
pub struct DependencyResolutionIssue {
    pub code: &'static str,
    pub dependency: String,
    pub message: String,
}

#[derive(Debug, Clone, Default)]
pub struct DependencyPolicy {
    pub allowed_git_hosts: Vec<String>,
    pub allow_http: bool,
    pub allow_ssh: bool,
}
