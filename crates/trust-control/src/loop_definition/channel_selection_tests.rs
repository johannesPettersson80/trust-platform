//! Tests of the channel selection (specification 35, A8.1 and the Tune page's **Channels…**): the rules a selection keeps, the
//! loop the stages read once it is applied, the rows the editor shows, and the stored section's own rules.

use serde_json::{json, Value};

use super::super::tests::{complete_loop, decode, with};
use super::*;

/// A loop with two inputs (the heater and a measured load, which is a disturbance) and two outputs (the water temperature, a process
/// variable, and a second state of the same unit), so that the outputs have a swap that keeps every role.
fn two_by_two_value() -> Value {
    let value = with(
        complete_loop(),
        &["signals", "additional"],
        json!([
            { "id": "load", "channel": "load", "role": "disturbance", "symbol": "Main.Load", "address": "%IL16",
              "iecType": "LREAL", "direction": "read", "unit": "%", "range": { "minimum": 0, "maximum": 100 } },
            { "id": "level", "channel": "level", "role": "state", "symbol": "Main.Level", "address": "%IL32",
              "iecType": "LREAL", "direction": "read", "unit": "%", "range": { "minimum": 0, "maximum": 100 } }
        ]),
    );
    with(
        value,
        &["channels"],
        json!({
            "inputs": [
                { "modelChannelId": "heater", "signalId": "manipulatedVariable",
                  "actuator": { "kind": "continuous", "minimum": 0, "maximum": 100 } },
                { "modelChannelId": "load", "signalId": "load", "manualOutputSignal": "Main.LoadCommand",
                  "manualOutputType": "LREAL", "actuator": { "kind": "continuous", "minimum": 0, "maximum": 50 },
                  "experimentStep": 10 }
            ],
            "outputs": [
                { "modelChannelId": "water", "signalId": "processVariable" },
                { "modelChannelId": "tank", "signalId": "level" }
            ]
        }),
    )
}

/// The two-by-two loop, decoded: its bindings hold.
fn two_by_two() -> LoopDefinition {
    decode(&two_by_two_value()).expect("the two-by-two channels decode")
}

fn pick(model: &str, signal: &str) -> ChannelPick {
    ChannelPick {
        model_channel_id: model.to_owned(),
        signal_id: signal.to_owned(),
    }
}

/// The selection that keeps every declared binding of the loop.
fn declared_selection(declared: &LoopDefinition) -> ChannelSelection {
    let channels = declared
        .channels
        .as_ref()
        .expect("the loop declares channels");
    ChannelSelection {
        inputs: channels
            .inputs
            .iter()
            .map(|input| pick(&input.model_channel_id, &input.signal_id))
            .collect(),
        outputs: channels
            .outputs
            .iter()
            .map(|output| pick(&output.model_channel_id, &output.signal_id))
            .collect(),
    }
}

/// The selection with the outputs swapped: the water temperature reads the level and the tank reads the process variable.
fn swapped_outputs() -> ChannelSelection {
    let mut selection = declared_selection(&two_by_two());
    selection.outputs = vec![pick("water", "level"), pick("tank", "processVariable")];
    selection
}

#[test]
fn the_declared_bindings_are_a_selection_that_changes_nothing() {
    let declared = two_by_two();
    let selection = declared_selection(&declared);
    assert_eq!(check_selection(&declared, &selection), Ok(()));
    assert_eq!(
        effective_loop(&declared, Some(&selection)).expect("the declared bindings hold"),
        declared
    );
    assert_eq!(
        effective_loop(&declared, None).expect("no selection"),
        declared
    );
}

#[test]
fn a_swap_of_two_outputs_of_one_unit_is_accepted_and_the_stages_read_it_while_the_declared_loop_keeps_its_own(
) {
    let declared = two_by_two();
    let selection = swapped_outputs();
    assert_eq!(check_selection(&declared, &selection), Ok(()));
    let effective = effective_loop(&declared, Some(&selection)).expect("the swapped bindings hold");
    let outputs = &effective.channels.as_ref().expect("channels").outputs;
    assert_eq!(outputs[0].signal_id, "level");
    assert_eq!(outputs[1].signal_id, "processVariable");
    assert_eq!(
        declared.channels.as_ref().expect("channels").outputs[0].signal_id,
        "processVariable"
    );
}

#[test]
fn a_selection_is_refused_for_a_foreign_signal_a_repeated_signal_or_a_missing_channel() {
    let declared = two_by_two();
    let mut foreign = declared_selection(&declared);
    foreign.outputs[0] = pick("water", "setpoint");
    let refusal = check_selection(&declared, &foreign).expect_err("the setpoint is not an output");
    assert_eq!(
        refusal.message,
        "Review the plant signal for water; keep its declared limits and channel order."
    );
    assert_eq!(refusal.next_action, NextAction::OpenChannels);

    let mut repeated = declared_selection(&declared);
    repeated.outputs[0] = pick("water", "level");
    repeated.outputs[1] = pick("tank", "level");
    let refusal = check_selection(&declared, &repeated).expect_err("two channels share a signal");
    assert_eq!(
        refusal.message,
        "Choose a different signal; this signal is already assigned to another channel."
    );

    let mut missing = declared_selection(&declared);
    missing.outputs.pop();
    let refusal = check_selection(&declared, &missing).expect_err("a channel is missing");
    assert_eq!(refusal.message, "Map every selected plant channel.");
}

#[test]
fn a_signal_of_another_unit_is_refused_as_a_review_of_its_channel() {
    let mut declared = two_by_two();
    if let Some(signals) = declared.signals.additional.as_mut() {
        signals[1].unit = "K".to_owned();
    }
    let refusal =
        check_selection(&declared, &swapped_outputs()).expect_err("the level has another unit");
    assert_eq!(
        refusal.message,
        "Review the plant signal for water; keep its declared limits and channel order."
    );
}

#[test]
fn the_choices_list_each_side_with_its_owner_and_a_single_channel_loop_has_none() {
    let declared = two_by_two();
    let choices = channel_choices(&declared).expect("a two-channel loop has rows");
    assert_eq!(choices.outputs.len(), 2);
    assert_eq!(choices.outputs[0].model_channel_id, "water");
    assert_eq!(choices.outputs[0].signal_id, "processVariable");
    assert_eq!(
        choices.outputs[0]
            .choices
            .iter()
            .map(|choice| choice.signal_id.as_str())
            .collect::<Vec<_>>(),
        ["processVariable", "level"]
    );
    assert_eq!(choices.outputs[0].choices[1].symbol, "Main.Level");
    assert_eq!(choices.outputs[0].choices[1].unit, "%");
    assert!(choices.outputs[0].owner.is_none());
    assert_eq!(choices.inputs[1].signal_id, "load");

    let single = decode(&complete_loop()).expect("the single-channel loop decodes");
    assert!(channel_choices(&single).is_none());
}

#[test]
fn a_stored_selection_with_an_empty_or_repeated_side_is_not_read() {
    let valid = declared_selection(&two_by_two());
    assert!(valid.check().is_ok());

    let mut repeated = valid.clone();
    repeated.outputs[1] = pick("tank", "processVariable");
    repeated.outputs[0] = pick("water", "processVariable");
    assert!(
        repeated.check().is_err(),
        "a signal is kept twice on one side"
    );

    let mut empty = valid;
    empty.inputs.clear();
    assert!(empty.check().is_err(), "a side with no channel is not kept");
}

#[test]
fn a_pid_group_keeps_its_declared_wiring_and_each_row_names_the_member_that_owns_it() {
    let mut declared = two_by_two();
    declared.controller.group = Some(
        serde_json::from_value(json!({ "members": [
            { "instance": "P1.Zone1", "type": "Control.PID", "inputChannelId": "heater", "outputChannelId": "water",
              "modeSymbol": "P1.Auto", "parameterSymbols": ["P1.Zone1Parameters.ProportionalGain"] },
            { "instance": "P1.Zone2", "type": "Control.PID", "inputChannelId": "load", "outputChannelId": "tank",
              "modeSymbol": "P1.Auto", "parameterSymbols": ["P1.Zone2Parameters.ProportionalGain"] }
        ] }))
        .expect("a typed group"),
    );
    assert_eq!(
        check_selection(&declared, &declared_selection(&declared)),
        Ok(())
    );
    let refusal =
        check_selection(&declared, &swapped_outputs()).expect_err("a PID group keeps its signals");
    assert_eq!(
        refusal.message,
        "Keep the PID group's declared signal LevelPV; select a compatible controller group for different wiring."
    );
    let choices = channel_choices(&declared).expect("a two-channel group has rows");
    assert_eq!(choices.inputs[0].owner.as_deref(), Some("P1.Zone1"));
    assert_eq!(choices.outputs[1].owner.as_deref(), Some("P1.Zone2"));
}
