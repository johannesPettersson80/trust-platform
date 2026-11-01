#![allow(clippy::float_cmp)] // The lesson's feedforward values are exact products of its gain and its operating input.

use super::*;
use crate::commissioning::fixture_loops::{control_pid_loop, first_order_model};
use crate::commissioning::transfer::TransferValueType;
use crate::loop_definition::{InitialSource, InitialValue, PostScan};
use crate::model::GeneralModel;
use serde_json::json;

/// A state of the given type and unit, bounded by `minimum` and `maximum`, with the given initial value.
fn state(
    symbol: &str,
    unit: &str,
    iec_type: StateType,
    bounds: (f64, f64),
    initial: InitialValue,
) -> InitializedState {
    InitializedState {
        symbol: symbol.to_owned(),
        iec_type,
        unit: unit.to_owned(),
        minimum: bounds.0,
        maximum: bounds.1,
        initial,
        post_scan: PostScan::Bounded,
    }
}

fn model_output(index: usize) -> InitialValue {
    InitialValue::From(InitialSource::ModelOutput { index })
}

fn model_input(index: usize) -> InitialValue {
    InitialValue::From(InitialSource::ModelInput { index })
}

fn close(actual: f64, expected: f64) -> bool {
    (actual - expected).abs() <= 1e-12
}

#[test]
fn a_list_of_valid_states_is_accepted_and_each_broken_declaration_is_refused() {
    let good = state(
        "Main.A",
        "%",
        StateType::Lreal,
        (-10.0, 10.0),
        InitialValue::Constant(0.0),
    );
    assert!(ControllerInitialization::new(vec![good.clone()]).is_ok());
    assert!(
        ControllerInitialization::new(Vec::new()).is_ok(),
        "an empty list is a valid declaration"
    );
    assert!(
        ControllerInitialization::new(vec![good.clone(), good.clone()]).is_err(),
        "a symbol is declared once"
    );
    let mut unitless = good.clone();
    unitless.unit = String::new();
    assert!(
        ControllerInitialization::new(vec![unitless]).is_err(),
        "a state has a unit"
    );
    let reversed = state(
        "Main.B",
        "%",
        StateType::Lreal,
        (10.0, -10.0),
        InitialValue::Constant(0.0),
    );
    assert!(
        ControllerInitialization::new(vec![reversed]).is_err(),
        "the bounds are ordered"
    );
    let infinite = state(
        "Main.C",
        "%",
        StateType::Lreal,
        (-10.0, f64::INFINITY),
        InitialValue::Constant(0.0),
    );
    assert!(
        ControllerInitialization::new(vec![infinite]).is_err(),
        "the bounds are finite"
    );
    let boolean = state(
        "Main.Flag",
        "%",
        StateType::Bool,
        (0.0, 1.0),
        InitialValue::Constant(0.0),
    );
    assert!(
        ControllerInitialization::new(vec![boolean]).is_err(),
        "a BOOL state is dimensionless"
    );
}

#[test]
fn a_weight_is_a_symbol_or_a_number_never_both_and_a_parked_formula_is_refused() {
    let both = InitialValue::From(InitialSource::PidIntegral {
        index: 0,
        input_index: 0,
        gain_symbol: "Main.Parameters.Ki".to_owned(),
        weight_symbol: Some("Main.Parameters.W".to_owned()),
        weight: Some(0.5),
    });
    assert!(ControllerInitialization::new(vec![state(
        "Main.I",
        "%",
        StateType::Lreal,
        (-1.0, 1.0),
        both
    )])
    .is_err());
    let by_symbol = InitialValue::From(InitialSource::WeightedError {
        index: 0,
        weight_symbol: Some("Main.Parameters.W".to_owned()),
        weight: None,
    });
    assert!(ControllerInitialization::new(vec![state(
        "Main.E",
        "%",
        StateType::Lreal,
        (-1.0, 1.0),
        by_symbol
    )])
    .is_ok());
    // The measured-disturbance feedforward's `scaled-model-input` (C8) and the decoupler's `decoupled-output` (C7) are served, so only
    // the other kinds are parked.
    let parked = [
        InitialValue::From(InitialSource::ModelState { index: 0 }),
        InitialValue::From(InitialSource::NotchHistory {
            index: 0,
            denominator_symbols: ["Main.A1".to_owned(), "Main.A2".to_owned()],
        }),
    ];
    for initial in parked {
        let declared = state("Main.P", "%", StateType::Lreal, (-1.0, 1.0), initial);
        let refusal =
            ControllerInitialization::new(vec![declared]).expect_err("a parked formula is not run");
        assert!(
            matches!(refusal.cause, Cause::ControllerCapability),
            "a parked formula is a controller capability"
        );
    }
}

/// The decoupler of two zones, D = [[1, −0.5], [−0.5, 1]], by the symbols that the group's parameter record gives its gains.
fn decoupler_symbols() -> Vec<Vec<String>> {
    vec![
        vec!["P1.D11".to_owned(), "P1.D12".to_owned()],
        vec!["P1.D21".to_owned(), "P1.D22".to_owned()],
    ]
}

#[test]
fn a_decoupled_share_is_the_row_of_the_inverse_applied_to_the_operating_inputs() {
    // D⁻¹·[50, 50] = [100, 100], as control-studio-decoupler.test.ts checks it.
    let matrix = vec![vec![1.0, -0.5], vec![-0.5, 1.0]];
    for row in 0..2 {
        let share = decoupled_share(&matrix, &[50.0, 50.0], row).expect("D is invertible");
        assert!((share - 100.0).abs() <= 1e-9, "row {row} is {share}");
    }
}

#[test]
fn a_singular_or_unsquare_decoupler_is_refused() {
    let singular = vec![vec![1.0, 1.0], vec![1.0, 1.0]];
    let refusal = decoupled_share(&singular, &[50.0, 50.0], 0).expect_err("D is singular");
    assert_eq!(refusal.cause, Cause::Settings);
    assert!(refusal.message.contains("singular"), "{}", refusal.message);
    let ragged = vec![vec![1.0, 0.0], vec![1.0]];
    assert!(
        decoupled_share(&ragged, &[50.0, 50.0], 0).is_err(),
        "a ragged matrix is not square"
    );
    assert!(
        decoupled_share(&[vec![1.0]], &[1.0], 1).is_err(),
        "the row lies inside the matrix"
    );
}

#[test]
fn a_decoupled_state_starts_at_its_row_of_the_inverse_applied_to_the_plant_inputs_of_its_channels()
{
    // The two channels drive the heaters of `decoupler_zones_model` at its operating inputs, 50 % each, and D is bound as the group's gains.
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
    loop_definition.parameter_binding = Some(
        crate::json::decode_value(json!({
            "loop_id": "loop:zones", "task_name": "MainTask",
            "parameters": [
                { "symbol": "P1.D11", "iec_type": "LREAL", "unit": "1", "convention": "decoupler gain", "minimum": -10, "maximum": 10 },
                { "symbol": "P1.D12", "iec_type": "LREAL", "unit": "1", "convention": "decoupler gain", "minimum": -10, "maximum": 10 },
                { "symbol": "P1.D21", "iec_type": "LREAL", "unit": "1", "convention": "decoupler gain", "minimum": -10, "maximum": 10 },
                { "symbol": "P1.D22", "iec_type": "LREAL", "unit": "1", "convention": "decoupler gain", "minimum": -10, "maximum": 10 }
            ],
            "mode": { "symbol": "P1.ZonesAuto", "manual_value": false }
        }))
        .expect("the binding decodes"),
    );
    let declared = state(
        "Main.Row",
        "%",
        StateType::Lreal,
        (0.0, 200.0),
        InitialValue::From(InitialSource::DecoupledOutput {
            index: 1,
            matrix_symbols: decoupler_symbols(),
        }),
    );
    let definition = ControllerInitialization::new(vec![declared])
        .expect("a decoupled state is served, so it is declared");
    let parameters = BTreeMap::from([
        ("P1.D11".to_owned(), 1.0),
        ("P1.D12".to_owned(), -0.5),
        ("P1.D21".to_owned(), -0.5),
        ("P1.D22".to_owned(), 1.0),
    ]);
    let model = crate::tuning::structures::fixtures::decoupler_zones_model();
    let values = initialization_values(&loop_definition, &model, &definition, &parameters)
        .expect("the share at the operating point");
    assert!(
        (values["Main.Row"] - 100.0).abs() <= 1e-9,
        "D⁻¹·[50, 50] gives 100 in row 1: {}",
        values["Main.Row"]
    );
}

#[test]
fn the_operating_point_gives_each_state_its_coordinate_and_a_constant_its_value() {
    let loop_definition = control_pid_loop();
    let model = first_order_model();
    let definition = ControllerInitialization::new(vec![
        state(
            "Main.Setpoint",
            "%",
            StateType::Lreal,
            (0.0, 100.0),
            model_output(0),
        ),
        state(
            "Main.Command",
            "%",
            StateType::Lreal,
            (0.0, 100.0),
            model_input(0),
        ),
        state(
            "Main.FeedForward",
            "%",
            StateType::Lreal,
            (-10.0, 10.0),
            InitialValue::Constant(0.25),
        ),
    ])
    .expect("the states are valid");
    let values = initialization_values(&loop_definition, &model, &definition, &BTreeMap::new())
        .expect("the values");
    assert!(
        close(values["Main.Setpoint"], 2.0),
        "the output at the operating point is 2"
    );
    assert!(
        close(values["Main.Command"], 0.0),
        "the input at the operating point is 0"
    );
    assert!(
        close(values["Main.FeedForward"], 0.25),
        "a constant is its own value"
    );
}

#[test]
fn a_real_state_is_rounded_to_single_precision_as_it_is_written() {
    let loop_definition = control_pid_loop();
    let definition = ControllerInitialization::new(vec![state(
        "Main.Real",
        "%",
        StateType::Real,
        (-10.0, 10.0),
        InitialValue::Constant(0.1),
    )])
    .expect("the state is valid");
    let values = initialization_values(
        &loop_definition,
        &first_order_model(),
        &definition,
        &BTreeMap::new(),
    )
    .expect("the value");
    assert!(
        close(values["Main.Real"], f64::from(0.1_f32)),
        "a REAL is written as its single-precision value"
    );
}

#[test]
fn a_state_whose_unit_does_not_match_its_coordinate_is_refused() {
    let definition = ControllerInitialization::new(vec![state(
        "Main.Bad",
        "degC",
        StateType::Lreal,
        (0.0, 100.0),
        model_output(0),
    )])
    .expect("the declaration is valid");
    let refusal = initialization_values(
        &control_pid_loop(),
        &first_order_model(),
        &definition,
        &BTreeMap::new(),
    )
    .expect_err("the unit differs from the model output's unit");
    assert!(refusal.message.contains("do not match Main.Bad"));
}

#[test]
fn a_boolean_state_follows_whether_the_operating_command_is_its_predicate() {
    let loop_definition = control_pid_loop();
    let model = first_order_model();
    let at_zero = ControllerInitialization::new(vec![state(
        "Main.Flag",
        "1",
        StateType::Bool,
        (0.0, 1.0),
        InitialValue::From(InitialSource::ModelInputIs {
            index: 0,
            value: 0.0,
        }),
    )])
    .expect("the state is valid");
    let values = initialization_values(&loop_definition, &model, &at_zero, &BTreeMap::new())
        .expect("the value");
    assert!(
        close(values["Main.Flag"], 1.0),
        "the operating command 0 is the predicate 0"
    );
    let elsewhere = ControllerInitialization::new(vec![state(
        "Main.Flag",
        "1",
        StateType::Bool,
        (0.0, 1.0),
        InitialValue::From(InitialSource::ModelInputIs {
            index: 0,
            value: 50.0,
        }),
    )])
    .expect("the state is valid");
    let values = initialization_values(&loop_definition, &model, &elsewhere, &BTreeMap::new())
        .expect("the value");
    assert!(
        close(values["Main.Flag"], 0.0),
        "another level of the actuator gives 0"
    );
    let outside = ControllerInitialization::new(vec![state(
        "Main.Flag",
        "1",
        StateType::Bool,
        (0.0, 1.0),
        InitialValue::From(InitialSource::ModelInputIs {
            index: 0,
            value: 150.0,
        }),
    )])
    .expect("the state is valid");
    assert!(
        initialization_values(&loop_definition, &model, &outside, &BTreeMap::new()).is_err(),
        "a predicate outside the actuator's range is refused"
    );
}

#[test]
fn an_integral_takes_the_operating_command_less_the_weighted_proportional_term() {
    let loop_definition = control_pid_loop();
    let model = first_order_model();
    let definition = ControllerInitialization::new(vec![state(
        "Main.Integral",
        "%",
        StateType::Lreal,
        (-100.0, 100.0),
        InitialValue::From(InitialSource::PidIntegral {
            index: 0,
            input_index: 0,
            gain_symbol: "Main.Parameters.Ki".to_owned(),
            weight_symbol: None,
            weight: Some(0.5),
        }),
    )])
    .expect("the state is valid");
    let parameters = BTreeMap::from([("Main.Parameters.Ki".to_owned(), 2.0)]);
    let values = initialization_values(&loop_definition, &model, &definition, &parameters)
        .expect("the value");
    // u₀ − Ki·(β − 1)·y₀ = 0 − 2·(0.5 − 1)·2 = 2.
    assert!(close(values["Main.Integral"], 2.0));
    assert!(
        initialization_values(&loop_definition, &model, &definition, &BTreeMap::new()).is_err(),
        "the gain must be bound and finite"
    );
}

#[test]
fn a_weighted_error_history_is_the_weight_less_one_times_the_process_value() {
    let definition = ControllerInitialization::new(vec![state(
        "Main.Error",
        "%",
        StateType::Lreal,
        (-100.0, 100.0),
        InitialValue::From(InitialSource::WeightedError {
            index: 0,
            weight_symbol: None,
            weight: Some(0.5),
        }),
    )])
    .expect("the state is valid");
    let values = initialization_values(
        &control_pid_loop(),
        &first_order_model(),
        &definition,
        &BTreeMap::new(),
    )
    .expect("the value");
    // (β − 1)·y₀ = (0.5 − 1)·2 = −1.
    assert!(close(values["Main.Error"], -1.0));
}

#[test]
fn the_operating_point_reads_zero_where_the_model_has_no_coordinate_and_the_state_types_map_to_transfer_types(
) {
    let model: GeneralModel = first_order_model();
    assert!(close(operating_input(&model, 0), 0.0));
    assert!(close(operating_output(&model, 0), 2.0));
    assert!(
        close(operating_output(&model, 3), 0.0),
        "a missing coordinate reads 0"
    );
    assert_eq!(
        TransferValueType::from(StateType::Bool),
        TransferValueType::Bool
    );
    assert_eq!(
        TransferValueType::from(StateType::Lreal),
        TransferValueType::Lreal
    );
}

/// Lesson 84's control loop as the shipped file holds it: the measured-disturbance feedforward (C8).
const LESSON_84: &str = include_str!(
    "../../../../examples/control/14-integrated-capstones/84-independent/control-loop.json"
);

#[test]
fn the_feedforward_state_starts_at_its_gain_times_the_disturbance_at_the_operating_point() {
    // C8: FeedForward starts at K_ff·d with d the disturbance's operating value, 10 kW on the heater, and z starts at d.
    let loop_definition = LoopDefinition::decode(LESSON_84).expect("lesson 84 decodes");
    let model = crate::tuning::structures::fixtures::heater_model();
    let definition = loop_definition
        .controller
        .project
        .as_ref()
        .expect("a project block")
        .initialization
        .clone();
    let parameters = BTreeMap::from([("P1.FeedforwardGain".to_owned(), 0.5)]);
    let values = initialization_values(&loop_definition, &model, &definition, &parameters)
        .expect("the states start at the operating point");
    assert_eq!(values["P1.Controller.StateValue.FeedForward"], 5.0);
    assert_eq!(values["P1.FilteredLoadKw"], 10.0);
    assert_eq!(values["P1.Initialized"], 1.0);
}

#[test]
fn a_feedforward_gain_in_another_unit_than_the_state_per_the_disturbance_is_refused() {
    // The gain must be `%/kW`: the state's unit per the disturbance's unit (controllerInitialState.ts).
    let text = LESSON_84.replace("\"unit\": \"%/kW\"", "\"unit\": \"%\"");
    let loop_definition = LoopDefinition::decode(&text).expect("the unit is not checked at decode");
    let model = crate::tuning::structures::fixtures::heater_model();
    let definition = loop_definition
        .controller
        .project
        .as_ref()
        .expect("a project block")
        .initialization
        .clone();
    let parameters = BTreeMap::from([("P1.FeedforwardGain".to_owned(), 0.5)]);
    let refusal = initialization_values(&loop_definition, &model, &definition, &parameters)
        .expect_err("the gain's unit is wrong");
    assert_eq!(refusal.cause, Cause::Settings);
    assert!(
        refusal
            .message
            .contains("P1.Controller.StateValue.FeedForward"),
        "{}",
        refusal.message
    );
}
