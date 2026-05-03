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

fn runtime_fixture_path(name: &str) -> PathBuf {
    let manifest_dir = PathBuf::from(env!("CARGO_MANIFEST_DIR"));
    manifest_dir.join("tests/fixtures").join(name)
}

fn trust_dev_command() -> Command {
    Command::new(trust_dev_bin())
}

fn trust_runtime_command_with_dev_alias() -> Command {
    let mut command = Command::new(env!("CARGO_BIN_EXE_trust-runtime"));
    command.env("TRUST_DEV_BIN", trust_dev_bin());
    command
}

fn trust_dev_bin() -> PathBuf {
    if let Some(path) = option_env!("CARGO_BIN_EXE_trust-dev") {
        return path.into();
    }
    if let Ok(path) = std::env::var("TRUST_DEV_BIN") {
        return path.into();
    }
    let exe = std::env::current_exe().expect("current test exe path");
    let debug_dir = exe
        .parent()
        .and_then(|deps| deps.parent())
        .expect("target debug dir");
    debug_dir.join(format!("trust-dev{}", std::env::consts::EXE_SUFFIX))
}

#[test]
fn list_flag_lists_tutorial_10_tests_without_executing() {
    let tutorial = tutorial_project_path("10_unit_testing_101");
    let output = trust_dev_command()
        .args(["test", "--project"])
        .arg(&tutorial)
        .arg("--list")
        .output()
        .expect("run trust-dev test --list");

    assert!(
        output.status.success(),
        "expected --list success.\nstdout:\n{}\nstderr:\n{}",
        String::from_utf8_lossy(&output.stdout),
        String::from_utf8_lossy(&output.stderr)
    );

    let text = String::from_utf8_lossy(&output.stdout);
    assert!(text.contains("TEST_PROGRAM::LimitAddAndScaling"));
    assert!(text.contains("TEST_FUNCTION_BLOCK::StartStopSequence"));
    assert!(text.contains("TEST_PROGRAM::ComparisonAssertions"));
    assert!(text.contains("3 test(s) listed"));
}

#[test]
fn trust_runtime_test_alias_forwards_to_trust_dev() {
    let tutorial = tutorial_project_path("10_unit_testing_101");
    let output = trust_runtime_command_with_dev_alias()
        .args(["test", "--project"])
        .arg(&tutorial)
        .arg("--list")
        .output()
        .expect("run trust-runtime test alias");

    assert!(
        output.status.success(),
        "expected alias success.\nstdout:\n{}\nstderr:\n{}",
        String::from_utf8_lossy(&output.stdout),
        String::from_utf8_lossy(&output.stderr)
    );
    let stderr = String::from_utf8_lossy(&output.stderr);
    assert!(stderr.contains("trust-runtime test"));
    assert!(stderr.contains("trust-dev test"));
    assert!(String::from_utf8_lossy(&output.stdout).contains("3 test(s) listed"));
}

#[test]
fn filter_zero_message_is_clear_in_human_output() {
    let tutorial = tutorial_project_path("10_unit_testing_101");
    let output = trust_dev_command()
        .args(["test", "--project"])
        .arg(&tutorial)
        .args(["--filter", "NONEXISTENT_CASE"])
        .output()
        .expect("run trust-dev test --filter NONEXISTENT_CASE");

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
    let sources = project.join("src");
    std::fs::create_dir_all(&sources).expect("create src dir");
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

    let output = trust_dev_command()
        .args([
            "test",
            "--project",
            project.to_str().expect("project path utf-8"),
            "--timeout",
            "1",
        ])
        .output()
        .expect("run trust-dev test --timeout 1");

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
fn timeout_budget_does_not_count_project_recompilation_per_case() {
    let fixture = runtime_fixture_path("oscat/core");
    let output = trust_dev_command()
        .args(["test", "--project"])
        .arg(&fixture)
        .args([
            "--filter",
            "oscat_logic_jk_rs_and_selector_behave",
            "--timeout",
            "2",
        ])
        .output()
        .expect("run trust-dev test on OSCAT core fixture with tight timeout");

    assert!(
        output.status.success(),
        "expected filtered OSCAT run success.\nstdout:\n{}\nstderr:\n{}",
        String::from_utf8_lossy(&output.stdout),
        String::from_utf8_lossy(&output.stderr)
    );

    let text = String::from_utf8_lossy(&output.stdout);
    assert!(text.contains("PASS [1/1] TEST_PROGRAM::oscat_logic_jk_rs_and_selector_behave"));
}

#[test]
fn json_output_includes_duration_fields() {
    let tutorial = tutorial_project_path("10_unit_testing_101");
    let output = trust_dev_command()
        .args(["test", "--project"])
        .arg(&tutorial)
        .args(["--output", "json"])
        .output()
        .expect("run trust-dev test --output json");

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

#[test]
fn test_program_runs_when_configuration_is_present() {
    let project = unique_temp_dir("config-test-program-project");
    let sources = project.join("src");
    std::fs::create_dir_all(&sources).expect("create src dir");
    std::fs::write(
        sources.join("tests.st"),
        r#"
CONFIGURATION Cfg
    RESOURCE Res ON PLC
        TASK MainTask(INTERVAL := T#10ms, PRIORITY := 1);
        PROGRAM MainInst WITH MainTask : Main;
    END_RESOURCE
END_CONFIGURATION

PROGRAM Main
END_PROGRAM

TEST_PROGRAM Probe
ASSERT_TRUE(TRUE);
END_TEST_PROGRAM
"#,
    )
    .expect("write config + test source");

    let output = trust_dev_command()
        .args(["test", "--project"])
        .arg(&project)
        .args(["--filter", "Probe"])
        .output()
        .expect("run trust-dev test with configuration");

    assert!(
        output.status.success(),
        "expected test run success.\nstdout:\n{}\nstderr:\n{}",
        String::from_utf8_lossy(&output.stdout),
        String::from_utf8_lossy(&output.stderr)
    );

    let text = String::from_utf8_lossy(&output.stdout);
    assert!(text.contains("TEST_PROGRAM::Probe"));
    assert!(text.contains("passed"));

    let _ = std::fs::remove_dir_all(project);
}

#[test]
fn build_accepts_recent_language_regression_cases() {
    let project = unique_temp_dir("build-regression-cases");
    let sources = project.join("src");
    std::fs::create_dir_all(&sources).expect("create src dir");
    std::fs::write(
        sources.join("main.st"),
        r#"
TYPE Axis : (X, Z, G)
END_TYPE

TYPE StepData :
STRUCT
    cyl : INT;
    ext : BOOL;
END_STRUCT
END_TYPE

FUNCTION ValueOrDefault : DINT
VAR_INPUT
    cond : BOOL;
    value : DINT;
END_VAR
IF cond THEN
    ValueOrDefault := value;
    RETURN;
END_IF;
RETURN DINT#0;
END_FUNCTION

PROGRAM Main
VAR CONSTANT
    K : INT := 2;
END_VAR
VAR
    choice : INT := 2;
    axis : Axis := Axis#Z;
    arr : ARRAY[1..3] OF INT := [1, 2, 3];
    seq : ARRAY[0..1] OF StepData;
    idx : INT := 1;
    outv : DINT := DINT#0;
END_VAR

CASE choice OF
    K: outv := DINT#1;
END_CASE;

CASE axis OF
    X: outv := DINT#10;
    Z: outv := ValueOrDefault(cond := TRUE, value := DINT#20);
    G: outv := DINT#30;
END_CASE;

seq[idx].cyl := arr[2];
seq[idx].ext := TRUE;
END_PROGRAM
"#,
    )
    .expect("write regression source");

    let output = Command::new(env!("CARGO_BIN_EXE_trust-runtime"))
        .args(["build", "--project"])
        .arg(&project)
        .args(["--sources", "src", "--ci"])
        .output()
        .expect("run trust-runtime build");

    assert!(
        output.status.success(),
        "expected build success.\nstdout:\n{}\nstderr:\n{}",
        String::from_utf8_lossy(&output.stdout),
        String::from_utf8_lossy(&output.stderr)
    );

    let _ = std::fs::remove_dir_all(project);
}
