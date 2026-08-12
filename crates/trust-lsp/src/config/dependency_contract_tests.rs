use super::git::extract_git_host;
use super::*;
use std::sync::atomic::{AtomicUsize, Ordering};

static NEXT_DEPENDENCY_PROJECT_ID: AtomicUsize = AtomicUsize::new(0);

struct DependencyProject {
    root: PathBuf,
}

impl DependencyProject {
    fn new(label: &str) -> Self {
        let id = NEXT_DEPENDENCY_PROJECT_ID.fetch_add(1, Ordering::Relaxed);
        let root = std::env::temp_dir().join(format!(
            "trust-lsp-dependency-contract-{label}-{}-{id}",
            std::process::id()
        ));
        std::fs::create_dir_all(&root).expect("create dependency project");
        Self {
            root: root.canonicalize().expect("canonical project"),
        }
    }

    fn write(&self, relative: &str, text: &str) {
        let path = self.root.join(relative);
        if let Some(parent) = path.parent() {
            std::fs::create_dir_all(parent).expect("create fixture parent");
        }
        std::fs::write(path, text).expect("write fixture");
    }

    fn mkdir(&self, relative: &str) -> PathBuf {
        let path = self.root.join(relative);
        std::fs::create_dir_all(&path).expect("create fixture directory");
        path
    }

    fn config(&self, text: &str) -> ProjectConfig {
        ProjectConfig::from_contents(&self.root, Some(self.root.join("trust-lsp.toml")), text)
    }
}

impl Drop for DependencyProject {
    fn drop(&mut self) {
        let _ = std::fs::remove_dir_all(&self.root);
    }
}

fn dependency_issue<'a>(
    config: &'a ProjectConfig,
    code: &str,
    dependency: &str,
) -> Option<&'a DependencyResolutionIssue> {
    config
        .dependency_resolution_issues
        .iter()
        .find(|issue| issue.code == code && issue.dependency.eq_ignore_ascii_case(dependency))
}

fn path_lock(path: &Path, name: &str, dependency_path: &Path) {
    let mut dependencies = BTreeMap::new();
    dependencies.insert(
        name.to_string(),
        DependencyLockEntry::Path {
            path: dependency_path.to_string_lossy().into_owned(),
        },
    );
    write_dependency_lock(path, dependencies).expect("write path lock");
}

#[test]
fn dependency_parser_accepts_simple_relative_path_entry() {
    let project = DependencyProject::new("simple-path");
    let mut entries = BTreeMap::new();
    entries.insert(
        "Core".to_string(),
        ManifestDependencyEntry::Path("deps/core".to_string()),
    );

    let (dependencies, issues) = parse_project_dependencies(&project.root, &entries);

    assert!(issues.is_empty(), "{issues:#?}");
    assert_eq!(dependencies.len(), 1);
    assert_eq!(dependencies[0].name, "Core");
    assert_eq!(dependencies[0].path, Some(project.root.join("deps/core")));
    assert!(dependencies[0].git.is_none());
    assert!(dependencies[0].version.is_none());
}

#[test]
fn dependency_parser_accepts_detailed_path_with_version() {
    let project = DependencyProject::new("detailed-path");
    let mut entries = BTreeMap::new();
    entries.insert(
        "Core".to_string(),
        ManifestDependencyEntry::Detailed(ManifestDependencySection {
            path: Some("deps/core".to_string()),
            git: None,
            version: Some("1.2.3".to_string()),
            rev: None,
            tag: None,
            branch: None,
        }),
    );

    let (dependencies, issues) = parse_project_dependencies(&project.root, &entries);

    assert!(issues.is_empty(), "{issues:#?}");
    assert_eq!(dependencies[0].version.as_deref(), Some("1.2.3"));
    assert_eq!(dependencies[0].path, Some(project.root.join("deps/core")));
}

#[test]
fn dependency_parser_accepts_git_with_each_exclusive_selector() {
    let project = DependencyProject::new("git-selectors");
    for (rev, tag, branch) in [
        (Some("abc"), None, None),
        (None, Some("v1"), None),
        (None, None, Some("stable")),
        (None, None, None),
    ] {
        let mut entries = BTreeMap::new();
        entries.insert(
            "Core".to_string(),
            ManifestDependencyEntry::Detailed(ManifestDependencySection {
                path: None,
                git: Some("https://example.com/core.git".to_string()),
                version: None,
                rev: rev.map(str::to_string),
                tag: tag.map(str::to_string),
                branch: branch.map(str::to_string),
            }),
        );

        let (dependencies, issues) = parse_project_dependencies(&project.root, &entries);
        assert!(issues.is_empty(), "{issues:#?}");
        let git = dependencies[0].git.as_ref().expect("git dependency");
        assert_eq!(git.rev.as_deref(), rev);
        assert_eq!(git.tag.as_deref(), tag);
        assert_eq!(git.branch.as_deref(), branch);
    }
}

#[test]
fn dependency_parser_rejects_missing_or_multiple_sources() {
    let project = DependencyProject::new("source-exclusivity");
    for (path, git) in [
        (None, None),
        (
            Some("deps/core".to_string()),
            Some("https://example.com/core.git".to_string()),
        ),
    ] {
        let mut entries = BTreeMap::new();
        entries.insert(
            "Core".to_string(),
            ManifestDependencyEntry::Detailed(ManifestDependencySection {
                path,
                git,
                version: None,
                rev: None,
                tag: None,
                branch: None,
            }),
        );

        let (dependencies, issues) = parse_project_dependencies(&project.root, &entries);
        assert!(dependencies.is_empty());
        assert_eq!(issues.len(), 1);
        assert_eq!(issues[0].code, "L005");
    }
}

#[test]
fn dependency_parser_rejects_blank_simple_path_and_blank_name() {
    let project = DependencyProject::new("blank-identity");
    for (name, path) in [("Core", ""), ("   ", "deps/core")] {
        let mut entries = BTreeMap::new();
        entries.insert(
            name.to_string(),
            ManifestDependencyEntry::Path(path.to_string()),
        );

        let (dependencies, issues) = parse_project_dependencies(&project.root, &entries);
        assert!(dependencies.is_empty(), "{dependencies:#?}");
        assert_eq!(issues.len(), 1, "{issues:#?}");
        assert_eq!(issues[0].code, "L005");
    }
}

#[test]
fn dependency_parser_trims_identity_version_and_selector_values() {
    let project = DependencyProject::new("trimmed-fields");
    let mut entries = BTreeMap::new();
    entries.insert(
        "  Core  ".to_string(),
        ManifestDependencyEntry::Detailed(ManifestDependencySection {
            path: None,
            git: Some("  https://example.com/core.git  ".to_string()),
            version: Some("  1.2.3  ".to_string()),
            rev: Some("  abcdef  ".to_string()),
            tag: None,
            branch: None,
        }),
    );

    let (dependencies, issues) = parse_project_dependencies(&project.root, &entries);

    assert!(issues.is_empty(), "{issues:#?}");
    assert_eq!(dependencies[0].name, "Core");
    assert_eq!(dependencies[0].version.as_deref(), Some("1.2.3"));
    let git = dependencies[0].git.as_ref().expect("git dependency");
    assert_eq!(git.url, "https://example.com/core.git");
    assert_eq!(git.rev.as_deref(), Some("abcdef"));
}

#[test]
fn dependency_parser_rejects_blank_optional_version_and_selector() {
    let project = DependencyProject::new("blank-optionals");
    for (version, rev) in [(Some(" ".to_string()), None), (None, Some(" ".to_string()))] {
        let mut entries = BTreeMap::new();
        entries.insert(
            "Core".to_string(),
            ManifestDependencyEntry::Detailed(ManifestDependencySection {
                path: None,
                git: Some("https://example.com/core.git".to_string()),
                version,
                rev,
                tag: None,
                branch: None,
            }),
        );

        let (dependencies, issues) = parse_project_dependencies(&project.root, &entries);
        assert!(dependencies.is_empty(), "{dependencies:#?}");
        assert_eq!(issues.len(), 1, "{issues:#?}");
        assert_eq!(issues[0].code, "L005");
    }
}

#[test]
fn dependency_parser_rejects_multiple_git_selectors() {
    let project = DependencyProject::new("multiple-selectors");
    let mut entries = BTreeMap::new();
    entries.insert(
        "Core".to_string(),
        ManifestDependencyEntry::Detailed(ManifestDependencySection {
            path: None,
            git: Some("https://example.com/core.git".to_string()),
            version: None,
            rev: Some("abc".to_string()),
            tag: Some("v1".to_string()),
            branch: None,
        }),
    );

    let (dependencies, issues) = parse_project_dependencies(&project.root, &entries);

    assert!(dependencies.is_empty());
    assert_eq!(issues.len(), 1);
    assert_eq!(issues[0].code, "L005");
}

#[test]
fn dependency_parser_rejects_git_selector_on_path_entry() {
    let project = DependencyProject::new("path-selector");
    let mut entries = BTreeMap::new();
    entries.insert(
        "Core".to_string(),
        ManifestDependencyEntry::Detailed(ManifestDependencySection {
            path: Some("deps/core".to_string()),
            git: None,
            version: None,
            rev: None,
            tag: Some("v1".to_string()),
            branch: None,
        }),
    );

    let (dependencies, issues) = parse_project_dependencies(&project.root, &entries);

    assert!(dependencies.is_empty());
    assert_eq!(issues.len(), 1);
    assert_eq!(issues[0].code, "L005");
}

#[test]
fn dependency_parser_order_is_deterministic_by_case_insensitive_identity() {
    let project = DependencyProject::new("parse-order");
    let mut entries = BTreeMap::new();
    for name in ["zeta", "Beta", "alpha"] {
        entries.insert(
            name.to_string(),
            ManifestDependencyEntry::Path(format!("deps/{name}")),
        );
    }

    let (dependencies, issues) = parse_project_dependencies(&project.root, &entries);

    assert!(issues.is_empty());
    assert_eq!(
        dependencies
            .iter()
            .map(|dependency| dependency.name.as_str())
            .collect::<Vec<_>>(),
        vec!["alpha", "Beta", "zeta"]
    );
}

#[test]
fn dependency_resolver_resolves_transitive_local_graph() {
    let project = DependencyProject::new("transitive");
    project.mkdir("deps/a");
    project.mkdir("deps/b");
    project.write(
        "deps/a/trust-lsp.toml",
        "[package]\nversion = \"1\"\n[dependencies]\nB = { path = \"../b\", version = \"2\" }\n",
    );
    project.write("deps/b/trust-lsp.toml", "[package]\nversion = \"2\"\n");

    let config = project.config("[dependencies]\nA = { path = \"deps/a\", version = \"1\" }\n");

    assert!(
        config.dependency_resolution_issues.is_empty(),
        "{:#?}",
        config.dependency_resolution_issues
    );
    assert_eq!(
        config
            .libraries
            .iter()
            .map(|library| library.name.as_str())
            .collect::<Vec<_>>(),
        vec!["A", "B"]
    );
}

#[test]
fn dependency_resolver_allows_source_only_package_without_manifest() {
    let project = DependencyProject::new("source-only");
    let dependency = project.mkdir("deps/source-only");

    let config = project.config("[dependencies]\nSourceOnly = \"deps/source-only\"\n");

    assert!(config.dependency_resolution_issues.is_empty());
    let library = config
        .libraries
        .iter()
        .find(|library| library.name == "SourceOnly")
        .expect("source-only library");
    assert_eq!(
        library.path,
        dependency.canonicalize().expect("canonical dependency")
    );
    assert_eq!(library.version, None);
}

#[test]
fn dependency_resolver_reports_missing_or_nondirectory_path_without_library() {
    let project = DependencyProject::new("missing-paths");
    project.write("deps/file", "not a directory");
    for (name, path) in [("Missing", "deps/missing"), ("File", "deps/file")] {
        let config = project.config(&format!("[dependencies]\n{name} = \"{path}\"\n"));

        assert!(dependency_issue(&config, "L001", name).is_some());
        assert!(!config.libraries.iter().any(|library| library.name == name));
    }
}

#[test]
fn dependency_resolver_reports_malformed_manifest_without_partial_library() {
    let project = DependencyProject::new("malformed-manifest");
    project.mkdir("deps/broken");
    project.write("deps/broken/trust-lsp.toml", "[package\n");

    let config = project.config("[dependencies]\nBroken = \"deps/broken\"\n");

    assert!(dependency_issue(&config, "L001", "Broken").is_some());
    assert!(!config
        .libraries
        .iter()
        .any(|library| library.name == "Broken"));
}

#[test]
fn dependency_resolver_reports_exact_version_mismatch() {
    let project = DependencyProject::new("version-mismatch");
    project.mkdir("deps/core");
    project.write(
        "deps/core/trust-lsp.toml",
        "[package]\nversion = \"2.0.0\"\n",
    );

    let config =
        project.config("[dependencies]\nCore = { path = \"deps/core\", version = \"1.0.0\" }\n");

    let issue = dependency_issue(&config, "L002", "Core").expect("version issue");
    assert!(issue.message.contains("1.0.0"));
    assert!(issue.message.contains("2.0.0"));
}

#[test]
fn dependency_resolver_reports_self_cycle_with_cycle_path() {
    let project = DependencyProject::new("self-cycle");
    project.mkdir("deps/a");
    project.write(
        "deps/a/trust-lsp.toml",
        "[dependencies]\nA = { path = \".\" }\n",
    );

    let config = project.config("[dependencies]\nA = \"deps/a\"\n");

    let issue = dependency_issue(&config, "L004", "A").expect("cycle issue");
    assert!(issue.message.contains("A -> A"), "{}", issue.message);
}

#[test]
fn dependency_resolver_reports_long_cycle_with_complete_path() {
    let project = DependencyProject::new("long-cycle");
    project.mkdir("deps/a");
    project.mkdir("deps/b");
    project.mkdir("deps/c");
    project.write("deps/a/trust-lsp.toml", "[dependencies]\nB = \"../b\"\n");
    project.write("deps/b/trust-lsp.toml", "[dependencies]\nC = \"../c\"\n");
    project.write("deps/c/trust-lsp.toml", "[dependencies]\nA = \"../a\"\n");

    let config = project.config("[dependencies]\nA = \"deps/a\"\n");

    let issue = dependency_issue(&config, "L004", "A").expect("cycle issue");
    for name in ["A", "B", "C"] {
        assert!(issue.message.contains(name), "{}", issue.message);
    }
}

#[test]
fn dependency_resolver_deduplicates_case_insensitive_same_source() {
    let project = DependencyProject::new("case-dedup");
    project.mkdir("deps/core");
    project.mkdir("deps/parent");
    project.write(
        "deps/parent/trust-lsp.toml",
        "[dependencies]\ncore = \"../core\"\n",
    );

    let config = project.config("[dependencies]\nCore = \"deps/core\"\nParent = \"deps/parent\"\n");

    assert!(config.dependency_resolution_issues.is_empty());
    assert_eq!(
        config
            .libraries
            .iter()
            .filter(|library| library.name.eq_ignore_ascii_case("core"))
            .count(),
        1
    );
}

#[test]
fn dependency_resolver_reports_case_insensitive_source_conflict() {
    let project = DependencyProject::new("source-conflict");
    project.mkdir("deps/core-a");
    project.mkdir("deps/core-b");
    project.mkdir("deps/parent");
    project.write(
        "deps/parent/trust-lsp.toml",
        "[dependencies]\ncore = \"../core-b\"\n",
    );

    let config =
        project.config("[dependencies]\nCore = \"deps/core-a\"\nParent = \"deps/parent\"\n");

    assert!(dependency_issue(&config, "L003", "Core").is_some());
}

#[test]
fn dependency_resolver_reports_diamond_version_conflict() {
    let project = DependencyProject::new("diamond-version");
    for path in ["deps/a", "deps/b", "deps/core"] {
        project.mkdir(path);
    }
    project.write("deps/core/trust-lsp.toml", "[package]\nversion = \"2\"\n");
    project.write(
        "deps/a/trust-lsp.toml",
        "[dependencies]\nCore = { path = \"../core\", version = \"1\" }\n",
    );
    project.write(
        "deps/b/trust-lsp.toml",
        "[dependencies]\nCore = { path = \"../core\", version = \"2\" }\n",
    );

    let config = project.config("[dependencies]\nA = \"deps/a\"\nB = \"deps/b\"\n");

    assert!(dependency_issue(&config, "L002", "Core").is_some());
}

#[cfg(unix)]
#[test]
fn dependency_resolver_canonicalizes_local_symlink_source() {
    use std::os::unix::fs::symlink;

    let project = DependencyProject::new("symlink-canonical");
    let actual = project.mkdir("deps/actual");
    symlink("actual", project.root.join("deps/alias")).expect("create dependency symlink");

    let config = project.config("[dependencies]\nCore = \"deps/alias\"\n");

    assert!(config.dependency_resolution_issues.is_empty());
    assert_eq!(
        config
            .libraries
            .iter()
            .find(|library| library.name == "Core")
            .expect("Core")
            .path,
        actual.canonicalize().expect("canonical actual")
    );
}

#[test]
fn dependency_resolver_keeps_independent_valid_component_when_another_fails() {
    let project = DependencyProject::new("independent-components");
    project.mkdir("deps/valid");

    let config =
        project.config("[dependencies]\nMissing = \"deps/missing\"\nValid = \"deps/valid\"\n");

    assert!(dependency_issue(&config, "L001", "Missing").is_some());
    assert!(config
        .libraries
        .iter()
        .any(|library| library.name == "Valid"));
}

#[test]
fn dependency_lock_absent_loads_empty_versioned_model() {
    let project = DependencyProject::new("absent-lock");
    let lock = load_dependency_lock(&project.root.join("missing.lock")).expect("load absent");

    assert_eq!(lock.version, dependency_lock_version());
    assert!(lock.dependencies.is_empty());
}

#[test]
fn dependency_lock_roundtrip_preserves_source_kinds_and_deterministic_order() {
    let project = DependencyProject::new("lock-roundtrip");
    let path = project.root.join("trust-lsp.lock");
    let mut dependencies = BTreeMap::new();
    dependencies.insert(
        "Zeta".to_string(),
        DependencyLockEntry::Git {
            url: "https://example.com/zeta.git".to_string(),
            rev: "abcdef".to_string(),
        },
    );
    dependencies.insert(
        "Alpha".to_string(),
        DependencyLockEntry::Path {
            path: "/deps/alpha".to_string(),
        },
    );

    write_dependency_lock(&path, dependencies).expect("write lock");
    let text = std::fs::read_to_string(&path).expect("read lock");
    let lock = load_dependency_lock(&path).expect("load lock");

    assert!(text.starts_with("version = 1\n"));
    assert!(text.find("[dependencies.Alpha]").unwrap() < text.find("[dependencies.Zeta]").unwrap());
    assert!(matches!(
        lock.dependencies.get("Alpha"),
        Some(DependencyLockEntry::Path { path }) if path == "/deps/alpha"
    ));
    assert!(matches!(
        lock.dependencies.get("Zeta"),
        Some(DependencyLockEntry::Git { url, rev })
            if url == "https://example.com/zeta.git" && rev == "abcdef"
    ));
}

#[test]
fn dependency_lock_malformed_content_is_l006_and_blocks_locked_resolution() {
    let project = DependencyProject::new("malformed-lock");
    project.mkdir("deps/core");
    project.write("trust-lsp.lock", "[dependencies\n");

    let config = project
        .config("[build]\ndependencies_locked = true\n[dependencies]\nCore = \"deps/core\"\n");

    assert!(dependency_issue(&config, "L006", "lockfile").is_some());
    assert!(!config
        .libraries
        .iter()
        .any(|library| library.name == "Core"));
}

#[test]
fn dependency_lock_unsupported_version_is_l006() {
    let project = DependencyProject::new("lock-version");
    let dependency = project.mkdir("deps/core").canonicalize().unwrap();
    project.write(
        "trust-lsp.lock",
        &format!(
            "version = 99\n[dependencies.Core]\nsource = \"path\"\npath = {:?}\n",
            dependency.to_string_lossy()
        ),
    );

    let config = project
        .config("[build]\ndependencies_locked = true\n[dependencies]\nCore = \"deps/core\"\n");

    assert!(dependency_issue(&config, "L006", "lockfile").is_some());
}

#[test]
fn dependency_locked_local_path_requires_matching_canonical_entry() {
    let project = DependencyProject::new("locked-path-match");
    let core = project.mkdir("deps/core").canonicalize().unwrap();
    path_lock(&project.root.join("trust-lsp.lock"), "Core", &core);

    let config = project
        .config("[build]\ndependencies_locked = true\n[dependencies]\nCore = \"deps/core\"\n");

    assert!(
        config.dependency_resolution_issues.is_empty(),
        "{:#?}",
        config.dependency_resolution_issues
    );
    assert!(config
        .libraries
        .iter()
        .any(|library| library.name == "Core"));
}

#[test]
fn dependency_locked_local_path_mismatch_is_l006() {
    let project = DependencyProject::new("locked-path-mismatch");
    project.mkdir("deps/core");
    let other = project.mkdir("deps/other").canonicalize().unwrap();
    path_lock(&project.root.join("trust-lsp.lock"), "Core", &other);

    let config = project
        .config("[build]\ndependencies_locked = true\n[dependencies]\nCore = \"deps/core\"\n");

    assert!(dependency_issue(&config, "L006", "Core").is_some());
    assert!(!config
        .libraries
        .iter()
        .any(|library| library.name == "Core"));
}

#[test]
fn dependency_locked_local_path_missing_entry_is_l006() {
    let project = DependencyProject::new("locked-path-missing");
    project.mkdir("deps/core");
    project.write("trust-lsp.lock", "version = 1\n");

    let config = project
        .config("[build]\ndependencies_locked = true\n[dependencies]\nCore = \"deps/core\"\n");

    assert!(dependency_issue(&config, "L006", "Core").is_some());
}

#[test]
fn dependency_unlocked_resolution_records_canonical_path() {
    let project = DependencyProject::new("path-lock-write");
    let core = project.mkdir("deps/core").canonicalize().unwrap();

    let config = project.config("[dependencies]\nCore = \"deps/core\"\n");
    let lock = load_dependency_lock(&project.root.join("trust-lsp.lock")).expect("load lock");

    assert!(config.dependency_resolution_issues.is_empty());
    assert!(matches!(
        lock.dependencies.get("Core"),
        Some(DependencyLockEntry::Path { path })
            if Path::new(path) == core
    ));
}

#[test]
fn dependency_resolution_with_any_issue_does_not_publish_new_lock() {
    let project = DependencyProject::new("no-partial-lock");

    let config = project.config("[dependencies]\nMissing = \"deps/missing\"\n");

    assert!(dependency_issue(&config, "L001", "Missing").is_some());
    assert!(!project.root.join("trust-lsp.lock").exists());
}

#[test]
fn dependency_resolution_issue_preserves_existing_lock_byte_for_byte() {
    let project = DependencyProject::new("preserve-lock");
    let original = "version = 1\n\n[dependencies.Old]\nsource = \"path\"\npath = \"/old\"\n";
    project.write("trust-lsp.lock", original);

    let _config = project.config("[dependencies]\nMissing = \"deps/missing\"\n");

    assert_eq!(
        std::fs::read_to_string(project.root.join("trust-lsp.lock")).unwrap(),
        original
    );
}

#[test]
fn dependency_custom_lock_path_is_project_relative() {
    let project = DependencyProject::new("custom-lock");
    project.mkdir("deps/core");

    let config = project.config(
        "[build]\ndependency_lockfile = \"locks/dependencies.lock\"\n[dependencies]\nCore = \"deps/core\"\n",
    );

    assert!(config.dependency_resolution_issues.is_empty());
    assert!(project.root.join("locks/dependencies.lock").is_file());
    assert!(!project.root.join("trust-lsp.lock").exists());
}

#[test]
fn dependency_git_policy_accepts_local_sources_and_https_by_default() {
    let policy = DependencyPolicy::default();

    for source in [
        "./repo",
        "../repo",
        "/absolute/repo",
        "C:\\repo",
        "file:///repo",
        "https://example.com/repo.git",
    ] {
        assert!(
            validate_git_source_policy(source, &policy).is_ok(),
            "{source}"
        );
    }
}

#[test]
fn dependency_git_policy_blocks_http_ssh_and_scp_by_default() {
    let policy = DependencyPolicy::default();

    for source in [
        "http://example.com/repo.git",
        "ssh://git@example.com/repo.git",
        "git@example.com:repo.git",
    ] {
        assert!(
            validate_git_source_policy(source, &policy).is_err(),
            "{source}"
        );
    }
}

#[test]
fn dependency_git_policy_flags_enable_http_and_ssh_independently() {
    let http = DependencyPolicy {
        allow_http: true,
        ..DependencyPolicy::default()
    };
    let ssh = DependencyPolicy {
        allow_ssh: true,
        ..DependencyPolicy::default()
    };

    assert!(validate_git_source_policy("http://example.com/repo.git", &http).is_ok());
    assert!(validate_git_source_policy("ssh://git@example.com/repo.git", &http).is_err());
    assert!(validate_git_source_policy("ssh://git@example.com/repo.git", &ssh).is_ok());
    assert!(validate_git_source_policy("git@example.com:repo.git", &ssh).is_ok());
    assert!(validate_git_source_policy("http://example.com/repo.git", &ssh).is_err());
}

#[test]
fn dependency_git_host_allowlist_accepts_exact_and_subdomains_case_insensitively() {
    let policy = DependencyPolicy {
        allowed_git_hosts: vec!["example.com".to_string()],
        ..DependencyPolicy::default()
    };

    for source in [
        "https://example.com/repo.git",
        "https://EXAMPLE.COM/repo.git",
        "https://git.example.com/repo.git",
        "https://deep.git.example.com/repo.git",
    ] {
        assert!(
            validate_git_source_policy(source, &policy).is_ok(),
            "{source}"
        );
    }
}

#[test]
fn dependency_git_host_allowlist_rejects_suffix_lookalikes() {
    let policy = DependencyPolicy {
        allowed_git_hosts: vec!["example.com".to_string()],
        ..DependencyPolicy::default()
    };

    for source in [
        "https://badexample.com/repo.git",
        "https://example.com.evil.test/repo.git",
        "https://not-example.com/repo.git",
    ] {
        assert!(
            validate_git_source_policy(source, &policy).is_err(),
            "{source}"
        );
    }
}

#[test]
fn dependency_git_host_parser_ignores_userinfo_and_port() {
    assert_eq!(
        extract_git_host("user:secret@example.com:8443/repo.git"),
        Some("example.com".to_string())
    );
    assert_eq!(
        extract_git_host("example.com:443/repo.git"),
        Some("example.com".to_string())
    );
}

#[test]
fn dependency_git_host_parser_preserves_bracketed_ipv6_identity() {
    assert_eq!(
        extract_git_host("[2001:db8::1]:8443/repo.git"),
        Some("2001:db8::1".to_string())
    );
}

#[test]
fn dependency_git_policy_rejects_blank_and_unknown_schemes() {
    let policy = DependencyPolicy::default();
    for source in [
        "",
        "   ",
        "git://example.com/repo.git",
        "ftp://example.com/repo",
    ] {
        assert!(
            validate_git_source_policy(source, &policy).is_err(),
            "{source}"
        );
    }
}

#[test]
fn dependency_cache_components_are_stable_and_path_safe() {
    for name in [
        "Vendor/Core",
        " Vendor Core ",
        "vendor@core",
        "../../escape",
    ] {
        let sanitized = sanitize_for_path(name);
        assert!(!sanitized.contains('/'), "{sanitized}");
        assert!(!sanitized.contains('\\'), "{sanitized}");
        assert!(!sanitized.starts_with('.'), "{sanitized}");
        assert!(!sanitized.ends_with('.'), "{sanitized}");
    }

    let url = "https://user:secret@example.com/vendor/core.git";
    let first = stable_hash_hex(url);
    assert_eq!(first, stable_hash_hex(url));
    assert_eq!(first.len(), 16);
    assert_ne!(
        first,
        stable_hash_hex("https://example.com/vendor/other.git")
    );
    assert!(!first.contains("secret"));
}
