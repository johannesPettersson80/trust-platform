//! Workspace/project configuration for trust-lsp.

use serde::{Deserialize, Serialize};
use std::collections::{BTreeMap, HashMap};
use std::path::{Path, PathBuf};
use tower_lsp::lsp_types::DiagnosticSeverity;

mod deps;
mod git;
mod load;
mod lockfile;

use deps::{parse_project_dependencies, resolve_manifest_dependencies};
use git::{resolve_git_revision, run_git_command, validate_git_source_policy};
use lockfile::{
    dependency_lock_path, dependency_lock_version, load_dependency_lock, sanitize_for_path,
    stable_hash_hex, write_dependency_lock,
};

pub(crate) const CONFIG_FILES: &[&str] = &["trust-lsp.toml", ".trust-lsp.toml", "trustlsp.toml"];
mod model;

pub use model::*;

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
                "mitsubishi" | "gxworks3" => {
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
        if let Some(value) = section.warn_numeric_hazards {
            settings.warn_numeric_hazards = value;
        }

        apply_severity_overrides(&mut settings, section.severity_overrides);
        settings
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
        "codesys-safety" | "beckhoff-safety" | "twincat-safety" | "mitsubishi-safety"
        | "gxworks3-safety" => {
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
    warn_numeric_hazards: Option<bool>,
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

#[cfg(test)]
#[path = "config/tests.rs"]
mod tests;
