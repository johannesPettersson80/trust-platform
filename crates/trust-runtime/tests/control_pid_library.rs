use std::path::PathBuf;
use std::process::Command;

fn fixture_path() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("tests")
        .join("fixtures")
        .join("control_pid")
}

fn trust_dev_bin() -> PathBuf {
    if let Some(path) = std::env::var_os("CARGO_BIN_EXE_trust-dev") {
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
fn control_pid_native_conformance_tests_pass() {
    let project = fixture_path();
    let output = Command::new(trust_dev_bin())
        .args(["test", "--project"])
        .arg(&project)
        .output()
        .expect("run trust-dev test");

    assert!(
        output.status.success(),
        "expected Control PID ST tests to pass for {}\nstdout:\n{}\nstderr:\n{}",
        project.display(),
        String::from_utf8_lossy(&output.stdout),
        String::from_utf8_lossy(&output.stderr)
    );
}
