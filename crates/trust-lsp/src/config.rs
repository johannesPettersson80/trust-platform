//! Workspace/project configuration for trust-lsp.

use serde::{Deserialize, Serialize};
use std::collections::{BTreeMap, HashSet};
use std::path::{Component, Path, PathBuf};
use tower_lsp::lsp_types::DiagnosticSeverity;

mod deps;
mod diagnostics;
mod git;
mod load;
mod lockfile;

use deps::{parse_project_dependencies, resolve_manifest_dependencies};
pub(crate) use diagnostics::configuration_issues;
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

fn apply_severity_overrides(
    settings: &mut DiagnosticSettings,
    overrides: BTreeMap<String, String>,
) {
    let mut aliases = Vec::new();
    let mut canonical = Vec::new();
    for (code, severity) in overrides {
        if let Some(parsed) = parse_severity(&severity) {
            let trimmed = code.trim();
            let normalized = trimmed.to_ascii_uppercase();
            if !normalized.is_empty() {
                if trimmed == normalized {
                    canonical.push((normalized, parsed));
                } else {
                    aliases.push((normalized, parsed));
                }
            }
        }
    }
    for (code, severity) in aliases.into_iter().chain(canonical) {
        settings.severity_overrides.insert(code, severity);
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
        let path = section
            .path
            .and_then(|path| resolve_optional_path(root, &path));
        let path = if enabled {
            Some(path.unwrap_or_else(|| resolve_path(root, ".trust-lsp/telemetry.jsonl")))
        } else {
            path
        };
        TelemetryConfig {
            enabled,
            path,
            flush_every: section
                .flush_every
                .filter(|value| *value != 0)
                .unwrap_or(25),
        }
    }
}

fn normalize_optional_string(value: Option<String>) -> Option<String> {
    value.and_then(|value| {
        let value = value.trim();
        (!value.is_empty()).then(|| value.to_string())
    })
}

fn normalize_strings(values: Vec<String>, case_insensitive: bool) -> Vec<String> {
    let mut seen = HashSet::new();
    values
        .into_iter()
        .filter_map(|value| normalize_optional_string(Some(value)))
        .filter(|value| {
            let key = if case_insensitive {
                value.to_ascii_lowercase()
            } else {
                value.clone()
            };
            seen.insert(key)
        })
        .collect()
}

fn normalize_path(path: PathBuf) -> PathBuf {
    let mut normalized = PathBuf::new();
    for component in path.components() {
        match component {
            Component::CurDir => {}
            Component::ParentDir => {
                if !normalized.pop() {
                    normalized.push(component.as_os_str());
                }
            }
            Component::Normal(value) => normalized.push(value),
            Component::RootDir | Component::Prefix(_) => normalized.push(component.as_os_str()),
        }
    }
    normalized
}

fn resolve_optional_path(root: &Path, entry: &str) -> Option<PathBuf> {
    let entry = entry.trim();
    if entry.is_empty() {
        return None;
    }
    let path = PathBuf::from(entry);
    Some(normalize_path(if path.is_absolute() {
        path
    } else {
        root.join(path)
    }))
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
    evict_to_percent: Option<i64>,
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
    severity_overrides: BTreeMap<String, String>,
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

#[derive(Debug, Clone, Serialize, Deserialize)]
struct DependencyLockFile {
    #[serde(default = "dependency_lock_version")]
    version: u32,
    #[serde(default)]
    dependencies: BTreeMap<String, DependencyLockEntry>,
}

impl Default for DependencyLockFile {
    fn default() -> Self {
        Self {
            version: dependency_lock_version(),
            dependencies: BTreeMap::new(),
        }
    }
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
                allow: Some(normalize_strings(list, true)),
            },
            StdlibSelection::Profile(profile) => {
                let normalized = profile.trim().to_ascii_lowercase();
                if normalized == "none" {
                    StdlibSettings {
                        profile: Some(normalized),
                        allow: Some(Vec::new()),
                    }
                } else if matches!(normalized.as_str(), "full" | "iec") {
                    StdlibSettings {
                        profile: Some(normalized),
                        allow: None,
                    }
                } else {
                    StdlibSettings {
                        profile: Some("full".to_string()),
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
            control_endpoint: normalize_optional_string(section.control_endpoint),
            control_auth_token: normalize_optional_string(section.control_auth_token),
        }
    }
}

impl From<BuildSection> for BuildConfig {
    fn from(section: BuildSection) -> Self {
        BuildConfig {
            target: normalize_optional_string(section.target),
            profile: normalize_optional_string(section.profile),
            flags: normalize_strings(section.flags, false),
            defines: normalize_strings(section.defines, true),
            dependencies_offline: section.dependencies_offline.unwrap_or(false),
            dependencies_locked: section.dependencies_locked.unwrap_or(false),
            dependency_lockfile: section
                .dependency_lockfile
                .and_then(|path| normalize_optional_string(Some(path)))
                .map(PathBuf::from)
                .unwrap_or_else(|| PathBuf::from("trust-lsp.lock")),
        }
    }
}

impl From<DependencyPolicySection> for DependencyPolicy {
    fn from(section: DependencyPolicySection) -> Self {
        DependencyPolicy {
            allowed_git_hosts: normalize_strings(section.allowed_git_hosts, true)
                .into_iter()
                .map(|host| host.to_ascii_lowercase())
                .collect(),
            allow_http: section.allow_http.unwrap_or(false),
            allow_ssh: section.allow_ssh.unwrap_or(false),
        }
    }
}

impl From<TargetSection> for TargetProfile {
    fn from(section: TargetSection) -> Self {
        TargetProfile {
            name: section.name.trim().to_string(),
            profile: normalize_optional_string(section.profile),
            flags: normalize_strings(section.flags, false),
            defines: normalize_strings(section.defines, true),
        }
    }
}

impl From<LibraryDependencyEntry> for LibraryDependency {
    fn from(entry: LibraryDependencyEntry) -> Self {
        match entry {
            LibraryDependencyEntry::Name(name) => {
                let mut parts = name.splitn(2, '@');
                let base = parts.next().unwrap_or("").trim().to_string();
                let version = parts
                    .next()
                    .and_then(|part| normalize_optional_string(Some(part.to_string())));
                LibraryDependency {
                    name: base,
                    version,
                }
            }
            LibraryDependencyEntry::Detailed(section) => LibraryDependency {
                name: section.name.trim().to_string(),
                version: normalize_optional_string(section.version),
            },
        }
    }
}

impl From<IndexingSection> for IndexingConfig {
    fn from(section: IndexingSection) -> Self {
        let defaults = IndexingConfig::default();
        let throttle_idle_ms = section
            .throttle_idle_ms
            .unwrap_or(defaults.throttle_idle_ms);
        let throttle_active_ms = section
            .throttle_active_ms
            .unwrap_or(defaults.throttle_active_ms);
        let throttle_max_ms = section.throttle_max_ms.unwrap_or(defaults.throttle_max_ms);
        let throttle_active_window_ms = section
            .throttle_active_window_ms
            .unwrap_or(defaults.throttle_active_window_ms);
        let coherent = throttle_idle_ms <= throttle_active_ms
            && throttle_active_ms <= throttle_max_ms
            && throttle_active_window_ms > 0;
        IndexingConfig {
            max_files: (section.max_files != Some(0))
                .then_some(section.max_files)
                .flatten(),
            max_ms: (section.max_ms != Some(0))
                .then_some(section.max_ms)
                .flatten(),
            cache_enabled: section.cache.unwrap_or(true),
            cache_dir: normalize_optional_string(section.cache_dir).map(PathBuf::from),
            memory_budget_mb: (section.memory_budget_mb != Some(0))
                .then_some(section.memory_budget_mb)
                .flatten(),
            evict_to_percent: section.evict_to_percent.unwrap_or(80).clamp(1, 100) as u8,
            throttle_idle_ms: if coherent {
                throttle_idle_ms
            } else {
                defaults.throttle_idle_ms
            },
            throttle_active_ms: if coherent {
                throttle_active_ms
            } else {
                defaults.throttle_active_ms
            },
            throttle_max_ms: if coherent {
                throttle_max_ms
            } else {
                defaults.throttle_max_ms
            },
            throttle_active_window_ms: if coherent {
                throttle_active_window_ms
            } else {
                defaults.throttle_active_window_ms
            },
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
    let mut paths = Vec::new();
    for entry in entries {
        if let Some(path) = resolve_optional_path(root, entry) {
            if !paths.contains(&path) {
                paths.push(path);
            }
        }
    }
    paths
}

fn resolve_path(root: &Path, entry: &str) -> PathBuf {
    resolve_optional_path(root, entry).unwrap_or_else(|| root.to_path_buf())
}

#[cfg(test)]
#[path = "config/configuration_contract_tests.rs"]
mod configuration_contract_tests;
#[cfg(test)]
#[path = "config/dependency_contract_tests.rs"]
mod dependency_contract_tests;
#[cfg(test)]
#[path = "config/tests.rs"]
mod tests;
