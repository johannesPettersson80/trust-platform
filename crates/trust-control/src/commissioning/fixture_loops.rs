//! Loops, models and requirements that the commissioning tests share. Test-only: the module is declared under `cfg(test)`.
//! Each loop is written as its loop file and decoded with `LoopDefinition::decode`, so a fixture is valid only when the
//! loop contract accepts it.

// Each fixture is valid by construction, so none of them can panic on its input: the `expect`s only state that.
#![allow(clippy::missing_panics_doc)]

use serde_json::{json, Value};

use crate::loop_definition::{
    Direction, InitialSource, InitialValue, InitializedState, LoopDefinition, LoopSignal,
    PidInitialization, PostScan, Range, StateType,
};
use crate::model::general::decode_general_model;
use crate::model::GeneralModel;
use crate::validation::engineer_verification_plan::{engineer_requirements, EngineerRequirements};

/// The loop file of a loop with the given law, initialization and parameter record.
#[allow(clippy::needless_pass_by_value)] // A test fixture: it takes the JSON literals its callers write in place.
fn loop_file(
    law: Value,
    initialization: Option<Value>,
    parameters: Value,
    actuator: bool,
) -> Value {
    let mut signals = json!({
        "setpoint": { "symbol": "LevelSP", "address": "%ML0", "iecType": "LREAL", "direction": "read",
            "unit": "%", "range": { "minimum": 0, "maximum": 100 } },
        "processVariable": { "symbol": "LevelPV", "address": "%IL0", "iecType": "LREAL", "direction": "read",
            "unit": "%", "range": { "minimum": 0, "maximum": 100 } },
        "manipulatedVariable": { "symbol": "ValveMV", "address": "%QL0", "iecType": "LREAL", "direction": "write",
            "unit": "%", "range": { "minimum": 0, "maximum": 100 } }
    });
    if actuator {
        signals["actuator"] = json!({ "symbol": "ValvePosition", "address": "%IL8", "iecType": "LREAL",
            "direction": "read", "unit": "%", "range": { "minimum": 0, "maximum": 100 } });
    }
    let mut controller = json!({
        "instance": "Main.TankController", "type": "Control.PID", "revision": "sha256:controller-1",
        "action": "direct", "taskPeriodMs": 10, "law": law,
        "manualOutputSignal": "Main.ManualCommand", "manualOutputType": "LREAL"
    });
    if let Some(initialization) = initialization {
        controller["pidInitialization"] = initialization;
    }
    json!({
        "schemaVersion": 2, "id": "loop:tank", "name": "Tank level", "revision": "sha256:loop-1",
        "projectId": "project:sha256:abc", "runtimeId": "runtime:local",
        "controller": controller,
        "parameterBinding": { "loop_id": "loop:tank", "task_name": "MainTask", "parameters": parameters,
            "mode": { "symbol": "Main.Auto", "manual_value": false } },
        "signals": signals,
        "limits": { "outputMinimum": 0, "outputMaximum": 100 },
        "observedSampleTiming": { "nominalPeriodMs": 10, "maximumJitterMs": 1 }
    })
}

/// A parameter of the record: a gain, bounded by magnitude (N6.4).
fn gain(symbol: &str, convention: &str) -> Value {
    json!({ "symbol": symbol, "iec_type": "LREAL", "unit": "%/%", "convention": convention, "minimum": -100, "maximum": 100 })
}

fn decode(file: &Value) -> LoopDefinition {
    LoopDefinition::decode(&file.to_string()).expect("the fixture loop file decodes")
}

/// A closed loop with a parallel PID (`parallel-pid-v1`): gains `Main.Parameters.Kp`, `Ki` and `Kd`, the integral state
/// `Main.Integral` and the derivative state `Main.PreviousError`, and the actuator `ValvePosition`.
#[must_use]
pub fn parallel_pid_loop() -> LoopDefinition {
    decode(&loop_file(
        pid_law(),
        Some(parallel_pid_initialization(
            "Main.Integral",
            "Main.PreviousError",
        )),
        pid_parameters(),
        true,
    ))
}

/// A closed loop with a parallel PID and no actuator measurement.
#[must_use]
pub fn parallel_pid_loop_without_actuator() -> LoopDefinition {
    decode(&loop_file(
        pid_law(),
        Some(parallel_pid_initialization(
            "Main.Integral",
            "Main.PreviousError",
        )),
        pid_parameters(),
        false,
    ))
}

/// A closed loop of a shipped `Control.PID` (`control-pid-v1`): its states are declared by the library, not by a
/// `parallel-pid-v1` contract.
#[must_use]
pub fn control_pid_loop() -> LoopDefinition {
    decode(&loop_file(
        pid_law(),
        Some(json!({ "kind": "control-pid-v1", "maximumSnapshotAgeMs": 100, "states": [] })),
        pid_parameters(),
        true,
    ))
}

/// A closed loop with a proportional law and its gain `Main.Parameters.Kp`, and the actuator `ValvePosition`.
#[must_use]
pub fn proportional_loop() -> LoopDefinition {
    decode(&loop_file(
        json!({ "kind": "proportional", "gainSymbol": "Main.Parameters.Kp" }),
        None,
        json!([gain("Main.Parameters.Kp", "proportional gain")]),
        true,
    ))
}

/// The requirements of the study: a 4 s test duration and a 0.02 settling band (B6.2, B6.3).
///
/// # Panics
/// Never for this fixture: its values are valid.
#[must_use]
pub fn requirements() -> EngineerRequirements {
    engineer_requirements(
        &json!({ "referenceChange": 1, "maximumError": 0.1, "settlingBand": 0.02,
        "settlingTimeSeconds": 2, "durationSeconds": 4 }),
    )
    .expect("the fixture requirements are valid")
}

/// A stable first-order model, `1/(s + 1)`, operating at input 0 and output 2 (the validated operating point).
#[must_use]
pub fn first_order_model() -> GeneralModel {
    model(json!([1]), json!([1, 1]))
}

/// An integrating model, `1/s`, operating at input 0 and output 2.
#[must_use]
pub fn integrating_model() -> GeneralModel {
    model(json!([1]), json!([1, 0]))
}

#[allow(clippy::needless_pass_by_value)] // A test fixture: it takes the JSON literals its callers write in place.
fn model(numerator: Value, denominator: Value) -> GeneralModel {
    let file = json!({ "kind": "transfer-function", "revision": "model:1", "source": "provided",
        "validation": "validated", "evidence": ["bench"], "delaySeconds": 0,
        "numerator": numerator, "denominator": denominator,
        "inputs": [{ "id": "u", "unit": "%", "operatingRange": { "minimum": 0, "maximum": 100 } }],
        "outputs": [{ "id": "y", "unit": "%", "operatingRange": { "minimum": 0, "maximum": 100 } }],
        "operatingPoint": { "inputs": [0], "outputs": [2] } });
    decode_general_model(&file).expect("the fixture model decodes")
}

fn pid_law() -> Value {
    json!({ "kind": "pid", "parameterRecordSymbol": "Main.Parameters", "proportionalGainSymbol": "Main.Parameters.Kp",
        "integralGainSymbol": "Main.Parameters.Ki", "derivativeGainSymbol": "Main.Parameters.Kd" })
}

fn pid_parameters() -> Value {
    json!([
        gain("Main.Parameters.Kp", "proportional gain"),
        gain("Main.Parameters.Ki", "integral gain"),
        gain("Main.Parameters.Kd", "derivative gain")
    ])
}

fn parallel_pid_initialization(integral: &str, derivative: &str) -> Value {
    json!({ "kind": "parallel-pid-v1",
        "integralState": { "symbol": integral, "iecType": "LREAL", "unit": "%", "minimum": -100, "maximum": 100 },
        "derivativeState": { "symbol": derivative, "iecType": "LREAL", "unit": "%", "minimum": -100, "maximum": 100 },
        "proportionalSetpointWeight": 1, "derivativeSetpointWeight": 0, "maximumSnapshotAgeMs": 100 })
}

/// The seven retained states of the fixture's `Main.TankController` (C1): the setpoint and the process value at the output, the
/// rates and the feedforward at zero, and the nominal and saturated outputs at the input.
#[must_use]
pub fn library_pid_states() -> Vec<InitializedState> {
    let state = |member: &str, unit: &str, initial: InitialValue| InitializedState {
        symbol: format!("Main.TankController.StateValue.{member}"),
        iec_type: StateType::Lreal,
        unit: unit.to_owned(),
        minimum: -10_000.0,
        maximum: 10_000.0,
        initial,
        post_scan: PostScan::Bounded,
    };
    let output = || InitialValue::From(InitialSource::ModelOutput { index: 0 });
    let input = || InitialValue::From(InitialSource::ModelInput { index: 0 });
    vec![
        state("Setpoint", "%", output()),
        state("SetpointRate", "%/s", InitialValue::Constant(0.0)),
        state("ProcessValue", "%", output()),
        state("ProcessValueRate", "%/s", InitialValue::Constant(0.0)),
        state("FeedForward", "%", InitialValue::Constant(0.0)),
        state("NominalOutput", "%", input()),
        state("SaturatedOutput", "%", input()),
    ]
}

/// The fixture loop with its seven states declared, and its process value the PID's own measurement (C1).
#[must_use]
pub fn library_pid_loop() -> LoopDefinition {
    let mut loop_definition = control_pid_loop();
    loop_definition.signals.process_variable = LoopSignal {
        symbol: "Main.TankController.StateValue.ProcessValue".to_owned(),
        address: None,
        iec_type: "LREAL".to_owned(),
        direction: Direction::Read,
        unit: "%".to_owned(),
        range: Range {
            minimum: 0.0,
            maximum: 100.0,
        },
    };
    loop_definition.controller.pid_initialization = Some(PidInitialization::ControlPid {
        maximum_snapshot_age_ms: 100,
        states: library_pid_states(),
        application_states: None,
    });
    loop_definition
}
