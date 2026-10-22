//! The sources of a group's states (C7): the decoupled source (`decoupled-output`), its serialized formula, its fresh value at a
//! transfer, and the formula that a group's channels give it; and the plain model sources of each member's states, which read the
//! member's own declared channels.

use serde_json::json;

use super::*;
use crate::commissioning::fixture_loops::{control_pid_loop, first_order_model};
use crate::commissioning::initial_state::initialization_values;
use crate::commissioning::transfer::{TransferMember, TransferModeBinding, TransferValueType};
use crate::loop_definition::{InitialSource, InitialValue, PostScan, StateType};
use crate::model::decode_general_model;
use crate::tuning::structures::fixtures::decoupler_zones_model;

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

/// The loop of a group of two members on two plant inputs and two measured outputs: the first channel drives the manipulated variable and
/// measures the process variable; the second drives the additional signal `mv-2` of its own member and measures `pv-2`.
fn zones_loop() -> LoopDefinition {
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
        crate::json::decode_value(json!([
            { "id": "mv-2", "channel": "member-2", "role": "MV", "symbol": "P1.Heater2Pct", "iecType": "LREAL",
              "direction": "write", "unit": "%", "range": { "minimum": 0, "maximum": 100 } },
            { "id": "pv-2", "channel": "member-2", "role": "PV", "symbol": "P1.Zone2C", "iecType": "LREAL",
              "direction": "read", "unit": "degC", "range": { "minimum": 0, "maximum": 100 } },
            { "id": "sp-2", "channel": "member-2", "role": "SP", "symbol": "P1.Zone2SetpointC", "iecType": "LREAL",
              "direction": "read", "unit": "degC", "range": { "minimum": 0, "maximum": 100 } }
        ]))
        .expect("the additional signals decode"),
    );
    loop_definition
}

#[test]
fn a_decoupled_state_takes_its_formula_from_the_plant_input_of_each_channel() {
    let loop_definition = zones_loop();
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

/// A declared state of a member of the zones group, bounded 0 to 200 like the decoupled row, with the model source given.
fn member_state(symbol: &str, source: InitialSource) -> InitializedState {
    InitializedState {
        symbol: symbol.to_owned(),
        iec_type: StateType::Lreal,
        unit: "%".to_owned(),
        minimum: 0.0,
        maximum: 200.0,
        initial: InitialValue::From(source),
        post_scan: PostScan::Bounded,
    }
}

#[test]
fn each_members_states_read_the_members_own_declared_channels() {
    // The second member's setpoint and nominal output start from its own measurement and its own plant input, not from the scalar
    // connection of the first member (C7).
    let loop_definition = zones_loop();
    let definition = ControllerInitialization::new(vec![
        member_state("P1.Zone1.Setpoint", InitialSource::ModelOutput { index: 0 }),
        member_state("P1.Zone2.Setpoint", InitialSource::ModelOutput { index: 1 }),
        member_state(
            "P1.Zone1.NominalOutput",
            InitialSource::ModelInput { index: 0 },
        ),
        member_state(
            "P1.Zone2.NominalOutput",
            InitialSource::ModelInput { index: 1 },
        ),
    ])
    .expect("the members' states are declared");
    let formulas = initialization_formulas(&loop_definition, &decoupler_zones_model(), &definition)
        .expect("the members' formulas");
    assert_eq!(
        formulas["P1.Zone1.Setpoint"],
        InitializationFormula::ModelOutput {
            process_variable: loop_definition.signals.process_variable.symbol.clone(),
        }
    );
    assert_eq!(
        formulas["P1.Zone2.Setpoint"],
        InitializationFormula::ModelOutput {
            process_variable: "P1.Zone2C".to_owned(),
        }
    );
    assert_eq!(
        formulas["P1.Zone1.NominalOutput"],
        InitializationFormula::ModelInput {
            output_symbol: loop_definition.signals.manipulated_variable.symbol.clone(),
        }
    );
    assert_eq!(
        formulas["P1.Zone2.NominalOutput"],
        InitializationFormula::ModelInput {
            output_symbol: "P1.Heater2Pct".to_owned(),
        }
    );
}

#[test]
fn a_state_whose_index_names_no_declared_channel_of_the_group_is_refused_by_name() {
    // A group of two members declares two channels on each side, so a third has no signal to read.
    let definition = ControllerInitialization::new(vec![member_state(
        "P1.Zone3.Setpoint",
        InitialSource::ModelOutput { index: 2 },
    )])
    .expect("the state is declared");
    let refusal = initialization_formulas(&zones_loop(), &decoupler_zones_model(), &definition)
        .expect_err("the group declares no third output channel");
    assert!(
        refusal.message.contains("P1.Zone3.Setpoint"),
        "{}",
        refusal.message
    );
}

#[test]
fn each_members_states_take_the_operating_point_of_their_own_channels_whatever_the_models_order() {
    // The model lists the second zone's channels first, so the declared channel 0 (the first member's) is the model's channel 1 (C7,
    // `controllerModelIndex`): each state's index names the declared channel, and the model gives that channel's coordinate.
    let channel = |id: &str, unit: &str| json!({ "id": id, "unit": unit, "operatingRange": { "minimum": 0, "maximum": 100 } });
    let model = decode_general_model(&json!({
        "kind": "continuous-state-space", "revision": "model:zones-listed-second-first", "source": "provided",
        "validation": "validated", "evidence": ["the zones of the group, listed in the other order"], "delaySeconds": 0.0,
        "inputs": [channel("P1.Heater2Pct", "%"), channel("P1.Heater1Pct", "%")],
        "outputs": [channel("P1.Zone2C", "degC"), channel("P1.Zone1C", "degC")],
        "states": [channel("x1", "degC"), channel("x2", "degC")],
        "a": [[-0.25, 0.0], [0.0, -0.25]], "b": [[0.375, 0.0], [0.0, 0.375]],
        "c": [[1.0, 0.0], [0.0, 1.0]], "d": [[0.0, 0.0], [0.0, 0.0]],
        "operatingPoint": { "inputs": [20.0, 10.0], "outputs": [40.0, 30.0] }
    }))
    .expect("the model decodes");
    let measurement = |symbol: &str, index: usize| InitializedState {
        unit: "degC".to_owned(),
        ..member_state(symbol, InitialSource::ModelOutput { index })
    };
    let definition = ControllerInitialization::new(vec![
        measurement("P1.Zone1.Setpoint", 0),
        measurement("P1.Zone2.Setpoint", 1),
        member_state(
            "P1.Zone1.NominalOutput",
            InitialSource::ModelInput { index: 0 },
        ),
        member_state(
            "P1.Zone2.NominalOutput",
            InitialSource::ModelInput { index: 1 },
        ),
    ])
    .expect("the members' states are declared");
    let values = initialization_values(&zones_loop(), &model, &definition, &BTreeMap::new())
        .expect("the members' values at the operating point");
    let expected = [
        ("P1.Zone1.Setpoint", 30.0),
        ("P1.Zone2.Setpoint", 40.0),
        ("P1.Zone1.NominalOutput", 10.0),
        ("P1.Zone2.NominalOutput", 20.0),
    ];
    for (symbol, value) in expected {
        assert!(
            (values[symbol] - value).abs() <= 1e-12,
            "{symbol} is {} where its own channel holds {value}",
            values[symbol]
        );
    }
}
