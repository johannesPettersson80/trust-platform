use super::*;
use std::fs;
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicU64, Ordering};

static NEXT_PROJECT_ID: AtomicU64 = AtomicU64::new(0);

struct ConfigProject {
    root: PathBuf,
}

impl ConfigProject {
    fn new(label: &str) -> Self {
        let id = NEXT_PROJECT_ID.fetch_add(1, Ordering::Relaxed);
        let root = std::env::temp_dir().join(format!(
            "trust-lsp-config-contract-{}-{label}-{id}",
            std::process::id()
        ));
        fs::create_dir_all(&root).expect("create configuration contract project");
        Self { root }
    }

    fn write(&self, relative: &str, contents: &str) -> PathBuf {
        let path = self.root.join(relative);
        if let Some(parent) = path.parent() {
            fs::create_dir_all(parent).expect("create configuration contract parent");
        }
        fs::write(&path, contents).expect("write configuration contract fixture");
        path
    }

    fn config(&self, contents: &str) -> ProjectConfig {
        ProjectConfig::from_contents(&self.root, Some(self.root.join("trust-lsp.toml")), contents)
    }
}

impl Drop for ConfigProject {
    fn drop(&mut self) {
        fs::remove_dir_all(&self.root).ok();
    }
}

#[test]
fn configuration_filename_precedence_is_stable() {
    let project = ConfigProject::new("filename-precedence");
    project.write("trustlsp.toml", "");
    project.write(".trust-lsp.toml", "");
    project.write("trust-lsp.toml", "");

    assert_eq!(
        find_config_file(&project.root),
        Some(project.root.join("trust-lsp.toml"))
    );
}

#[test]
fn configuration_filename_falls_through_aliases_in_order() {
    let project = ConfigProject::new("filename-aliases");
    project.write("trustlsp.toml", "");
    assert_eq!(
        find_config_file(&project.root),
        Some(project.root.join("trustlsp.toml"))
    );

    project.write(".trust-lsp.toml", "");
    assert_eq!(
        find_config_file(&project.root),
        Some(project.root.join(".trust-lsp.toml"))
    );
}

#[test]
fn absent_configuration_uses_complete_safe_defaults() {
    let project = ConfigProject::new("absent-defaults");
    let config = ProjectConfig::load(&project.root);

    assert_eq!(config.root, project.root);
    assert!(config.config_path.is_none());
    assert!(config.include_paths.is_empty());
    assert!(config.libraries.is_empty());
    assert_eq!(config.stdlib.profile.as_deref(), Some("full"));
    assert_eq!(config.workspace.visibility, WorkspaceVisibility::Public);
    assert_eq!(config.indexing.evict_to_percent, 80);
    assert_eq!(config.telemetry.flush_every, 25);
}

#[test]
fn malformed_configuration_is_not_partially_applied() {
    let project = ConfigProject::new("malformed");
    let path = project.write(
        "trust-lsp.toml",
        "[project]\nvendor_profile = \"codesys\"\n[indexing\nmax_files = 5",
    );
    let config = ProjectConfig::load(&project.root);

    assert_eq!(config.config_path.as_deref(), Some(path.as_path()));
    assert!(config.vendor_profile.is_none());
    assert!(config.include_paths.is_empty());
    assert!(config.indexing.max_files.is_none());
    assert_eq!(config.workspace.visibility, WorkspaceVisibility::Public);
}

#[test]
fn resolve_path_preserves_absolute_and_roots_relative_entries() {
    let root = Path::new("/workspace/project");
    assert_eq!(resolve_path(root, "src"), root.join("src"));
    assert_eq!(
        resolve_path(root, "/opt/vendor/library"),
        PathBuf::from("/opt/vendor/library")
    );
}

#[test]
fn resolve_paths_preserves_declaration_order() {
    let root = Path::new("/workspace/project");
    let entries = vec![
        "src".to_string(),
        "/opt/vendor".to_string(),
        "generated".to_string(),
    ];
    assert_eq!(
        resolve_paths(root, &entries),
        vec![
            root.join("src"),
            PathBuf::from("/opt/vendor"),
            root.join("generated"),
        ]
    );
}

#[test]
fn project_path_lists_trim_drop_blanks_and_deduplicate() {
    let project = ConfigProject::new("project-path-normalization");
    let config = project.config(
        r#"
[project]
include_paths = [" src ", "", "src", " lib/../src "]
library_paths = [" vendor ", "vendor", " "]
"#,
    );

    assert_eq!(config.include_paths, vec![project.root.join("src")]);
    assert_eq!(
        config
            .libraries
            .iter()
            .map(|library| library.path.clone())
            .collect::<Vec<_>>(),
        vec![project.root.join("vendor")]
    );
}

#[test]
fn explicit_include_roots_replace_workspace_root() {
    let project = ConfigProject::new("explicit-roots");
    let config = project.config(
        r#"
[project]
include_paths = ["src", "generated"]
"#,
    );

    assert_eq!(
        config.indexing_roots(),
        vec![project.root.join("src"), project.root.join("generated")]
    );
}

#[test]
fn indexing_roots_fall_back_to_root_and_add_each_library_once() {
    let project = ConfigProject::new("root-library-dedupe");
    let config = project.config(
        r#"
[project]
library_paths = ["vendor", "vendor"]

[[libraries]]
name = "VendorAgain"
path = "vendor"
"#,
    );

    assert_eq!(
        config.indexing_roots(),
        vec![project.root.clone(), project.root.join("vendor")]
    );
}

#[test]
fn disabled_index_cache_has_no_resolved_directory() {
    let project = ConfigProject::new("cache-disabled");
    let config = project.config(
        r#"
[indexing]
cache = false
cache_dir = "persisted-for-later"
"#,
    );
    assert!(config.index_cache_dir().is_none());
}

#[test]
fn enabled_index_cache_uses_documented_default_directory() {
    let project = ConfigProject::new("cache-default");
    let config = project.config("");
    assert_eq!(
        config.index_cache_dir(),
        Some(project.root.join(".trust-lsp/index-cache"))
    );
}

#[test]
fn relative_index_cache_directory_resolves_against_root() {
    let project = ConfigProject::new("cache-relative");
    let config = project.config("[indexing]\ncache_dir = \"cache/index\"");
    assert_eq!(
        config.index_cache_dir(),
        Some(project.root.join("cache/index"))
    );
}

#[test]
fn absolute_index_cache_directory_is_preserved() {
    let project = ConfigProject::new("cache-absolute");
    let cache_dir = std::env::temp_dir().join("trust-lsp-absolute-cache");
    let config = project.config(&format!(
        "[indexing]\ncache_dir = {}",
        toml::Value::String(cache_dir.to_string_lossy().into_owned())
    ));
    assert_eq!(config.index_cache_dir(), Some(cache_dir));
}

#[test]
fn explicit_empty_configuration_selects_full_stdlib() {
    let project = ConfigProject::new("stdlib-default");
    let config = project.config("");
    assert_eq!(config.stdlib.profile.as_deref(), Some("full"));
    assert!(config.stdlib.allow.is_none());
}

#[test]
fn named_stdlib_profile_is_trimmed_and_canonical_lowercase() {
    let project = ConfigProject::new("stdlib-profile-normalization");
    let config = project.config("[project]\nstdlib = \" IEC \"");
    assert_eq!(config.stdlib.profile.as_deref(), Some("iec"));
    assert!(config.stdlib.allow.is_none());
}

#[test]
fn none_stdlib_profile_has_explicit_empty_allow_list() {
    let project = ConfigProject::new("stdlib-none");
    let config = project.config("[project]\nstdlib = \"NoNe\"");
    assert_eq!(config.stdlib.profile.as_deref(), Some("none"));
    assert_eq!(config.stdlib.allow.as_deref(), Some([].as_slice()));
}

#[test]
fn stdlib_allow_list_trims_blanks_and_deduplicates_case_insensitively() {
    let project = ConfigProject::new("stdlib-allow");
    let config = project.config(
        r#"
[project]
stdlib = [" ABS ", "", "abs", "CTU", "ctu", " TON "]
"#,
    );
    assert!(config.stdlib.profile.is_none());
    assert_eq!(
        config.stdlib.allow.as_deref(),
        Some(["ABS".to_string(), "CTU".to_string(), "TON".to_string()].as_slice())
    );
}

#[test]
fn unknown_stdlib_profile_falls_back_to_full() {
    let project = ConfigProject::new("stdlib-unknown");
    let config = project.config("[project]\nstdlib = \"vendor-mystery\"");
    assert_eq!(config.stdlib.profile.as_deref(), Some("full"));
    assert!(config.stdlib.allow.is_none());
}

#[test]
fn indexing_defaults_are_exact_and_coherent() {
    let project = ConfigProject::new("indexing-defaults");
    let indexing = project.config("").indexing;
    assert_eq!(indexing.max_files, None);
    assert_eq!(indexing.max_ms, None);
    assert!(indexing.cache_enabled);
    assert_eq!(indexing.memory_budget_mb, None);
    assert_eq!(indexing.evict_to_percent, 80);
    assert_eq!(indexing.throttle_idle_ms, 0);
    assert_eq!(indexing.throttle_active_ms, 8);
    assert_eq!(indexing.throttle_max_ms, 50);
    assert_eq!(indexing.throttle_active_window_ms, 250);
}

#[test]
fn zero_optional_indexing_budgets_become_unbounded() {
    let project = ConfigProject::new("indexing-zero-budgets");
    let indexing = project
        .config(
            r#"
[indexing]
max_files = 0
max_ms = 0
memory_budget_mb = 0
"#,
        )
        .indexing;
    assert_eq!(indexing.max_files, None);
    assert_eq!(indexing.max_ms, None);
    assert_eq!(indexing.memory_budget_mb, None);
}

#[test]
fn eviction_target_is_clamped_to_positive_minimum() {
    let project = ConfigProject::new("eviction-minimum");
    for raw in [0, -1] {
        let indexing = project
            .config(&format!("[indexing]\nevict_to_percent = {raw}"))
            .indexing;
        assert_eq!(indexing.evict_to_percent, 1, "raw value {raw}");
    }
}

#[test]
fn eviction_target_is_clamped_to_one_hundred() {
    let project = ConfigProject::new("eviction-maximum");
    for raw in [101, 255, 256] {
        let indexing = project
            .config(&format!("[indexing]\nevict_to_percent = {raw}"))
            .indexing;
        assert_eq!(indexing.evict_to_percent, 100, "raw value {raw}");
    }
}

#[test]
fn incoherent_throttle_tuple_resets_atomically() {
    let project = ConfigProject::new("throttle-invalid");
    let indexing = project
        .config(
            r#"
[indexing]
throttle_idle_ms = 70
throttle_active_ms = 80
throttle_max_ms = 50
throttle_active_window_ms = 0
"#,
        )
        .indexing;
    assert_eq!(indexing.throttle_idle_ms, 0);
    assert_eq!(indexing.throttle_active_ms, 8);
    assert_eq!(indexing.throttle_max_ms, 50);
    assert_eq!(indexing.throttle_active_window_ms, 250);
}

#[test]
fn coherent_throttle_tuple_is_retained() {
    let project = ConfigProject::new("throttle-valid");
    let indexing = project
        .config(
            r#"
[indexing]
throttle_idle_ms = 3
throttle_active_ms = 12
throttle_max_ms = 40
throttle_active_window_ms = 500
"#,
        )
        .indexing;
    assert_eq!(indexing.throttle_idle_ms, 3);
    assert_eq!(indexing.throttle_active_ms, 12);
    assert_eq!(indexing.throttle_max_ms, 40);
    assert_eq!(indexing.throttle_active_window_ms, 500);
}

#[test]
fn telemetry_is_opt_in_with_stable_defaults() {
    let project = ConfigProject::new("telemetry-defaults");
    let telemetry = project.config("").telemetry;
    assert!(!telemetry.enabled);
    assert!(telemetry.path.is_none());
    assert_eq!(telemetry.flush_every, 25);
}

#[test]
fn enabled_telemetry_without_path_uses_project_default() {
    let project = ConfigProject::new("telemetry-enabled-default");
    let telemetry = project.config("[telemetry]\nenabled = true").telemetry;
    assert!(telemetry.enabled);
    assert_eq!(
        telemetry.path,
        Some(project.root.join(".trust-lsp/telemetry.jsonl"))
    );
}

#[test]
fn disabled_telemetry_retains_trimmed_explicit_path() {
    let project = ConfigProject::new("telemetry-disabled-path");
    let telemetry = project
        .config("[telemetry]\nenabled = false\npath = \" logs/events.jsonl \"")
        .telemetry;
    assert!(!telemetry.enabled);
    assert_eq!(telemetry.path, Some(project.root.join("logs/events.jsonl")));
}

#[test]
fn zero_telemetry_flush_interval_uses_default() {
    let project = ConfigProject::new("telemetry-zero-flush");
    let telemetry = project
        .config("[telemetry]\nenabled = true\nflush_every = 0")
        .telemetry;
    assert_eq!(telemetry.flush_every, 25);
}

#[test]
fn workspace_visibility_values_are_case_insensitive_and_trimmed() {
    assert_eq!(
        WorkspaceVisibility::from_str(" PuBLic "),
        WorkspaceVisibility::Public
    );
    assert_eq!(
        WorkspaceVisibility::from_str(" PRIVATE "),
        WorkspaceVisibility::Private
    );
    assert_eq!(
        WorkspaceVisibility::from_str("HiDdEn"),
        WorkspaceVisibility::Hidden
    );
}

#[test]
fn workspace_visibility_query_policy_is_exact() {
    assert!(WorkspaceVisibility::Public.allows_query(true));
    assert!(WorkspaceVisibility::Public.allows_query(false));
    assert!(!WorkspaceVisibility::Private.allows_query(true));
    assert!(WorkspaceVisibility::Private.allows_query(false));
    assert!(!WorkspaceVisibility::Hidden.allows_query(true));
    assert!(!WorkspaceVisibility::Hidden.allows_query(false));
}

#[test]
fn unknown_workspace_visibility_falls_back_to_public() {
    let project = ConfigProject::new("visibility-unknown");
    let config = project.config("[workspace]\nvisibility = \"partners\"");
    assert_eq!(config.workspace.visibility, WorkspaceVisibility::Public);
}

#[test]
fn target_names_are_trimmed_nonblank_and_case_insensitively_unique() {
    let project = ConfigProject::new("target-identities");
    let config = project.config(
        r#"
[[targets]]
name = " Simulator "

[[targets]]
name = "simulator"

[[targets]]
name = " "

[[targets]]
name = "Hardware"
"#,
    );
    assert_eq!(
        config
            .targets
            .iter()
            .map(|target| target.name.as_str())
            .collect::<Vec<_>>(),
        vec!["Simulator", "Hardware"]
    );
}

#[test]
fn build_optional_scalars_are_trimmed_and_blanks_are_absent() {
    let project = ConfigProject::new("build-scalars");
    let config = project.config(
        r#"
[build]
target = " Simulator "
profile = " "
"#,
    );
    assert_eq!(config.build.target.as_deref(), Some("Simulator"));
    assert!(config.build.profile.is_none());
}

#[test]
fn build_and_target_lists_drop_blanks_and_deduplicate() {
    let project = ConfigProject::new("build-lists");
    let config = project.config(
        r#"
[build]
flags = [" --release ", "", "--release", "--locked"]
defines = [" SIM=1 ", "sim=1", " ", "TRACE=1"]

[[targets]]
name = "sim"
profile = " debug "
flags = [" -g ", "-g", ""]
defines = [" SIM=1 ", "sim=1"]
"#,
    );
    assert_eq!(config.build.flags, vec!["--release", "--locked"]);
    assert_eq!(config.build.defines, vec!["SIM=1", "TRACE=1"]);
    assert_eq!(config.targets[0].profile.as_deref(), Some("debug"));
    assert_eq!(config.targets[0].flags, vec!["-g"]);
    assert_eq!(config.targets[0].defines, vec!["SIM=1"]);
}

#[test]
fn diagnostic_severity_aliases_are_case_insensitive() {
    for (text, expected) in [
        (" ERROR ", DiagnosticSeverity::ERROR),
        ("err", DiagnosticSeverity::ERROR),
        ("Warning", DiagnosticSeverity::WARNING),
        ("warn", DiagnosticSeverity::WARNING),
        ("INFO", DiagnosticSeverity::INFORMATION),
        ("information", DiagnosticSeverity::INFORMATION),
        ("Hint", DiagnosticSeverity::HINT),
    ] {
        assert_eq!(parse_severity(text), Some(expected), "{text}");
    }
    assert_eq!(parse_severity("fatal"), None);
}

#[test]
fn safety_rule_pack_enables_complete_warning_baseline() {
    let project = ConfigProject::new("diagnostic-safety-pack");
    let diagnostics = project
        .config(
            r#"
[project]
vendor_profile = "siemens"

[diagnostics]
rule_pack = " IEC-SAFETY "
"#,
        )
        .diagnostics;
    assert!(diagnostics.warn_unused);
    assert!(diagnostics.warn_unreachable);
    assert!(diagnostics.warn_missing_else);
    assert!(diagnostics.warn_implicit_conversion);
    assert!(diagnostics.warn_shadowed);
    assert!(diagnostics.warn_deprecated);
    assert!(diagnostics.warn_complexity);
    assert!(diagnostics.warn_nondeterminism);
    assert!(diagnostics.warn_numeric_hazards);
}

#[test]
fn explicit_warning_booleans_override_rule_pack() {
    let project = ConfigProject::new("diagnostic-explicit-booleans");
    let diagnostics = project
        .config(
            r#"
[diagnostics]
rule_pack = "iec-safety"
warn_missing_else = false
warn_nondeterminism = false
"#,
        )
        .diagnostics;
    assert!(!diagnostics.warn_missing_else);
    assert!(!diagnostics.warn_nondeterminism);
    assert!(diagnostics.warn_numeric_hazards);
}

#[test]
fn explicit_severity_overrides_apply_after_pack_defaults() {
    let project = ConfigProject::new("diagnostic-severity-precedence");
    let diagnostics = project
        .config(
            r#"
[diagnostics]
rule_pack = "iec-safety"
severity_overrides = { W010 = "hint", W004 = "warning" }
"#,
        )
        .diagnostics;
    assert_eq!(
        diagnostics.severity_overrides.get("W010"),
        Some(&DiagnosticSeverity::HINT)
    );
    assert_eq!(
        diagnostics.severity_overrides.get("W004"),
        Some(&DiagnosticSeverity::WARNING)
    );
}

#[test]
fn severity_override_codes_are_uppercase_and_invalid_entries_are_dropped() {
    let project = ConfigProject::new("diagnostic-code-normalization");
    let diagnostics = project
        .config(
            r#"
[diagnostics.severity_overrides]
" w003 " = " error "
w010 = "WARN"
"" = "hint"
W999 = "fatal"
"#,
        )
        .diagnostics;
    assert_eq!(
        diagnostics.severity_overrides.get("W003"),
        Some(&DiagnosticSeverity::ERROR)
    );
    assert_eq!(
        diagnostics.severity_overrides.get("W010"),
        Some(&DiagnosticSeverity::WARNING)
    );
    assert_eq!(diagnostics.severity_overrides.len(), 2);
}

#[test]
fn canonical_severity_override_wins_normalized_key_collision() {
    let project = ConfigProject::new("diagnostic-code-collision");
    let diagnostics = project
        .config(
            r#"
[diagnostics.severity_overrides]
" w010 " = "hint"
w010 = "warning"
W010 = "error"
"#,
        )
        .diagnostics;
    assert_eq!(
        diagnostics.severity_overrides.get("W010"),
        Some(&DiagnosticSeverity::ERROR)
    );
    assert_eq!(diagnostics.severity_overrides.len(), 1);
}

#[test]
fn alias_only_severity_override_collision_uses_lexically_greatest_raw_key() {
    let project = ConfigProject::new("diagnostic-alias-only-collision");
    let diagnostics = project
        .config(
            r#"
[diagnostics.severity_overrides]
" w010 " = "hint"
w010 = "warning"
"#,
        )
        .diagnostics;
    assert_eq!(
        diagnostics.severity_overrides.get("W010"),
        Some(&DiagnosticSeverity::WARNING)
    );
    assert_eq!(diagnostics.severity_overrides.len(), 1);
}

#[test]
fn external_diagnostic_paths_follow_common_path_normalization() {
    let project = ConfigProject::new("external-paths");
    let config = project.config(
        r#"
[diagnostics]
external_paths = [" lint/a.json ", "", "lint/a.json", "lint/b.json"]
"#,
    );
    assert_eq!(
        config.diagnostic_external_paths,
        vec![
            project.root.join("lint/a.json"),
            project.root.join("lint/b.json"),
        ]
    );
}

#[test]
fn runtime_endpoint_and_token_are_trimmed_and_blank_values_are_absent() {
    let project = ConfigProject::new("runtime-normalization");
    let configured = project.config(
        r#"
[runtime]
control_endpoint = " tcp://127.0.0.1:9000 "
control_auth_token = " secret-token "
"#,
    );
    assert_eq!(
        configured.runtime.control_endpoint.as_deref(),
        Some("tcp://127.0.0.1:9000")
    );
    assert_eq!(
        configured.runtime.control_auth_token.as_deref(),
        Some("secret-token")
    );

    let blank = project.config(
        r#"
[runtime]
control_endpoint = " "
control_auth_token = ""
"#,
    );
    assert!(blank.runtime.control_endpoint.is_none());
    assert!(blank.runtime.control_auth_token.is_none());
}

#[test]
fn dependency_policy_hosts_are_normalized_and_deduplicated() {
    let policy = DependencyPolicy::from(DependencyPolicySection {
        allowed_git_hosts: vec![
            " GitHub.COM ".to_string(),
            "github.com".to_string(),
            "".to_string(),
            " GITLAB.EXAMPLE ".to_string(),
        ],
        allow_http: Some(true),
        allow_ssh: Some(false),
    });
    assert_eq!(
        policy.allowed_git_hosts,
        vec!["github.com", "gitlab.example"]
    );
    assert!(policy.allow_http);
    assert!(!policy.allow_ssh);
}

#[test]
fn compact_library_dependency_is_trimmed_and_versioned() {
    let dependency =
        LibraryDependency::from(LibraryDependencyEntry::Name(" Core @ 2.1.0 ".to_string()));
    assert_eq!(dependency.name, "Core");
    assert_eq!(dependency.version.as_deref(), Some("2.1.0"));
}

#[test]
fn compact_library_dependency_blank_version_is_absent() {
    let dependency = LibraryDependency::from(LibraryDependencyEntry::Name("Utils@ ".to_string()));
    assert_eq!(dependency.name, "Utils");
    assert!(dependency.version.is_none());
}

#[test]
fn library_sections_normalize_identity_paths_docs_and_dependencies() {
    let project = ConfigProject::new("library-normalization");
    let config = project.config(
        r#"
[[libraries]]
name = " Vendor "
path = " vendor "
version = " 1.2.3 "
dependencies = [" Core @ 2.0 ", "core@2.0", { name = " Utils ", version = " 3 " }]
docs = [" docs/api.md ", "", "docs/api.md", "docs/types.md"]
"#,
    );
    let library = &config.libraries[0];
    assert_eq!(library.name, "Vendor");
    assert_eq!(library.path, project.root.join("vendor"));
    assert_eq!(library.version.as_deref(), Some("1.2.3"));
    assert_eq!(
        library
            .dependencies
            .iter()
            .map(|dependency| { (dependency.name.as_str(), dependency.version.as_deref(),) })
            .collect::<Vec<_>>(),
        vec![("Core", Some("2.0")), ("Utils", Some("3"))]
    );
    assert_eq!(
        library.docs,
        vec![
            project.root.join("docs/api.md"),
            project.root.join("docs/types.md")
        ]
    );
}

#[test]
fn duplicate_library_paths_have_one_index_identity() {
    let project = ConfigProject::new("library-index-identity");
    let config = project.config(
        r#"
[[libraries]]
name = "One"
path = "vendor"

[[libraries]]
name = "Two"
path = " vendor "
"#,
    );
    assert_eq!(
        config.indexing_roots(),
        vec![project.root.clone(), project.root.join("vendor")]
    );
}

#[test]
fn vendor_profile_is_trimmed_and_canonical_lowercase() {
    let project = ConfigProject::new("vendor-profile-normalization");
    let config = project.config("[project]\nvendor_profile = \" TwinCAT \"");
    assert_eq!(config.vendor_profile.as_deref(), Some("twincat"));
}
