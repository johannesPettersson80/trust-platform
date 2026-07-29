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

fn assert_trust_dev_test_rejects_undefined_types(project: PathBuf, type_names: &[&str]) {
    let output = Command::new(trust_dev_bin())
        .args(["test", "--project"])
        .arg(&project)
        .output()
        .expect("run trust-dev test");
    let stdout = String::from_utf8_lossy(&output.stdout);
    let stderr = String::from_utf8_lossy(&output.stderr);
    let diagnostics = format!("{stdout}\n{stderr}");

    assert!(
        !output.status.success(),
        "expected deferred PLCopen surface to remain absent for {}",
        project.display()
    );
    for type_name in type_names {
        assert!(
            diagnostics.contains(type_name),
            "expected diagnostic for absent type {type_name} in {}\nstdout:\n{stdout}\nstderr:\n{stderr}",
            project.display()
        );
    }
}

#[test]
fn plcopen_motion_oop_single_axis_st_unit_tests_pass() {
    assert_trust_dev_test_passes(fixture_path("oop_single_axis"));
}

#[test]
fn plcopen_motion_deferred_public_surfaces_remain_absent() {
    for (fixture, absent_types) in [
        (
            "coordinated_motion_negative_deferred_public_surface",
            &["MC_MoveCircularAbsolute", "MC_TrackRotaryTable"][..],
        ),
        (
            "homing_negative_deferred_public_surface",
            &["MC_StepReferenceFlyingSwitch", "MC_AbortPassiveHoming"][..],
        ),
        (
            "synchronization_negative_deferred_public_surface",
            &["MC_PhasingAbsolute", "MC_CombineAxes"][..],
        ),
        (
            "single_axis_negative_deferred_public_surface",
            &["MC_MoveSuperimposed", "MC_AbortTrigger"][..],
        ),
    ] {
        assert_trust_dev_test_rejects_undefined_types(fixture_path(fixture), absent_types);
    }
}
