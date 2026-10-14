// The tracking tests compare the exact references and weights the fixture sets.
#![allow(clippy::float_cmp)]

use super::*;
use crate::commissioning::fixture_loops::{control_pid_loop, parallel_pid_loop, proportional_loop};
use crate::commissioning::transfer::{pid_transfer_binding, proportional_transfer_binding};
use serde_json::json;

#[test]
fn a_proportional_tracking_names_its_states_gain_and_process_value() {
    let loop_definition = proportional_loop();
    let binding = proportional_transfer_binding(&loop_definition).expect("the fixture binds");
    let tracking = proportional_tracking(&loop_definition, &binding, true).expect("the tracking");
    assert_eq!(
        tracking,
        ProportionalTracking {
            kind: ProportionalKind::Proportional,
            reference_symbol: "LevelSP".into(),
            manual_command_symbol: "Main.ManualCommand".into(),
            process_variable: "LevelPV".into(),
            gain_symbol: "Main.Parameters.Kp".into(),
            align_reference: true,
        }
    );
}

#[test]
fn an_aligned_reference_makes_the_first_output_equal_the_held_output() {
    let binding = proportional_transfer_binding(&proportional_loop()).expect("the fixture binds");
    // r = PV + MV / Kp = 2 + 10 / 2.
    assert_eq!(
        proportional_tracking_reference(&binding, 2.0, 10.0, 2.0),
        Ok(7.0)
    );
}

#[test]
fn a_zero_gain_or_a_reference_outside_its_bounds_cannot_align_the_entry() {
    let binding = proportional_transfer_binding(&proportional_loop()).expect("the fixture binds");
    let failure =
        proportional_tracking_reference(&binding, 2.0, 10.0, 0.0).expect_err("a zero gain");
    assert_eq!(
        failure.message,
        "A zero or invalid P gain cannot align the reference for Auto entry."
    );
    assert_eq!(
        (failure.cause, failure.next_action),
        (Cause::Settings, NextAction::Tune)
    );
    assert!(proportional_tracking_reference(&binding, 2.0, 10.0, f64::NAN).is_err());

    let failure = proportional_tracking_reference(&binding, 95.0, 10.0, 1.0)
        .expect_err("105 is above the setpoint's maximum");
    assert_eq!(
        failure.message,
        "LevelSP is outside its declared range after IEC rounding."
    );
}

#[test]
fn a_parallel_pid_tracking_names_the_gains_states_weights_and_task_period() {
    let loop_definition = parallel_pid_loop();
    let binding = pid_transfer_binding(&loop_definition).expect("the fixture binds");
    let tracking = parallel_pid_tracking(&loop_definition, &binding, false).expect("the tracking");
    assert_eq!(tracking.kind, ParallelPidKind::ParallelPidV1);
    assert_eq!(tracking.reference_symbol, "LevelSP");
    assert_eq!(tracking.manual_command_symbol, "Main.ManualCommand");
    assert_eq!(tracking.process_variable, "LevelPV");
    assert_eq!(tracking.proportional_gain_symbol, "Main.Parameters.Kp");
    assert_eq!(tracking.integral_gain_symbol, "Main.Parameters.Ki");
    assert_eq!(tracking.derivative_gain_symbol, "Main.Parameters.Kd");
    assert_eq!(tracking.integral_state_symbol, "Main.Integral");
    assert_eq!(tracking.derivative_state_symbol, "Main.PreviousError");
    assert_eq!(
        (
            tracking.proportional_setpoint_weight,
            tracking.derivative_setpoint_weight
        ),
        (1.0, 0.0)
    );
    assert!(
        (tracking.cycle_seconds - 0.01).abs() <= 1e-12,
        "a 10 ms task is 0.01 s"
    );
    assert!(!tracking.align_output);
}

#[test]
fn a_control_pid_without_the_parallel_contract_has_no_parallel_tracking() {
    let binding = pid_transfer_binding(&parallel_pid_loop()).expect("the fixture binds");
    let failure = parallel_pid_tracking(&control_pid_loop(), &binding, true)
        .expect_err("control-pid-v1 declares its states elsewhere");
    assert_eq!(
        failure.message,
        "Typed PI/PID initialization adapter is required for Auto commissioning; choose Manual staging after Verify."
    );
}

#[test]
fn a_pid_transfer_reports_each_declared_state_as_a_number_within_its_bounds() {
    let binding = pid_transfer_binding(&parallel_pid_loop()).expect("the fixture binds");
    let observed = BTreeMap::from([
        ("LevelSP".to_owned(), TransferValue::Number(2.0)),
        ("Main.ManualCommand".to_owned(), TransferValue::Number(0.0)),
        ("Main.Integral".to_owned(), TransferValue::Number(-3.5)),
        ("Main.PreviousError".to_owned(), TransferValue::Number(0.25)),
    ]);
    let states = pid_transfer_states(&binding, &observed).expect("all four states are numbers");
    assert_eq!(states.len(), 4);
    assert_eq!(states["Main.Integral"], -3.5);

    let mut boolean = observed.clone();
    boolean.insert("Main.Integral".to_owned(), TransferValue::Boolean(true));
    let failure = pid_transfer_states(&binding, &boolean).expect_err("a BOOL is not a state");
    assert_eq!(
        failure.message,
        "The runtime did not return the declared PID state Main.Integral."
    );
    assert_eq!(
        (failure.cause, failure.next_action),
        (Cause::Runtime, NextAction::None)
    );

    let mut missing = observed.clone();
    missing.remove("Main.PreviousError");
    assert!(
        pid_transfer_states(&binding, &missing).is_err(),
        "no state is guessed"
    );

    let mut high = observed;
    high.insert("Main.Integral".to_owned(), TransferValue::Number(150.0));
    let failure = pid_transfer_states(&binding, &high).expect_err("above the state's maximum");
    assert_eq!(
        failure.message,
        "Main.Integral is outside its declared range after IEC rounding."
    );
}

#[test]
fn a_tracking_is_decoded_by_its_kind_word_and_keeps_the_runtime_names() {
    let loop_definition = parallel_pid_loop();
    let binding = pid_transfer_binding(&loop_definition).expect("the fixture binds");
    let tracking = TransferTracking::ParallelPid(
        parallel_pid_tracking(&loop_definition, &binding, true).expect("the tracking"),
    );
    let value = serde_json::to_value(&tracking).expect("the tracking serializes");
    assert_eq!(value["kind"], "parallel-pid-v1");
    assert_eq!(value["align_output"], true);
    let back: TransferTracking = crate::json::decode_value(value).expect("decodes by its kind");
    assert_eq!(back, tracking);

    let proportional = TransferTracking::Proportional(
        proportional_tracking(
            &proportional_loop(),
            &proportional_transfer_binding(&proportional_loop()).expect("binds"),
            false,
        )
        .expect("the tracking"),
    );
    let value = serde_json::to_value(&proportional).expect("the tracking serializes");
    assert_eq!(value["kind"], "proportional");
    assert!(crate::json::decode_value::<TransferTracking>(json!({ "kind": "sideways" })).is_err());
}
