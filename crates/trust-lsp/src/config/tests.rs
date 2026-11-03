use super::*;
use std::fs;
use std::process::Command;
use std::time::{SystemTime, UNIX_EPOCH};
use tower_lsp::lsp_types::Url;

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

fn toml_git_source(path: &Path) -> String {
    let canonical = path.canonicalize().unwrap_or_else(|_| path.to_path_buf());
    Url::from_file_path(&canonical)
        .map(|url| url.to_string())
        .unwrap_or_else(|_| canonical.to_string_lossy().replace('\\', "/"))
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
warn_numeric_hazards = false
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
    assert!(!config.diagnostics.warn_numeric_hazards);
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
fn mitsubishi_vendor_profile_keeps_default_diagnostics_enabled() {
    let root = temp_dir("trustlsp-config-diagnostics-mitsubishi");
    let config_path = root.join("trust-lsp.toml");
    fs::write(
        &config_path,
        r#"
[project]
vendor_profile = "mitsubishi"
"#,
    )
    .expect("write config");

    let config = ProjectConfig::load(&root);
    assert!(config.diagnostics.warn_missing_else);
    assert!(config.diagnostics.warn_implicit_conversion);
    assert!(config.diagnostics.warn_numeric_hazards);

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
    let repo_source = toml_git_source(&repo);

    fs::write(
        root.join("trust-lsp.toml"),
        format!(
            r#"
[dependencies]
ByRev = {{ git = "{repo}", rev = "{rev}" }}
ByTag = {{ git = "{repo}", tag = "v1" }}
ByBranch = {{ git = "{repo}", branch = "stable" }}
"#,
            repo = repo_source,
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
    let repo_source = toml_git_source(&repo);

    fs::write(
        root.join("trust-lsp.toml"),
        format!(
            r#"
[build]
dependencies_locked = true

[dependencies]
Floating = {{ git = "{repo}" }}
"#,
            repo = repo_source
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
    let repo_source = toml_git_source(&repo);

    let initial_config = format!(
        r#"
[dependencies]
Floating = {{ git = "{repo}" }}
"#,
        repo = repo_source
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
            repo = repo_source
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
fn indexing_roots_replaces_root_when_include_paths_set() {
    // When `include_paths` is configured, the workspace root must not be added
    // to the indexer roots. Otherwise the indexer scans everything under the
    // root regardless of the explicit scoping, causing cross-project diagnostic
    // pollution (e.g. E104 duplicate imports from sibling examples).
    let root = temp_dir("trustlsp-config-include-paths-scope");
    fs::write(
        root.join("trust-lsp.toml"),
        r#"
[project]
include_paths = ["src"]
"#,
    )
    .expect("write config");
    fs::create_dir_all(root.join("src")).expect("create src dir");

    let config = ProjectConfig::load(&root);
    let roots = config.indexing_roots();

    assert!(
        !roots.iter().any(|p| p == &config.root),
        "indexing_roots must not include the workspace root when include_paths is explicit; got {:?}",
        roots
    );
    assert!(
        roots.iter().any(|p| p.ends_with("src")),
        "indexing_roots should include the configured include_paths; got {:?}",
        roots
    );

    fs::remove_dir_all(root).ok();
}

#[test]
fn indexing_roots_uses_root_when_no_include_paths() {
    // Without `include_paths`, the indexer falls back to scanning the workspace
    // root so that loose .st files at the top level are still discovered.
    let root = temp_dir("trustlsp-config-no-include-paths");
    fs::write(
        root.join("trust-lsp.toml"),
        r#"
[project]
"#,
    )
    .expect("write config");

    let config = ProjectConfig::load(&root);
    let roots = config.indexing_roots();

    assert!(
        roots.iter().any(|p| p == &config.root),
        "indexing_roots should fall back to the workspace root when include_paths is empty; got {:?}",
        roots
    );

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
