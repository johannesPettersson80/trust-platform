//! Loops and models that the state-controller tests share. Each is written as the loop file or the model file and decoded
//! by the reader that the product uses, so a fixture is valid only when that reader accepts it.

use serde_json::{json, Value};

use crate::loop_definition::LoopDefinition;
use crate::model::general::decode_general_model;
use crate::model::GeneralModel;

/// A channel of a model: its identifier, unit and operating range.
fn channel(id: &str, unit: &str, minimum: f64, maximum: f64) -> Value {
    json!({ "id": id, "unit": unit, "operatingRange": { "minimum": minimum, "maximum": maximum } })
}

/// A signal of the loop file.
fn signal(symbol: &str, address: &str, direction: &str, unit: &str, maximum: f64) -> Value {
    json!({ "symbol": symbol, "address": address, "iecType": "LREAL", "direction": direction,
        "unit": unit, "range": { "minimum": 0, "maximum": maximum } })
}

/// The loop that declares `law` as its state-feedback law. `parameters` are the symbols of its parameter record, each
/// with its unit; each is a `LREAL` from −100 to 100. The loop's task runs at `task_period_ms`.
#[must_use]
pub fn loop_with_law(
    law: &Value,
    parameters: &[(&str, &str)],
    task_period_ms: f64,
) -> LoopDefinition {
    let parameter_values: Vec<Value> = parameters
        .iter()
        .map(|(symbol, unit)| {
            json!({ "symbol": symbol, "iec_type": "LREAL", "unit": unit,
                "convention": "state feedback gain", "minimum": -100, "maximum": 100 })
        })
        .collect();
    let file = json!({
        "schemaVersion": 2, "id": "loop:state", "name": "State feedback", "revision": "sha256:loop-state",
        "projectId": "project:sha256:abc", "runtimeId": "runtime:local",
        "controller": { "instance": "Main.Controller", "type": "State feedback",
            "revision": "sha256:controller-state", "action": "direct", "taskPeriodMs": task_period_ms,
            "design": "project", "stateFeedback": law },
        "parameterBinding": { "loop_id": "loop:state", "task_name": "MainTask",
            "parameters": parameter_values, "mode": { "symbol": "Main.Auto", "manual_value": false } },
        "signals": {
            "processVariable": signal("P1.Tank2M", "%IL8", "read", "m", 0.5),
            "setpoint": signal("P1.Tank2SetpointM", "%ML16", "read", "m", 0.5),
            "manipulatedVariable": signal("P1.PumpM3PerS", "%QL0", "write", "m3/s", 0.02)
        },
        "limits": { "outputMinimum": 0, "outputMaximum": 100 },
        "observedSampleTiming": { "nominalPeriodMs": task_period_ms, "maximumJitterMs": 1 }
    });
    LoopDefinition::decode(&file.to_string()).expect("the fixture loop file decodes")
}

/// The two-tank model of the TypeScript tests, sampled at one second with the given input delay: one input, two measured
/// outputs and two states, each state being an output (`C` is the identity).
fn two_tank_file(delay_seconds: f64) -> Value {
    json!({
        "kind": "discrete-state-space", "revision": "tanks", "source": "fitted", "validation": "validated",
        "evidence": ["recording"], "delaySeconds": delay_seconds, "sampleTimeSeconds": 1.0,
        "inputs": [channel("P1.PumpM3PerS", "m3/s", 0.0, 0.02)],
        "outputs": [channel("P1.Tank2M", "m", 0.0, 0.5), channel("P1.Tank1M", "m", 0.0, 0.5)],
        "states": [channel("history-0", "m", -0.25, 0.25), channel("history-1", "m", -0.25, 0.25)],
        "a": [[0.96, 0.04], [0.0, 0.96]], "b": [[0.0], [2.0]], "c": [[1.0, 0.0], [0.0, 1.0]],
        "d": [[0.0], [0.0]], "operatingPoint": { "inputs": [0.01], "outputs": [0.25, 0.25] }
    })
}

/// The two-tank model with no delay.
#[must_use]
pub fn two_tank_model() -> GeneralModel {
    decode_general_model(&two_tank_file(0.0)).expect("the fixture model decodes")
}

/// The two-tank model with an input delay of `delay_seconds`.
#[must_use]
pub fn two_tank_model_with_delay(delay_seconds: f64) -> GeneralModel {
    decode_general_model(&two_tank_file(delay_seconds)).expect("the fixture model decodes")
}

/// A continuous first-order model, `ẋ = −x + u`, with one input, one output and one state.
#[must_use]
pub fn scalar_model() -> GeneralModel {
    decode_general_model(&json!({
        "kind": "continuous-state-space", "revision": "scalar", "source": "provided",
        "validation": "validated", "evidence": ["bench"], "delaySeconds": 0,
        "inputs": [channel("P1.PumpM3PerS", "m3/s", 0.0, 0.02)],
        "outputs": [channel("P1.Tank2M", "m", 0.0, 0.5)],
        "states": [channel("P1.Tank2M", "m", -1.0, 1.0)],
        "a": [[-1.0]], "b": [[1.0]], "c": [[1.0]], "d": [[0.0]],
        "operatingPoint": { "inputs": [0.01], "outputs": [0.25] }
    }))
    .expect("the fixture model decodes")
}

/// A transfer function `1/(s + 1)`, which no state-feedback law designs on.
#[must_use]
pub fn transfer_function_model() -> GeneralModel {
    decode_general_model(&json!({
        "kind": "transfer-function", "revision": "transfer", "source": "provided",
        "validation": "validated", "evidence": ["bench"], "delaySeconds": 0,
        "numerator": [1.0], "denominator": [1.0, 1.0],
        "inputs": [channel("P1.PumpM3PerS", "m3/s", 0.0, 0.02)],
        "outputs": [channel("P1.Tank2M", "m", 0.0, 0.5)],
        "operatingPoint": { "inputs": [0.01], "outputs": [0.25] }
    }))
    .expect("the fixture model decodes")
}
