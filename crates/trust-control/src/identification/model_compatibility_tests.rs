use serde_json::{json, Value};

use super::*;
use crate::model::decode_general_model;

/// A loop with a setpoint in degC, a measured output in 20 to 80 degC, and a manipulated variable in 0 to 100 %.
fn loop_with_setpoint() -> LoopDefinition {
    serde_json::from_value(json!({
        "schemaVersion": 2,
        "id": "tank",
        "name": "Tank",
        "revision": "sha256:tank",
        "projectId": "test",
        "runtimeId": "test",
        "controller": {
            "instance": "P1.Controller",
            "type": "Control.PID",
            "revision": "sha256:controller",
            "action": "direct",
            "taskPeriodMs": 100.0
        },
        "observedSampleTiming": { "nominalPeriodMs": 100.0 },
        "signals": {
            "setpoint": {
                "symbol": "P1.SP", "iecType": "LREAL", "direction": "read",
                "unit": "degC", "range": { "minimum": 20.0, "maximum": 80.0 }
            },
            "processVariable": {
                "symbol": "P1.PV", "iecType": "LREAL", "direction": "read",
                "unit": "degC", "range": { "minimum": 20.0, "maximum": 80.0 }
            },
            "manipulatedVariable": {
                "symbol": "P1.MV", "iecType": "LREAL", "direction": "write",
                "unit": "%", "range": { "minimum": 0.0, "maximum": 100.0 }
            }
        },
        "limits": { "outputMinimum": 0.0, "outputMaximum": 100.0 }
    }))
    .expect("the fixture loop decodes")
}

/// A first-order transfer function `2/(s + 2)` from a drive in `input_unit` to a speed in degC.
#[allow(clippy::needless_pass_by_value)] // json! only borrows its arguments.
fn transfer_model(
    input_unit: &str,
    evidence: Value,
    source: &str,
    validation: &str,
) -> GeneralModel {
    decode_general_model(&json!({
        "kind": "transfer-function",
        "numerator": [2.0],
        "denominator": [1.0, 2.0],
        "delaySeconds": 0.0,
        "revision": "model-1",
        "source": source,
        "validation": validation,
        "evidence": evidence,
        "inputs": [{
            "id": "drive",
            "unit": input_unit,
            "operatingRange": { "minimum": 0.0, "maximum": 100.0 }
        }],
        "outputs": [{
            "id": "speed",
            "unit": "degC",
            "operatingRange": { "minimum": 20.0, "maximum": 80.0 }
        }]
    }))
    .expect("the model decodes")
}

#[test]
fn a_validated_model_of_the_loops_units_and_ranges_is_compatible() {
    let model = transfer_model("%", json!(["step test 1"]), "provided", "validated");
    assert_eq!(
        assert_engineer_model_compatibility(&loop_with_setpoint(), &model),
        Ok(())
    );
    assert_eq!(
        assert_qualified_model_result(&loop_with_setpoint(), &model),
        Ok(())
    );
}

#[test]
fn a_model_input_in_another_unit_names_the_plant_input_it_does_not_match() {
    let model = transfer_model("V", json!(["step test 1"]), "provided", "validated");
    assert_eq!(
        assert_engineer_model_compatibility(&loop_with_setpoint(), &model),
        Err(CompatibilityRefusal::InputUnit {
            id: "drive".to_string(),
            symbol: "P1.MV".to_string(),
        })
    );
}

#[test]
fn a_loop_without_a_setpoint_has_no_controlled_output() {
    let mut loop_definition = loop_with_setpoint();
    loop_definition.signals.setpoint = None;
    let model = transfer_model("%", json!(["step test 1"]), "provided", "validated");
    assert_eq!(
        assert_engineer_model_compatibility(&loop_definition, &model),
        Err(CompatibilityRefusal::NoControlledOutput)
    );
}

#[test]
fn a_loop_without_its_output_maximum_has_no_actuator_range_for_the_model_input_to_meet() {
    let mut loop_definition = loop_with_setpoint();
    loop_definition.limits.output_maximum = None;
    let model = transfer_model("%", json!(["step test 1"]), "provided", "validated");
    assert_eq!(
        assert_qualified_model_result(&loop_definition, &model),
        Err(CompatibilityRefusal::InputActuatorRange {
            id: "drive".to_string(),
        })
    );
}

#[test]
fn a_supplied_model_without_evidence_is_refused_but_a_fit_is_not() {
    let unproven = transfer_model("%", json!([]), "provided", "unvalidated");
    assert_eq!(
        assert_engineer_model_compatibility(&loop_with_setpoint(), &unproven),
        Err(CompatibilityRefusal::Evidence)
    );
    assert_eq!(
        assert_qualified_model_result(&loop_with_setpoint(), &unproven),
        Err(CompatibilityRefusal::Evidence)
    );
    let fitted = transfer_model("%", json!(["recording 7"]), "fitted", "unvalidated");
    assert_eq!(
        assert_engineer_model_compatibility(&loop_with_setpoint(), &fitted),
        Ok(())
    );
}

#[test]
fn a_discrete_model_must_sample_at_the_controller_task_period() {
    let discrete = |sample_time_seconds: f64| {
        decode_general_model(&json!({
            "kind": "discrete-state-space",
            "states": [{
                "id": "x", "unit": "degC", "operatingRange": { "minimum": 20.0, "maximum": 80.0 }
            }],
            "a": [[0.9]],
            "b": [[0.1]],
            "c": [[1.0]],
            "d": [[0.0]],
            "delaySeconds": 0.0,
            "sampleTimeSeconds": sample_time_seconds,
            "revision": "model-2",
            "source": "provided",
            "validation": "validated",
            "evidence": ["step test 2"],
            "inputs": [{
                "id": "drive", "unit": "%", "operatingRange": { "minimum": 0.0, "maximum": 100.0 }
            }],
            "outputs": [{
                "id": "speed", "unit": "degC", "operatingRange": { "minimum": 20.0, "maximum": 80.0 }
            }]
        }))
        .expect("the discrete model decodes")
    };
    assert_eq!(
        assert_engineer_model_compatibility(&loop_with_setpoint(), &discrete(0.2)),
        Err(CompatibilityRefusal::SampleTime),
        "0.2 s is not the 100 ms task period"
    );
    assert_eq!(
        assert_engineer_model_compatibility(&loop_with_setpoint(), &discrete(0.1)),
        Ok(())
    );
}

/// A loop with two plant inputs and two measured outputs (C7, C8): the heater (`P1.MV`, 0 to 100 %) and a measured load (`P1.LoadKw`,
/// 0 to 50 kW), and the process temperature with its setpoint and a flow (`P1.Flow`, 0 to 20 L/s).
fn two_channel_loop() -> LoopDefinition {
    serde_json::from_value(json!({
        "schemaVersion": 2,
        "id": "tank",
        "name": "Tank",
        "revision": "sha256:tank",
        "projectId": "test",
        "runtimeId": "test",
        "controller": {
            "instance": "P1.Controller",
            "type": "Control.PID",
            "revision": "sha256:controller",
            "action": "direct",
            "taskPeriodMs": 100.0
        },
        "observedSampleTiming": { "nominalPeriodMs": 100.0 },
        "signals": {
            "setpoint": {
                "symbol": "P1.SP", "iecType": "LREAL", "direction": "read",
                "unit": "degC", "range": { "minimum": 20.0, "maximum": 80.0 }
            },
            "processVariable": {
                "symbol": "P1.PV", "iecType": "LREAL", "direction": "read",
                "unit": "degC", "range": { "minimum": 20.0, "maximum": 80.0 }
            },
            "manipulatedVariable": {
                "symbol": "P1.MV", "iecType": "LREAL", "direction": "write",
                "unit": "%", "range": { "minimum": 0.0, "maximum": 100.0 }
            },
            "additional": [
                {
                    "id": "load", "channel": "load", "role": "disturbance", "symbol": "P1.LoadKw",
                    "iecType": "LREAL", "direction": "read", "unit": "kW",
                    "range": { "minimum": 0.0, "maximum": 50.0 }
                },
                {
                    "id": "flow", "channel": "flow", "role": "state", "symbol": "P1.Flow",
                    "iecType": "LREAL", "direction": "read", "unit": "L/s",
                    "range": { "minimum": 0.0, "maximum": 20.0 }
                }
            ]
        },
        "limits": { "outputMinimum": 0.0, "outputMaximum": 100.0 },
        "channels": {
            "inputs": [
                {
                    "modelChannelId": "heater", "signalId": "manipulatedVariable",
                    "actuator": { "kind": "continuous", "minimum": 0.0, "maximum": 100.0 }
                },
                {
                    "modelChannelId": "load", "signalId": "load",
                    "actuator": { "kind": "continuous", "minimum": 0.0, "maximum": 50.0 }
                }
            ],
            "outputs": [
                { "modelChannelId": "temp", "signalId": "processVariable", "setpointSignalId": "setpoint" },
                { "modelChannelId": "flow", "signalId": "flow" }
            ]
        }
    }))
    .expect("the two-channel loop decodes")
}

/// A validated one-state model of the two-channel loop: the heater and the load drive the temperature and the flow. The load and the
/// flow carry the units given.
fn two_channel_model(load_unit: &str, flow_unit: &str) -> GeneralModel {
    decode_general_model(&json!({
        "kind": "discrete-state-space",
        "states": [{
            "id": "history-0", "unit": "degC", "operatingRange": { "minimum": -60.0, "maximum": 60.0 }
        }],
        "a": [[0.9]],
        "b": [[0.1, 0.05]],
        "c": [[1.0], [0.5]],
        "d": [[0.0, 0.0], [0.0, 0.0]],
        "delaySeconds": 0.0,
        "sampleTimeSeconds": 0.1,
        "revision": "model-3",
        "source": "fitted",
        "validation": "validated",
        "evidence": ["recording 9"],
        "inputs": [
            { "id": "heater", "unit": "%", "operatingRange": { "minimum": 0.0, "maximum": 100.0 } },
            { "id": "load", "unit": load_unit, "operatingRange": { "minimum": 0.0, "maximum": 50.0 } }
        ],
        "outputs": [
            { "id": "temp", "unit": "degC", "operatingRange": { "minimum": 20.0, "maximum": 80.0 } },
            { "id": "flow", "unit": flow_unit, "operatingRange": { "minimum": 0.0, "maximum": 20.0 } }
        ]
    }))
    .expect("the two-channel model decodes")
}

#[test]
fn a_two_channel_model_is_qualified_on_every_input_and_output_of_the_loop() {
    let loop_definition = two_channel_loop();
    assert_eq!(
        assert_qualified_model_result(&loop_definition, &two_channel_model("kW", "L/s")),
        Ok(())
    );
    assert_eq!(
        assert_qualified_model_result(&loop_definition, &two_channel_model("kW", "m3/s")),
        Err(CompatibilityRefusal::OutputUnit {
            id: "flow".to_string(),
            symbol: "P1.Flow".to_string(),
        }),
        "the second output's unit differs from the loop's"
    );
    assert_eq!(
        assert_qualified_model_result(&loop_definition, &two_channel_model("MW", "L/s")),
        Err(CompatibilityRefusal::InputUnit {
            id: "load".to_string(),
            symbol: "P1.LoadKw".to_string(),
        }),
        "the second input's unit differs from the loop's"
    );
}

#[test]
fn an_engineer_compatibility_check_binds_every_channel_of_a_loop_with_channels() {
    // Validate binds every model input and output through the loop's channels, so a bound two-input model may tune the loop.
    assert_eq!(
        assert_engineer_model_compatibility(&two_channel_loop(), &two_channel_model("kW", "L/s")),
        Ok(())
    );
    // Without channel bindings the loop has one plant input and one measured output, which the two-input model does not fit.
    let mut unbound = two_channel_loop();
    unbound.channels = None;
    assert_eq!(
        assert_engineer_model_compatibility(&unbound, &two_channel_model("kW", "L/s")),
        Err(CompatibilityRefusal::UnmappedChannels)
    );
}
