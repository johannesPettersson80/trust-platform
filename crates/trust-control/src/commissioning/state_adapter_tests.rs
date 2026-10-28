// The coefficient values are compared as the record writes them, exactly.
#![allow(clippy::float_cmp)]

use std::collections::BTreeMap;

use serde_json::Value;

use super::*;
use crate::commissioning::adapters::{controller_adapter, AdapterKind};
use crate::commissioning::coefficient_record::coefficient_values;
use crate::commissioning::fixture_loops::requirements;
use crate::loop_definition::LoopDefinition;

/// Lesson 68's control loop, which declares the state-feedback law `K1, K2, N` (C4).
const LESSON_68_LOOP: &str = include_str!(
    "../../../../editors/vscode/media/examples/control/11-state-space-estimation-and-predictive-control/68-state-feedback/control-loop.json"
);
/// Lesson 68's supplied model: the two-tank cascade with both levels measured, under the outputs its channels name.
const LESSON_68_MODEL: &str = include_str!(
    "../../../../editors/vscode/media/examples/control/11-state-space-estimation-and-predictive-control/68-state-feedback/provided-model.json"
);
/// Lesson 69's control loop: the LQR law on lesson 68's cascade.
const LESSON_69_LOOP: &str = include_str!(
    "../../../../editors/vscode/media/examples/control/11-state-space-estimation-and-predictive-control/69-lqr/control-loop.json"
);
/// Lesson 72's control loop: the LQG law with its estimator.
const LESSON_72_LOOP: &str = include_str!(
    "../../../../editors/vscode/media/examples/control/11-state-space-estimation-and-predictive-control/72-lqg/control-loop.json"
);

/// A loop file decoded as the engine reads it.
fn loop_of(text: &str) -> LoopDefinition {
    LoopDefinition::decode(text).expect("the shipped loop file decodes")
}

/// Lesson 68's design, as Tune writes it (the README's K1, K2 and N).
fn lesson_68_design() -> BTreeMap<String, f64> {
    BTreeMap::from([
        ("P1.K1".to_owned(), 0.0562),
        ("P1.K2".to_owned(), 0.0369),
        ("P1.N".to_owned(), 0.1131),
    ])
}

#[test]
fn lesson_68_declares_the_state_feedback_law_with_its_gains_and_reference_gain() {
    let law = state_law(&loop_of(LESSON_68_LOOP))
        .expect("the law decodes")
        .expect("lesson 68 declares a state law");
    assert_eq!(law.binding.family, StateControllerFamily::StateFeedback);
    assert_eq!(
        coefficient_symbols(&law.binding),
        ["P1.K1", "P1.K2", "P1.N"]
    );
    assert_eq!(family_label(law.binding.family), "State feedback");
    assert_eq!(
        law.initialization.states(),
        [] as [crate::loop_definition::InitializedState; 0]
    );
}

#[test]
fn lesson_69_is_lqr_and_lesson_72_is_lqg_with_its_observer_gains() {
    let lqr = state_law(&loop_of(LESSON_69_LOOP))
        .expect("the law decodes")
        .expect("lesson 69 declares a state law");
    assert_eq!(lqr.binding.family, StateControllerFamily::Lqr);
    assert_eq!(family_label(lqr.binding.family), "LQR");
    let lqg = state_law(&loop_of(LESSON_72_LOOP))
        .expect("the law decodes")
        .expect("lesson 72 declares a state law");
    assert_eq!(lqg.binding.family, StateControllerFamily::Lqg);
    assert_eq!(
        coefficient_symbols(&lqg.binding),
        ["P1.K1", "P1.K2", "P1.N", "P1.L1", "P1.L2"]
    );
}

#[test]
fn a_loop_without_state_feedback_has_no_state_law() {
    let mut loop_definition = loop_of(LESSON_68_LOOP);
    loop_definition.controller.state_feedback = None;
    assert!(state_law(&loop_definition).expect("no law").is_none());
}

#[test]
fn the_engine_offers_lesson_68_the_state_law_adapter_and_no_pid_family() {
    let lesson_68 = loop_of(LESSON_68_LOOP);
    let adapter = controller_adapter(&lesson_68).expect("a state law has an adapter");
    assert!(matches!(adapter.kind(), AdapterKind::State(_)));
    assert_eq!(
        adapter.families(),
        [] as [crate::tuning::structure::Structure; 0]
    );
}

#[test]
fn the_coefficients_of_lesson_68_are_written_inside_their_bounds() {
    let loop_definition = loop_of(LESSON_68_LOOP);
    let values = coefficient_values(&loop_definition, &lesson_68_design(), &BTreeMap::new())
        .expect("the design's values are inside the bounds of the record");
    assert_eq!(values, lesson_68_design());
    let outside = BTreeMap::from([("P1.K1".to_owned(), 2.0)]);
    let failure = coefficient_values(&loop_definition, &outside, &BTreeMap::new())
        .expect_err("a gain of 2 is outside the bounds of -1 to 1");
    assert_eq!(failure.next_action, NextAction::Tune);
    assert_eq!(failure.cause, Cause::Settings);
    let undeclared = BTreeMap::from([("P1.Unknown".to_owned(), 0.1)]);
    let failure = coefficient_values(&loop_definition, &undeclared, &BTreeMap::new())
        .expect_err("a symbol the record does not declare is refused");
    assert_eq!(failure.next_action, NextAction::OpenLoopSetup);
}

#[test]
fn the_observed_values_stay_for_every_parameter_the_law_does_not_write() {
    let loop_definition = loop_of(LESSON_68_LOOP);
    let observed = BTreeMap::from([
        ("P1.K1".to_owned(), 0.0),
        ("P1.K2".to_owned(), 0.0),
        ("P1.N".to_owned(), 0.0),
    ]);
    let values = coefficient_values(
        &loop_definition,
        &BTreeMap::from([("P1.K1".to_owned(), 0.5)]),
        &observed,
    )
    .expect("one coefficient is written");
    assert_eq!(values["P1.K1"], 0.5);
    assert_eq!(values["P1.K2"], 0.0);
    assert_eq!(values["P1.N"], 0.0);
}

#[test]
fn lesson_68_allowance_on_its_two_tank_model_is_the_first_output_formula() {
    // The state-law allowance needs the model, and lesson 68's model measures both states.
    let loop_definition = loop_of(LESSON_68_LOOP);
    let model_value: Value =
        serde_json::from_str(LESSON_68_MODEL).expect("lesson 68's model file is JSON");
    let model = crate::model::general::decode_general_model(&model_value)
        .expect("lesson 68's model decodes");
    let task = EngineerRequirements {
        settling_band: 0.001,
        ..requirements()
    };
    let adapter = controller_adapter(&loop_definition).expect("a state law has an adapter");
    assert!(matches!(adapter.kind(), AdapterKind::State(_)));
    let allowance = crate::commissioning::entry_allowance::auto_entry_allowance(
        &loop_definition,
        &model,
        &task,
        &lesson_68_design(),
        None,
        None,
    )
    .expect("the C4 allowance");
    assert!((allowance - 0.000_206_2).abs() < 1e-12, "{allowance}");
}

/// Lesson 72's model measures tank 2 only, so its estimate is not a measured output and its allowance keeps the exact check (C4).
#[test]
fn lesson_72_keeps_the_exact_check_because_its_estimate_is_not_a_measured_output() {
    let loop_definition = loop_of(LESSON_72_LOOP);
    let model = serde_json::from_str::<Value>(
        include_str!(
            "../../../../editors/vscode/media/examples/control/11-state-space-estimation-and-predictive-control/72-lqg/lqg-model.json"
        ),
    )
    .expect("the model file is JSON");
    let model = crate::model::general::decode_general_model(&model).expect("the model decodes");
    let task = EngineerRequirements {
        settling_band: 0.001,
        ..requirements()
    };
    let values = BTreeMap::from([
        ("P1.K1".to_owned(), 0.0787),
        ("P1.K2".to_owned(), 0.0239),
        ("P1.N".to_owned(), 0.1226),
        ("P1.L1".to_owned(), 0.0023),
        ("P1.L2".to_owned(), 0.0068),
    ]);
    let allowance = crate::commissioning::entry_allowance::auto_entry_allowance(
        &loop_definition,
        &model,
        &task,
        &values,
        None,
        None,
    )
    .expect("the exact check");
    // The exact check of the pump output's 0.02 m³/s span: 1e-6 · max(0.02, 1).
    assert!((allowance - 1e-6).abs() < 1e-12, "{allowance}");
}
