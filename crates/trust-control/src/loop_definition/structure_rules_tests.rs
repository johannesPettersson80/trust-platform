//! The controller structures a loop declares, checked at open (`structure_rules.rs`, specification 35, C2 and C4): a state-feedback law
//! is the project's, takes no controller law, and decodes against the loop's parameter record.

use serde_json::{json, Value};

use super::super::tests::complete_loop;
use super::*;

/// Lesson 69's LQR loop (C4): a state-feedback law of the project's design over its `P1.K1`, `P1.K2` and `P1.N` record.
const LESSON_69_LOOP: &str = include_str!(
    "../../../../editors/vscode/media/examples/control/11-state-space-estimation-and-predictive-control/69-lqr/control-loop.json"
);

fn refused(value: &Value) -> String {
    match LoopDefinition::decode(&value.to_string()) {
        Ok(_) => panic!("the loop must be refused at open"),
        Err(rejection) => rejection.to_string(),
    }
}

#[test]
fn a_loop_with_a_controller_law_and_no_other_structure_opens() {
    assert!(LoopDefinition::decode(&complete_loop().to_string()).is_ok());
}

#[test]
fn a_state_feedback_law_in_a_pid_designed_loop_is_refused_at_open() {
    let mut value = complete_loop();
    value["controller"]["stateFeedback"] = json!({});
    value["controller"]
        .as_object_mut()
        .expect("the controller")
        .remove("law");
    assert!(refused(&value).contains("a state-feedback law needs the project design"));
}

#[test]
fn a_state_feedback_law_with_a_controller_law_is_refused_at_open_by_the_gate() {
    // Two controller structures are the gate's refusal, with its sentence (C2), not a decode error: lesson 69's LQR law, which decodes
    // against its own parameter record, with a controller law beside it.
    let mut loop_definition =
        LoopDefinition::decode(LESSON_69_LOOP).expect("lesson 69's loop decodes");
    loop_definition.controller.law = Some(crate::loop_definition::ControllerLaw::Proportional {
        gain_symbol: "P1.K1".to_owned(),
    });
    let failure = loop_definition
        .require_one_controller_structure()
        .expect_err("two controller structures are refused");
    assert!(
        failure
            .message
            .contains("more than one controller structure"),
        "{}",
        failure.message
    );
}

#[test]
fn a_state_feedback_law_that_does_not_decode_against_the_parameter_record_is_refused_at_open() {
    let mut value = complete_loop();
    value["controller"]["design"] = json!("project");
    value["controller"]["stateFeedback"] = json!({ "family": "LQR" });
    value["controller"]
        .as_object_mut()
        .expect("the controller")
        .remove("law");
    assert!(refused(&value).contains("does not decode against the loop's parameter record"));
}

use crate::loop_definition::{PostScan, StateType};

/// A declared state of the given symbol and initial value, bounded and LREAL.
fn declared(symbol: &str, initial: InitialValue) -> InitializedState {
    InitializedState {
        symbol: symbol.to_owned(),
        iec_type: StateType::Lreal,
        unit: "%".to_owned(),
        minimum: -1.0,
        maximum: 1.0,
        initial,
        post_scan: PostScan::Bounded,
    }
}

/// The complete loop with a Control.PID initialization of the given states, as its declared block.
fn with_states(states: Vec<InitializedState>) -> LoopDefinition {
    let mut loop_definition =
        LoopDefinition::decode(&complete_loop().to_string()).expect("the loop decodes");
    loop_definition.controller.pid_initialization = Some(PidInitialization::ControlPid {
        maximum_snapshot_age_ms: 100,
        states,
        application_states: None,
    });
    loop_definition
}

#[test]
fn a_declared_block_holds_at_most_thirty_two_states() {
    let states = |count: usize| {
        (0..count)
            .map(|index| declared(&format!("Main.S{index}"), InitialValue::Constant(0.0)))
            .collect::<Vec<_>>()
    };
    assert!(check_blocks(&with_states(states(32))).is_ok());
    assert!(check_blocks(&with_states(states(33))).is_err());
}

#[test]
fn a_decoupled_output_names_a_row_of_a_square_matrix_of_at_most_eight_rows() {
    let with_matrix = |matrix: Vec<Vec<String>>, index: usize| {
        with_states(vec![declared(
            "Main.D",
            InitialValue::From(InitialSource::DecoupledOutput {
                index,
                matrix_symbols: matrix,
            }),
        )])
    };
    let square = vec![
        vec!["A".to_owned(), "B".to_owned()],
        vec!["C".to_owned(), "D".to_owned()],
    ];
    assert!(check_blocks(&with_matrix(square.clone(), 1)).is_ok());
    assert!(
        check_blocks(&with_matrix(square.clone(), 2)).is_err(),
        "the index names no row"
    );
    assert!(
        check_blocks(&with_matrix(vec![vec!["A".to_owned(), "B".to_owned()]], 0)).is_err(),
        "the matrix is not square"
    );
    assert!(
        check_blocks(&with_matrix(vec![vec![String::new()]], 0)).is_err(),
        "an entry has no symbol"
    );
    let nine = vec![vec!["A".to_owned(); 9]; 9];
    assert!(
        check_blocks(&with_matrix(nine, 0)).is_err(),
        "at most eight rows"
    );
}
