// The coefficient values are compared as the record writes them, exactly.
#![allow(clippy::float_cmp)]

use std::collections::BTreeMap;

use super::*;
use crate::commissioning::adapters::{controller_adapter, AdapterKind};
use crate::commissioning::fixture_loops::requirements;
use crate::loop_definition::LoopDefinition;
use crate::tuning::state_controller::fixtures::two_tank_model;

/// Lesson 89's control loop: a program's PID whose notch the finite search tunes over two axes (C5).
const LESSON_89_LOOP: &str = include_str!(
    "../../../../examples/control/07-classical-stability-and-frequency-design/89-mechanical-resonance/control-loop.json"
);

/// Lesson 89's loop, decoded as the engine reads it.
fn lesson_89() -> LoopDefinition {
    LoopDefinition::decode(LESSON_89_LOOP).expect("lesson 89's loop file decodes")
}

/// The notch of lesson 89's chosen trial: 2 Hz at a quality of 3, both inside their bounds.
fn chosen_trial() -> BTreeMap<String, f64> {
    BTreeMap::from([
        ("P1.NotchFrequencyHz".to_owned(), 2.0),
        ("P1.NotchQuality".to_owned(), 3.0),
    ])
}

#[test]
fn lesson_89_is_the_search_law_form_and_the_engine_offers_its_search_adapter() {
    let loop_definition = lesson_89();
    assert!(search_law(&loop_definition).is_some());
    let adapter = controller_adapter(&loop_definition).expect("a search law has an adapter");
    assert!(matches!(adapter.kind(), AdapterKind::Search { .. }));
    assert_eq!(
        adapter.families(),
        [] as [crate::tuning::structure::Structure; 0]
    );
}

#[test]
fn a_search_law_enters_auto_only_with_a_reviewed_tolerance() {
    let loop_definition = lesson_89();
    let adapter = controller_adapter(&loop_definition).expect("a search law has an adapter");
    let model = two_tank_model();
    let without = requirements();
    let failure = adapter
        .initialization_plan(&model, &without, &chosen_trial(), None)
        .expect_err("no reviewed tolerance, no Auto entry");
    assert_eq!(failure.next_action, NextAction::OpenRequirements);
    assert_eq!(failure.cause, Cause::Settings);
    let text = failure.detail.clone().unwrap_or(failure.message.clone());
    assert_eq!(text, REVIEWED_TOLERANCE_REQUIRED);
}

#[test]
fn the_chosen_trial_is_the_write_set_of_a_search_law_and_a_gain_is_refused() {
    let loop_definition = lesson_89();
    let adapter = controller_adapter(&loop_definition).expect("a search law has an adapter");
    let values = adapter
        .materialize_coefficients(&chosen_trial(), &BTreeMap::new())
        .expect("the axes are inside their bounds");
    assert_eq!(values, chosen_trial());
    let gains = crate::commissioning::adapters::ControllerGains {
        proportional_gain: Some(1.0),
        ..Default::default()
    };
    let failure = adapter
        .materialize(&gains, &BTreeMap::new())
        .expect_err("a search law writes its coefficients, not gains");
    assert_eq!(failure.cause, Cause::Implementation);
}
