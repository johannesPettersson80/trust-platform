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

/// A program that declares enumerations with every member name of the Control library's own (Off, Manual, Auto, Track,
/// BackCalculation, Combined, FirstOrder, SecondOrder, Parallel, Ideal, Series, Valid, InvalidDomain, NonRepresentable), as a
/// CODESYS or TwinCAT project's `E_Mode` does, and runs a PID in Manual, a PID in Track and a second-order filter.
const SHARED_MEMBER_NAMES: &str = r#"
TYPE E_Mode : (Off, Manual, Auto, Track); END_TYPE
TYPE E_Windup : (BackCalculation, Combined); END_TYPE
TYPE E_Order : (FirstOrder, SecondOrder); END_TYPE
TYPE E_Form : (Parallel, Ideal, Series); END_TYPE
TYPE E_Status : (Valid, InvalidDomain, NonRepresentable); END_TYPE

PROGRAM Main
VAR
    Mode : E_Mode := E_Mode#Manual;
    Hand : Control.PID;
    Follow : Control.PID;
    Filter : Control.LowPass;
    Parameters : Control.PIDParameters := (ProportionalGain := LREAL#2.0, IntegralGain := LREAL#1.0,
        OutputMinimum := LREAL#0.0, OutputMaximum := LREAL#100.0, RateMinimum := LREAL#-1000.0,
        RateMaximum := LREAL#1000.0, TrackingTime := LREAL#1.0, AntiWindup := Control.PIDAntiWindup#Combined);
    Input : Control.PIDInput := (Setpoint := LREAL#50.0, ProcessValue := LREAL#50.0, ManualOutput := LREAL#30.0,
        TrackingOutput := LREAL#70.0, CycleTime := LREAL#0.1);
    Start : Control.PIDInitialState := (Setpoint := LREAL#50.0, ProcessValue := LREAL#50.0, SaturatedOutput := LREAL#70.0);
    Smoothing : Control.LowPassParameters := (TimeConstant := LREAL#1.0, Order := Control.FilterOrder#SecondOrder);
END_VAR
VAR_OUTPUT
    ManualOut : LREAL;
    TrackOut : LREAL;
    Filtered : LREAL;
END_VAR
Hand.Configure(Parameters := Parameters);
Input.Mode := Control.PIDMode#Manual;
ManualOut := Hand.Update(Input := Input);
Follow.Configure(Parameters := Parameters);
Follow.Initialize(State := Start);
Input.Mode := Control.PIDMode#Track;
TrackOut := Follow.Update(Input := Input);
Filter.Configure(Parameters := Smoothing);
Filter.Initialize(Input := LREAL#0.0);
Filtered := Filter.Update(Input := LREAL#10.0, CycleTime := LREAL#0.1);
END_PROGRAM
"#;

/// IEC 61131-3 Ed.3 6.4.4.2.1: an enumerated value that several enumeration types name must be qualified with its type
/// (`IF My_Traffic_light = Red THEN ... // ERROR - Red is not unique`). So a library that writes `Manual` unqualified stops
/// compiling as soon as a project declares its own `E_Mode (Off, Manual, Auto)`, as CODESYS and TwinCAT projects commonly do. The
/// Control library qualifies its literals, and its modes and filter order still select what they name. Catches an unqualified
/// literal (the project fails with "ambiguous enum value") and a qualified one that names another member.
///
/// Worked by hand from the library's code, one scan with a cycle of 0.1 s:
/// - Manual: the output is the manual output, 30, inside the rate limits [0, 100] from a zero state.
/// - Track: the state's setpoint and measurement are 50, as the input's, so every increment is 0 and the output is the tracking
///   output, 70, inside [0, 100] around the saturated 70.
/// - The second-order filter with `T = 1 s` from 0 towards 10: `c = 0.1 / 1.05`, the first stage `10·c = 0.952381`, the output
///   `0.952381·c = 0.0907029`; a first-order filter would answer 0.952381.
#[test]
#[allow(clippy::float_cmp)] // The manual and the tracking output are exact: no arithmetic changes them.
fn a_project_may_reuse_the_control_librarys_enumeration_member_names() {
    use trust_runtime::harness::{CompileSession, SourceFile, TestHarness};
    use trust_runtime::value::Value;

    let mut harness = TestHarness::from_session(
        CompileSession::from_sources(vec![
            SourceFile::with_path(
                "Control/control.st",
                include_str!("../../../libraries/control/src/control.st"),
            ),
            SourceFile::with_path(
                "Control/advanced_control.st",
                include_str!("../../../libraries/control/src/advanced_control.st"),
            ),
            SourceFile::with_path("main.st", SHARED_MEMBER_NAMES),
        ])
        .label_errors(true),
    )
    .unwrap_or_else(|error| panic!("a project may reuse the library's member names: {error}"));
    assert!(harness.cycle().errors.is_empty());
    let number = |name: &str| match harness.get_output(name) {
        Some(Value::LReal(value)) => value,
        other => panic!("{name} is an LREAL: {other:?}"),
    };
    assert_eq!(number("ManualOut"), 30.0);
    assert_eq!(number("TrackOut"), 70.0);
    let coefficient = 0.1 / 1.05;
    assert!((number("Filtered") - 10.0 * coefficient * coefficient).abs() < 1e-12);
}
