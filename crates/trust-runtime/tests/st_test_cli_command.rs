use std::path::PathBuf;
use std::process::Command;
use std::time::{SystemTime, UNIX_EPOCH};

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

fn tutorial_project_path(name: &str) -> PathBuf {
    let manifest_dir = PathBuf::from(env!("CARGO_MANIFEST_DIR"));
    manifest_dir.join("../../examples/tutorials").join(name)
}

#[test]
fn list_flag_lists_tutorial_10_tests_without_executing() {
    let tutorial = tutorial_project_path("10_unit_testing_101");
    let output = Command::new(env!("CARGO_BIN_EXE_trust-runtime"))
        .args(["test", "--project"])
        .arg(&tutorial)
        .arg("--list")
        .output()
        .expect("run trust-runtime test --list");

    assert!(
        output.status.success(),
        "expected --list success.\nstdout:\n{}\nstderr:\n{}",
        String::from_utf8_lossy(&output.stdout),
        String::from_utf8_lossy(&output.stderr)
    );

    let text = String::from_utf8_lossy(&output.stdout);
    assert!(text.contains("TEST_PROGRAM::TEST_LIMIT_ADD_AND_SCALING"));
    assert!(text.contains("TEST_FUNCTION_BLOCK::TEST_FB_START_STOP_SEQUENCE"));
    assert!(text.contains("TEST_PROGRAM::TEST_COMPARISON_ASSERTIONS"));
    assert!(text.contains("3 test(s) listed"));
}

#[test]
fn filter_zero_message_is_clear_in_human_output() {
    let tutorial = tutorial_project_path("10_unit_testing_101");
    let output = Command::new(env!("CARGO_BIN_EXE_trust-runtime"))
        .args(["test", "--project"])
        .arg(&tutorial)
        .args(["--filter", "NONEXISTENT_CASE"])
        .output()
        .expect("run trust-runtime test --filter NONEXISTENT_CASE");

    assert!(
        output.status.success(),
        "expected filtered run success.\nstdout:\n{}\nstderr:\n{}",
        String::from_utf8_lossy(&output.stdout),
        String::from_utf8_lossy(&output.stderr)
    );

    let text = String::from_utf8_lossy(&output.stdout);
    assert!(text.contains("0 tests matched filter \"NONEXISTENT_CASE\""));
    assert!(text.contains("tests discovered, all filtered out"));
}

#[test]
fn timeout_flag_reports_error_for_infinite_loop_test() {
    let project = unique_temp_dir("timeout-project");
    let sources = project.join("sources");
    std::fs::create_dir_all(&sources).expect("create sources dir");
    std::fs::write(
        sources.join("tests.st"),
        r#"
TEST_PROGRAM InfiniteLoop
WHILE TRUE DO
END_WHILE;
END_TEST_PROGRAM
"#,
    )
    .expect("write timeout test source");

    let output = Command::new(env!("CARGO_BIN_EXE_trust-runtime"))
        .args([
            "test",
            "--project",
            project.to_str().expect("project path utf-8"),
            "--timeout",
            "1",
        ])
        .output()
        .expect("run trust-runtime test --timeout 1");

    assert!(
        !output.status.success(),
        "expected timeout run to fail.\nstdout:\n{}\nstderr:\n{}",
        String::from_utf8_lossy(&output.stdout),
        String::from_utf8_lossy(&output.stderr)
    );

    let text = String::from_utf8_lossy(&output.stdout);
    assert!(text.contains("test timed out after 1 second"));

    let _ = std::fs::remove_dir_all(project);
}

#[test]
fn json_output_includes_duration_fields() {
    let tutorial = tutorial_project_path("10_unit_testing_101");
    let output = Command::new(env!("CARGO_BIN_EXE_trust-runtime"))
        .args(["test", "--project"])
        .arg(&tutorial)
        .args(["--output", "json"])
        .output()
        .expect("run trust-runtime test --output json");

    assert!(
        output.status.success(),
        "expected JSON run success.\nstdout:\n{}\nstderr:\n{}",
        String::from_utf8_lossy(&output.stdout),
        String::from_utf8_lossy(&output.stderr)
    );

    let payload: serde_json::Value =
        serde_json::from_slice(&output.stdout).expect("parse test json payload");
    assert!(
        payload["summary"]["duration_ms"].is_number(),
        "summary duration_ms must be numeric"
    );
    let tests = payload["tests"].as_array().expect("tests array");
    assert!(
        tests.iter().all(|case| case["duration_ms"].is_number()),
        "every test case must include numeric duration_ms"
    );
}
