use trust_runtime::harness::TestHarness;
use trust_runtime::value::Value;
use trust_runtime::RestartMode;

fn bool_output(harness: &TestHarness, name: &str) -> bool {
    match harness.get_output(name) {
        Some(Value::Bool(value)) => value,
        other => panic!("expected BOOL output {name}, got {other:?}"),
    }
}

#[test]
fn edge_declaration_program_rising_input_pulses_once_per_sampled_transition() {
    let mut harness = TestHarness::from_source(
        r#"
PROGRAM Main
VAR_INPUT
    Signal : BOOL R_EDGE;
END_VAR
VAR_OUTPUT
    Observed : BOOL;
END_VAR
Observed := Signal;
END_PROGRAM
"#,
    )
    .expect("rising-edge program must compile");

    harness.set_input("Signal", Value::Bool(false));
    harness.cycle();
    assert!(!bool_output(&harness, "Observed"));

    harness.set_input("Signal", Value::Bool(true));
    harness.cycle();
    assert!(bool_output(&harness, "Observed"));

    harness.cycle();
    assert!(!bool_output(&harness, "Observed"));

    harness.set_input("Signal", Value::Bool(false));
    harness.cycle();
    assert!(!bool_output(&harness, "Observed"));

    harness.set_input("Signal", Value::Bool(true));
    harness.cycle();
    assert!(bool_output(&harness, "Observed"));
}

#[test]
fn edge_declaration_program_falling_input_obeys_cold_low_and_held_level_rules() {
    let mut harness = TestHarness::from_source(
        r#"
PROGRAM Main
VAR_INPUT
    Signal : BOOL F_EDGE;
END_VAR
VAR_OUTPUT
    Observed : BOOL;
END_VAR
Observed := Signal;
END_PROGRAM
"#,
    )
    .expect("falling-edge program must compile");

    harness.set_input("Signal", Value::Bool(false));
    harness.cycle();
    assert!(bool_output(&harness, "Observed"));

    harness.cycle();
    assert!(!bool_output(&harness, "Observed"));

    harness.set_input("Signal", Value::Bool(true));
    harness.cycle();
    assert!(!bool_output(&harness, "Observed"));

    harness.set_input("Signal", Value::Bool(false));
    harness.cycle();
    assert!(bool_output(&harness, "Observed"));

    harness.cycle();
    assert!(!bool_output(&harness, "Observed"));
}

#[test]
fn edge_declaration_function_block_body_observes_trigger_outputs_not_raw_levels() {
    let mut harness = TestHarness::from_source(
        r#"
FUNCTION_BLOCK EdgeBlock
VAR_INPUT
    Rising : BOOL R_EDGE;
    Falling : BOOL F_EDGE;
END_VAR
VAR_OUTPUT
    RisingPulse : BOOL;
    FallingPulse : BOOL;
END_VAR
RisingPulse := Rising;
FallingPulse := Falling;
END_FUNCTION_BLOCK

PROGRAM Main
VAR
    Raw : BOOL;
    Block : EdgeBlock;
    RisingObserved : BOOL;
    FallingObserved : BOOL;
END_VAR
Block(
    Rising := Raw,
    Falling := Raw,
    RisingPulse => RisingObserved,
    FallingPulse => FallingObserved
);
END_PROGRAM
"#,
    )
    .expect("edge-qualified function block must compile");

    harness.set_input("Raw", Value::Bool(false));
    harness.cycle();
    assert!(!bool_output(&harness, "RisingObserved"));
    assert!(bool_output(&harness, "FallingObserved"));

    harness.set_input("Raw", Value::Bool(true));
    harness.cycle();
    assert!(bool_output(&harness, "RisingObserved"));
    assert!(!bool_output(&harness, "FallingObserved"));

    harness.cycle();
    assert!(!bool_output(&harness, "RisingObserved"));
    assert!(!bool_output(&harness, "FallingObserved"));
}

#[test]
fn edge_declaration_multi_name_inputs_keep_independent_trigger_phase() {
    let mut harness = TestHarness::from_source(
        r#"
PROGRAM Main
VAR_INPUT
    First, Second : BOOL R_EDGE;
END_VAR
VAR_OUTPUT
    FirstObserved : BOOL;
    SecondObserved : BOOL;
END_VAR
FirstObserved := First;
SecondObserved := Second;
END_PROGRAM
"#,
    )
    .expect("multi-name edge declaration must compile");

    harness.set_input("First", Value::Bool(true));
    harness.set_input("Second", Value::Bool(false));
    harness.cycle();
    assert!(bool_output(&harness, "FirstObserved"));
    assert!(!bool_output(&harness, "SecondObserved"));

    harness.set_input("Second", Value::Bool(true));
    harness.cycle();
    assert!(!bool_output(&harness, "FirstObserved"));
    assert!(bool_output(&harness, "SecondObserved"));
}

#[test]
fn edge_declaration_unexecuted_function_block_does_not_sample_missed_level() {
    let mut harness = TestHarness::from_source(
        r#"
FUNCTION_BLOCK EdgeBlock
VAR_INPUT
    Signal : BOOL R_EDGE;
END_VAR
VAR_OUTPUT
    Pulse : BOOL;
END_VAR
Pulse := Signal;
END_FUNCTION_BLOCK

PROGRAM Main
VAR
    Enabled : BOOL;
    Raw : BOOL;
    Block : EdgeBlock;
    Observed : BOOL;
END_VAR
IF Enabled THEN
    Block(Signal := Raw, Pulse => Observed);
END_IF;
END_PROGRAM
"#,
    )
    .expect("conditionally called edge block must compile");

    harness.set_input("Enabled", Value::Bool(true));
    harness.set_input("Raw", Value::Bool(false));
    harness.cycle();
    assert!(!bool_output(&harness, "Observed"));

    harness.set_input("Enabled", Value::Bool(false));
    harness.set_input("Raw", Value::Bool(true));
    harness.cycle();
    harness.set_input("Raw", Value::Bool(false));
    harness.cycle();

    harness.set_input("Enabled", Value::Bool(true));
    harness.cycle();
    assert!(
        !bool_output(&harness, "Observed"),
        "an entirely missed high level must not fabricate a rising pulse"
    );
}

#[test]
fn edge_declaration_hidden_trigger_identity_cannot_collide_with_user_storage() {
    let mut harness = TestHarness::from_source(
        r#"
PROGRAM Main
VAR_INPUT
    Start : BOOL R_EDGE;
END_VAR
VAR
    Start_TRIG : BOOL;
END_VAR
VAR_OUTPUT
    Pulse : BOOL;
    UserState : BOOL;
END_VAR
Start_TRIG := NOT Start_TRIG;
Pulse := Start;
UserState := Start_TRIG;
END_PROGRAM
"#,
    )
    .expect("hidden trigger identity must not collide");

    harness.set_input("Start", Value::Bool(true));
    harness.cycle();
    assert!(bool_output(&harness, "Pulse"));
    assert!(bool_output(&harness, "UserState"));

    harness.cycle();
    assert!(!bool_output(&harness, "Pulse"));
    assert!(!bool_output(&harness, "UserState"));
}

#[test]
fn edge_declaration_retain_preserves_level_and_phase_across_warm_restart() {
    let mut harness = TestHarness::from_source(
        r#"
PROGRAM Main
VAR_INPUT RETAIN
    Signal : BOOL R_EDGE;
END_VAR
VAR_OUTPUT
    Observed : BOOL;
END_VAR
Observed := Signal;
END_PROGRAM
"#,
    )
    .expect("retained edge input must compile");

    harness.set_input("Signal", Value::Bool(true));
    harness.cycle();
    assert!(bool_output(&harness, "Observed"));
    harness.cycle();
    assert!(!bool_output(&harness, "Observed"));

    harness.restart(RestartMode::Warm).expect("warm restart");
    harness.cycle();
    assert!(
        !bool_output(&harness, "Observed"),
        "unchanged retained high level must not fabricate a rising edge"
    );
}

#[test]
fn edge_declaration_persistent_preserves_level_and_phase_across_warm_restart() {
    let mut harness = TestHarness::from_source(
        r#"
PROGRAM Main
VAR_INPUT PERSISTENT
    Signal : BOOL R_EDGE;
END_VAR
VAR_OUTPUT
    Observed : BOOL;
END_VAR
Observed := Signal;
END_PROGRAM
"#,
    )
    .expect("persistent edge input must compile");

    harness.set_input("Signal", Value::Bool(true));
    harness.cycle();
    harness.cycle();
    harness.restart(RestartMode::Warm).expect("warm restart");
    harness.cycle();
    assert!(!bool_output(&harness, "Observed"));
}

#[test]
fn edge_declaration_non_retain_reinitializes_falling_phase_on_warm_restart() {
    let mut harness = TestHarness::from_source(
        r#"
PROGRAM Main
VAR_INPUT NON_RETAIN
    Signal : BOOL F_EDGE;
END_VAR
VAR_OUTPUT
    Observed : BOOL;
END_VAR
Observed := Signal;
END_PROGRAM
"#,
    )
    .expect("non-retained edge input must compile");

    harness.set_input("Signal", Value::Bool(false));
    harness.cycle();
    assert!(bool_output(&harness, "Observed"));
    harness.cycle();
    assert!(!bool_output(&harness, "Observed"));

    harness.restart(RestartMode::Warm).expect("warm restart");
    harness.cycle();
    assert!(
        bool_output(&harness, "Observed"),
        "reinitialized falling phase must follow the cold-low rule"
    );
}
