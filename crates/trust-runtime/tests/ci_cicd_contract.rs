use std::path::{Path, PathBuf};
use std::process::{Command, Output};
use std::time::{SystemTime, UNIX_EPOCH};

fn unique_temp_dir(prefix: &str) -> PathBuf {
    let nanos = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .expect("system time before unix epoch")
        .as_nanos();
    std::env::temp_dir().join(format!(
        "trust-runtime-{prefix}-{}-{nanos}",
        std::process::id()
    ))
}

fn fixture_root(name: &str) -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("tests")
        .join("fixtures")
        .join("ci")
        .join(name)
}

fn copy_dir_recursive(src: &Path, dst: &Path) {
    std::fs::create_dir_all(dst).expect("create destination fixture directory");
    for entry in std::fs::read_dir(src).expect("read fixture directory") {
        let entry = entry.expect("read fixture entry");
        let src_path = entry.path();
        let dst_path = dst.join(entry.file_name());
        if src_path.is_dir() {
            copy_dir_recursive(&src_path, &dst_path);
        } else {
            std::fs::copy(&src_path, &dst_path).expect("copy fixture file");
        }
    }
}

fn copy_fixture(name: &str) -> PathBuf {
    let target = unique_temp_dir(&format!("ci-{name}"));
    copy_dir_recursive(&fixture_root(name), &target);
    target
}

fn run_trust_runtime(project: &Path, args: &[&str]) -> Output {
    let mut command = Command::new(env!("CARGO_BIN_EXE_trust-runtime"));
    command.args(args);
    command.args(["--project", project.to_str().expect("project path utf-8")]);
    command.output().expect("run trust-runtime")
}

fn assert_success(output: &Output, context: &str) {
    assert!(
        output.status.success(),
        "{context} should succeed.\nstdout:\n{}\nstderr:\n{}",
        String::from_utf8_lossy(&output.stdout),
        String::from_utf8_lossy(&output.stderr)
    );
}

#[test]
fn ci_json_summary_mode_contract_is_stable() {
    let project = copy_fixture("green");

    let build = run_trust_runtime(&project, &["build", "--ci"]);
    assert_success(&build, "build --ci");
    let build_json: serde_json::Value =
        serde_json::from_slice(&build.stdout).expect("parse build --ci JSON");
    assert_eq!(build_json["version"], 1);
    assert_eq!(build_json["command"], "build");
    assert_eq!(build_json["status"], "ok");
    assert_eq!(build_json["project"], project.display().to_string());
    assert!(
        build_json["source_count"]
            .as_u64()
            .is_some_and(|count| count >= 2),
        "expected at least two source files in build output"
    );

    let validate = run_trust_runtime(&project, &["validate", "--ci"]);
    assert_success(&validate, "validate --ci");
    let validate_json: serde_json::Value =
        serde_json::from_slice(&validate.stdout).expect("parse validate --ci JSON");
    assert_eq!(validate_json["version"], 1);
    assert_eq!(validate_json["command"], "validate");
    assert_eq!(validate_json["status"], "ok");
    assert_eq!(validate_json["project"], project.display().to_string());
    assert_eq!(validate_json["resource"], "Res");

    let test = run_trust_runtime(&project, &["test", "--ci", "--output", "json"]);
    assert_success(&test, "test --ci --output json");
    let test_json: serde_json::Value =
        serde_json::from_slice(&test.stdout).expect("parse test --ci --output json");
    assert_eq!(test_json["version"], 1);
    assert_eq!(test_json["project"], project.display().to_string());
    assert_eq!(test_json["summary"]["failed"], 0);
    assert_eq!(test_json["summary"]["errors"], 0);
    assert!(
        test_json["summary"]["total"]
            .as_u64()
            .is_some_and(|total| total >= 1),
        "expected at least one discovered test"
    );
    assert!(test_json["tests"]
        .as_array()
        .is_some_and(|tests| tests.iter().any(|case| case["name"] == "CI_Passes")));

    let _ = std::fs::remove_dir_all(project);
}

#[test]
fn ci_template_workflow_passes_on_green_fixture() {
    let project = copy_fixture("green");

    let build = run_trust_runtime(&project, &["build", "--ci"]);
    assert_success(&build, "build --ci");

    let validate = run_trust_runtime(&project, &["validate", "--ci"]);
    assert_success(&validate, "validate --ci");

    let tests = run_trust_runtime(&project, &["test", "--ci", "--output", "junit"]);
    assert_success(&tests, "test --ci --output junit");
    let junit = String::from_utf8_lossy(&tests.stdout);
    assert!(
        junit.contains("<testsuite"),
        "expected junit testsuite output"
    );
    assert!(
        junit.contains("failures=\"0\""),
        "expected no junit failures"
    );

    let _ = std::fs::remove_dir_all(project);
}

#[test]
fn ci_template_workflow_fails_on_broken_fixture_with_expected_code_and_report() {
    let project = copy_fixture("broken");

    let build = run_trust_runtime(&project, &["build", "--ci"]);
    assert_success(&build, "build --ci");

    let validate = run_trust_runtime(&project, &["validate", "--ci"]);
    assert_success(&validate, "validate --ci");

    let tests = run_trust_runtime(&project, &["test", "--ci", "--output", "junit"]);
    assert!(
        !tests.status.success(),
        "broken fixture test command should fail"
    );
    assert_eq!(
        tests.status.code(),
        Some(12),
        "expected deterministic CI test-failure exit code"
    );
    let junit = String::from_utf8_lossy(&tests.stdout);
    assert!(
        junit.contains("<testsuite"),
        "expected junit testsuite output"
    );
    assert!(
        junit.contains("<failure"),
        "expected junit failure entry for broken fixture"
    );
    let stderr = String::from_utf8_lossy(&tests.stderr);
    assert!(
        stderr.contains("ST test(s) failed"),
        "expected deterministic CI error message in stderr"
    );

    let _ = std::fs::remove_dir_all(project);
}

#[test]
fn ci_template_file_contains_expected_command_sequence() {
    let template = Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("..")
        .join("..")
        .join(".github")
        .join("workflows")
        .join("templates")
        .join("trust-runtime-project-ci.yml");
    let text = std::fs::read_to_string(&template).expect("read CI template");
    assert!(
        text.contains("target/debug/trust-runtime build --project . --ci"),
        "template must include build --ci step"
    );
    assert!(
        text.contains("target/debug/trust-runtime validate --project . --ci"),
        "template must include validate --ci step"
    );
    assert!(
        text.contains("target/debug/trust-runtime test --project . --ci --output junit"),
        "template must include junit test step"
    );
    assert!(
        text.contains("Upload JUnit report"),
        "template must keep junit artifact upload"
    );
}
