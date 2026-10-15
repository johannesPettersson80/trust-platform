//! The decoupled source of a group's state (C7, `decoupled-output`): its serialized formula, its fresh value at a transfer, and the
//! formula that a group's channels give it.

use serde_json::json;

use super::*;
use crate::commissioning::fixture_loops::{control_pid_loop, first_order_model};
use crate::commissioning::transfer::{TransferMember, TransferModeBinding, TransferValueType};
use crate::loop_definition::{InitialSource, InitialValue, PostScan, StateType};

/// The decoupler of two zones, D = [[1, −0.5], [−0.5, 1]], by the symbols that the group's parameter record gives its gains.
fn decoupler() -> Vec<Vec<String>> {
    vec![
        vec!["P1.D11".to_owned(), "P1.D12".to_owned()],
        vec!["P1.D21".to_owned(), "P1.D22".to_owned()],
    ]
}

/// The formula of row 1 on the plant inputs `P1.Heater1Pct` and `P1.Heater2Pct`.
fn row_one() -> InitializationFormula {
    InitializationFormula::DecoupledOutput {
        output_symbols: vec!["P1.Heater1Pct".to_owned(), "P1.Heater2Pct".to_owned()],
        matrix_symbols: decoupler(),
        row: 1,
    }
}

/// The declared state of row 1 of the decoupler, bounded 0 to 200 so that the share of the inputs fits it.
fn row_one_state() -> InitializedState {
    InitializedState {
        symbol: "Main.Row".to_owned(),
        iec_type: StateType::Lreal,
        unit: "%".to_owned(),
        minimum: 0.0,
        maximum: 200.0,
        initial: InitialValue::From(InitialSource::DecoupledOutput {
            index: 1,
            matrix_symbols: decoupler(),
        }),
        post_scan: PostScan::Bounded,
    }
}

/// A transfer binding of one output, with no tracking.
fn binding() -> TransferBinding {
    TransferBinding {
        loop_id: "loop:zones".to_owned(),
        task_name: "MainTask".to_owned(),
        mode: TransferModeBinding {
            symbol: "P1.ZonesAuto".to_owned(),
            manual_value: false,
        },
        additional_modes: Vec::new(),
        tracking_outputs: BTreeMap::new(),
        states: Vec::new(),
        guards: Vec::new(),
        output: TransferMember {
            symbol: "P1.Heater1Pct".to_owned(),
            iec_type: TransferValueType::Lreal,
            unit: "%".to_owned(),
            minimum: 0.0,
            maximum: 100.0,
        },
        additional_outputs: Vec::new(),
    }
}

#[test]
fn the_decoupled_formula_serializes_to_the_runtime_json() {
    assert_eq!(
        serde_json::to_value(row_one()).expect("the formula serializes"),
        json!({
            "kind": "decoupled-output",
            "output_symbols": ["P1.Heater1Pct", "P1.Heater2Pct"],
            "matrix_symbols": [["P1.D11", "P1.D12"], ["P1.D21", "P1.D22"]],
            "row": 1
        })
    );
}

#[test]
fn the_fresh_plant_inputs_give_the_row_of_the_inverse_applied_to_them() {
    // D⁻¹·[50, 50] = [100, 100]: the plant inputs and the gains are read fresh from the snapshot.
    let snapshot = BTreeMap::from([
        ("P1.Heater1Pct".to_owned(), TransferValue::Number(50.0)),
        ("P1.Heater2Pct".to_owned(), TransferValue::Number(50.0)),
        ("P1.D11".to_owned(), TransferValue::Number(1.0)),
        ("P1.D12".to_owned(), TransferValue::Number(-0.5)),
        ("P1.D21".to_owned(), TransferValue::Number(-0.5)),
        ("P1.D22".to_owned(), TransferValue::Number(1.0)),
    ]);
    let value =
        evaluate_initialization_formula(&binding(), &row_one(), &snapshot).expect("the share");
    assert!((value - 100.0).abs() <= 1e-9, "{value}");
}

#[test]
fn a_decoupler_gain_missing_from_the_snapshot_is_refused() {
    let snapshot = BTreeMap::from([
        ("P1.Heater1Pct".to_owned(), TransferValue::Number(50.0)),
        ("P1.Heater2Pct".to_owned(), TransferValue::Number(50.0)),
    ]);
    let refusal = evaluate_initialization_formula(&binding(), &row_one(), &snapshot)
        .expect_err("the gains are not in the snapshot");
    assert_eq!(refusal.cause, Cause::Runtime);
}

#[test]
fn a_decoupled_state_takes_its_formula_from_the_plant_input_of_each_channel() {
    // The first channel drives the manipulated variable; the second drives the additional signal `mv-2` of its own member.
    let mut loop_definition = control_pid_loop();
    loop_definition.channels = Some(
        crate::json::decode_value(json!({
            "inputs": [
                { "modelChannelId": "P1.Heater1Pct", "signalId": "manipulatedVariable",
                  "actuator": { "kind": "continuous", "minimum": 0, "maximum": 100 } },
                { "modelChannelId": "P1.Heater2Pct", "signalId": "mv-2", "manualOutputSignal": "P1.Zone2ManualPct",
                  "manualOutputType": "LREAL", "actuator": { "kind": "continuous", "minimum": 0, "maximum": 100 } }
            ],
            "outputs": [
                { "modelChannelId": "P1.Zone1C", "signalId": "processVariable", "setpointSignalId": "setpoint" },
                { "modelChannelId": "P1.Zone2C", "signalId": "pv-2", "setpointSignalId": "sp-2" }
            ]
        }))
        .expect("the channels decode"),
    );
    loop_definition.controller.group = Some(
        crate::json::decode_value(json!({
            "members": [
                { "instance": "P1.Zone1", "type": "Control.PID", "inputChannelId": "P1.Heater1Pct", "outputChannelId": "P1.Zone1C",
                  "modeSymbol": "P1.ZonesAuto", "parameterSymbols": ["P1.Zone1Parameters.ProportionalGain"] },
                { "instance": "P1.Zone2", "type": "Control.PID", "inputChannelId": "P1.Heater2Pct", "outputChannelId": "P1.Zone2C",
                  "modeSymbol": "P1.ZonesAuto", "parameterSymbols": ["P1.Zone2Parameters.ProportionalGain"] }
            ]
        }))
        .expect("the group decodes"),
    );
    loop_definition.signals.additional = Some(
        crate::json::decode_value(json!([{
            "id": "mv-2", "channel": "member-2", "role": "MV", "symbol": "P1.Heater2Pct", "iecType": "LREAL",
            "direction": "write", "unit": "%", "range": { "minimum": 0, "maximum": 100 }
        }]))
        .expect("the additional signal decodes"),
    );
    let definition = ControllerInitialization::new(vec![row_one_state()])
        .expect("a decoupled state is declared");
    let formulas = initialization_formulas(&loop_definition, &first_order_model(), &definition)
        .expect("the group's formula");
    assert_eq!(
        formulas["Main.Row"],
        InitializationFormula::DecoupledOutput {
            output_symbols: vec![
                loop_definition.signals.manipulated_variable.symbol.clone(),
                "P1.Heater2Pct".to_owned(),
            ],
            matrix_symbols: decoupler(),
            row: 1,
        }
    );
}

#[test]
fn a_decoupled_state_of_a_loop_without_channels_is_refused_by_name() {
    let definition = ControllerInitialization::new(vec![row_one_state()])
        .expect("a decoupled state is declared");
    let refusal = initialization_formulas(&control_pid_loop(), &first_order_model(), &definition)
        .expect_err("a loop without channels has no plant input for the decoupler");
    assert!(refusal.message.contains("Main.Row"), "{}", refusal.message);
}
