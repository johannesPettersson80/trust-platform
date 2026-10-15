//! Tests of the steps of a loop's plant inputs (`channel_steps.rs`): an input with its own identification step is stepped at that step, as
//! a step of that input, and an input without one keeps the loop's experiment. Each input's method follows the rule of C7 and C8: an
//! input with its own step is a manual-output step, and an input without one takes the method Identify chose for the loop.

#![allow(clippy::float_cmp)]

use serde_json::{json, Value};

use super::{campaign_method, input_steps, own_steps_check, InputStep};
use crate::experiment::ControlExperimentKind;
use crate::failure::{Cause, NextAction};
use crate::identification::policy::IdentificationMethod;
use crate::loop_definition::{ExperimentKind, LoopDefinition};

/// A loop with two plant inputs, as `connected_run_tests.rs` declares one: the valve, which is the manipulated variable, and a measured
/// load stepped through its own Manual command. The loop asks for `experiment`; each input has its own identification step where one is
/// given.
fn two_input_loop(
    experiment: &Value,
    valve_step: Option<f64>,
    load_step: Option<f64>,
) -> LoopDefinition {
    let mut valve = json!({
        "modelChannelId": "valve", "signalId": "manipulatedVariable", "manualOutputSignal": "P1.ManualPct",
        "manualOutputType": "LREAL", "actuator": { "kind": "continuous", "minimum": 0.0, "maximum": 100.0 }
    });
    let mut load = json!({
        "modelChannelId": "load", "signalId": "load", "manualOutputSignal": "P1.LoadKw",
        "manualOutputType": "LREAL", "actuator": { "kind": "continuous", "minimum": 0.0, "maximum": 50.0 }
    });
    if let Some(step) = valve_step {
        valve["experimentStep"] = json!(step);
    }
    if let Some(step) = load_step {
        load["experimentStep"] = json!(step);
    }
    serde_json::from_value(json!({
        "schemaVersion": 2, "id": "heater", "name": "Heater", "revision": "sha256:heater",
        "projectId": "project", "runtimeId": "runtime",
        "controller": {
            "instance": "P1.Controller", "type": "Control.PID", "revision": "sha256:controller",
            "action": "direct", "taskPeriodMs": 100.0,
            "manualOutputSignal": "P1.ManualPct", "manualOutputType": "LREAL"
        },
        "observedSampleTiming": { "nominalPeriodMs": 100.0 },
        "signals": {
            "setpoint": { "symbol": "LevelSP", "iecType": "LREAL", "direction": "read", "unit": "%",
                "range": { "minimum": 0.0, "maximum": 100.0 } },
            "processVariable": { "symbol": "LevelPV", "iecType": "LREAL", "direction": "read", "unit": "%",
                "range": { "minimum": 0.0, "maximum": 100.0 } },
            "manipulatedVariable": { "symbol": "ValveMV", "iecType": "LREAL", "direction": "write", "unit": "%",
                "range": { "minimum": 0.0, "maximum": 100.0 } },
            "additional": [{
                "id": "load", "channel": "load", "role": "disturbance", "symbol": "P1.LoadKw", "iecType": "LREAL",
                "direction": "read", "unit": "kW", "range": { "minimum": 0.0, "maximum": 50.0 }
            }]
        },
        "limits": { "outputMinimum": 0.0, "outputMaximum": 100.0 },
        "experiment": experiment,
        "channels": {
            "inputs": [valve, load],
            "outputs": [{ "modelChannelId": "level", "signalId": "processVariable", "setpointSignalId": "setpoint" }]
        }
    }))
    .expect("the two-input loop decodes")
}

/// The I/O list that names the load's Manual command, so that the load is stepped through it.
fn io_list() -> Value {
    json!({ "memory": [{ "name": "P1.LoadKw", "address": "%MD4" }] })
}

#[test]
fn an_input_with_its_own_step_is_stepped_at_it_as_a_step_of_that_input() {
    // The group's automatic request (B3.4): an observation with no amplitude. The valve has a step of its own, and the load has none.
    let loop_definition = two_input_loop(
        &json!({
            "execution": "control-studio", "kind": "observation", "amplitude": 0.0,
            "durationSeconds": 40.0, "seed": 1
        }),
        Some(10.0),
        None,
    );
    let steps = input_steps(&loop_definition, &io_list()).expect("two inputs step in order");
    let valve = steps[0]
        .loop_definition
        .experiment
        .as_ref()
        .expect("the valve is stepped on the experiment");
    assert_eq!(
        (valve.kind, valve.amplitude),
        (ExperimentKind::MvStep, 10.0),
        "the valve is stepped at its own step, as a step of the valve"
    );
    let load = steps[1]
        .loop_definition
        .experiment
        .as_ref()
        .expect("the load is stepped on the experiment");
    assert_eq!(
        (load.kind, load.amplitude),
        (ExperimentKind::Observation, 0.0),
        "the load without a step keeps the loop's observation"
    );
}

#[test]
fn an_input_without_a_step_keeps_the_loops_kind_and_amplitude() {
    // The loop is a PRBS of amplitude 2 and neither input has a step of its own, so both keep it.
    let loop_definition = two_input_loop(
        &json!({
            "execution": "control-studio", "kind": "prbs", "amplitude": 2.0,
            "durationSeconds": 40.0, "seed": 1
        }),
        None,
        None,
    );
    let steps = input_steps(&loop_definition, &io_list()).expect("two inputs step in order");
    for step in &steps {
        let experiment = step
            .loop_definition
            .experiment
            .as_ref()
            .expect("each input is stepped on the experiment");
        assert_eq!(
            (experiment.kind, experiment.amplitude),
            (ExperimentKind::Prbs, 2.0),
            "{} keeps the loop's kind and amplitude",
            step.input_symbol
        );
    }
}

/// The loop's experiment when it declares no method, so that Identify chooses the method (B3.3).
fn observation() -> Value {
    json!({
        "execution": "control-studio", "kind": "observation", "amplitude": 0.0,
        "durationSeconds": 40.0, "seed": 1
    })
}

/// The method of a manual-output step, which an input's own identification step runs (C7, C8).
fn manual_output() -> IdentificationMethod {
    IdentificationMethod {
        kind: ControlExperimentKind::MvStep,
        manual: true,
        automatic: false,
    }
}

/// The method Identify chose for a loop in Manual: a plant-input step, which runs in Manual (B3.3).
fn chosen_in_manual() -> IdentificationMethod {
    IdentificationMethod {
        kind: ControlExperimentKind::MvStep,
        manual: true,
        automatic: true,
    }
}

/// The method Identify chose for a loop in Auto with a setpoint: a setpoint step, which runs in Auto (B3.3).
fn chosen_in_auto() -> IdentificationMethod {
    IdentificationMethod {
        kind: ControlExperimentKind::SetpointStep,
        manual: false,
        automatic: true,
    }
}

/// The size of an input's step: the amplitude of the experiment its step is planned on.
fn step_size(step: &InputStep) -> Option<f64> {
    step.loop_definition
        .experiment
        .as_ref()
        .map(|experiment| experiment.amplitude)
}

// C7, C8: when every input declares its own step, each is stepped by a manual-output step of its own size, and the campaign runs in Manual
// whatever the loop's own method is, so no setpoint step shares its recording.
#[test]
fn inputs_that_all_declare_steps_are_manual_output_steps_of_those_sizes() {
    let loop_definition = two_input_loop(&observation(), Some(10.0), Some(5.0));
    let steps = input_steps(&loop_definition, &io_list()).expect("two inputs step in order");
    assert_eq!(steps[0].own_method, Some(manual_output()));
    assert_eq!(steps[1].own_method, Some(manual_output()));
    let sizes: Vec<Option<f64>> = steps.iter().map(step_size).collect();
    assert_eq!(
        sizes,
        [Some(10.0), Some(5.0)],
        "each input moves by its own step"
    );
    assert_eq!(
        campaign_method(&loop_definition, chosen_in_auto()),
        manual_output(),
        "the campaign is a manual-output campaign"
    );
    assert_eq!(
        own_steps_check(&loop_definition, chosen_in_auto()),
        Ok(()),
        "every input steps in Manual, so no setpoint step shares the recording"
    );
}

// C7, C8: an input without its own step takes the method Identify chose for the loop, beside the input that is stepped at its own step.
#[test]
fn an_input_without_a_step_takes_the_loops_method_beside_one_with_a_step() {
    let loop_definition = two_input_loop(&observation(), Some(4.0), None);
    let steps = input_steps(&loop_definition, &io_list()).expect("two inputs step in order");
    assert_eq!(
        steps[0].own_method,
        Some(manual_output()),
        "the valve steps at its own step"
    );
    assert_eq!(
        steps[1].own_method, None,
        "the load takes the loop's method"
    );
    assert_eq!(
        campaign_method(&loop_definition, chosen_in_manual()),
        chosen_in_manual(),
        "in Manual the campaign runs the method Identify chose"
    );
    assert_eq!(
        own_steps_check(&loop_definition, chosen_in_manual()),
        Ok(()),
        "a manual-output step and Identify's choice in Manual share one recording"
    );
    let refusal = own_steps_check(&loop_definition, chosen_in_auto())
        .expect_err("a setpoint step cannot share the recording with the valve's step");
    assert_eq!(
        (refusal.cause, refusal.next_action),
        (Cause::Settings, NextAction::OpenLoopSetup)
    );
    assert!(
        refusal.message.contains("P1.LoadKw"),
        "the refusal names the load: {}",
        refusal.message
    );
}

// C7, C8: a loop with one plant input is planned as it was: its step is the loop itself, and it runs the loop's own method.
#[test]
fn a_loop_with_one_plant_input_is_planned_as_it_was() {
    let mut loop_definition = two_input_loop(&observation(), Some(10.0), Some(5.0));
    if let Some(channels) = loop_definition.channels.as_mut() {
        channels.inputs.truncate(1);
    }
    let steps = input_steps(&loop_definition, &Value::Null).expect("one input steps as the loop");
    assert_eq!(steps.len(), 1);
    assert_eq!(
        steps[0].loop_definition, loop_definition,
        "the step is the loop itself"
    );
    assert_eq!(steps[0].own_method, None, "the loop's own method runs");
    assert_eq!(
        campaign_method(&loop_definition, chosen_in_auto()),
        chosen_in_auto()
    );
    assert_eq!(own_steps_check(&loop_definition, chosen_in_auto()), Ok(()));
}

// C7, C8: an own step is a manual-output step, so the identification policy must permit one, as it permits a declared `mv_step`.
#[test]
fn an_own_step_is_refused_where_the_policy_forbids_a_sustained_step() {
    let mut loop_definition = two_input_loop(&observation(), Some(10.0), None);
    loop_definition.identification_policy = Some(json!({
        "openLoopPermitted": true, "sustainedHoldPermitted": false, "information": "basic"
    }));
    let refusal = own_steps_check(&loop_definition, chosen_in_manual())
        .expect_err("a sustained step is not permitted");
    assert_eq!(
        (refusal.cause, refusal.next_action),
        (Cause::Settings, NextAction::ChooseMethod)
    );
}
