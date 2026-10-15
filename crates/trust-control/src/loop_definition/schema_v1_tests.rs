//! Tests of the version 1 loop file (specification 35, B1): what a version 1 file declares, and the typed loop it gives.

use serde_json::{json, Value};

use crate::loop_definition::{
    LoopDefinition, LoopRejection, LoopScope, LOOP_SCHEMA_VERSION, NO_CONTROLLER_LAW,
};

/// The loop of a file, decoded as the engine decodes any loop file.
fn decode(value: &Value) -> Result<LoopDefinition, LoopRejection> {
    LoopDefinition::decode(&value.to_string())
}

fn scope(value: &Value) -> LoopScope {
    LoopScope {
        project_id: value["projectId"].as_str().unwrap_or_default().into(),
        runtime_id: value["runtimeId"].as_str().unwrap_or_default().into(),
    }
}

/// `document` with the key at `path` set to `replacement`; `path` names the keys from the top of the document down.
fn with(mut document: Value, path: &[&str], replacement: Value) -> Value {
    let (last, parents) = path.split_last().expect("a key");
    let mut target = &mut document;
    for key in parents {
        target = &mut target[*key];
    }
    target[*last] = replacement;
    document
}

/// A version 1 observation loop of the shape of the course's cold-storage lessons: a project-scripted observation with no law and no
/// key that only version 2 declares.
fn observation_loop() -> Value {
    json!({
        "schemaVersion": 1,
        "id": "loop:cold-room",
        "name": "Cold room",
        "revision": "sha256:cold-room-1",
        "projectId": "project:cold-room",
        "runtimeId": "runtime:local",
        "controller": {
            "instance": "P1", "type": "ColdStorageOnOff", "design": "project",
            "revision": "sha256:controller-1", "action": "reverse", "taskPeriodMs": 100,
            "modeSignal": "%MX16.0"
        },
        "signals": {
            "setpoint": { "symbol": "P1.Setpoint", "address": "%ML0", "iecType": "LREAL", "direction": "read", "unit": "degC", "range": { "minimum": -10, "maximum": 10 } },
            "processVariable": { "symbol": "P1.ProcessValue", "address": "%IL0", "iecType": "LREAL", "direction": "read", "unit": "degC", "range": { "minimum": -30, "maximum": 20 } },
            "manipulatedVariable": { "symbol": "P1.ManipulatedValue", "address": "%QL0", "iecType": "LREAL", "direction": "read", "unit": "%", "range": { "minimum": 0, "maximum": 100 } },
            "additional": [{
                "id": "door-open-indication", "channel": "door-open-indication", "role": "state",
                "symbol": "P1.DoorOpenIndication", "address": "%IL8", "iecType": "LREAL", "direction": "read",
                "unit": "1", "range": { "minimum": 0, "maximum": 1 }
            }]
        },
        "limits": { "outputMinimum": 0, "outputMaximum": 100 },
        "observedSampleTiming": { "nominalPeriodMs": 100 },
        "plantId": "cold-storage-room",
        "experiment": {
            "execution": "project-scripted", "kind": "observation", "amplitude": 0,
            "durationSeconds": 120.1, "seed": 20_260_828,
            "activation": { "symbol": "P1.ExperimentEnabled", "address": "%MX16.2", "iecType": "BOOL", "direction": "write" }
        }
    })
}

#[test]
fn a_version_one_file_reads_as_the_version_two_observation_loop() {
    let document = observation_loop();
    let loop_definition = decode(&document).expect("the version 1 loop decodes");
    assert_eq!(loop_definition.schema_version, LOOP_SCHEMA_VERSION);
    assert_eq!(
        loop_definition.controller.mode_signal.as_deref(),
        Some("%MX16.0")
    );
    loop_definition
        .validate(&scope(&document))
        .expect("it validates");
    loop_definition
        .rust_engine_support()
        .expect("the engine runs it as an observation loop (C12)");
    let refusal = loop_definition
        .require_controller_law()
        .expect_err("no law to tune");
    assert_eq!(refusal.message, NO_CONTROLLER_LAW);
}

#[test]
fn reading_a_version_one_document_is_reading_the_same_document_as_version_two() {
    let version_two = with(observation_loop(), &["schemaVersion"], json!(2));
    assert_eq!(
        decode(&observation_loop()),
        decode(&version_two),
        "the same loop, whichever version the file is written in"
    );
}

#[test]
fn a_loop_read_from_version_one_is_written_as_version_two() {
    let loop_definition = decode(&observation_loop()).expect("decodes");
    let written = serde_json::to_string(&loop_definition).expect("json");
    assert!(
        written.contains("\"schemaVersion\":2"),
        "it is written as version 2"
    );
    assert_eq!(
        LoopDefinition::decode(&written).expect("read back"),
        loop_definition
    );
}

#[test]
fn a_version_one_file_refuses_each_key_that_only_version_two_declares() {
    let declared = [
        ("parameterBinding", json!({})),
        ("parameterPurpose", json!("experiment")),
        ("verificationScenarios", json!([])),
        ("channels", json!({})),
        ("analysis", json!({})),
        ("analysisParameterSymbols", json!({})),
        (
            "controller.law",
            json!({ "kind": "proportional", "gainSymbol": "P1.Gain" }),
        ),
        ("controller.stateFeedback", json!({})),
        ("controller.onOff", json!({})),
        ("controller.project", json!({})),
        ("controller.group", json!({})),
        ("experiment.abort", json!({})),
        ("experiment.baselineSeconds", json!(10.0)),
        ("experiment.holdParameter", json!("P1.Hold")),
    ];
    for (dotted, replacement) in declared {
        let path: Vec<&str> = dotted.split('.').collect();
        let refusal = decode(&with(observation_loop(), &path, replacement));
        assert!(
            matches!(&refusal, Err(LoopRejection::Malformed(text)) if text.contains(dotted)),
            "{dotted}: {refusal:?}"
        );
    }
}

#[test]
fn a_key_declared_as_null_is_still_declared_in_a_version_one_file() {
    let refusal = decode(&with(
        observation_loop(),
        &["controller", "law"],
        Value::Null,
    ));
    assert!(
        matches!(&refusal, Err(LoopRejection::Malformed(_))),
        "{refusal:?}"
    );
}

#[test]
fn a_version_one_file_keeps_the_field_rules_of_version_two() {
    // A closed loop needs a setpoint and an open loop has none, as in version 2.
    let mut document = observation_loop();
    document["signals"]
        .as_object_mut()
        .expect("signals")
        .remove("setpoint");
    assert!(
        matches!(decode(&document), Err(LoopRejection::Malformed(_))),
        "a closed loop needs a setpoint"
    );
    document["topology"] = json!("open-loop");
    assert!(decode(&document).is_ok(), "an open loop has no setpoint");
}
