use std::path::PathBuf;
use std::process::Command;

include!("../../../tests/support/repository_source_oracle.rs");

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

fn read_fixture_sources(project: &std::path::Path) -> String {
    let manifest_dir = PathBuf::from(env!("CARGO_MANIFEST_DIR"));
    let repository_root = manifest_dir
        .parent()
        .and_then(std::path::Path::parent)
        .expect("workspace root")
        .to_path_buf();
    let mut sources = Vec::new();
    for file_name in ["main.st", "tests.st"] {
        match repository_source_tree_read_to_string!(
            (project.join("src").join(file_name), &repository_root),
            roots = [
                "crates/trust-runtime/tests/fixtures/plcopen_motion/coordinated_motion_negative_deferred_public_surface",
                "crates/trust-runtime/tests/fixtures/plcopen_motion/homing_negative_deferred_public_surface",
                "crates/trust-runtime/tests/fixtures/plcopen_motion/single_axis_negative_deferred_public_surface",
                "crates/trust-runtime/tests/fixtures/plcopen_motion/single_axis_negative_group_label",
                "crates/trust-runtime/tests/fixtures/plcopen_motion/single_axis_negative_power_enable_split",
                "crates/trust-runtime/tests/fixtures/plcopen_motion/single_axis_negative_public_surface",
                "crates/trust-runtime/tests/fixtures/plcopen_motion/single_axis_negative_stop_active",
                "crates/trust-runtime/tests/fixtures/plcopen_motion/single_axis_negative_transition_vel_next",
                "crates/trust-runtime/tests/fixtures/plcopen_motion/single_axis_negative_transition_vel_zero",
                "crates/trust-runtime/tests/fixtures/plcopen_motion/synchronization_negative_deferred_public_surface",
            ],
            extension = "st",
        ) {
            Ok(source) => sources.push(source),
            Err(error) if error.kind() == std::io::ErrorKind::NotFound => {}
            Err(error) => {
                panic!(
                    "read PLCopen negative fixture source {file_name} from {}: {error}",
                    project.display()
                )
            }
        }
    }
    assert!(
        !sources.is_empty(),
        "PLCopen negative fixture {} must contain main.st or tests.st",
        project.display()
    );
    sources.join("\n")
}

fn assert_trust_dev_test_rejects_absent_surfaces(
    project: PathBuf,
    forbidden_surfaces: &[&str],
    diagnostic_fragments: &[&str],
) {
    let fixture_sources = read_fixture_sources(&project);
    for forbidden_surface in forbidden_surfaces {
        assert!(
            fixture_sources.contains(forbidden_surface),
            "negative fixture {} must bind forbidden surface {forbidden_surface}",
            project.display()
        );
    }

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
    for diagnostic_fragment in diagnostic_fragments {
        assert!(
            diagnostics.contains(diagnostic_fragment),
            "expected diagnostic fragment {diagnostic_fragment} in {}\nstdout:\n{stdout}\nstderr:\n{stderr}",
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
    for (fixture, forbidden_surfaces, diagnostic_fragments) in [
        (
            "coordinated_motion_negative_deferred_public_surface",
            &["MC_MoveCircularAbsolute", "MC_TrackRotaryTable"][..],
            &["MC_MoveCircularAbsolute", "MC_TrackRotaryTable"][..],
        ),
        (
            "homing_negative_deferred_public_surface",
            &["MC_StepReferenceFlyingSwitch", "MC_AbortPassiveHoming"][..],
            &["MC_StepReferenceFlyingSwitch", "MC_AbortPassiveHoming"][..],
        ),
        (
            "synchronization_negative_deferred_public_surface",
            &["MC_PhasingAbsolute", "MC_CombineAxes"][..],
            &["MC_PhasingAbsolute", "MC_CombineAxes"][..],
        ),
        (
            "single_axis_negative_deferred_public_surface",
            &["MC_MoveSuperimposed", "MC_AbortTrigger"][..],
            &["MC_MoveSuperimposed", "MC_AbortTrigger"][..],
        ),
        (
            "single_axis_negative_public_surface",
            &["MC_ERROR", "MC_PAYLOAD_REF"][..],
            &["MC_ERROR", "MC_PAYLOAD_REF"][..],
        ),
        (
            "single_axis_negative_transition_vel_zero",
            &["mcTransitionVelZero"][..],
            &["invalid typed literal"][..],
        ),
        (
            "single_axis_negative_transition_vel_next",
            &["mcTransitionVelNext"][..],
            &["invalid typed literal"][..],
        ),
        (
            "single_axis_negative_group_label",
            &["GroupStandby"][..],
            &["invalid typed literal"][..],
        ),
        (
            "single_axis_negative_power_enable_split",
            &["EnablePositive", "EnableNegative"][..],
            &["EnablePositive", "EnableNegative"][..],
        ),
        (
            "single_axis_negative_stop_active",
            &["Stop.Active"][..],
            &["no member 'Active' on type"][..],
        ),
    ] {
        assert_trust_dev_test_rejects_absent_surfaces(
            fixture_path(fixture),
            forbidden_surfaces,
            diagnostic_fragments,
        );
    }
}
