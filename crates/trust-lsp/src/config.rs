//! Workspace/project configuration for trust-lsp.

use serde::Deserialize;
use std::collections::{BTreeMap, HashMap};
use std::path::{Path, PathBuf};
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
        let dependencies = parse_project_dependencies(root, &parsed.dependencies);
        let (dependency_libraries, dependency_resolution_issues) =
            resolve_manifest_dependencies(&dependencies);
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
#[derive(Debug, Clone, Default)]
pub struct BuildConfig {
    /// Optional target name to select.
    pub target: Option<String>,
    /// Optional profile (e.g., debug/release).
    pub profile: Option<String>,
    /// Additional compile flags.
    pub flags: Vec<String>,
    /// Preprocessor/define flags.
    pub defines: Vec<String>,
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

/// A local package dependency declared in `[dependencies]`.
#[derive(Debug, Clone)]
pub struct ProjectDependency {
    pub name: String,
    pub path: PathBuf,
    pub version: Option<String>,
}

/// Dependency resolver issue surfaced as a config diagnostic.
#[derive(Debug, Clone)]
pub struct DependencyResolutionIssue {
    pub code: &'static str,
    pub dependency: String,
    pub message: String,
}

#[derive(Debug, Deserialize)]
struct ConfigFile {
    #[serde(default)]
    dependencies: BTreeMap<String, ManifestDependencyEntry>,
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
struct BuildSection {
    target: Option<String>,
    profile: Option<String>,
    #[serde(default)]
    flags: Vec<String>,
    #[serde(default)]
    defines: Vec<String>,
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
    path: String,
    version: Option<String>,
}

#[derive(Debug, Default, Deserialize)]
struct DependencyManifestFile {
    #[serde(default)]
    package: PackageSection,
    #[serde(default)]
    dependencies: BTreeMap<String, ManifestDependencyEntry>,
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

impl ManifestDependencyEntry {
    fn path(&self) -> &str {
        match self {
            ManifestDependencyEntry::Path(path) => path,
            ManifestDependencyEntry::Detailed(entry) => entry.path.as_str(),
        }
    }

    fn version(&self) -> Option<String> {
        match self {
            ManifestDependencyEntry::Path(_) => None,
            ManifestDependencyEntry::Detailed(entry) => entry.version.clone(),
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
) -> Vec<ProjectDependency> {
    entries
        .iter()
        .map(|(name, entry)| ProjectDependency {
            name: name.clone(),
            path: resolve_path(root, entry.path()),
            version: entry.version(),
        })
        .collect()
}

fn resolve_manifest_dependencies(
    dependencies: &[ProjectDependency],
) -> (Vec<LibrarySpec>, Vec<DependencyResolutionIssue>) {
    let mut libraries = BTreeMap::new();
    let mut issues = Vec::new();
    let mut states = HashMap::new();

    for dependency in dependencies {
        resolve_dependency_recursive(dependency, &mut states, &mut libraries, &mut issues);
    }

    (libraries.into_values().collect(), issues)
}

fn resolve_dependency_recursive(
    dependency: &ProjectDependency,
    states: &mut HashMap<String, DependencyVisitState>,
    libraries: &mut BTreeMap<String, LibrarySpec>,
    issues: &mut Vec<DependencyResolutionIssue>,
) {
    let path = canonicalize_or_self(&dependency.path);
    if !path.is_dir() {
        issues.push(DependencyResolutionIssue {
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

    if let Some(existing) = libraries.get(&dependency.name) {
        if let Some(required) = dependency.version.as_deref() {
            if existing.version.as_deref() != Some(required) {
                let available = existing.version.as_deref().unwrap_or("unspecified");
                issues.push(DependencyResolutionIssue {
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

    if states
        .get(dependency.name.as_str())
        .copied()
        .is_some_and(|state| state == DependencyVisitState::Visiting)
    {
        return;
    }

    states.insert(dependency.name.clone(), DependencyVisitState::Visiting);

    let (package, nested_dependencies) = match load_dependency_manifest(&path) {
        Ok(manifest) => (
            manifest.package,
            parse_project_dependencies(&path, &manifest.dependencies),
        ),
        Err(message) => {
            issues.push(DependencyResolutionIssue {
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
            issues.push(DependencyResolutionIssue {
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
        resolve_dependency_recursive(nested, states, libraries, issues);
    }

    libraries.insert(
        dependency.name.clone(),
        LibrarySpec {
            name: dependency.name.clone(),
            path,
            version: package.version,
            dependencies: library_dependencies,
            docs: Vec::new(),
        },
    );
    states.insert(dependency.name.clone(), DependencyVisitState::Done);
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
}
