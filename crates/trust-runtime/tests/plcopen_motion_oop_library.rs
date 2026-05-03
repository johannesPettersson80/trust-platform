use std::path::PathBuf;
use std::process::Command;

fn fixture_path(name: &str) -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("tests")
        .join("fixtures")
        .join("plcopen_motion")
        .join(name)
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

fn assert_trust_dev_test_passes(project: PathBuf) {
    let output = Command::new(trust_dev_bin())
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
fn plcopen_motion_oop_single_axis_st_unit_tests_pass() {
    assert_trust_dev_test_passes(fixture_path("oop_single_axis"));
}
