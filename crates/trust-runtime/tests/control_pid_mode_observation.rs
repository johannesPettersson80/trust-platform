//! Specification 45 §3.1: the observation extension's `ModeValue` and `ModeObserved`, which every
//! `Update` of a `Control.PID` sets to the mode the call ran in, which a restart clears and an
//! online change keeps. The program is built for this computer with the toolchain of
//! specification 44 §4.6 at `TRUST_TOOLCHAIN`.

use trust_runtime::harness::TestHarness;
use trust_runtime::value::Value;
use trust_runtime::RestartMode;

const LIBRARY: &str = include_str!("../../../libraries/control/src/control.st");

/// A proportional loop (gain 2, no integral action: the positional form of specification 45
/// §3.10) whose controller is declared in a `section` of its own (`VAR` or `VAR RETAIN`). It is
/// configured and initialized once; `Update` runs only while `Run` is TRUE, in the mode `Selected`
/// picks: 1 Manual, 2 Track, anything else Auto.
fn program(section: &str) -> String {
    format!(
        r#"
USING Control;

PROGRAM Main
VAR
    Settings : PIDParameters := (ProportionalGain := 2.0, OutputMinimum := 0.0,
        OutputMaximum := 100.0, RateMinimum := -1000.0, RateMaximum := 1000.0,
        TrackingTime := 1.0);
    Initial : PIDInitialState;
    Input : PIDInput;
    Selected : INT;
    Run : BOOL;
    Initialized : BOOL;
    Output : LREAL;
END_VAR
{section}
    Controller : PID;
END_VAR
IF NOT Initialized THEN
    Controller.Configure(Parameters := Settings);
    Controller.Initialize(State := Initial);
    Initialized := TRUE;
END_IF;
Input.Setpoint := 35.0;
Input.ProcessValue := 20.0;
Input.ManualOutput := 12.5;
Input.TrackingOutput := 40.0;
Input.CycleTime := 0.1;
CASE Selected OF
    1: Input.Mode := PIDMode#Manual;
    2: Input.Mode := PIDMode#Track;
ELSE
    Input.Mode := PIDMode#Auto;
END_CASE;
IF Run THEN
    Output := Controller.Update(Input := Input);
END_IF;
END_PROGRAM
"#
    )
}

fn harness(section: &str) -> TestHarness {
    TestHarness::from_sources(&[LIBRARY, &program(section)])
        .expect("the loop compiles with the Control library")
}

/// The value at `path` below the program instance, as the last scan left it.
fn read(harness: &TestHarness, path: &str) -> Value {
    let path = format!("Main.{path}");
    harness
        .runtime()
        .with_program_view(|program| program.value(&path))
        .unwrap_or_else(|| panic!("{path} reads"))
}

fn observed(harness: &TestHarness) -> bool {
    match read(harness, "Controller.ModeObserved") {
        Value::Bool(observed) => observed,
        other => panic!("ModeObserved is a BOOL, got {other:?}"),
    }
}

/// `ModeValue`'s name and number: the library declares `Auto := 0`, `Manual := 1`, `Track := 2`.
fn mode(harness: &TestHarness) -> (String, i64) {
    match read(harness, "Controller.ModeValue") {
        Value::Enum(mode) => (
            mode.variant_name().to_ascii_uppercase(),
            mode.numeric_value(),
        ),
        other => panic!("ModeValue is a PIDMode, got {other:?}"),
    }
}

fn lreal(harness: &TestHarness, path: &str) -> f64 {
    match read(harness, path) {
        Value::LReal(value) => value,
        other => panic!("{path} is an LREAL, got {other:?}"),
    }
}

fn scan(harness: &mut TestHarness) {
    let result = harness.cycle();
    assert!(result.errors.is_empty(), "{:?}", result.errors);
}

#[test]
fn every_update_records_the_mode_it_ran_in_and_nothing_records_one_before() {
    // Specification 45 §3.1: `ModeObserved` is FALSE until the first `Update` after a start, and
    // each `Update` sets `ModeValue` to its `Input.Mode` and `ModeObserved` to TRUE. The outputs
    // are worked out from §3.5, §3.8 and §3.10 and show the observed controller computes as one
    // without the members. Catches members that `Configure` or `Initialize` set, a mode recorded
    // from the call before, and an observation that changes the controller's output.
    let mut harness = harness("VAR");
    assert!(
        !observed(&harness),
        "nothing is observed before the first scan"
    );
    assert_eq!(
        mode(&harness),
        ("AUTO".to_string(), 0),
        "ModeValue starts at Auto, the first value of PIDMode"
    );

    // A scan that configures and initializes the controller without calling `Update`.
    scan(&mut harness);
    assert!(
        !observed(&harness),
        "Configure and Initialize observe no mode; only Update does"
    );

    // Manual: the output is ManualOutput 12.5, inside the limits [0, 100] and the rate band of
    // ±1000 × 0.1 = ±100 around the previous output 0.
    harness.set_input("Run", Value::Bool(true));
    harness.set_input("Selected", Value::Int(1));
    scan(&mut harness);
    assert!(observed(&harness));
    assert_eq!(mode(&harness), ("MANUAL".to_string(), 1));
    assert_eq!(lreal(&harness, "Output"), 12.5);

    // Track without integral action computes as Auto (§5): 2 × (35 − 20) = 30, inside [0, 100]
    // and within 100 of the previous output 12.5.
    harness.set_input("Selected", Value::Int(2));
    scan(&mut harness);
    assert_eq!(mode(&harness), ("TRACK".to_string(), 2));
    assert_eq!(lreal(&harness, "Output"), 30.0);

    // Auto: the same positional output, 30.
    harness.set_input("Selected", Value::Int(0));
    scan(&mut harness);
    assert_eq!(mode(&harness), ("AUTO".to_string(), 0));
    assert_eq!(lreal(&harness, "Output"), 30.0);

    // A scan without `Update` leaves the mode of the last call.
    harness.set_input("Run", Value::Bool(false));
    harness.set_input("Selected", Value::Int(1));
    scan(&mut harness);
    assert!(observed(&harness));
    assert_eq!(
        mode(&harness),
        ("AUTO".to_string(), 0),
        "the mode of the last Update, not of the program's input"
    );
}

#[test]
fn a_restart_clears_the_observation_also_in_a_retained_controller() {
    // Specification 45 §3.1 with IEC 61131-3 Ed.3 §6.5.6.2: the members are NON_RETAIN, so a warm
    // restart gives them their initial values even in an instance declared RETAIN, which keeps
    // its parameters and history (specification 10 §4.8); a cold restart clears everything.
    // Catches the members declared in a plain VAR block, which a RETAIN instance keeps through a
    // warm restart.
    let mut harness = harness("VAR RETAIN");
    harness.set_input("Run", Value::Bool(true));
    harness.set_input("Selected", Value::Int(1));
    scan(&mut harness);
    assert!(observed(&harness));
    assert_eq!(mode(&harness), ("MANUAL".to_string(), 1));
    assert_eq!(
        lreal(&harness, "Controller.StateValue.SaturatedOutput"),
        12.5
    );

    harness
        .restart(RestartMode::Warm)
        .expect("the program restarts warm");
    assert!(!observed(&harness), "a warm restart clears ModeObserved");
    assert_eq!(mode(&harness), ("AUTO".to_string(), 0));
    assert_eq!(
        lreal(&harness, "Controller.StateValue.SaturatedOutput"),
        12.5,
        "the RETAIN instance keeps its history"
    );
    assert_eq!(
        lreal(&harness, "Controller.ParametersValue.ProportionalGain"),
        2.0,
        "the RETAIN instance keeps its parameters"
    );

    // The restart also reset the program's own `Run` and `Selected`: no `Update` runs, and
    // nothing is observed until one does.
    scan(&mut harness);
    assert!(!observed(&harness));
    harness.set_input("Run", Value::Bool(true));
    harness.set_input("Selected", Value::Int(2));
    scan(&mut harness);
    assert!(
        observed(&harness),
        "the first Update after the start observes again"
    );
    assert_eq!(mode(&harness), ("TRACK".to_string(), 2));

    harness
        .restart(RestartMode::Cold)
        .expect("the program restarts cold");
    assert!(!observed(&harness), "a cold restart clears ModeObserved");
    assert_eq!(mode(&harness), ("AUTO".to_string(), 0));
    assert_eq!(
        lreal(&harness, "Controller.StateValue.SaturatedOutput"),
        0.0,
        "a cold restart starts the history over"
    );
}

#[test]
fn an_online_change_keeps_the_observation_until_the_next_update() {
    // Specification 45 §3.1 with specification 44 §12.2: an online change keeps every member of
    // an instance by name, these too, and the next `Update` sets them again. Catches an online
    // change that drops the observation, or a next call that keeps the old mode.
    let mut harness = harness("VAR");
    harness.set_input("Run", Value::Bool(true));
    harness.set_input("Selected", Value::Int(1));
    scan(&mut harness);
    assert_eq!(mode(&harness), ("MANUAL".to_string(), 1));

    harness
        .reload_sources(&[LIBRARY, &program("VAR")])
        .expect("the program changes online");
    assert!(observed(&harness), "the online change keeps ModeObserved");
    assert_eq!(mode(&harness), ("MANUAL".to_string(), 1));

    harness.set_input("Selected", Value::Int(0));
    scan(&mut harness);
    assert_eq!(mode(&harness), ("AUTO".to_string(), 0));
}
