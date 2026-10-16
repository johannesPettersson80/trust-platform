#[test]
fn scaffold_init_fails_when_hmi_dir_exists_without_force() {
    let root = temp_dir("trust-runtime-hmi-scaffold-init-guard");
    let source = r#"
PROGRAM Main
VAR_OUTPUT
    speed : REAL := 0.0;
END_VAR
END_PROGRAM
"#;
    let _initial = scaffold_from_sources_with_mode(
        &root,
        "industrial",
        &[("sources/main.st", source)],
        HmiScaffoldMode::Reset,
        false,
    );
    let compile_sources = [HarnessSourceFile::with_path("sources/main.st", source)];
    let runtime = CompileSession::from_sources(compile_sources.to_vec())
        .build_runtime()
        .expect("build runtime");
    let metadata = runtime.metadata_snapshot();
    let snapshot = crate::debug::DebugSnapshot {
        storage_abi: Default::default(),
        storage: runtime.storage().clone(),
        now: runtime.current_time(),
    };
    let refs = [HmiSourceRef {
        path: Path::new("sources/main.st"),
        text: source,
    }];
    let err = scaffold_hmi_dir_with_sources_mode(
        &root,
        &metadata,
        Some(&snapshot),
        &refs,
        "industrial",
        HmiScaffoldMode::Init,
        false,
    )
    .expect_err("init should fail when hmi exists without force");
    assert!(err.to_string().contains("hmi directory already exists"));
    std::fs::remove_dir_all(root).ok();
}

#[test]
fn scaffold_reset_creates_backup_snapshot() {
    let root = temp_dir("trust-runtime-hmi-scaffold-reset-backup");
    let source = r#"
PROGRAM Main
VAR_OUTPUT
    speed : REAL := 0.0;
END_VAR
END_PROGRAM
"#;
    let _initial = scaffold_from_sources_with_mode(
        &root,
        "industrial",
        &[("sources/main.st", source)],
        HmiScaffoldMode::Reset,
        false,
    );
    std::fs::write(root.join("hmi/custom.txt"), "keep me").expect("write custom file");
    let summary = scaffold_from_sources_with_mode(
        &root,
        "industrial",
        &[("sources/main.st", source)],
        HmiScaffoldMode::Reset,
        false,
    );
    let backup_entry = summary
        .files
        .iter()
        .find(|entry| entry.detail.contains("backup snapshot"))
        .expect("backup entry present");
    let backup_path = root.join(&backup_entry.path);
    assert!(backup_path.is_dir());
    assert!(backup_path.join("custom.txt").is_file());
    std::fs::remove_dir_all(root).ok();
}
