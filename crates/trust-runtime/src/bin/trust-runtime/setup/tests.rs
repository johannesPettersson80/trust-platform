use super::*;
use std::sync::atomic::{AtomicU64, Ordering};

static TEMP_SEQUENCE: AtomicU64 = AtomicU64::new(1);

fn temp_dir(name: &str) -> std::path::PathBuf {
    let sequence = TEMP_SEQUENCE.fetch_add(1, Ordering::Relaxed);
    let root = std::env::temp_dir().join(format!(
        "trust-setup-{name}-{}-{sequence}",
        std::process::id()
    ));
    std::fs::create_dir(&root).expect("create setup test directory");
    root
}

#[test]
fn browser_profile_local_enforces_loopback_and_no_token() {
    let profile = BrowserSetupProfile::build(SetupAccessArg::Local, None, DEFAULT_SETUP_PORT, None)
        .expect("local profile");
    assert_eq!(profile.bind, "127.0.0.1");
    assert!(!profile.token_required);
    assert_eq!(profile.token_ttl_minutes, 0);

    let err = BrowserSetupProfile::build(
        SetupAccessArg::Local,
        Some("0.0.0.0".to_string()),
        DEFAULT_SETUP_PORT,
        None,
    )
    .expect_err("local non-loopback must fail");
    assert!(err.to_string().contains("loopback"));
}

#[test]
fn browser_profile_remote_requires_non_loopback_and_token_ttl() {
    let profile =
        BrowserSetupProfile::build(SetupAccessArg::Remote, None, DEFAULT_SETUP_PORT, None)
            .expect("remote profile");
    assert_eq!(profile.bind, "0.0.0.0");
    assert!(profile.token_required);
    assert_eq!(profile.token_ttl_minutes, DEFAULT_REMOTE_TOKEN_TTL_MINUTES);

    let loopback_err = BrowserSetupProfile::build(
        SetupAccessArg::Remote,
        Some("127.0.0.1".to_string()),
        DEFAULT_SETUP_PORT,
        Some(15),
    )
    .expect_err("remote loopback must fail");
    assert!(loopback_err
        .to_string()
        .contains("must not use a loopback bind"));

    let ttl_err =
        BrowserSetupProfile::build(SetupAccessArg::Remote, None, DEFAULT_SETUP_PORT, Some(0))
            .expect_err("remote ttl zero must fail");
    assert!(ttl_err.to_string().contains("token_ttl_minutes > 0"));
}

#[test]
fn interactive_browser_port_rejects_u16_overflow() {
    assert_eq!(
        setup_port_from_prompt_value(u16::MAX.into()).expect("maximum TCP port"),
        u16::MAX
    );
    let error = setup_port_from_prompt_value(u64::from(u16::MAX) + 1)
        .expect_err("interactive port must not wrap to zero");
    assert!(error.to_string().contains("port must be <= 65535"));
}

#[test]
fn generated_resource_names_are_valid_for_numeric_and_reserved_folders() {
    assert_eq!(
        wizard::default_resource_name(std::path::Path::new("/tmp/123")),
        "Res123"
    );
    assert_eq!(
        wizard::default_resource_name(std::path::Path::new("/tmp/PROGRAM")),
        "ResPROGRAM"
    );
    assert_eq!(
        wizard::default_resource_name(std::path::Path::new("/tmp/---")),
        "Res"
    );
}

#[test]
fn zero_cycle_configuration_is_rejected_before_runtime_file_mutation() {
    let root = temp_dir("zero-cycle");
    wizard::create_bundle_auto(Some(root.clone())).expect("create initial bundle");
    let runtime_path = root.join("runtime.toml");
    let before = std::fs::read(&runtime_path).expect("read original runtime.toml");

    let error = wizard::write_runtime_toml(&runtime_path, &smol_str::SmolStr::new("CycleProof"), 0)
        .expect_err("zero cycle must fail");

    assert!(error.to_string().contains("cycle interval must be >= 1"));
    assert_eq!(
        std::fs::read(&runtime_path).expect("read retained runtime.toml"),
        before
    );
    let _ = std::fs::remove_dir_all(root);
}

#[test]
fn runtime_generation_preserves_existing_artifacts_when_sources_fail_to_compile() {
    let root = temp_dir("compile-failure");
    wizard::create_bundle_auto(Some(root.clone())).expect("create initial bundle");
    let runtime_path = root.join("runtime.toml");
    let config_path = root.join("src").join("config.st");
    let bytecode_path = root.join("program.stbc");
    let runtime_before = std::fs::read(&runtime_path).expect("read original runtime.toml");
    let config_before = std::fs::read(&config_path).expect("read original config.st");
    let bytecode_before = std::fs::read(&bytecode_path).expect("read original bytecode");
    std::fs::write(root.join("src").join("main.st"), "PROGRAM Main\nBROKEN")
        .expect("write malformed source");

    wizard::write_runtime_toml(
        &runtime_path,
        &smol_str::SmolStr::new("ReplacementResource"),
        25,
    )
    .expect_err("malformed project source must reject setup update");

    assert_eq!(
        std::fs::read(&runtime_path).expect("read retained runtime.toml"),
        runtime_before
    );
    assert_eq!(
        std::fs::read(&config_path).expect("read retained config.st"),
        config_before
    );
    assert_eq!(
        std::fs::read(&bytecode_path).expect("read retained bytecode"),
        bytecode_before
    );
    let _ = std::fs::remove_dir_all(root);
}

#[cfg(unix)]
#[test]
fn selecting_system_io_removes_dangling_project_io_link() {
    use std::os::unix::fs::symlink;

    let root = temp_dir("dangling-io");
    let io_path = root.join("io.toml");
    symlink(root.join("missing-io.toml"), &io_path).expect("create dangling io.toml link");

    wizard::remove_io_toml(&io_path).expect("remove dangling project I/O link");

    assert!(std::fs::symlink_metadata(&io_path).is_err());
    let _ = std::fs::remove_dir_all(root);
}

#[test]
fn selecting_system_io_rejects_project_io_directory() {
    let root = temp_dir("io-directory");
    let io_path = root.join("io.toml");
    std::fs::create_dir(&io_path).expect("create io.toml directory");

    let error = wizard::remove_io_toml(&io_path)
        .expect_err("setup must not recursively delete an io.toml directory");

    assert!(error.to_string().contains("not a removable file"));
    assert!(io_path.is_dir());
    let _ = std::fs::remove_dir_all(root);
}

#[test]
fn auto_bundle_migrates_legacy_sources_and_compiles_dependencies() {
    let root = temp_dir("legacy-source-migration");
    let legacy_sources = root.join("sources");
    std::fs::create_dir_all(&legacy_sources).expect("create legacy sources");
    std::fs::write(
        legacy_sources.join("MAIN.POU"),
        r#"
PROGRAM Main
VAR
    Value : INT;
END_VAR
Value := LibValue();
END_PROGRAM
"#,
    )
    .expect("write legacy project source");
    std::fs::write(
        legacy_sources.join("io.st"),
        r#"
VAR_GLOBAL
    InSignal AT %IX0.0 : BOOL;
    OutSignal AT %QX0.0 : BOOL;
END_VAR
"#,
    )
    .expect("write legacy generated IO source");
    let dependency = root.join("deps/lib-a/src");
    std::fs::create_dir_all(&dependency).expect("create dependency source root");
    std::fs::write(
        dependency.join("lib.st"),
        r#"
FUNCTION LibValue : INT
LibValue := 7;
END_FUNCTION
"#,
    )
    .expect("write dependency source");
    std::fs::write(
        root.join("trust-lsp.toml"),
        "[dependencies]\nLibA = \"deps/lib-a\"\n",
    )
    .expect("write dependency manifest");

    wizard::create_bundle_auto(Some(root.clone())).expect("migrate and compile bundle");

    assert!(!root.join("sources").exists());
    assert!(root.join("src/MAIN.POU").is_file());
    assert!(!root.join("src/io.st").exists());
    assert!(root.join("runtime.toml").is_file());
    assert!(
        root.join("io.toml").is_file() || trust_runtime::config::system_io_config_path().is_file(),
        "automatic setup requires project or system I/O configuration"
    );
    let bytecode = std::fs::read(root.join("program.stbc")).expect("read compiled bytecode");
    let module = trust_runtime::bytecode::BytecodeModule::decode(&bytecode)
        .expect("decode migrated bytecode");
    module.validate().expect("validate migrated bytecode");
    let _ = std::fs::remove_dir_all(root);
}
