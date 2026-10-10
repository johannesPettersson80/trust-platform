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
