use std::collections::BTreeMap;

use super::*;
use crate::commissioning::fixture_loops::{parallel_pid_loop, parallel_pid_loop_without_actuator};
use crate::commissioning::transfer::{pid_transfer_binding, TransferModeBinding};

fn member(symbol: &str, minimum: f64, maximum: f64) -> TransferMember {
    TransferMember {
        symbol: symbol.into(),
        iec_type: TransferValueType::Lreal,
        unit: "%".into(),
        minimum,
        maximum,
    }
}

fn binding(output: TransferMember, guards: Vec<TransferMember>) -> TransferBinding {
    TransferBinding {
        loop_id: "loop:tank".into(),
        task_name: "MainTask".into(),
        mode: TransferModeBinding {
            symbol: "Main.Auto".into(),
            manual_value: false,
        },
        additional_modes: Vec::new(),
        tracking_outputs: BTreeMap::new(),
        states: Vec::new(),
        guards,
        output,
        additional_outputs: Vec::new(),
    }
}

#[test]
fn a_loop_without_a_measured_actuator_keeps_its_binding_unchanged() {
    let binding =
        pid_transfer_binding(&parallel_pid_loop_without_actuator()).expect("the fixture binds");
    assert!(
        binding.tracking_outputs.is_empty(),
        "no feedback without an actuator"
    );
    assert_eq!(binding.guards.len(), 4, "three gains and the process value");
    assert!(binding
        .guards
        .iter()
        .all(|guard| guard.symbol != "ValvePosition"));
}

#[test]
fn a_measured_actuator_is_tracked_as_the_output_feedback_guard() {
    let binding = pid_transfer_binding(&parallel_pid_loop()).expect("the fixture binds");
    assert_eq!(
        binding.tracking_outputs,
        BTreeMap::from([("ValveMV".to_owned(), "ValvePosition".to_owned())]),
        "the output's feedback is the actuator"
    );
    let feedback = binding
        .guards
        .iter()
        .find(|guard| guard.symbol == "ValvePosition")
        .expect("the actuator is a guard");
    assert_eq!(
        (feedback.minimum, feedback.maximum),
        (0.0, 100.0),
        "in the output's limits"
    );
    assert_eq!(binding.output.symbol, "ValveMV");
}

#[test]
fn an_actuator_that_is_not_a_typed_input_in_the_command_unit_is_refused() {
    let mut loop_definition = parallel_pid_loop();
    if let Some(actuator) = loop_definition.signals.actuator.as_mut() {
        actuator.address = Some("%QL8".into());
    }
    let failure =
        pid_transfer_binding(&loop_definition).expect_err("a written actuator is not a feedback");
    assert_eq!(
        failure.message,
        "Actuator tracking needs a separate typed input in command coordinates."
    );
    assert_eq!(
        (failure.cause, failure.next_action),
        (Cause::Implementation, NextAction::OpenLoopSetup)
    );

    let mut other_unit = parallel_pid_loop();
    if let Some(actuator) = other_unit.signals.actuator.as_mut() {
        actuator.unit = "m".into();
    }
    assert!(
        pid_transfer_binding(&other_unit).is_err(),
        "the actuator must share the command's unit"
    );
}

#[test]
fn the_feedback_basis_is_the_guard_of_a_tracked_output_and_the_output_otherwise() {
    let tracked = pid_transfer_binding(&parallel_pid_loop()).expect("the fixture binds");
    let output = tracked.output.clone();
    let basis = transfer_output_basis(&tracked, &output).expect("the feedback guard is bound");
    assert_eq!(basis.symbol, "ValvePosition");

    let plain = binding(member("ValveMV", 0.0, 100.0), Vec::new());
    assert_eq!(
        transfer_output_basis(&plain, &plain.output),
        Some(&plain.output)
    );
}

#[test]
fn a_tracked_output_naming_a_missing_guard_has_no_basis() {
    let mut broken = binding(member("ValveMV", 0.0, 100.0), Vec::new());
    broken
        .tracking_outputs
        .insert("ValveMV".into(), "Ghost".into());
    assert_eq!(transfer_output_basis(&broken, &broken.output), None);
    assert_eq!(transfer_target_allowance(&broken), None);
}

#[test]
fn the_target_allowance_scales_the_output_span_to_the_primary_basis() {
    // The output spans 0..50 while its feedback guard spans 0..100: the output's span, relative to its basis, is 0.5 of the
    // primary span of 100.
    let mut scaled = binding(
        member("ValveMV", 0.0, 50.0),
        vec![member("ValvePosition", 0.0, 100.0)],
    );
    scaled
        .tracking_outputs
        .insert("ValveMV".into(), "ValvePosition".into());
    assert_eq!(transfer_target_allowance(&scaled), Some(50.0));

    // Without feedback the output is its own basis, and its allowance is its span.
    let plain = binding(member("ValveMV", 0.0, 100.0), Vec::new());
    assert_eq!(transfer_target_allowance(&plain), Some(100.0));
}
