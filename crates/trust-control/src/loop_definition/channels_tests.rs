//! Tests of the loop's channels (`loopChannels.ts`; specification 35, C7 and C8): the shape rules refuse a malformed declaration at
//! load, and the binding rules check the roles, claims, applied feedback and ranges of its signals.

use serde_json::{json, Value};

use super::super::tests::{complete_loop, decode, with};
use super::*;

/// The two-input loop of lesson 84: the heater is the manipulated variable, and a measured load is an additional disturbance that
/// the program writes through its own Manual command.
fn two_input_loop(channels: Value) -> Value {
    let loop_value = with(
        complete_loop(),
        &["signals", "additional"],
        json!([
            { "id": "load", "channel": "load", "role": "disturbance", "symbol": "Main.Load", "address": "%IL16",
              "iecType": "LREAL", "direction": "read", "unit": "%", "range": { "minimum": 0, "maximum": 100 } },
            { "id": "loadAlias", "channel": "load", "role": "disturbance", "symbol": "Main.Load", "address": "%IL24",
              "iecType": "LREAL", "direction": "read", "unit": "%", "range": { "minimum": 0, "maximum": 100 } }
        ]),
    );
    with(loop_value, &["channels"], channels)
}

/// Two inputs (the heater and the load) and one output (the water temperature) with its setpoint.
fn valid_channels() -> Value {
    json!({
        "inputs": [
            { "modelChannelId": "heater", "signalId": "manipulatedVariable",
              "actuator": { "kind": "continuous", "minimum": 0, "maximum": 100 } },
            { "modelChannelId": "load", "signalId": "load", "manualOutputSignal": "Main.LoadCommand", "manualOutputType": "LREAL",
              "actuator": { "kind": "continuous", "minimum": 0, "maximum": 50 }, "experimentStep": 10 }
        ],
        "outputs": [
            { "modelChannelId": "water", "signalId": "processVariable", "setpointSignalId": "setpoint",
              "requirements": { "referenceChange": 5, "maximumError": 1, "settlingBand": 0.5, "settlingTimeSeconds": 120 } }
        ]
    })
}

/// The loop with the given channels, decoded: the load refusal of the decoder, or the typed loop.
fn load(channels: Value) -> Result<LoopDefinition, crate::loop_definition::LoopRejection> {
    decode(&two_input_loop(channels))
}

/// The refusal of a loop's bindings at load, when the loop breaks one of their rules (the decoder runs them, as the TypeScript
/// loader does).
fn binding_refusal(channels: Value) -> Option<String> {
    match load(channels) {
        Ok(_) => None,
        Err(crate::loop_definition::LoopRejection::Malformed(message)) => Some(message),
        Err(other) => panic!("expected a refusal of the bindings, got {other:?}"),
    }
}

#[test]
fn a_two_input_one_output_binding_decodes_and_passes_both_rules() {
    let loop_definition = load(valid_channels()).expect("the channels decode");
    let channels = loop_definition
        .channels
        .as_ref()
        .expect("the loop declares channels");
    assert_eq!(channels.check(), Ok(()));
    assert_eq!(channels.check_bindings(&loop_definition), Ok(()));
    assert_eq!(channels.inputs.len(), 2);
    assert_eq!(
        channels.inputs[1].manual_output_type,
        Some(ManualOutputType::Lreal)
    );
    assert_eq!(channels.inputs[1].experiment_step, Some(10.0));
    assert_eq!(
        channels.outputs[0].setpoint_signal_id.as_deref(),
        Some("setpoint")
    );
    assert!(measured_disturbance(&loop_definition, &channels.inputs[1]));
    assert!(!measured_disturbance(&loop_definition, &channels.inputs[0]));
}

#[test]
fn the_shape_rules_refuse_a_malformed_declaration_when_the_loop_loads() {
    let mut without_inputs = valid_channels();
    without_inputs["inputs"] = json!([]);
    let mut duplicate_model = valid_channels();
    duplicate_model["inputs"][1]["modelChannelId"] = json!("heater");
    let mut blank_identifier = valid_channels();
    blank_identifier["outputs"][0]["modelChannelId"] = json!("  ");
    let mut step_beyond_span = valid_channels();
    step_beyond_span["inputs"][1]["experimentStep"] = json!(60);
    let mut band_not_below_error = valid_channels();
    band_not_below_error["outputs"][0]["requirements"]["settlingBand"] = json!(1);
    let mut on_off_with_rate = valid_channels();
    on_off_with_rate["inputs"][1]["actuator"] =
        json!({ "kind": "on-off", "minimum": 0, "maximum": 50, "rateUpPerS": 1 });
    let mut too_many_states = valid_channels();
    too_many_states["initialState"] = json!(vec![0.0; 33]);
    for channels in [
        without_inputs,
        duplicate_model,
        blank_identifier,
        step_beyond_span,
        band_not_below_error,
        on_off_with_rate,
        too_many_states,
    ] {
        match load(channels) {
            Err(crate::loop_definition::LoopRejection::Malformed(message)) => {
                assert!(message.contains(INVALID_CHANNELS), "{message}");
            }
            other => panic!("expected a malformed refusal, got {other:?}"),
        }
    }
}

#[test]
fn the_binding_rules_check_the_roles_of_the_inputs_and_the_setpoint() {
    // An input that is neither the manipulated variable nor, after the first input, a measured disturbance.
    let mut input_role = valid_channels();
    input_role["inputs"][1]["signalId"] = json!("processVariable");
    assert_eq!(binding_refusal(input_role), Some(ROLE_MISMATCH.to_owned()));
    // A setpoint that is not the setpoint role.
    let mut setpoint_role = valid_channels();
    setpoint_role["outputs"][0]["setpointSignalId"] = json!("processVariable");
    assert_eq!(
        binding_refusal(setpoint_role),
        Some(ROLE_MISMATCH.to_owned())
    );
}

#[test]
fn the_binding_rules_check_the_claims_and_the_actuator_range_of_each_input() {
    // The disturbance and its alias are two loop signals with one symbol, so one variable is claimed twice.
    let mut claimed_twice = valid_channels();
    claimed_twice["inputs"]
        .as_array_mut()
        .expect("the inputs are a list")
        .push(json!({ "modelChannelId": "alias", "signalId": "loadAlias",
            "actuator": { "kind": "continuous", "minimum": 0, "maximum": 50 } }));
    assert_eq!(
        binding_refusal(claimed_twice),
        Some("Signal Main.Load is assigned to multiple model channels.".to_owned())
    );
    // A heater whose actuator reaches past the manipulated variable's declared range of 0 to 100.
    let mut beyond_range = valid_channels();
    beyond_range["inputs"][0]["actuator"]["maximum"] = json!(150);
    assert_eq!(
        binding_refusal(beyond_range),
        Some("Actuator limits exceed ValveMV's declared range.".to_owned())
    );
}

#[test]
fn a_measured_disturbance_is_an_additional_signal_with_the_disturbance_role() {
    let loop_definition = load(valid_channels()).expect("the channels decode");
    let manipulated = channel_signal(&loop_definition, "manipulatedVariable")
        .expect("the manipulated variable is declared");
    assert_eq!(manipulated.symbol, "ValveMV");
    assert_eq!(manipulated.unit, "%");
    assert_eq!(manipulated.address, Some("%QL0"));
    let load_signal = channel_signal(&loop_definition, "load").expect("the load is declared");
    assert_eq!(load_signal.symbol, "Main.Load");
    assert!(channel_signal(&loop_definition, "undeclared").is_none());
}
