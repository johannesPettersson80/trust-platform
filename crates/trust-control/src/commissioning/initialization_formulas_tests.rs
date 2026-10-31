// The fixture's values are exact, and the formulas are compared against the values the specification gives.
#![allow(clippy::float_cmp)]

use super::*;
use crate::commissioning::fixture_loops::{control_pid_loop, first_order_model};
use crate::commissioning::transfer::{TransferMember, TransferModeBinding, TransferValueType};
use crate::loop_definition::{InitialSource, InitialValue, PostScan, StateType};

fn state(symbol: &str, unit: &str, iec_type: StateType, initial: InitialValue) -> InitializedState {
    InitializedState {
        symbol: symbol.to_owned(),
        iec_type,
        unit: unit.to_owned(),
        minimum: -100.0,
        maximum: 100.0,
        initial,
        post_scan: PostScan::Bounded,
    }
}

/// A dimensionless BOOL state, bounded 0 to 1 as a BOOL must be.
fn flag(symbol: &str, initial: InitialValue) -> InitializedState {
    InitializedState {
        minimum: 0.0,
        maximum: 1.0,
        ..state(symbol, "1", StateType::Bool, initial)
    }
}

/// The declared states of a loop that uses each formula kind the Rust engine runs, with the fixture's signals.
fn declared() -> ControllerInitialization {
    ControllerInitialization::new(vec![
        state(
            "Main.Setpoint",
            "%",
            StateType::Lreal,
            InitialValue::From(InitialSource::ModelOutput { index: 0 }),
        ),
        state(
            "Main.Command",
            "%",
            StateType::Lreal,
            InitialValue::From(InitialSource::ModelInput { index: 0 }),
        ),
        state(
            "Main.Integral",
            "%",
            StateType::Lreal,
            InitialValue::From(InitialSource::PidIntegral {
                index: 0,
                input_index: 0,
                gain_symbol: "Main.Parameters.Ki".to_owned(),
                weight_symbol: None,
                weight: Some(0.5),
            }),
        ),
        flag(
            "Main.Flag",
            InitialValue::From(InitialSource::ModelInputIs {
                index: 0,
                value: 0.0,
            }),
        ),
        state(
            "Main.Constant",
            "%",
            StateType::Lreal,
            InitialValue::Constant(1.0),
        ),
    ])
    .expect("the states are valid")
}

fn binding() -> TransferBinding {
    TransferBinding {
        loop_id: "loop:tank".to_owned(),
        task_name: "MainTask".to_owned(),
        mode: TransferModeBinding {
            symbol: "Main.Auto".to_owned(),
            manual_value: false,
        },
        additional_modes: Vec::new(),
        tracking_outputs: BTreeMap::from([("ValveMV".to_owned(), "ValvePosition".to_owned())]),
        states: Vec::new(),
        guards: Vec::new(),
        output: TransferMember {
            symbol: "ValveMV".to_owned(),
            iec_type: TransferValueType::Lreal,
            unit: "%".to_owned(),
            minimum: 0.0,
            maximum: 100.0,
        },
        additional_outputs: Vec::new(),
    }
}

fn snapshot() -> BTreeMap<String, TransferValue> {
    BTreeMap::from([
        ("ValvePosition".to_owned(), TransferValue::Number(12.5)),
        ("LevelPV".to_owned(), TransferValue::Number(10.0)),
        ("Main.Parameters.Ki".to_owned(), TransferValue::Number(2.0)),
        ("Main.Flag".to_owned(), TransferValue::Boolean(true)),
    ])
}

#[test]
fn each_state_with_a_source_gets_the_formula_of_its_kind_and_a_constant_gets_none() {
    let formulas = initialization_formulas(&control_pid_loop(), &first_order_model(), &declared())
        .expect("the formulas");
    assert_eq!(
        formulas["Main.Setpoint"],
        InitializationFormula::ModelOutput {
            process_variable: "LevelPV".to_owned()
        }
    );
    assert_eq!(
        formulas["Main.Command"],
        InitializationFormula::ModelInput {
            output_symbol: "ValveMV".to_owned()
        }
    );
    assert_eq!(
        formulas["Main.Integral"],
        InitializationFormula::PidIntegral {
            output_symbol: "ValveMV".to_owned(),
            process_variable: "LevelPV".to_owned(),
            gain_symbol: "Main.Parameters.Ki".to_owned(),
            weight_symbol: None,
            weight: Some(0.5),
        }
    );
    assert_eq!(
        formulas["Main.Flag"],
        InitializationFormula::ModelInputIs {
            output_symbol: "ValveMV".to_owned(),
            value: 0.0,
        }
    );
    assert!(
        !formulas.contains_key("Main.Constant"),
        "a constant has no source at the boundary"
    );
}

#[test]
fn a_predicate_outside_the_actuator_range_has_no_formula() {
    let definition = ControllerInitialization::new(vec![flag(
        "Main.Flag",
        InitialValue::From(InitialSource::ModelInputIs {
            index: 0,
            value: 150.0,
        }),
    )])
    .expect("the declaration is valid");
    assert!(
        initialization_formulas(&control_pid_loop(), &first_order_model(), &definition).is_err()
    );
}

#[test]
fn the_fresh_values_of_a_snapshot_give_each_formula_its_value() {
    let binding = binding();
    let snapshot = snapshot();
    let model_input = InitializationFormula::ModelInput {
        output_symbol: "ValveMV".to_owned(),
    };
    assert!(
        (evaluate_initialization_formula(&binding, &model_input, &snapshot).expect("the command")
            - 12.5)
            .abs()
            <= 1e-12,
        "the command is read at its feedback guard"
    );
    // u − Ki·(β − 1)·y = 12.5 − 2·(0.5 − 1)·10 = 22.5.
    let integral = InitializationFormula::PidIntegral {
        output_symbol: "ValveMV".to_owned(),
        process_variable: "LevelPV".to_owned(),
        gain_symbol: "Main.Parameters.Ki".to_owned(),
        weight_symbol: None,
        weight: Some(0.5),
    };
    assert!(
        (evaluate_initialization_formula(&binding, &integral, &snapshot).expect("the integral")
            - 22.5)
            .abs()
            <= 1e-12
    );
    // (β − 1)·y = (0.5 − 1)·10 = −5.
    let error = InitializationFormula::WeightedError {
        process_variable: "LevelPV".to_owned(),
        weight_symbol: None,
        weight: Some(0.5),
    };
    assert!(
        (evaluate_initialization_formula(&binding, &error, &snapshot).expect("the error") + 5.0)
            .abs()
            <= 1e-12
    );
    let open = InitializationFormula::ModelInputIs {
        output_symbol: "ValveMV".to_owned(),
        value: 12.5,
    };
    assert_eq!(
        evaluate_initialization_formula(&binding, &open, &snapshot).expect("a level"),
        1.0
    );
    let closed = InitializationFormula::ModelInputIs {
        output_symbol: "ValveMV".to_owned(),
        value: 3.0,
    };
    assert_eq!(
        evaluate_initialization_formula(&binding, &closed, &snapshot).expect("another level"),
        0.0
    );
    let process = InitializationFormula::ModelOutput {
        process_variable: "Main.Flag".to_owned(),
    };
    assert_eq!(
        evaluate_initialization_formula(&binding, &process, &snapshot).expect("a flag reads as 1"),
        1.0
    );
}

#[test]
fn a_formula_whose_snapshot_value_is_missing_is_refused() {
    let formula = InitializationFormula::ModelOutput {
        process_variable: "Main.Missing".to_owned(),
    };
    let refusal = evaluate_initialization_formula(&binding(), &formula, &snapshot())
        .expect_err("the value is missing");
    assert!(refusal.message.contains("Main.Missing"));
}

#[test]
fn a_formula_serializes_with_the_kind_word_of_the_protocol() {
    let formula = InitializationFormula::ModelInputIs {
        output_symbol: "ValveMV".to_owned(),
        value: 0.0,
    };
    let value = serde_json::to_value(&formula).expect("the formula serializes");
    assert_eq!(value["kind"], "model-input-is");
    assert_eq!(value["output_symbol"], "ValveMV");
}

/// Lesson 84's feedforward state reads the measured disturbance fresh at the task boundary, as a scaled measurement (C8).
#[test]
fn the_feedforward_state_reads_the_measured_disturbance_fresh_as_a_scaled_measurement() {
    let loop_definition = LoopDefinition::decode(include_str!(
        "../../../../examples/control/14-integrated-capstones/84-independent/control-loop.json"
    ))
    .expect("lesson 84 decodes");
    let model = crate::tuning::structures::fixtures::heater_model();
    let definition = loop_definition
        .controller
        .project
        .as_ref()
        .expect("a project block")
        .initialization
        .clone();
    let formulas = initialization_formulas(&loop_definition, &model, &definition)
        .expect("the formulas are read");
    assert_eq!(
        formulas["P1.Controller.StateValue.FeedForward"],
        InitializationFormula::ScaledMeasurement {
            process_variable: "P1.LoadKw".to_owned(),
            gain_symbol: "P1.FeedforwardGain".to_owned(),
        }
    );
    // The filtered disturbance reads the measured disturbance as a measurement, never as an actuator (C0, C8).
    assert_eq!(
        formulas["P1.FilteredLoadKw"],
        InitializationFormula::ModelOutput {
            process_variable: "P1.LoadKw".to_owned(),
        }
    );
    assert_eq!(
        formulas["P1.Controller.StateValue.NominalOutput"],
        InitializationFormula::ModelInput {
            output_symbol: "P1.HeaterPct".to_owned(),
        }
    );
}
