use std::path::PathBuf;
use std::sync::atomic::{AtomicU64, Ordering};

use super::*;

static NEXT_FIXTURE: AtomicU64 = AtomicU64::new(1);

struct TempPath {
    root: PathBuf,
}

impl TempPath {
    fn new(label: &str) -> Self {
        let root = std::env::temp_dir().join(format!(
            "trust-web-ide-paths-{}-{label}-{}",
            std::process::id(),
            NEXT_FIXTURE.fetch_add(1, Ordering::Relaxed)
        ));
        std::fs::create_dir_all(&root).expect("create root");
        Self { root }
    }
}

impl Drop for TempPath {
    fn drop(&mut self) {
        let _ = std::fs::remove_dir_all(&self.root);
    }
}

#[test]
fn workspace_path_trims_and_normalizes_current_directory_components() {
    assert_eq!(
        normalize_workspace_path("  src/./nested/main.st  ", false).expect("path"),
        "src/nested/main.st"
    );
}

#[test]
fn workspace_path_uses_forward_slash_projection() {
    assert_eq!(
        normalize_workspace_file_path("src/main.st").expect("path"),
        "src/main.st"
    );
    assert_eq!(
        pathbuf_to_display(PathBuf::from(r"src\nested\main.st")),
        "src/nested/main.st"
    );
}

#[test]
fn empty_workspace_path_requires_explicit_root_permission() {
    assert_eq!(normalize_workspace_path(" \t ", true).expect("root"), "");
    assert_eq!(
        normalize_workspace_path("", false).unwrap_err().kind(),
        IdeErrorKind::InvalidInput
    );
}

#[test]
fn current_directory_path_maps_to_root_only_when_root_is_allowed() {
    assert_eq!(normalize_workspace_path(".", true).expect("root"), "");
    assert_eq!(
        normalize_workspace_path(".", false).unwrap_err().kind(),
        IdeErrorKind::InvalidInput
    );
}

#[test]
fn absolute_and_parent_workspace_paths_are_forbidden() {
    for raw in [
        "/etc/passwd",
        "../main.st",
        "src/../../main.st",
        "src/..",
        r"C:\Windows\system.ini",
    ] {
        let error = normalize_workspace_path(raw, false).expect_err(raw);
        assert_eq!(error.kind(), IdeErrorKind::Forbidden, "{raw}");
    }
}

#[test]
fn hidden_workspace_components_are_forbidden() {
    for raw in [".git/config", "src/.secret.st", ".env", "src/..hidden/file"] {
        let error = normalize_workspace_path(raw, false).expect_err(raw);
        assert_eq!(error.kind(), IdeErrorKind::Forbidden, "{raw}");
    }
}

#[test]
fn source_path_requires_st_extension_case_insensitively() {
    assert_eq!(
        normalize_source_path("nested/Main.ST").expect("source"),
        "nested/Main.ST"
    );
    for raw in ["main", "main.txt", "main.st.bak", "nested/"] {
        let error = normalize_source_path(raw).expect_err(raw);
        assert_eq!(error.kind(), IdeErrorKind::InvalidInput, "{raw}");
    }
}

#[test]
fn project_root_rejects_blank_and_preserves_absolute_path() {
    assert_eq!(
        normalize_project_root("").unwrap_err().kind(),
        IdeErrorKind::InvalidInput
    );
    let absolute = std::env::temp_dir().join("trust-project-root");
    assert_eq!(
        normalize_project_root(absolute.to_string_lossy().as_ref()).expect("absolute"),
        absolute
    );
}

#[test]
fn relative_project_root_resolves_from_current_directory() {
    assert_eq!(
        normalize_project_root("relative/project").expect("relative"),
        std::env::current_dir()
            .expect("current directory")
            .join("relative/project")
    );
}

#[test]
fn closest_existing_parent_walks_up_from_missing_descendant() {
    let fixture = TempPath::new("existing-parent");
    let missing = fixture.root.join("missing").join("child");
    assert_eq!(
        closest_existing_parent(Some(&missing), &fixture.root).expect("parent"),
        fixture.root.canonicalize().expect("canonical root")
    );
}

#[test]
fn closest_existing_parent_uses_fallback_when_cursor_is_absent() {
    let fixture = TempPath::new("fallback");
    assert_eq!(
        closest_existing_parent(None, &fixture.root).expect("fallback"),
        fixture.root
    );
}

#[test]
fn source_fingerprint_reports_exact_byte_size() {
    let fixture = TempPath::new("fingerprint");
    let path = fixture.root.join("main.st");
    std::fs::write(&path, "å\n").expect("write source");
    let fingerprint = source_fingerprint(&path).expect("fingerprint");
    assert_eq!(fingerprint.size_bytes, 3);
    assert!(fingerprint.modified_ms > 0);
}

#[test]
fn source_fingerprint_missing_file_is_not_found() {
    let fixture = TempPath::new("fingerprint-missing");
    assert_eq!(
        source_fingerprint(&fixture.root.join("missing.st"))
            .unwrap_err()
            .kind(),
        IdeErrorKind::NotFound
    );
}

#[test]
fn source_read_accepts_exact_byte_limit() {
    let fixture = TempPath::new("read-exact");
    let path = fixture.root.join("main.st");
    std::fs::write(&path, "åx").expect("write source");
    assert_eq!(read_source_with_limit(&path, 3).expect("exact"), "åx");
}

#[test]
fn source_read_rejects_one_byte_over_limit() {
    let fixture = TempPath::new("read-large");
    let path = fixture.root.join("main.st");
    std::fs::write(&path, "åx").expect("write source");
    let error = read_source_with_limit(&path, 2).expect_err("too large");
    assert_eq!(error.kind(), IdeErrorKind::TooLarge);
    assert!(error.to_string().contains("3 > 2 bytes"));
}

#[test]
fn source_read_missing_file_is_not_found() {
    let fixture = TempPath::new("read-missing");
    assert_eq!(
        read_source_with_limit(&fixture.root.join("missing.st"), 100)
            .unwrap_err()
            .kind(),
        IdeErrorKind::NotFound
    );
}

#[test]
fn known_project_templates_have_expected_primary_pou() {
    assert!(project_template_source("blinky", "ignored").contains("PROGRAM Main"));
    assert!(project_template_source("pid_loop", "ignored").contains("pid : PID_FB;"));
    assert!(project_template_source("motor_control", "ignored").contains("motor : MOTOR_FB;"));
}

#[test]
fn unknown_project_template_uses_minimal_program() {
    assert_eq!(
        project_template_source("unknown", "ignored"),
        "PROGRAM Main\nVAR\nEND_VAR\nEND_PROGRAM\n"
    );
}

#[test]
fn project_template_extra_sources_are_closed_and_deterministic() {
    assert!(project_template_extra_sources("blinky").is_empty());
    assert!(project_template_extra_sources("unknown").is_empty());
    assert_eq!(
        project_template_extra_sources("pid_loop")
            .iter()
            .map(|(path, _)| path.as_str())
            .collect::<Vec<_>>(),
        vec!["pid_fb.st"]
    );
    assert_eq!(
        project_template_extra_sources("motor_control")
            .iter()
            .map(|(path, _)| path.as_str())
            .collect::<Vec<_>>(),
        vec!["motor_fb.st", "safety.st"]
    );
}

#[test]
fn absent_or_blank_glob_disables_filter() {
    assert!(compile_glob_pattern(None, "include")
        .expect("absent")
        .is_none());
    assert!(compile_glob_pattern(Some(" \t "), "include")
        .expect("blank")
        .is_none());
}

#[test]
fn valid_glob_is_trimmed_and_matches() {
    let pattern = compile_glob_pattern(Some("  src/**/*.st  "), "include")
        .expect("glob")
        .expect("pattern");
    assert!(pattern.matches("src/nested/main.st"));
    assert!(!pattern.matches("tests/main.st"));
}

#[test]
fn malformed_glob_is_invalid_input_and_names_field() {
    let error = compile_glob_pattern(Some("["), "exclude").expect_err("invalid glob");
    assert_eq!(error.kind(), IdeErrorKind::InvalidInput);
    assert!(error.to_string().contains("invalid exclude glob pattern"));
}
