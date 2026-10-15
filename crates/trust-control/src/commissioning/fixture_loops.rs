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

/// The gains of a program's own law: `Main.Program.Kp`, `Ki` and `Kd`, bounded by magnitude (N6.4).
fn program_gains() -> Value {
    json!([
        gain("Main.Program.Kp", "proportional gain"),
        gain("Main.Program.Ki", "integral gain"),
        gain("Main.Program.Kd", "derivative gain")
    ])
}

/// The integral state of a program's PI or PID law, started at the operating point by its `pid-integral` formula on `Kp`.
fn program_integral_state() -> InitializedState {
    InitializedState {
        symbol: "Main.Program.Integral".to_owned(),
        iec_type: StateType::Lreal,
        unit: "%".to_owned(),
        minimum: -110.0,
        maximum: 110.0,
        initial: InitialValue::From(InitialSource::PidIntegral {
            index: 0,
            input_index: 0,
            gain_symbol: "Main.Program.Kp".to_owned(),
            weight_symbol: None,
            weight: None,
        }),
        post_scan: PostScan::Bounded,
    }
}

/// The previous error of a program's PD or PID law, started by its `weighted-error` formula (C2 Validate).
fn program_previous_error_state() -> InitializedState {
    InitializedState {
        symbol: "Main.Program.PreviousError".to_owned(),
        iec_type: StateType::Lreal,
        unit: "%".to_owned(),
        minimum: -100.0,
        maximum: 100.0,
        initial: InitialValue::From(InitialSource::WeightedError {
            index: 0,
            weight_symbol: None,
            weight: Some(1.0),
        }),
        post_scan: PostScan::Bounded,
    }
}

/// The loop file of a program's own law (C2): `design: project`, no `law` and no `pidInitialization`, and the `project` block that
/// the setup writes.
#[allow(clippy::needless_pass_by_value)] // A test fixture: it takes the JSON literals its callers write in place.
fn project_loop_file(
    family: &str,
    seed: Value,
    states: &[InitializedState],
    application: &[InitializedState],
    parameters: Value,
) -> Value {
    let mut file = loop_file(Value::Null, None, parameters, true);
    file["controller"]["design"] = json!("project");
    file["controller"]["project"] = json!({
        "family": family,
        "convention": "project-owned-native-controller",
        "initialization": { "kind": "declared-state-v1", "states": states, "applicationStates": application },
        "seed": seed,
        "axes": [],
        "objective": { "requirementId": "target-error", "direction": "minimize" },
        "maximumCandidates": 4,
        "maximumCycles": 50000,
        "wallBudgetMs": 60000
    });
    file
}

/// A closed loop of a program instance's own PI law (C2): its seed names `P1.Kp` and `P1.Ki`, and its declared state is the integral
/// term, which its `pid-integral` formula starts on `P1.Kp`.
#[must_use]
pub fn project_pi_loop_on_p1() -> LoopDefinition {
    let integral = InitializedState {
        symbol: "P1.Integral".to_owned(),
        iec_type: StateType::Lreal,
        unit: "%".to_owned(),
        minimum: -110.0,
        maximum: 110.0,
        initial: InitialValue::From(InitialSource::PidIntegral {
            index: 0,
            input_index: 0,
            gain_symbol: "P1.Kp".to_owned(),
            weight_symbol: None,
            weight: None,
        }),
        post_scan: PostScan::Bounded,
    };
    decode(&project_loop_file(
        "PI",
        json!({ "kind": "parallel-pid", "family": "PI", "proportionalSymbol": "P1.Kp",
            "integralSymbol": "P1.Ki" }),
        &[integral],
        &[],
        json!([
            gain("P1.Kp", "parallel proportional gain"),
            gain("P1.Ki", "parallel integral gain")
        ]),
    ))
}

/// A closed loop of a program's own PID law (C2): its seed names the variables `Main.Program.Kp`, `Ki` and `Kd`, and its declared
/// states are the integral and the previous error.
#[must_use]
pub fn project_pid_loop() -> LoopDefinition {
    let states = [program_integral_state(), program_previous_error_state()];
    decode(&project_loop_file(
        "PID",
        json!({ "kind": "parallel-pid", "family": "PID", "proportionalSymbol": "Main.Program.Kp",
            "integralSymbol": "Main.Program.Ki", "derivativeSymbol": "Main.Program.Kd" }),
        &states,
        &[],
        program_gains(),
    ))
}

/// A closed loop of a Smith predictor (C9): its PI's variables `Main.Program.Kp`, `Ki` and `Kd`, the model's gain, time constant and delay,
/// and no library block. Its parameters bound the PI's gains and the model's three values as the setup writes them (N10).
#[must_use]
pub fn smith_predictor_loop() -> LoopDefinition {
    let mut parameters = program_gains();
    let values = parameters.as_array_mut().expect("the parameters");
    for (symbol, unit, convention, minimum, maximum) in [
        (
            "Main.Program.ModelGain",
            "degC/%",
            "Smith predictor model gain",
            -1e6,
            1e6,
        ),
        (
            "Main.Program.ModelTau",
            "s",
            "Smith predictor model time constant",
            0.001,
            1e5,
        ),
        (
            "Main.Program.ModelDelay",
            "s",
            "Smith predictor model delay",
            0.0,
            1e4,
        ),
    ] {
        values.push(json!({ "symbol": symbol, "iec_type": "LREAL", "unit": unit,
            "convention": convention, "minimum": minimum, "maximum": maximum }));
    }
    decode(&project_loop_file(
        "Smith",
        json!({ "kind": "smith-pid", "family": "PI", "proportionalSymbol": "Main.Program.Kp",
            "integralSymbol": "Main.Program.Ki", "derivativeSymbol": "Main.Program.Kd",
            "gainSymbol": "Main.Program.ModelGain", "timeConstantSymbol": "Main.Program.ModelTau",
            "delaySymbol": "Main.Program.ModelDelay" }),
        &[program_integral_state(), program_previous_error_state()],
        &[],
        parameters,
    ))
}

/// A closed loop of a program's own P law (C2): its seed names the variable `Main.Program.Kp` alone, and it declares no state.
#[must_use]
pub fn project_p_loop() -> LoopDefinition {
    decode(&project_loop_file(
        "P",
        json!({ "kind": "parallel-pid", "family": "P", "proportionalSymbol": "Main.Program.Kp" }),
        &[],
        &[],
        json!([gain("Main.Program.Kp", "proportional gain")]),
    ))
}

/// A program's own P law with a startup flag (C1): the flag is an application state, a BOOL that the connected P entry writes TRUE at
/// the Auto entry (C13).
#[must_use]
pub fn project_p_loop_with_startup() -> LoopDefinition {
    let started = InitializedState {
        symbol: "Main.Program.Started".to_owned(),
        iec_type: StateType::Bool,
        unit: "1".to_owned(),
        minimum: 0.0,
        maximum: 1.0,
        initial: InitialValue::Constant(1.0),
        post_scan: PostScan::Unchanged,
    };
    decode(&project_loop_file(
        "P",
        json!({ "kind": "parallel-pid", "family": "P", "proportionalSymbol": "Main.Program.Kp" }),
        &[],
        &[started],
        json!([gain("Main.Program.Kp", "proportional gain")]),
    ))
}

/// A program's own PI law with its startup flag (C1, C2): its seed names `P1.Kp` and `P1.Ki`, its declared state is the integral term
/// that its `pid-integral` formula starts on `P1.Kp`, and the startup-done flag is an application state that the Auto entry sets TRUE.
#[must_use]
pub fn project_pi_loop_with_startup() -> LoopDefinition {
    let integral = InitializedState {
        symbol: "P1.Integral".to_owned(),
        iec_type: StateType::Lreal,
        unit: "%".to_owned(),
        minimum: -110.0,
        maximum: 110.0,
        initial: InitialValue::From(InitialSource::PidIntegral {
            index: 0,
            input_index: 0,
            gain_symbol: "P1.Kp".to_owned(),
            weight_symbol: None,
            weight: None,
        }),
        post_scan: PostScan::Bounded,
    };
    let started = InitializedState {
        symbol: "Main.Program.Started".to_owned(),
        iec_type: StateType::Bool,
        unit: "1".to_owned(),
        minimum: 0.0,
        maximum: 1.0,
        initial: InitialValue::Constant(1.0),
        post_scan: PostScan::Unchanged,
    };
    decode(&project_loop_file(
        "PI",
        json!({ "kind": "parallel-pid", "family": "PI", "proportionalSymbol": "P1.Kp",
            "integralSymbol": "P1.Ki" }),
        &[integral],
        &[started],
        json!([
            gain("P1.Kp", "parallel proportional gain"),
            gain("P1.Ki", "parallel integral gain")
        ]),
    ))
}

/// A program's own P law that declares a law state, an integral term that a P law cannot start (C13): the connected P refuses it by
/// name.
#[must_use]
pub fn project_p_loop_with_law_state() -> LoopDefinition {
    decode(&project_loop_file(
        "P",
        json!({ "kind": "parallel-pid", "family": "P", "proportionalSymbol": "Main.Program.Kp" }),
        &[program_integral_state()],
        &[],
        json!([gain("Main.Program.Kp", "proportional gain")]),
    ))
}

/// A program's own P law whose application state takes a formula (C13): the Auto entry cannot compute it without the model, so the
/// connected P refuses it by name.
#[must_use]
pub fn project_p_loop_with_formula_application() -> LoopDefinition {
    decode(&project_loop_file(
        "P",
        json!({ "kind": "parallel-pid", "family": "P", "proportionalSymbol": "Main.Program.Kp" }),
        &[],
        &[program_integral_state()],
        json!([gain("Main.Program.Kp", "proportional gain")]),
    ))
}

/// A program's own law without a seed: the gate parks it by name, and no adapter here realizes it.
#[must_use]
pub fn parked_project_loop() -> LoopDefinition {
    decode(&project_loop_file(
        "PI",
        Value::Null,
        &[],
        &[],
        program_gains(),
    ))
}

/// A program's own PID law whose block carries a tuning axis (C5): its seed is a served shape, but the block is a search, which the
/// gate parks.
#[must_use]
pub fn parked_axes_project_loop() -> LoopDefinition {
    let mut file = project_loop_file(
        "PID",
        json!({ "kind": "parallel-pid", "family": "PID", "proportionalSymbol": "Main.Program.Kp",
            "integralSymbol": "Main.Program.Ki", "derivativeSymbol": "Main.Program.Kd" }),
        &[program_integral_state(), program_previous_error_state()],
        &[],
        program_gains(),
    );
    file["controller"]["project"]["axes"] =
        json!([{ "symbol": "Main.Program.Kp", "values": [1.0, 2.0] }]);
    decode(&file)
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
