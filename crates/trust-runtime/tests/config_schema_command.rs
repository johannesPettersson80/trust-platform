use std::process::{Command, Output};
use std::time::{SystemTime, UNIX_EPOCH};

use smol_str::SmolStr;
use trust_runtime::bundle_builder::build_program_stbc;
use trust_runtime::bundle_template::{build_io_config_auto, render_io_toml, render_runtime_toml};

fn unique_temp_dir(prefix: &str) -> std::path::PathBuf {
    let nanos = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .expect("system time before unix epoch")
        .as_nanos();
    std::env::temp_dir().join(format!(
        "trust-runtime-{prefix}-{}-{nanos}",
        std::process::id()
    ))
}

fn write_project_fixture(root: &std::path::Path, resource_name: &str) {
    std::fs::create_dir_all(root.join("sources")).expect("create source directory");
    let runtime_toml = render_runtime_toml(&SmolStr::new(resource_name), 100);
    let io_template = build_io_config_auto("loopback").expect("build loopback io template");
    let io_toml = render_io_toml(&io_template);
    std::fs::write(root.join("runtime.toml"), runtime_toml).expect("write runtime.toml");
    std::fs::write(root.join("io.toml"), io_toml).expect("write io.toml");
    std::fs::write(
        root.join("sources").join("main.st"),
        r#"
PROGRAM Main
VAR
    Counter : INT := 0;
END_VAR
Counter := Counter + 1;
END_PROGRAM
"#,
    )
    .expect("write main source");
    let report = build_program_stbc(root, None).expect("build program.stbc");
    assert!(report.program_path.is_file(), "program.stbc should exist");
}

fn run_validate(project: &std::path::Path) -> Output {
    Command::new(env!("CARGO_BIN_EXE_trust-runtime"))
        .arg("validate")
        .arg("--project")
        .arg(project)
        .output()
        .expect("run trust-runtime validate")
}

#[test]
fn validate_accepts_canonical_schema_fixture() {
    let project = unique_temp_dir("validate-schema-ok");
    write_project_fixture(&project, "schema_ok");
    let output = run_validate(&project);
    assert!(
        output.status.success(),
        "expected validate success, stderr was:\n{}",
        String::from_utf8_lossy(&output.stderr)
    );
    let _ = std::fs::remove_dir_all(project);
}

#[test]
fn validate_rejects_runtime_unknown_key_after_offline_edit() {
    let project = unique_temp_dir("validate-schema-runtime-unknown");
    write_project_fixture(&project, "schema_runtime_unknown");
    let runtime_path = project.join("runtime.toml");
    let mut runtime_text = std::fs::read_to_string(&runtime_path).expect("read runtime.toml");
    runtime_text.push_str("\n[runtime.extra]\nflag = true\n");
    std::fs::write(&runtime_path, runtime_text).expect("write runtime.toml");

    let output = run_validate(&project);
    assert!(
        !output.status.success(),
        "expected validate failure for unknown runtime key"
    );
    let stderr = String::from_utf8_lossy(&output.stderr);
    assert!(
        stderr.contains("unknown field"),
        "expected unknown field diagnostic, got:\n{stderr}"
    );
    let _ = std::fs::remove_dir_all(project);
}

#[test]
fn validate_rejects_io_type_error_after_offline_edit() {
    let project = unique_temp_dir("validate-schema-io-type");
    write_project_fixture(&project, "schema_io_type");
    let io_path = project.join("io.toml");
    std::fs::write(
        &io_path,
        r#"
[io]
driver = "loopback"
params = 42
"#,
    )
    .expect("write io.toml");

    let output = run_validate(&project);
    assert!(
        !output.status.success(),
        "expected validate failure for io.params type error"
    );
    let stderr = String::from_utf8_lossy(&output.stderr);
    assert!(
        stderr.contains("io.params must be a table"),
        "expected io.params diagnostic, got:\n{stderr}"
    );
    let _ = std::fs::remove_dir_all(project);
}

#[test]
fn validate_rejects_runtime_range_error_after_offline_edit() {
    let project = unique_temp_dir("validate-schema-runtime-range");
    write_project_fixture(&project, "schema_runtime_range");
    let runtime_path = project.join("runtime.toml");
    let runtime_text = std::fs::read_to_string(&runtime_path).expect("read runtime.toml");
    let runtime_text = runtime_text.replace("cycle_interval_ms = 100", "cycle_interval_ms = 0");
    std::fs::write(&runtime_path, runtime_text).expect("write runtime.toml");

    let output = run_validate(&project);
    assert!(
        !output.status.success(),
        "expected validate failure for runtime range error"
    );
    let stderr = String::from_utf8_lossy(&output.stderr);
    assert!(
        stderr.contains("resource.cycle_interval_ms must be >= 1"),
        "expected range diagnostic, got:\n{stderr}"
    );
    let _ = std::fs::remove_dir_all(project);
}
