use super::*;
use crate::commissioning::declared_initialization::declared_commissioning_profile;
use crate::commissioning::fixture_loops::{control_pid_loop, first_order_model, requirements};
use crate::commissioning::initial_state::ControllerInitialization;
use crate::commissioning::transfer::TransferValue;
use std::collections::BTreeMap;

#[test]
fn a_required_tracking_word_is_the_word_the_runtime_reports() {
    assert_eq!(
        RequiredTracking::DeclaredInitializationFormulas.word(),
        "declared-initialization-formulas"
    );
}

#[test]
fn a_profile_without_formulas_requires_no_tracking_names_its_reference_and_targets_its_setpoint() {
    let definition = ControllerInitialization::new(Vec::new()).expect("an empty list is valid");
    let profile = declared_commissioning_profile(
        &control_pid_loop(),
        &first_order_model(),
        &definition,
        "Control.PID",
        &BTreeMap::new(),
    )
    .expect("the profile");
    assert_eq!(profile.label, "Control.PID");
    assert_eq!(profile.tracking_kind(), "declared-state-v1");
    assert_eq!(profile.references, vec!["LevelSP".to_owned()]);
    assert!(
        profile.required_tracking.is_empty(),
        "no state has a formula"
    );
    let binding = profile.binding().expect("the binding");
    let states: Vec<String> = binding
        .states
        .iter()
        .map(|member| member.symbol.clone())
        .collect();
    assert_eq!(
        states,
        vec!["LevelSP".to_owned(), "Main.ManualCommand".to_owned()]
    );
    let targets = profile.targets(&requirements()).expect("the targets");
    // The operating output 2 plus the reviewed reference change 1.
    assert!((targets["LevelSP"] - 3.0).abs() <= 1e-12);
}

#[test]
fn the_profile_reads_back_the_states_it_observes_and_refuses_a_missing_one() {
    let definition = ControllerInitialization::new(Vec::new()).expect("an empty list is valid");
    let profile = declared_commissioning_profile(
        &control_pid_loop(),
        &first_order_model(),
        &definition,
        "Control.PID",
        &BTreeMap::new(),
    )
    .expect("the profile");
    let binding = profile.binding().expect("the binding");
    let observed = BTreeMap::from([
        ("LevelSP".to_owned(), TransferValue::Number(3.0)),
        (
            "Main.ManualCommand".to_owned(),
            TransferValue::Boolean(true),
        ),
    ]);
    let states = profile.states(&binding, &observed).expect("the states");
    assert!((states["LevelSP"] - 3.0).abs() <= 1e-12);
    assert!(
        (states["Main.ManualCommand"] - 1.0).abs() <= 1e-12,
        "a BOOL reads as 1"
    );
    let missing = BTreeMap::from([("LevelSP".to_owned(), TransferValue::Number(3.0))]);
    assert!(
        profile.states(&binding, &missing).is_err(),
        "the Manual command must be reported"
    );
}
