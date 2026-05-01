use std::path::PathBuf;
use std::process::Command;

fn fixture_path(name: &str) -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("tests")
        .join("fixtures")
        .join("oscat")
        .join(name)
}

fn assert_trust_dev_test_passes(project: PathBuf) {
    let output = Command::new(env!("CARGO_BIN_EXE_trust-dev"))
        .args(["test", "--project"])
        .arg(&project)
        .output()
        .expect("run trust-dev test");

    assert!(
        output.status.success(),
        "expected ST fixture tests to pass for {}\nstdout:\n{}\nstderr:\n{}",
        project.display(),
        String::from_utf8_lossy(&output.stdout),
        String::from_utf8_lossy(&output.stderr)
    );
}

#[test]
fn oscat_oop_core_st_unit_tests_pass() {
    assert_trust_dev_test_passes(fixture_path("oop_core"));
}
