use std::sync::atomic::{AtomicU64, Ordering};

use super::*;

static NEXT_FIXTURE: AtomicU64 = AtomicU64::new(1);

struct TempWorkspace {
    root: PathBuf,
}

impl TempWorkspace {
    fn new(label: &str) -> Self {
        let root = std::env::temp_dir().join(format!(
            "trust-config-ui-workspace-{}-{label}-{}",
            std::process::id(),
            NEXT_FIXTURE.fetch_add(1, Ordering::Relaxed)
        ));
        std::fs::create_dir_all(&root).expect("create workspace");
        Self { root }
    }

    fn runtime(&self, folder: &str, runtime_id: &str) -> PathBuf {
        let root = self.root.join(folder);
        std::fs::create_dir_all(&root).expect("create runtime");
        std::fs::write(
            root.join("runtime.toml"),
            render_new_runtime_toml(runtime_id, None),
        )
        .expect("write runtime");
        root
    }
}

impl Drop for TempWorkspace {
    fn drop(&mut self) {
        let _ = std::fs::remove_dir_all(&self.root);
    }
}

fn accept_text(text: &str) -> Result<(), RuntimeError> {
    if text.is_empty() {
        Err(RuntimeError::InvalidConfig("text is empty".into()))
    } else {
        Ok(())
    }
}

fn reject_text(_text: &str) -> Result<(), RuntimeError> {
    Err(RuntimeError::InvalidConfig("candidate rejected".into()))
}

#[test]
fn runtime_id_is_trimmed_and_ascii_lowercased() {
    assert_eq!(
        normalize_runtime_id("  Line_A-17  ").expect("runtime ID"),
        "line_a-17"
    );
}

#[test]
fn runtime_id_accepts_each_documented_character_class() {
    for raw in ["a", "A0", "line-1", "line_1", "0"] {
        assert!(normalize_runtime_id(raw).is_ok(), "{raw}");
    }
}

#[test]
fn runtime_id_rejects_empty_or_whitespace_only_input() {
    for raw in ["", " ", "\t\n"] {
        assert!(normalize_runtime_id(raw).is_err(), "{raw:?}");
    }
}

#[test]
fn runtime_id_rejects_punctuation_unicode_and_embedded_whitespace() {
    for raw in ["line.one", "line/one", "line one", "line:one", "lïne"] {
        assert!(normalize_runtime_id(raw).is_err(), "{raw}");
    }
}

#[test]
fn host_group_normalization_preserves_documented_characters() {
    assert_eq!(
        normalize_host_group(Some("  Plant_A-17  ")).as_deref(),
        Some("plant_a-17")
    );
}

#[test]
fn host_group_normalization_replaces_other_characters() {
    assert_eq!(
        normalize_host_group(Some("Plant / Cell.1")).as_deref(),
        Some("plant---cell-1")
    );
}

#[test]
fn absent_or_empty_host_group_stays_absent() {
    assert_eq!(normalize_host_group(None), None);
    assert_eq!(normalize_host_group(Some("")), None);
    assert_eq!(normalize_host_group(Some(" / ")), None);
}

#[test]
fn generated_runtime_toml_binds_the_runtime_identity() {
    let text = render_new_runtime_toml("line_a", Some("plant_1"));
    assert!(text.contains("name = \"line_a\""));
    assert!(text.contains("endpoint = \"unix:///tmp/line_a.sock\""));
    assert!(text.contains("service_name = \"line_a\""));
    assert!(text.contains("host_group = \"plant_1\""));
    assert!(text.contains("profile = \"dev\""));
}

#[test]
fn generated_runtime_toml_uses_local_safe_defaults() {
    let text = render_new_runtime_toml("line_a", None);
    assert!(text.contains("listen = \"127.0.0.1:0\""));
    assert!(text.contains("auth = \"local\""));
    assert!(text.contains("tls = false"));
    assert!(text.contains("host_group = \"default-host\""));
    assert!(text.contains("debug_enabled = false"));
}

#[test]
fn generated_runtime_toml_passes_the_runtime_validator() {
    let text = render_new_runtime_toml("line_a", Some("plant_1"));
    crate::config::validate_runtime_toml_text(&text).expect("valid generated runtime");
}

#[test]
fn workspace_loader_accepts_runtime_at_selected_root() {
    let fixture = TempWorkspace::new("root-runtime");
    std::fs::write(
        fixture.root.join("runtime.toml"),
        render_new_runtime_toml("root_runtime", None),
    )
    .expect("write runtime");

    let workspace = load_workspace_model(&Some(fixture.root.clone())).expect("load root runtime");
    assert_eq!(workspace.root, fixture.root);
    assert_eq!(workspace.runtimes.len(), 1);
    assert_eq!(workspace.runtimes[0].runtime_id, "root_runtime");
    assert_eq!(workspace.runtimes[0].root, fixture.root);
}

#[test]
fn workspace_loader_discovers_only_direct_child_runtimes() {
    let fixture = TempWorkspace::new("direct-children");
    fixture.runtime("direct", "direct");
    let nested = fixture.root.join("outer").join("nested");
    std::fs::create_dir_all(&nested).expect("create nested");
    std::fs::write(
        nested.join("runtime.toml"),
        render_new_runtime_toml("nested", None),
    )
    .expect("write nested runtime");

    let workspace = load_workspace_model(&Some(fixture.root.clone())).expect("load workspace");
    assert_eq!(
        workspace
            .runtimes
            .iter()
            .map(|runtime| runtime.runtime_id.as_str())
            .collect::<Vec<_>>(),
        vec!["direct"]
    );
}

#[test]
fn workspace_loader_sorts_runtimes_by_id() {
    let fixture = TempWorkspace::new("sort");
    fixture.runtime("z-folder", "zeta");
    fixture.runtime("a-folder", "alpha");

    let workspace = load_workspace_model(&Some(fixture.root.clone())).expect("load workspace");
    assert_eq!(
        workspace
            .runtimes
            .iter()
            .map(|runtime| runtime.runtime_id.as_str())
            .collect::<Vec<_>>(),
        vec!["alpha", "zeta"]
    );
}

#[test]
fn workspace_loader_rejects_an_empty_workspace() {
    let fixture = TempWorkspace::new("empty");
    let error = load_workspace_model(&Some(fixture.root.clone())).expect_err("empty workspace");
    assert!(error.to_string().contains("no runtime.toml found"));
}

#[test]
fn workspace_loader_rejects_duplicate_resource_names() {
    let fixture = TempWorkspace::new("duplicate");
    fixture.runtime("one", "duplicate");
    fixture.runtime("two", "duplicate");

    let error =
        load_workspace_model(&Some(fixture.root.clone())).expect_err("duplicate runtime ID");
    assert!(error
        .to_string()
        .contains("duplicate runtime.resource.name 'duplicate'"));
}

#[test]
fn runtime_resolution_is_exact() {
    let fixture = TempWorkspace::new("resolve");
    fixture.runtime("line_a", "line_a");
    let workspace = load_workspace_model(&Some(fixture.root.clone())).expect("workspace");

    assert_eq!(
        resolve_runtime_by_id(&workspace, "line_a")
            .expect("known runtime")
            .runtime_id,
        "line_a"
    );
    assert!(resolve_runtime_by_id(&workspace, "LINE_A").is_err());
    assert!(resolve_runtime_by_id(&workspace, "line").is_err());
}

#[test]
fn text_revision_is_exact_lowercase_sha256() {
    assert_eq!(
        text_revision("abc"),
        "ba7816bf8f01cfea414140de5dae2223b00361a396177a9cb410ff61f20015ad"
    );
}

#[test]
fn text_revision_changes_for_whitespace_and_line_endings() {
    assert_ne!(text_revision("abc"), text_revision("abc "));
    assert_ne!(text_revision("a\nb\n"), text_revision("a\r\nb\r\n"));
}

#[test]
fn atomic_write_creates_parents_and_replaces_exact_text() {
    let fixture = TempWorkspace::new("atomic-write");
    let path = fixture.root.join("nested").join("runtime.toml");

    atomic_write_text(&path, "first").expect("first write");
    assert_eq!(std::fs::read_to_string(&path).expect("read first"), "first");
    atomic_write_text(&path, "second\n").expect("replacement");
    assert_eq!(
        std::fs::read_to_string(&path).expect("read replacement"),
        "second\n"
    );
}

#[test]
fn matching_revision_allows_validated_replacement() {
    let fixture = TempWorkspace::new("revision-match");
    let path = fixture.root.join("runtime.toml");
    std::fs::write(&path, "old").expect("write old");
    let expected = text_revision("old");

    let revision = write_config_file(&path, "new", Some(&expected), accept_text).expect("replace");
    assert_eq!(revision, text_revision("new"));
    assert_eq!(std::fs::read_to_string(&path).expect("read"), "new");
}

#[test]
fn stale_revision_rejects_without_changing_the_file() {
    let fixture = TempWorkspace::new("revision-stale");
    let path = fixture.root.join("runtime.toml");
    std::fs::write(&path, "current").expect("write current");

    let error = write_config_file(&path, "candidate", Some("stale"), accept_text)
        .expect_err("revision conflict");
    assert!(error
        .to_string()
        .contains(&format!("conflict: {}", text_revision("current"))));
    assert_eq!(
        std::fs::read_to_string(&path).expect("read current"),
        "current"
    );
}

#[test]
fn validation_failure_rejects_without_changing_the_file() {
    let fixture = TempWorkspace::new("validation-failure");
    let path = fixture.root.join("runtime.toml");
    std::fs::write(&path, "current").expect("write current");

    let error =
        write_config_file(&path, "candidate", None, reject_text).expect_err("invalid candidate");
    assert!(error.to_string().contains("candidate rejected"));
    assert_eq!(
        std::fs::read_to_string(&path).expect("read current"),
        "current"
    );
}

#[test]
fn blank_expected_revision_means_no_precondition() {
    let fixture = TempWorkspace::new("blank-revision");
    let path = fixture.root.join("runtime.toml");
    std::fs::write(&path, "old").expect("write old");

    write_config_file(&path, "new", Some(" \t"), accept_text).expect("no precondition");
    assert_eq!(std::fs::read_to_string(&path).expect("read"), "new");
}

#[test]
fn structured_text_path_is_trimmed_relative_and_case_insensitive() {
    assert_eq!(
        normalize_st_relative_path("  nested/Main.ST  ").expect("ST path"),
        PathBuf::from("nested/Main.ST")
    );
    assert_eq!(
        normalize_st_relative_path("./main.st").expect("current directory"),
        PathBuf::from("./main.st")
    );
}

#[test]
fn structured_text_path_rejects_empty_absolute_and_parent_paths() {
    for raw in [
        "",
        " ",
        "/tmp/main.st",
        "../main.st",
        "nested/../../main.st",
    ] {
        assert!(normalize_st_relative_path(raw).is_err(), "{raw:?}");
    }
}

#[test]
fn structured_text_path_rejects_other_extensions() {
    for raw in ["main", "main.txt", "main.st.bak", ".st"] {
        assert!(normalize_st_relative_path(raw).is_err(), "{raw:?}");
    }
}

#[test]
fn absent_project_io_file_projects_safe_loopback_default() {
    let fixture = TempWorkspace::new("default-io");
    let response = load_project_io_config_response(&fixture.root).expect("default I/O projection");
    assert_eq!(response.driver, "loopback");
    assert_eq!(response.source, "default");
    assert!(!response.use_system_io);
    assert!(response.drivers.is_empty());
    assert!(response.safe_state.is_empty());
}

#[test]
fn runtime_creation_writes_the_complete_minimal_bundle() {
    let fixture = TempWorkspace::new("create");
    fixture.runtime("existing", "existing");
    let workspace = load_workspace_model(&Some(fixture.root.clone())).expect("workspace");

    let result =
        create_workspace_runtime(&workspace, " Line_B ", Some(" Plant 1 ")).expect("create");
    let runtime_root = fixture.root.join("line_b");
    assert_eq!(result["runtime_id"], "line_b");
    assert!(runtime_root.join("runtime.toml").is_file());
    assert!(runtime_root.join("io.toml").is_file());
    assert!(runtime_root.join("src/main.st").is_file());
    assert!(std::fs::read_to_string(runtime_root.join("runtime.toml"))
        .expect("runtime text")
        .contains("host_group = \"plant-1\""));
}

#[test]
fn runtime_creation_rejects_existing_id_or_folder() {
    let fixture = TempWorkspace::new("create-existing");
    fixture.runtime("existing", "existing");
    let workspace = load_workspace_model(&Some(fixture.root.clone())).expect("workspace");
    assert!(create_workspace_runtime(&workspace, "existing", None).is_err());

    std::fs::create_dir_all(fixture.root.join("folder_only")).expect("folder");
    assert!(create_workspace_runtime(&workspace, "folder_only", None).is_err());
}

#[test]
fn deleting_the_last_runtime_is_rejected_without_removal() {
    let fixture = TempWorkspace::new("delete-last");
    let runtime_root = fixture.runtime("only", "only");
    let workspace = load_workspace_model(&Some(fixture.root.clone())).expect("workspace");

    assert!(delete_workspace_runtime(&workspace, "only").is_err());
    assert!(runtime_root.exists());
}

#[test]
fn deleting_a_child_runtime_removes_only_that_runtime() {
    let fixture = TempWorkspace::new("delete-child");
    let keep = fixture.runtime("keep", "keep");
    let remove = fixture.runtime("remove", "remove");
    let workspace = load_workspace_model(&Some(fixture.root.clone())).expect("workspace");

    let result = delete_workspace_runtime(&workspace, "remove").expect("delete child");
    assert_eq!(result["runtime_id"], "remove");
    assert!(keep.exists());
    assert!(!remove.exists());
    assert!(fixture.root.exists());
}

#[test]
fn project_state_revision_changes_with_exact_runtime_text() {
    let fixture = TempWorkspace::new("project-state");
    let runtime_root = fixture.runtime("line", "line");
    std::fs::write(runtime_root.join("io.toml"), "").expect("write I/O");
    std::fs::create_dir_all(runtime_root.join("src")).expect("create src");
    std::fs::write(
        runtime_root.join("src/main.st"),
        "PROGRAM Main\nEND_PROGRAM\n",
    )
    .expect("write ST");

    let first_workspace =
        load_workspace_model(&Some(fixture.root.clone())).expect("first workspace");
    let first = config_project_state(first_workspace).expect("first state");
    std::fs::write(
        runtime_root.join("runtime.toml"),
        render_new_runtime_toml("line", Some("changed")),
    )
    .expect("change runtime");
    let second_workspace =
        load_workspace_model(&Some(fixture.root.clone())).expect("second workspace");
    let second = config_project_state(second_workspace).expect("second state");

    assert_eq!(first["mode"], "config");
    assert_eq!(first["runtimes"][0]["runtime_id"], "line");
    assert_ne!(first["revision"], second["revision"]);
}
