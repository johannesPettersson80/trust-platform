//! Workspace/project configuration for trust-lsp.

use serde::{Deserialize, Serialize};
use std::collections::{BTreeMap, HashMap};
use std::hash::{Hash, Hasher};
use std::path::{Path, PathBuf};
use std::process::Command;
use tower_lsp::lsp_types::DiagnosticSeverity;
use tracing::warn;

pub(crate) const CONFIG_FILES: &[&str] = &["trust-lsp.toml", ".trust-lsp.toml", "trustlsp.toml"];

/// Project configuration loaded from `trust-lsp.toml`.
#[derive(Debug, Clone)]
pub struct ProjectConfig {
    /// Root directory for the workspace.
    pub root: PathBuf,
    /// Config file path (if found).
    pub config_path: Option<PathBuf>,
    /// Extra include paths to index.
    pub include_paths: Vec<PathBuf>,
    /// Vendor profile hint (e.g., codesys, twincat).
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
    /// Load configuration for a workspace root.
    pub fn load(root: &Path) -> Self {
        let config_path = find_config_file(root);
        let Some(path) = config_path.clone() else {
            return ProjectConfig::base(root, None);
        };
        let Ok(contents) = std::fs::read_to_string(&path) else {
            warn!("Failed to read trust-lsp config at {}", path.display());
            return ProjectConfig::base(root, config_path);
        };
        ProjectConfig::from_contents(root, config_path, &contents)
    }

    pub fn from_contents(root: &Path, config_path: Option<PathBuf>, contents: &str) -> Self {
        let mut config = ProjectConfig::base(root, config_path);
        let parsed: ConfigFile = match toml::from_str(contents) {
            Ok(parsed) => parsed,
            Err(err) => {
                if let Some(path) = &config.config_path {
                    warn!(
                        "Failed to parse trust-lsp config at {}: {err}",
                        path.display()
                    );
                } else {
                    warn!("Failed to parse trust-lsp config: {err}");
                }
                return config;
            }
        };

        config.vendor_profile = parsed.project.vendor_profile;
        config.stdlib = parsed.project.stdlib.into();
        config.build = parsed.build.into();
        config.targets = parsed
            .targets
            .into_iter()
            .map(TargetProfile::from)
            .collect();
        config.indexing = parsed.indexing.into();
        let diagnostics_section = parsed.diagnostics;
        config.diagnostic_external_paths = resolve_paths(root, &diagnostics_section.external_paths);
        config.diagnostics =
            DiagnosticSettings::from_config(config.vendor_profile.as_deref(), diagnostics_section);
        config.runtime = parsed.runtime.into();
        config.workspace = WorkspaceSettings::from(parsed.workspace);
        config.telemetry = TelemetryConfig::from_section(root, parsed.telemetry);

        let mut include_paths = resolve_paths(root, &parsed.project.include_paths);
        config.include_paths.append(&mut include_paths);

        let mut libraries = Vec::new();
        for path in resolve_paths(root, &parsed.project.library_paths) {
            let name = path
                .file_name()
                .and_then(|name| name.to_str())
                .unwrap_or("library")
                .to_string();
            libraries.push(LibrarySpec {
                name,
                path,
                version: None,
                dependencies: Vec::new(),
                docs: Vec::new(),
            });
        }
        for lib in parsed.libraries {
            let path = resolve_path(root, &lib.path);
            let name = lib
                .name
                .clone()
                .or_else(|| {
                    path.file_name()
                        .and_then(|name| name.to_str())
                        .map(|name| name.to_string())
                })
                .unwrap_or_else(|| "library".to_string());
            let dependencies = lib
                .dependencies
                .into_iter()
                .map(LibraryDependency::from)
                .collect();
            let docs = resolve_paths(root, &lib.docs);
            libraries.push(LibrarySpec {
                name,
                path,
                version: lib.version,
                dependencies,
                docs,
            });
        }
        let policy = DependencyPolicy::from(parsed.dependency_policy);
        let (dependencies, mut dependency_resolution_issues) =
            parse_project_dependencies(root, &parsed.dependencies);
        let (dependency_libraries, mut resolver_issues) =
            resolve_manifest_dependencies(root, &dependencies, &config.build, &policy);
        dependency_resolution_issues.append(&mut resolver_issues);
        libraries.extend(dependency_libraries);
        config.libraries = libraries;
        config.dependencies = dependencies;
        config.dependency_resolution_issues = dependency_resolution_issues;

        config
    }

    /// Returns all indexing roots (workspace + include + libraries).
    pub fn indexing_roots(&self) -> Vec<PathBuf> {
        let mut roots = Vec::new();
        roots.push(self.root.clone());
        roots.extend(self.include_paths.iter().cloned());
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
        Some(resolve_path(&self.root, dir.to_string_lossy().as_ref()))
    }
}

impl ProjectConfig {
    fn base(root: &Path, config_path: Option<PathBuf>) -> Self {
        ProjectConfig {
            root: root.to_path_buf(),
            config_path,
            include_paths: Vec::new(),
            vendor_profile: None,
            stdlib: StdlibSettings::default(),
            libraries: Vec::new(),
            dependencies: Vec::new(),
            dependency_resolution_issues: Vec::new(),
            diagnostic_external_paths: Vec::new(),
            build: BuildConfig::default(),
            targets: Vec::new(),
            indexing: IndexingConfig::default(),
            diagnostics: DiagnosticSettings::default(),
            runtime: RuntimeConfig::default(),
            workspace: WorkspaceSettings::default(),
            telemetry: TelemetryConfig::default(),
        }
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
            severity_overrides: HashMap::new(),
        }
    }
}

impl DiagnosticSettings {
    fn from_config(profile: Option<&str>, section: DiagnosticSection) -> Self {
        let mut settings = DiagnosticSettings::default();
        if let Some(profile) = profile {
            match profile.trim().to_ascii_lowercase().as_str() {
                "siemens" => {
                    settings.warn_missing_else = false;
                    settings.warn_implicit_conversion = false;
                }
                "codesys" => {
                    settings.warn_unused = true;
                    settings.warn_unreachable = true;
                    settings.warn_missing_else = true;
                    settings.warn_implicit_conversion = true;
                    settings.warn_shadowed = true;
                    settings.warn_deprecated = true;
                }
                "beckhoff" | "twincat" => {
                    settings.warn_unused = true;
                    settings.warn_unreachable = true;
                    settings.warn_missing_else = true;
                    settings.warn_implicit_conversion = true;
                    settings.warn_shadowed = true;
                    settings.warn_deprecated = true;
                }
                _ => {}
            }
        }

        if let Some(rule_pack) = section.rule_pack.as_deref() {
            apply_rule_pack(&mut settings, rule_pack);
        }

        if let Some(value) = section.warn_unused {
            settings.warn_unused = value;
        }
        if let Some(value) = section.warn_unreachable {
            settings.warn_unreachable = value;
        }
        if let Some(value) = section.warn_missing_else {
            settings.warn_missing_else = value;
        }
        if let Some(value) = section.warn_implicit_conversion {
            settings.warn_implicit_conversion = value;
        }
        if let Some(value) = section.warn_shadowed {
            settings.warn_shadowed = value;
        }
        if let Some(value) = section.warn_deprecated {
            settings.warn_deprecated = value;
        }
        if let Some(value) = section.warn_complexity {
            settings.warn_complexity = value;
        }
        if let Some(value) = section.warn_nondeterminism {
            settings.warn_nondeterminism = value;
        }

        apply_severity_overrides(&mut settings, section.severity_overrides);
        settings
    }
}

impl DiagnosticSettings {
    fn enable_all_warnings(&mut self) {
        self.warn_unused = true;
        self.warn_unreachable = true;
        self.warn_missing_else = true;
        self.warn_implicit_conversion = true;
        self.warn_shadowed = true;
        self.warn_deprecated = true;
        self.warn_complexity = true;
        self.warn_nondeterminism = true;
    }
}

fn apply_rule_pack(settings: &mut DiagnosticSettings, pack: &str) {
    let pack = pack.trim().to_ascii_lowercase();
    match pack.as_str() {
        "iec-safety" | "safety" => {
            settings.enable_all_warnings();
            apply_safety_overrides(settings);
        }
        "siemens-safety" => {
            settings.enable_all_warnings();
            settings.warn_missing_else = false;
            settings.warn_implicit_conversion = false;
            apply_safety_overrides(settings);
        }
        "codesys-safety" | "beckhoff-safety" | "twincat-safety" => {
            settings.enable_all_warnings();
            apply_safety_overrides(settings);
        }
        _ => {}
    }
}

fn apply_safety_overrides(settings: &mut DiagnosticSettings) {
    let overrides = [
        ("W004", DiagnosticSeverity::ERROR),
        ("W005", DiagnosticSeverity::ERROR),
        ("W010", DiagnosticSeverity::ERROR),
        ("W011", DiagnosticSeverity::ERROR),
    ];
    for (code, severity) in overrides {
        settings
            .severity_overrides
            .insert(code.to_string(), severity);
    }
}

fn apply_severity_overrides(settings: &mut DiagnosticSettings, overrides: HashMap<String, String>) {
    for (code, severity) in overrides {
        if let Some(parsed) = parse_severity(&severity) {
            settings.severity_overrides.insert(code, parsed);
        }
    }
}

fn parse_severity(value: &str) -> Option<DiagnosticSeverity> {
    match value.trim().to_ascii_lowercase().as_str() {
        "error" | "err" => Some(DiagnosticSeverity::ERROR),
        "warning" | "warn" => Some(DiagnosticSeverity::WARNING),
        "info" | "information" => Some(DiagnosticSeverity::INFORMATION),
        "hint" => Some(DiagnosticSeverity::HINT),
        _ => None,
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
    fn from_str(value: &str) -> Self {
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

impl From<WorkspaceSection> for WorkspaceSettings {
    fn from(section: WorkspaceSection) -> Self {
        let mut settings = WorkspaceSettings::default();
        if let Some(priority) = section.priority {
            settings.priority = priority;
        }
        if let Some(visibility) = section.visibility {
            settings.visibility = WorkspaceVisibility::from_str(&visibility);
        }
        settings
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

impl TelemetryConfig {
    fn from_section(root: &Path, section: TelemetrySection) -> Self {
        let enabled = section.enabled.unwrap_or(false);
        let path = section.path.map(|path| resolve_path(root, &path));
        let path = if enabled {
            Some(path.unwrap_or_else(|| resolve_path(root, ".trust-lsp/telemetry.jsonl")))
        } else {
            path
        };
        TelemetryConfig {
            enabled,
            path,
            flush_every: section.flush_every.unwrap_or(25),
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

#[derive(Debug, Deserialize)]
struct ConfigFile {
    #[serde(default)]
    dependencies: BTreeMap<String, ManifestDependencyEntry>,
    #[serde(default)]
    dependency_policy: DependencyPolicySection,
    #[serde(default)]
    project: ProjectSection,
    #[serde(default)]
    workspace: WorkspaceSection,
    #[serde(default)]
    build: BuildSection,
    #[serde(default)]
    targets: Vec<TargetSection>,
    #[serde(default)]
    indexing: IndexingSection,
    #[serde(default)]
    diagnostics: DiagnosticSection,
    #[serde(default)]
    libraries: Vec<LibrarySection>,
    #[serde(default)]
    runtime: RuntimeSection,
    #[serde(default)]
    telemetry: TelemetrySection,
}

#[derive(Debug, Default, Deserialize)]
struct ProjectSection {
    #[serde(default)]
    include_paths: Vec<String>,
    #[serde(default)]
    library_paths: Vec<String>,
    #[serde(default)]
    stdlib: StdlibSelection,
    vendor_profile: Option<String>,
}

#[derive(Debug, Default, Deserialize)]
struct WorkspaceSection {
    priority: Option<i32>,
    visibility: Option<String>,
}

#[derive(Debug, Default, Deserialize)]
struct IndexingSection {
    max_files: Option<usize>,
    max_ms: Option<u64>,
    cache: Option<bool>,
    cache_dir: Option<String>,
    memory_budget_mb: Option<usize>,
    evict_to_percent: Option<u8>,
    throttle_idle_ms: Option<u64>,
    throttle_active_ms: Option<u64>,
    throttle_max_ms: Option<u64>,
    throttle_active_window_ms: Option<u64>,
}

#[derive(Debug, Default, Deserialize)]
struct DiagnosticSection {
    rule_pack: Option<String>,
    warn_unused: Option<bool>,
    warn_unreachable: Option<bool>,
    warn_missing_else: Option<bool>,
    warn_implicit_conversion: Option<bool>,
    warn_shadowed: Option<bool>,
    warn_deprecated: Option<bool>,
    warn_complexity: Option<bool>,
    warn_nondeterminism: Option<bool>,
    #[serde(default)]
    external_paths: Vec<String>,
    #[serde(default)]
    severity_overrides: HashMap<String, String>,
}

#[derive(Debug, Default, Deserialize)]
struct RuntimeSection {
    control_endpoint: Option<String>,
    control_auth_token: Option<String>,
}

#[derive(Debug, Default, Deserialize)]
struct TelemetrySection {
    enabled: Option<bool>,
    path: Option<String>,
    flush_every: Option<usize>,
}

#[derive(Debug, Default, Deserialize)]
struct DependencyPolicySection {
    #[serde(default)]
    allowed_git_hosts: Vec<String>,
    allow_http: Option<bool>,
    allow_ssh: Option<bool>,
}

#[derive(Debug, Default, Deserialize)]
struct BuildSection {
    target: Option<String>,
    profile: Option<String>,
    #[serde(default)]
    flags: Vec<String>,
    #[serde(default)]
    defines: Vec<String>,
    dependencies_offline: Option<bool>,
    dependencies_locked: Option<bool>,
    dependency_lockfile: Option<String>,
}

#[derive(Debug, Default, Deserialize)]
struct TargetSection {
    name: String,
    profile: Option<String>,
    #[serde(default)]
    flags: Vec<String>,
    #[serde(default)]
    defines: Vec<String>,
}

#[derive(Debug, Deserialize)]
struct LibrarySection {
    name: Option<String>,
    path: String,
    version: Option<String>,
    #[serde(default)]
    dependencies: Vec<LibraryDependencyEntry>,
    #[serde(default)]
    docs: Vec<String>,
}

#[derive(Debug, Deserialize)]
#[serde(untagged)]
enum LibraryDependencyEntry {
    Name(String),
    Detailed(LibraryDependencySection),
}

#[derive(Debug, Deserialize)]
struct LibraryDependencySection {
    name: String,
    version: Option<String>,
}

#[derive(Debug, Default, Deserialize)]
struct PackageSection {
    version: Option<String>,
}

#[derive(Debug, Deserialize)]
#[serde(untagged)]
enum ManifestDependencyEntry {
    Path(String),
    Detailed(ManifestDependencySection),
}

#[derive(Debug, Deserialize)]
struct ManifestDependencySection {
    path: Option<String>,
    git: Option<String>,
    version: Option<String>,
    rev: Option<String>,
    tag: Option<String>,
    branch: Option<String>,
}

#[derive(Debug, Default, Deserialize)]
struct DependencyManifestFile {
    #[serde(default)]
    package: PackageSection,
    #[serde(default)]
    dependencies: BTreeMap<String, ManifestDependencyEntry>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(tag = "source", rename_all = "snake_case")]
enum DependencyLockEntry {
    Path { path: String },
    Git { url: String, rev: String },
}

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
struct DependencyLockFile {
    #[serde(default = "dependency_lock_version")]
    version: u32,
    #[serde(default)]
    dependencies: BTreeMap<String, DependencyLockEntry>,
}

#[derive(Debug, Clone)]
struct ResolvedGitDependency {
    path: PathBuf,
    rev: String,
}

#[derive(Debug, Clone)]
enum RevisionSelector {
    Rev(String),
    Tag(String),
    Branch(String),
    DefaultHead,
}

#[derive(Debug, Clone, Deserialize)]
#[serde(untagged)]
enum StdlibSelection {
    Profile(String),
    Allow(Vec<String>),
}

impl Default for StdlibSelection {
    fn default() -> Self {
        StdlibSelection::Profile("full".to_string())
    }
}

impl From<StdlibSelection> for StdlibSettings {
    fn from(selection: StdlibSelection) -> Self {
        match selection {
            StdlibSelection::Allow(list) => StdlibSettings {
                profile: None,
                allow: Some(list),
            },
            StdlibSelection::Profile(profile) => {
                let normalized = profile.to_ascii_lowercase();
                if normalized == "none" {
                    StdlibSettings {
                        profile: Some(profile),
                        allow: Some(Vec::new()),
                    }
                } else {
                    StdlibSettings {
                        profile: Some(profile),
                        allow: None,
                    }
                }
            }
        }
    }
}

impl From<RuntimeSection> for RuntimeConfig {
    fn from(section: RuntimeSection) -> Self {
        RuntimeConfig {
            control_endpoint: section.control_endpoint,
            control_auth_token: section.control_auth_token,
        }
    }
}

impl From<BuildSection> for BuildConfig {
    fn from(section: BuildSection) -> Self {
        BuildConfig {
            target: section.target,
            profile: section.profile,
            flags: section.flags,
            defines: section.defines,
            dependencies_offline: section.dependencies_offline.unwrap_or(false),
            dependencies_locked: section.dependencies_locked.unwrap_or(false),
            dependency_lockfile: section
                .dependency_lockfile
                .map(PathBuf::from)
                .unwrap_or_else(|| PathBuf::from("trust-lsp.lock")),
        }
    }
}

impl From<DependencyPolicySection> for DependencyPolicy {
    fn from(section: DependencyPolicySection) -> Self {
        DependencyPolicy {
            allowed_git_hosts: section
                .allowed_git_hosts
                .into_iter()
                .map(|host| host.trim().to_ascii_lowercase())
                .filter(|host| !host.is_empty())
                .collect(),
            allow_http: section.allow_http.unwrap_or(false),
            allow_ssh: section.allow_ssh.unwrap_or(false),
        }
    }
}

impl From<TargetSection> for TargetProfile {
    fn from(section: TargetSection) -> Self {
        TargetProfile {
            name: section.name,
            profile: section.profile,
            flags: section.flags,
            defines: section.defines,
        }
    }
}

impl From<LibraryDependencyEntry> for LibraryDependency {
    fn from(entry: LibraryDependencyEntry) -> Self {
        match entry {
            LibraryDependencyEntry::Name(name) => {
                let mut parts = name.splitn(2, '@');
                let base = parts.next().unwrap_or("").to_string();
                let version = parts.next().map(|part| part.trim().to_string());
                LibraryDependency {
                    name: base,
                    version: version.filter(|value| !value.is_empty()),
                }
            }
            LibraryDependencyEntry::Detailed(section) => LibraryDependency {
                name: section.name,
                version: section.version,
            },
        }
    }
}

impl From<IndexingSection> for IndexingConfig {
    fn from(section: IndexingSection) -> Self {
        IndexingConfig {
            max_files: section.max_files,
            max_ms: section.max_ms,
            cache_enabled: section.cache.unwrap_or(true),
            cache_dir: section.cache_dir.map(PathBuf::from),
            memory_budget_mb: section.memory_budget_mb,
            evict_to_percent: section.evict_to_percent.unwrap_or(80),
            throttle_idle_ms: section.throttle_idle_ms.unwrap_or(0),
            throttle_active_ms: section.throttle_active_ms.unwrap_or(8),
            throttle_max_ms: section.throttle_max_ms.unwrap_or(50),
            throttle_active_window_ms: section.throttle_active_window_ms.unwrap_or(250),
        }
    }
}

pub(crate) fn find_config_file(root: &Path) -> Option<PathBuf> {
    CONFIG_FILES
        .iter()
        .map(|name| root.join(name))
        .find(|path| path.is_file())
}

fn resolve_paths(root: &Path, entries: &[String]) -> Vec<PathBuf> {
    entries
        .iter()
        .map(|entry| resolve_path(root, entry))
        .collect()
}

fn resolve_path(root: &Path, entry: &str) -> PathBuf {
    let path = PathBuf::from(entry);
    if path.is_absolute() {
        path
    } else {
        root.join(path)
    }
}

fn parse_project_dependencies(
    root: &Path,
    entries: &BTreeMap<String, ManifestDependencyEntry>,
) -> (Vec<ProjectDependency>, Vec<DependencyResolutionIssue>) {
    let mut dependencies = Vec::new();
    let mut issues = Vec::new();
    for (name, entry) in entries {
        match parse_project_dependency(root, name, entry) {
            Ok(dependency) => dependencies.push(dependency),
            Err(message) => issues.push(DependencyResolutionIssue {
                code: "L005",
                dependency: name.clone(),
                message,
            }),
        }
    }
    (dependencies, issues)
}

fn resolve_manifest_dependencies(
    root: &Path,
    dependencies: &[ProjectDependency],
    build: &BuildConfig,
    policy: &DependencyPolicy,
) -> (Vec<LibrarySpec>, Vec<DependencyResolutionIssue>) {
    let mut issues = Vec::new();
    let lock_path = dependency_lock_path(root, build);
    let lock = match load_dependency_lock(&lock_path) {
        Ok(lock) => lock,
        Err(message) => {
            issues.push(DependencyResolutionIssue {
                code: "L006",
                dependency: "lockfile".to_string(),
                message,
            });
            DependencyLockFile::default()
        }
    };

    let mut resolver = DependencyResolver::new(root, build, policy, &lock, issues);
    resolver.resolve_all(dependencies);
    let (libraries, mut issues, resolved_lock) = resolver.finish();

    if issues.is_empty() && !build.dependencies_locked && !resolved_lock.is_empty() {
        if let Err(message) = write_dependency_lock(&lock_path, resolved_lock) {
            issues.push(DependencyResolutionIssue {
                code: "L006",
                dependency: "lockfile".to_string(),
                message,
            });
        }
    }

    (libraries.into_values().collect(), issues)
}

struct DependencyResolver<'a> {
    root: &'a Path,
    build: &'a BuildConfig,
    policy: &'a DependencyPolicy,
    lock: &'a DependencyLockFile,
    states: HashMap<String, DependencyVisitState>,
    libraries: BTreeMap<String, LibrarySpec>,
    issues: Vec<DependencyResolutionIssue>,
    resolved_lock: BTreeMap<String, DependencyLockEntry>,
}

impl<'a> DependencyResolver<'a> {
    fn new(
        root: &'a Path,
        build: &'a BuildConfig,
        policy: &'a DependencyPolicy,
        lock: &'a DependencyLockFile,
        issues: Vec<DependencyResolutionIssue>,
    ) -> Self {
        Self {
            root,
            build,
            policy,
            lock,
            states: HashMap::new(),
            libraries: BTreeMap::new(),
            issues,
            resolved_lock: BTreeMap::new(),
        }
    }

    fn resolve_all(&mut self, dependencies: &[ProjectDependency]) {
        for dependency in dependencies {
            self.resolve_dependency_recursive(dependency);
        }
    }

    fn finish(
        self,
    ) -> (
        BTreeMap<String, LibrarySpec>,
        Vec<DependencyResolutionIssue>,
        BTreeMap<String, DependencyLockEntry>,
    ) {
        (self.libraries, self.issues, self.resolved_lock)
    }

    fn resolve_dependency_recursive(&mut self, dependency: &ProjectDependency) {
        let path = match resolve_dependency_source(
            self.root,
            self.build,
            self.policy,
            self.lock,
            dependency,
            &mut self.resolved_lock,
        ) {
            Ok(path) => path,
            Err(issue) => {
                self.issues.push(issue);
                return;
            }
        };
        if !path.is_dir() {
            self.issues.push(DependencyResolutionIssue {
                code: "L001",
                dependency: dependency.name.clone(),
                message: format!(
                    "Dependency '{}' path does not exist: {}",
                    dependency.name,
                    path.display()
                ),
            });
            return;
        }

        if let Some(existing) = self.libraries.get(&dependency.name) {
            if let Some(required) = dependency.version.as_deref() {
                if existing.version.as_deref() != Some(required) {
                    let available = existing.version.as_deref().unwrap_or("unspecified");
                    self.issues.push(DependencyResolutionIssue {
                        code: "L002",
                        dependency: dependency.name.clone(),
                        message: format!(
                            "Dependency '{}' requested version {}, but resolved version is {}",
                            dependency.name, required, available
                        ),
                    });
                }
            }
            return;
        }

        if self
            .states
            .get(dependency.name.as_str())
            .copied()
            .is_some_and(|state| state == DependencyVisitState::Visiting)
        {
            return;
        }

        self.states
            .insert(dependency.name.clone(), DependencyVisitState::Visiting);

        let (package, nested_dependencies) = match load_dependency_manifest(&path) {
            Ok(manifest) => {
                let (nested, mut parse_issues) =
                    parse_project_dependencies(&path, &manifest.dependencies);
                self.issues.append(&mut parse_issues);
                (manifest.package, nested)
            }
            Err(message) => {
                self.issues.push(DependencyResolutionIssue {
                    code: "L001",
                    dependency: dependency.name.clone(),
                    message,
                });
                (PackageSection::default(), Vec::new())
            }
        };

        if let Some(required) = dependency.version.as_deref() {
            if package.version.as_deref() != Some(required) {
                let available = package.version.as_deref().unwrap_or("unspecified");
                self.issues.push(DependencyResolutionIssue {
                    code: "L002",
                    dependency: dependency.name.clone(),
                    message: format!(
                        "Dependency '{}' requested version {}, but resolved package version is {}",
                        dependency.name, required, available
                    ),
                });
            }
        }

        let mut library_dependencies = Vec::new();
        for nested in &nested_dependencies {
            library_dependencies.push(LibraryDependency {
                name: nested.name.clone(),
                version: nested.version.clone(),
            });
            self.resolve_dependency_recursive(nested);
        }

        self.libraries.insert(
            dependency.name.clone(),
            LibrarySpec {
                name: dependency.name.clone(),
                path,
                version: package.version,
                dependencies: library_dependencies,
                docs: Vec::new(),
            },
        );
        self.states
            .insert(dependency.name.clone(), DependencyVisitState::Done);
    }
}

fn parse_project_dependency(
    root: &Path,
    name: &str,
    entry: &ManifestDependencyEntry,
) -> Result<ProjectDependency, String> {
    match entry {
        ManifestDependencyEntry::Path(path) => Ok(ProjectDependency {
            name: name.to_string(),
            path: Some(resolve_path(root, path)),
            git: None,
            version: None,
        }),
        ManifestDependencyEntry::Detailed(section) => {
            let has_path = section
                .path
                .as_ref()
                .is_some_and(|path| !path.trim().is_empty());
            let has_git = section
                .git
                .as_ref()
                .is_some_and(|git| !git.trim().is_empty());

            if has_path == has_git {
                return Err(format!(
                    "Dependency '{name}' must set exactly one of `path` or `git`"
                ));
            }

            let selector_count = usize::from(section.rev.is_some())
                + usize::from(section.tag.is_some())
                + usize::from(section.branch.is_some());
            if selector_count > 1 {
                return Err(format!(
                    "Dependency '{name}' may set only one of `rev`, `tag`, or `branch`"
                ));
            }

            if has_path {
                if section.rev.is_some() || section.tag.is_some() || section.branch.is_some() {
                    return Err(format!(
                        "Dependency '{name}' path entries do not support `rev`, `tag`, or `branch`"
                    ));
                }
                let path = section.path.as_deref().unwrap_or_default();
                return Ok(ProjectDependency {
                    name: name.to_string(),
                    path: Some(resolve_path(root, path)),
                    git: None,
                    version: section.version.clone(),
                });
            }

            Ok(ProjectDependency {
                name: name.to_string(),
                path: None,
                git: Some(GitDependency {
                    url: section.git.clone().unwrap_or_default(),
                    rev: section.rev.clone(),
                    tag: section.tag.clone(),
                    branch: section.branch.clone(),
                }),
                version: section.version.clone(),
            })
        }
    }
}

fn resolve_dependency_source(
    root: &Path,
    build: &BuildConfig,
    policy: &DependencyPolicy,
    lock: &DependencyLockFile,
    dependency: &ProjectDependency,
    resolved_lock: &mut BTreeMap<String, DependencyLockEntry>,
) -> Result<PathBuf, DependencyResolutionIssue> {
    if let Some(path) = dependency.path.as_ref() {
        let resolved = canonicalize_or_self(path);
        resolved_lock.insert(
            dependency.name.clone(),
            DependencyLockEntry::Path {
                path: resolved.to_string_lossy().into_owned(),
            },
        );
        return Ok(resolved);
    }

    let Some(git) = dependency.git.as_ref() else {
        return Err(DependencyResolutionIssue {
            code: "L005",
            dependency: dependency.name.clone(),
            message: format!("Dependency '{}' has no source", dependency.name),
        });
    };

    let resolved = resolve_git_dependency(root, build, policy, lock, &dependency.name, git)?;
    resolved_lock.insert(
        dependency.name.clone(),
        DependencyLockEntry::Git {
            url: git.url.clone(),
            rev: resolved.rev.clone(),
        },
    );
    Ok(resolved.path)
}

fn resolve_git_dependency(
    root: &Path,
    build: &BuildConfig,
    policy: &DependencyPolicy,
    lock: &DependencyLockFile,
    dependency_name: &str,
    git: &GitDependency,
) -> Result<ResolvedGitDependency, DependencyResolutionIssue> {
    if let Err(message) = validate_git_source_policy(git.url.as_str(), policy) {
        return Err(DependencyResolutionIssue {
            code: "L005",
            dependency: dependency_name.to_string(),
            message: format!("Dependency '{dependency_name}' rejected by trust policy: {message}"),
        });
    }

    let lock_entry = lock.dependencies.get(dependency_name);
    let selector = match (git.rev.as_ref(), git.tag.as_ref(), git.branch.as_ref()) {
        (Some(rev), None, None) => RevisionSelector::Rev(rev.clone()),
        (None, Some(tag), None) => RevisionSelector::Tag(tag.clone()),
        (None, None, Some(branch)) => RevisionSelector::Branch(branch.clone()),
        (None, None, None) => {
            if build.dependencies_locked {
                match lock_entry {
                    Some(DependencyLockEntry::Git { url, rev }) if *url == git.url => {
                        RevisionSelector::Rev(rev.clone())
                    }
                    Some(DependencyLockEntry::Git { .. }) => {
                        return Err(DependencyResolutionIssue {
                            code: "L006",
                            dependency: dependency_name.to_string(),
                            message: format!(
                                "Dependency '{dependency_name}' lock entry URL mismatch for locked resolution"
                            ),
                        });
                    }
                    _ => {
                        return Err(DependencyResolutionIssue {
                            code: "L006",
                            dependency: dependency_name.to_string(),
                            message: format!(
                                "Dependency '{dependency_name}' requires `rev`/`tag`/`branch` or lock entry in locked mode"
                            ),
                        });
                    }
                }
            } else if let Some(DependencyLockEntry::Git { url, rev }) = lock_entry {
                if *url == git.url {
                    RevisionSelector::Rev(rev.clone())
                } else {
                    RevisionSelector::DefaultHead
                }
            } else {
                RevisionSelector::DefaultHead
            }
        }
        _ => {
            return Err(DependencyResolutionIssue {
                code: "L005",
                dependency: dependency_name.to_string(),
                message: format!(
                    "Dependency '{dependency_name}' may set only one of `rev`, `tag`, or `branch`"
                ),
            });
        }
    };

    let repo_root = root.join(".trust-lsp").join("deps").join("git");
    let repo_dir = repo_root.join(format!(
        "{}-{}",
        sanitize_for_path(dependency_name),
        stable_hash_hex(git.url.as_str())
    ));

    if !repo_dir.is_dir() {
        if build.dependencies_offline {
            return Err(DependencyResolutionIssue {
                code: "L007",
                dependency: dependency_name.to_string(),
                message: format!(
                    "Dependency '{dependency_name}' is not available in offline mode (missing cache at {})",
                    repo_dir.display()
                ),
            });
        }
        std::fs::create_dir_all(&repo_root).map_err(|err| DependencyResolutionIssue {
            code: "L001",
            dependency: dependency_name.to_string(),
            message: format!(
                "Dependency '{dependency_name}' failed to create git cache root: {err}"
            ),
        })?;
        run_git_command(
            None,
            &[
                "clone",
                "--no-checkout",
                git.url.as_str(),
                repo_dir.to_string_lossy().as_ref(),
            ],
        )
        .map_err(|message| DependencyResolutionIssue {
            code: "L001",
            dependency: dependency_name.to_string(),
            message: format!("Dependency '{dependency_name}' clone failed: {message}"),
        })?;
    } else if !build.dependencies_offline {
        run_git_command(Some(&repo_dir), &["fetch", "--tags", "--prune", "origin"]).map_err(
            |message| DependencyResolutionIssue {
                code: "L001",
                dependency: dependency_name.to_string(),
                message: format!("Dependency '{dependency_name}' fetch failed: {message}"),
            },
        )?;
    }

    let resolved_rev = resolve_git_revision(&repo_dir, &selector).ok_or_else(|| {
        let detail = match selector {
            RevisionSelector::Rev(rev) => format!("rev {rev}"),
            RevisionSelector::Tag(tag) => format!("tag {tag}"),
            RevisionSelector::Branch(branch) => format!("branch {branch}"),
            RevisionSelector::DefaultHead => "default HEAD".to_string(),
        };
        let code = if build.dependencies_offline {
            "L007"
        } else {
            "L001"
        };
        DependencyResolutionIssue {
            code,
            dependency: dependency_name.to_string(),
            message: format!(
                "Dependency '{dependency_name}' could not resolve git {detail} in {}",
                repo_dir.display()
            ),
        }
    })?;

    run_git_command(
        Some(&repo_dir),
        &["checkout", "--detach", "--force", resolved_rev.as_str()],
    )
    .map_err(|message| DependencyResolutionIssue {
        code: "L001",
        dependency: dependency_name.to_string(),
        message: format!("Dependency '{dependency_name}' checkout failed: {message}"),
    })?;

    Ok(ResolvedGitDependency {
        path: repo_dir,
        rev: resolved_rev,
    })
}

fn validate_git_source_policy(url: &str, policy: &DependencyPolicy) -> Result<(), String> {
    let trimmed = url.trim();
    if trimmed.is_empty() {
        return Err("git URL is empty".to_string());
    }

    if is_local_git_source(trimmed) || trimmed.starts_with("file://") {
        return Ok(());
    }

    if let Some(authority) = trimmed.strip_prefix("https://") {
        let host =
            extract_git_host(authority).ok_or_else(|| "failed to parse HTTPS host".to_string())?;
        return validate_git_host(host.as_str(), policy);
    }

    if let Some(authority) = trimmed.strip_prefix("http://") {
        if !policy.allow_http {
            return Err("HTTP git sources are disabled".to_string());
        }
        let host =
            extract_git_host(authority).ok_or_else(|| "failed to parse HTTP host".to_string())?;
        return validate_git_host(host.as_str(), policy);
    }

    if let Some(authority) = trimmed.strip_prefix("ssh://") {
        if !policy.allow_ssh {
            return Err("SSH git sources are disabled".to_string());
        }
        let host =
            extract_git_host(authority).ok_or_else(|| "failed to parse SSH host".to_string())?;
        return validate_git_host(host.as_str(), policy);
    }

    if looks_like_scp_git_source(trimmed) {
        if !policy.allow_ssh {
            return Err("SSH git sources are disabled".to_string());
        }
        let host = trimmed
            .split_once('@')
            .and_then(|(_, right)| right.split_once(':').map(|(host, _)| host.to_string()))
            .ok_or_else(|| "failed to parse SCP-style SSH host".to_string())?;
        return validate_git_host(host.as_str(), policy);
    }

    Err("unsupported git source scheme".to_string())
}

fn validate_git_host(host: &str, policy: &DependencyPolicy) -> Result<(), String> {
    let host = host.trim().trim_start_matches('[').trim_end_matches(']');
    if host.is_empty() {
        return Err("git host is empty".to_string());
    }
    if policy.allowed_git_hosts.is_empty() {
        return Ok(());
    }
    let host_lower = host.to_ascii_lowercase();
    if policy.allowed_git_hosts.iter().any(|allowed| {
        host_lower == *allowed || host_lower.ends_with(format!(".{allowed}").as_str())
    }) {
        Ok(())
    } else {
        Err(format!(
            "host '{host}' is not in dependency_policy.allowed_git_hosts"
        ))
    }
}

fn extract_git_host(authority_and_path: &str) -> Option<String> {
    let authority = authority_and_path.split('/').next()?;
    let authority = authority
        .split_once('@')
        .map_or(authority, |(_, value)| value);
    if authority.starts_with('[') {
        return authority
            .split_once(']')
            .map(|(host, _)| host.trim_start_matches('[').to_string());
    }
    let host = authority.split(':').next()?.trim();
    if host.is_empty() {
        None
    } else {
        Some(host.to_string())
    }
}

fn is_local_git_source(source: &str) -> bool {
    source.starts_with("./")
        || source.starts_with("../")
        || source.starts_with('/')
        || source
            .chars()
            .nth(1)
            .is_some_and(|second| second == ':' && source.chars().next().is_some())
}

fn looks_like_scp_git_source(source: &str) -> bool {
    source.contains('@') && source.contains(':') && !source.contains("://")
}

fn resolve_git_revision(repo: &Path, selector: &RevisionSelector) -> Option<String> {
    match selector {
        RevisionSelector::Rev(rev) => rev_parse_commit(repo, rev.as_str()),
        RevisionSelector::Tag(tag) => rev_parse_commit(repo, format!("refs/tags/{tag}").as_str())
            .or_else(|| rev_parse_commit(repo, tag.as_str())),
        RevisionSelector::Branch(branch) => {
            rev_parse_commit(repo, format!("refs/remotes/origin/{branch}").as_str())
                .or_else(|| rev_parse_commit(repo, format!("refs/heads/{branch}").as_str()))
                .or_else(|| rev_parse_commit(repo, branch.as_str()))
        }
        RevisionSelector::DefaultHead => rev_parse_commit(repo, "refs/remotes/origin/HEAD")
            .or_else(|| rev_parse_commit(repo, "origin/HEAD"))
            .or_else(|| rev_parse_commit(repo, "HEAD")),
    }
}

fn rev_parse_commit(repo: &Path, reference: &str) -> Option<String> {
    run_git_command(
        Some(repo),
        &[
            "rev-parse",
            "--verify",
            format!("{reference}^{{commit}}").as_str(),
        ],
    )
    .ok()
}

fn run_git_command(cwd: Option<&Path>, args: &[&str]) -> Result<String, String> {
    let mut command = Command::new("git");
    command.args(args);
    if let Some(dir) = cwd {
        command.current_dir(dir);
    }
    let output = command
        .output()
        .map_err(|err| format!("failed to execute git: {err}"))?;
    if output.status.success() {
        Ok(String::from_utf8_lossy(&output.stdout).trim().to_string())
    } else {
        let stderr = String::from_utf8_lossy(&output.stderr);
        let stdout = String::from_utf8_lossy(&output.stdout);
        let detail = if stderr.trim().is_empty() {
            stdout.trim()
        } else {
            stderr.trim()
        };
        Err(format!("git {}: {detail}", args.join(" ")))
    }
}

fn dependency_lock_path(root: &Path, build: &BuildConfig) -> PathBuf {
    if build.dependency_lockfile.is_absolute() {
        build.dependency_lockfile.clone()
    } else {
        root.join(&build.dependency_lockfile)
    }
}

fn load_dependency_lock(path: &Path) -> Result<DependencyLockFile, String> {
    if !path.is_file() {
        return Ok(DependencyLockFile::default());
    }
    let content = std::fs::read_to_string(path).map_err(|err| {
        format!(
            "failed to read dependency lock file {}: {err}",
            path.display()
        )
    })?;
    toml::from_str(&content).map_err(|err| {
        format!(
            "failed to parse dependency lock file {}: {err}",
            path.display()
        )
    })
}

fn write_dependency_lock(
    path: &Path,
    dependencies: BTreeMap<String, DependencyLockEntry>,
) -> Result<(), String> {
    let lock = DependencyLockFile {
        version: dependency_lock_version(),
        dependencies,
    };
    let content = toml::to_string_pretty(&lock)
        .map_err(|err| format!("failed to encode dependency lock file: {err}"))?;
    if let Some(parent) = path.parent() {
        std::fs::create_dir_all(parent).map_err(|err| {
            format!(
                "failed to create dependency lock parent {}: {err}",
                parent.display()
            )
        })?;
    }
    std::fs::write(path, content).map_err(|err| {
        format!(
            "failed to write dependency lock file {}: {err}",
            path.display()
        )
    })
}

fn dependency_lock_version() -> u32 {
    1
}

fn sanitize_for_path(value: &str) -> String {
    let mut out = String::with_capacity(value.len());
    for ch in value.chars() {
        if ch.is_ascii_alphanumeric() {
            out.push(ch.to_ascii_lowercase());
        } else {
            out.push('-');
        }
    }
    out.trim_matches('-').to_string()
}

fn stable_hash_hex(value: &str) -> String {
    let mut hasher = std::collections::hash_map::DefaultHasher::new();
    value.hash(&mut hasher);
    format!("{:016x}", hasher.finish())
}

fn load_dependency_manifest(path: &Path) -> Result<DependencyManifestFile, String> {
    let Some(config_path) = find_config_file(path) else {
        return Ok(DependencyManifestFile::default());
    };
    let contents = std::fs::read_to_string(&config_path).map_err(|err| {
        format!(
            "Failed to read dependency manifest for '{}': {} ({err})",
            path.display(),
            config_path.display()
        )
    })?;
    toml::from_str(&contents).map_err(|err| {
        format!(
            "Failed to parse dependency manifest for '{}': {err}",
            path.display()
        )
    })
}

fn canonicalize_or_self(path: &Path) -> PathBuf {
    path.canonicalize().unwrap_or_else(|_| path.to_path_buf())
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum DependencyVisitState {
    Visiting,
    Done,
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::fs;
    use std::process::Command;
    use std::time::{SystemTime, UNIX_EPOCH};

    fn temp_dir(prefix: &str) -> PathBuf {
        let stamp = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap()
            .as_millis();
        let dir = std::env::temp_dir().join(format!("{prefix}-{stamp}"));
        fs::create_dir_all(&dir).expect("create temp dir");
        dir
    }

    fn git(cwd: &Path, args: &[&str]) -> String {
        let output = Command::new("git")
            .current_dir(cwd)
            .args(args)
            .output()
            .expect("execute git command");
        assert!(
            output.status.success(),
            "git {} failed: {}",
            args.join(" "),
            String::from_utf8_lossy(&output.stderr)
        );
        String::from_utf8_lossy(&output.stdout).trim().to_string()
    }

    fn init_dependency_repo(path: &Path) -> (String, String) {
        fs::create_dir_all(path).expect("create dependency repo");
        git(path, &["init"]);
        git(path, &["config", "user.email", "test@example.com"]);
        git(path, &["config", "user.name", "trust-lsp test"]);
        fs::write(
            path.join("trust-lsp.toml"),
            r#"
[package]
version = "1.0.0"
"#,
        )
        .expect("write initial manifest");
        git(path, &["add", "."]);
        git(path, &["commit", "-m", "initial"]);
        let rev_v1 = git(path, &["rev-parse", "HEAD"]);
        git(path, &["tag", "v1"]);
        git(path, &["branch", "stable"]);

        fs::write(
            path.join("trust-lsp.toml"),
            r#"
[package]
version = "2.0.0"
"#,
        )
        .expect("write updated manifest");
        git(path, &["add", "."]);
        git(path, &["commit", "-m", "update"]);
        let rev_v2 = git(path, &["rev-parse", "HEAD"]);
        (rev_v1, rev_v2)
    }

    #[test]
    fn loads_project_config_with_includes_and_libraries() {
        let root = temp_dir("trustlsp-config");
        let config_path = root.join("trust-lsp.toml");
        fs::write(
            &config_path,
            r#"
[project]
vendor_profile = "codesys"
include_paths = ["src"]
library_paths = ["libs"]
stdlib = ["ABS", "CTU"]

[indexing]
max_files = 25
max_ms = 100
cache = false
cache_dir = ".trust-lsp/custom-cache"
memory_budget_mb = 64
evict_to_percent = 75
throttle_idle_ms = 2
throttle_active_ms = 10
throttle_max_ms = 40
throttle_active_window_ms = 200

[build]
target = "x86_64"
profile = "release"
flags = ["-O2", "-Wall"]
defines = ["SIM=1"]

[workspace]
priority = 10
visibility = "private"

[telemetry]
enabled = true
path = ".trust-lsp/telemetry.jsonl"
flush_every = 5

[[targets]]
name = "sim"
profile = "debug"
flags = ["-g"]
defines = ["SIM=1", "TRACE=1"]

[diagnostics]
warn_unused = false
warn_missing_else = false
rule_pack = "iec-safety"
severity_overrides = { W003 = "error" }
external_paths = ["lint.json"]

[[libraries]]
name = "VendorLib"
path = "vendor"
version = "1.2.3"
dependencies = [{ name = "Core", version = "2.0" }, { name = "Utils" }]
docs = ["docs/vendor.md"]
"#,
        )
        .expect("write config");

        let config = ProjectConfig::load(&root);
        assert_eq!(config.vendor_profile.as_deref(), Some("codesys"));
        assert_eq!(config.stdlib.allow.as_ref().unwrap().len(), 2);
        assert_eq!(config.indexing.max_files, Some(25));
        assert_eq!(config.indexing.max_ms, Some(100));
        assert!(!config.indexing.cache_enabled);
        assert!(config
            .indexing
            .cache_dir
            .as_ref()
            .is_some_and(|dir| dir.ends_with("custom-cache")));
        assert_eq!(config.indexing.memory_budget_mb, Some(64));
        assert_eq!(config.indexing.evict_to_percent, 75);
        assert_eq!(config.indexing.throttle_idle_ms, 2);
        assert_eq!(config.indexing.throttle_active_ms, 10);
        assert_eq!(config.indexing.throttle_max_ms, 40);
        assert_eq!(config.indexing.throttle_active_window_ms, 200);
        assert_eq!(config.build.target.as_deref(), Some("x86_64"));
        assert_eq!(config.build.profile.as_deref(), Some("release"));
        assert!(config.build.flags.contains(&"-O2".to_string()));
        assert!(config.build.defines.contains(&"SIM=1".to_string()));
        assert_eq!(config.targets.len(), 1);
        assert_eq!(config.targets[0].name, "sim");
        assert_eq!(config.targets[0].profile.as_deref(), Some("debug"));
        assert!(config.targets[0].flags.contains(&"-g".to_string()));
        assert!(config.targets[0].defines.contains(&"TRACE=1".to_string()));
        assert!(!config.diagnostics.warn_unused);
        assert!(!config.diagnostics.warn_missing_else);
        assert_eq!(config.workspace.priority, 10);
        assert_eq!(config.workspace.visibility, WorkspaceVisibility::Private);
        assert!(config
            .telemetry
            .path
            .as_ref()
            .is_some_and(|path| path.ends_with(".trust-lsp/telemetry.jsonl")));
        assert!(config.telemetry.enabled);
        assert_eq!(config.telemetry.flush_every, 5);
        assert_eq!(
            config.diagnostics.severity_overrides.get("W003").copied(),
            Some(DiagnosticSeverity::ERROR)
        );
        assert!(config.diagnostics.severity_overrides.contains_key("W010"));
        assert!(config.include_paths.iter().any(|p| p.ends_with("src")));
        let lib = config
            .libraries
            .iter()
            .find(|lib| lib.name == "VendorLib")
            .expect("vendor lib");
        assert_eq!(lib.version.as_deref(), Some("1.2.3"));
        assert!(lib
            .dependencies
            .iter()
            .any(|dep| dep.name == "Core" && dep.version.as_deref() == Some("2.0")));
        assert!(lib
            .dependencies
            .iter()
            .any(|dep| dep.name == "Utils" && dep.version.is_none()));
        assert!(lib.docs.iter().any(|doc| doc.ends_with("vendor.md")));
        assert!(config
            .diagnostic_external_paths
            .iter()
            .any(|path| path.ends_with("lint.json")));

        fs::remove_dir_all(root).ok();
    }

    #[test]
    fn vendor_profile_applies_diagnostic_defaults() {
        let root = temp_dir("trustlsp-config-diagnostics");
        let config_path = root.join("trust-lsp.toml");
        fs::write(
            &config_path,
            r#"
[project]
vendor_profile = "siemens"
"#,
        )
        .expect("write config");

        let config = ProjectConfig::load(&root);
        assert!(!config.diagnostics.warn_missing_else);
        assert!(!config.diagnostics.warn_implicit_conversion);

        fs::remove_dir_all(root).ok();
    }

    #[test]
    fn resolves_local_dependencies_transitively() {
        let root = temp_dir("trustlsp-config-dependencies");
        let root_config = root.join("trust-lsp.toml");
        let dep_a = root.join("deps").join("lib-a");
        let dep_b = root.join("deps").join("lib-b");
        fs::create_dir_all(&dep_a).expect("create dep a");
        fs::create_dir_all(&dep_b).expect("create dep b");
        fs::write(
            &root_config,
            r#"
[project]
include_paths = ["src"]

[dependencies]
LibA = { path = "deps/lib-a", version = "1.0.0" }
"#,
        )
        .expect("write root config");
        fs::write(
            dep_a.join("trust-lsp.toml"),
            r#"
[package]
version = "1.0.0"

[dependencies]
LibB = { path = "../lib-b", version = "2.0.0" }
"#,
        )
        .expect("write dep a manifest");
        fs::write(
            dep_b.join("trust-lsp.toml"),
            r#"
[package]
version = "2.0.0"
"#,
        )
        .expect("write dep b manifest");

        let config = ProjectConfig::load(&root);
        assert_eq!(config.dependencies.len(), 1);
        assert!(config.dependencies.iter().any(|dep| dep.name == "LibA"));
        assert!(config.libraries.iter().any(|lib| lib.name == "LibA"));
        assert!(config.libraries.iter().any(|lib| lib.name == "LibB"));
        assert!(config
            .indexing_roots()
            .iter()
            .any(|path| path.ends_with("lib-a")));
        assert!(config
            .indexing_roots()
            .iter()
            .any(|path| path.ends_with("lib-b")));
        assert!(config.dependency_resolution_issues.is_empty());

        fs::remove_dir_all(root).ok();
    }

    #[test]
    fn reports_dependency_missing_path_and_version_mismatch() {
        let root = temp_dir("trustlsp-config-dependency-issues");
        let root_config = root.join("trust-lsp.toml");
        let dep = root.join("deps").join("versioned");
        fs::create_dir_all(&dep).expect("create dependency dir");
        fs::write(
            dep.join("trust-lsp.toml"),
            r#"
[package]
version = "2.0.0"
"#,
        )
        .expect("write dependency manifest");
        fs::write(
            &root_config,
            r#"
[dependencies]
Missing = "deps/missing"
Versioned = { path = "deps/versioned", version = "1.0.0" }
"#,
        )
        .expect("write config");

        let config = ProjectConfig::load(&root);
        assert!(config
            .dependency_resolution_issues
            .iter()
            .any(|issue| issue.code == "L001" && issue.dependency == "Missing"));
        assert!(config
            .dependency_resolution_issues
            .iter()
            .any(|issue| issue.code == "L002" && issue.dependency == "Versioned"));

        fs::remove_dir_all(root).ok();
    }

    #[test]
    fn resolves_git_dependencies_with_rev_tag_and_branch_pinning() {
        let root = temp_dir("trustlsp-config-git-pins");
        let repo = root.join("repos/vendor");
        let (rev_v1, _rev_v2) = init_dependency_repo(&repo);

        fs::write(
            root.join("trust-lsp.toml"),
            format!(
                r#"
[dependencies]
ByRev = {{ git = "{repo}", rev = "{rev}" }}
ByTag = {{ git = "{repo}", tag = "v1" }}
ByBranch = {{ git = "{repo}", branch = "stable" }}
"#,
                repo = repo.to_string_lossy(),
                rev = rev_v1
            ),
        )
        .expect("write root config");

        let config = ProjectConfig::load(&root);
        assert!(config.dependency_resolution_issues.is_empty());
        let by_rev = config
            .libraries
            .iter()
            .find(|lib| lib.name == "ByRev")
            .expect("ByRev library");
        let by_tag = config
            .libraries
            .iter()
            .find(|lib| lib.name == "ByTag")
            .expect("ByTag library");
        let by_branch = config
            .libraries
            .iter()
            .find(|lib| lib.name == "ByBranch")
            .expect("ByBranch library");

        assert_eq!(by_rev.version.as_deref(), Some("1.0.0"));
        assert_eq!(by_tag.version.as_deref(), Some("1.0.0"));
        assert_eq!(by_branch.version.as_deref(), Some("1.0.0"));
        assert!(root.join("trust-lsp.lock").is_file());

        fs::remove_dir_all(root).ok();
    }

    #[test]
    fn locked_mode_requires_pin_or_lock_entry_for_git_dependencies() {
        let root = temp_dir("trustlsp-config-git-locked");
        let repo = root.join("repos/vendor");
        let _ = init_dependency_repo(&repo);

        fs::write(
            root.join("trust-lsp.toml"),
            format!(
                r#"
[build]
dependencies_locked = true

[dependencies]
Floating = {{ git = "{repo}" }}
"#,
                repo = repo.to_string_lossy()
            ),
        )
        .expect("write root config");

        let config = ProjectConfig::load(&root);
        assert!(config
            .dependency_resolution_issues
            .iter()
            .any(|issue| issue.code == "L006" && issue.dependency == "Floating"));

        fs::remove_dir_all(root).ok();
    }

    #[test]
    fn offline_locked_mode_uses_cached_lock_resolution() {
        let root = temp_dir("trustlsp-config-git-offline");
        let repo = root.join("repos/vendor");
        let _ = init_dependency_repo(&repo);

        let initial_config = format!(
            r#"
[dependencies]
Floating = {{ git = "{repo}" }}
"#,
            repo = repo.to_string_lossy()
        );
        fs::write(root.join("trust-lsp.toml"), initial_config).expect("write initial config");
        let first = ProjectConfig::load(&root);
        assert!(
            first.dependency_resolution_issues.is_empty(),
            "initial resolve should succeed"
        );
        assert!(root.join("trust-lsp.lock").is_file());

        fs::write(
            root.join("trust-lsp.toml"),
            format!(
                r#"
[build]
dependencies_locked = true
dependencies_offline = true

[dependencies]
Floating = {{ git = "{repo}" }}
"#,
                repo = repo.to_string_lossy()
            ),
        )
        .expect("write offline config");

        let offline = ProjectConfig::load(&root);
        assert!(
            offline.dependency_resolution_issues.is_empty(),
            "offline locked resolve should reuse lock/cache"
        );
        assert!(offline.libraries.iter().any(|lib| lib.name == "Floating"));

        fs::remove_dir_all(root).ok();
    }

    #[test]
    fn enforces_git_host_allowlist_policy() {
        let root = temp_dir("trustlsp-config-policy");
        fs::write(
            root.join("trust-lsp.toml"),
            r#"
[dependency_policy]
allowed_git_hosts = ["git.example.internal"]

[dependencies]
Vendor = { git = "https://github.com/example/vendor.git", rev = "deadbeef" }
"#,
        )
        .expect("write policy config");

        let config = ProjectConfig::load(&root);
        assert!(config
            .dependency_resolution_issues
            .iter()
            .any(|issue| issue.code == "L005" && issue.dependency == "Vendor"));

        fs::remove_dir_all(root).ok();
    }
}
