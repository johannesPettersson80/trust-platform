//! Tests of the inputs a study records (specification 35, A8.4, B4.2, B4.3 and B8).

use super::*;
use crate::failure::{Cause, NextAction};
use crate::identification::acquisition_settings::AcquisitionMethod;
use crate::identification::method_applicability::MethodRefusal;
use crate::identification::texts::method_refusal_text;
use crate::study::StudySection;
use crate::tuning::automatic_criteria::AutomaticCriteria;
use crate::tuning::objective::{TuningMethod, TuningObjective};

fn requirements() -> TuningRequirements {
    TuningRequirements {
        reference_change: 2.0,
        maximum_error: 0.4,
        settling_band: 0.1,
        settling_time_seconds: 45.0,
        duration_seconds: 90.0,
        constant_input_disturbance: None,
        measurement_noise_band: None,
        gain_uncertainty_fraction: None,
        transfer_tolerance: None,
        maximum_overshoot_percent: None,
        tuning_method: None,
        tuning_objective: None,
        automatic_criteria: None,
    }
}

fn settings(duration_seconds: f64) -> AcquisitionSettings {
    AcquisitionSettings {
        method: AcquisitionMethod::Automatic,
        duration_seconds,
        amplitude: 0.0,
        setpoint_signal_id: None,
        relay_direction: None,
        reuse_recordings: false,
        input_range: None,
        output_range: None,
        input_ranges: None,
        output_ranges: None,
    }
}

#[test]
fn an_empty_inputs_section_is_valid_and_is_written_as_an_empty_document() {
    let empty = InputsSection::default();
    assert!(empty.check().is_ok(), "every input is optional");
    let text = empty.encode().expect("encoded");
    assert_eq!(text, "{}", "an absent input is not written");
    assert_eq!(InputsSection::decode(&text).expect("decoded"), empty);
    assert!(!empty.needs_manual_settings());
}

#[test]
fn a_section_round_trips_through_its_document_with_every_input() {
    let section = InputsSection {
        acquisition: Some(settings(30.0)),
        requirements: Some(requirements()),
        method: Some(TuneMethod::Simc),
        structure: None,
        pins: None,
        settings: None,
        scenarios: None,
        recording_selection: None,
        state_design: None,
        on_off_pins: None,
    };
    let text = section.encode().expect("encoded");
    assert_eq!(
        InputsSection::decode(&text).expect("decoded"),
        section,
        "the recorded inputs come back as they were written"
    );
    assert_eq!(
        section.encode().expect("again"),
        text,
        "the same inputs give the same text"
    );
}

#[test]
fn a_document_with_an_unknown_field_is_refused() {
    assert!(InputsSection::decode(r#"{"bogus":1}"#).is_err());
}

#[test]
fn the_declared_scenarios_are_checked_and_a_scenario_without_identity_is_refused() {
    let declared = InputsSection {
        scenarios: Some(Vec::new()),
        ..InputsSection::default()
    };
    assert!(
        declared.check().is_ok(),
        "a declared list with no scenario is valid"
    );
    let unnamed: EngineerScenario = serde_json::from_value(serde_json::json!({
        "id": "",
        "initialValues": {},
        "events": [],
        "requirements": [],
    }))
    .expect("the scenario decodes; its empty identity is what is refused");
    let refused = InputsSection {
        scenarios: Some(vec![unnamed]),
        ..InputsSection::default()
    }
    .check()
    .expect_err("a scenario without an identity");
    assert_eq!(refused.cause, Cause::Settings);
    assert_eq!(refused.next_action, NextAction::OpenRequirements);
}

#[test]
fn the_requirements_are_checked_as_the_tune_section_checks_them() {
    let mut bad = requirements();
    bad.settling_band = 0.4;
    let refused = InputsSection {
        requirements: Some(bad),
        ..InputsSection::default()
    }
    .check()
    .expect_err("a settling band not below the error limit");
    assert_eq!(refused.cause, Cause::Settings);
    assert_eq!(refused.next_action, NextAction::OpenRequirements);
}

#[test]
fn an_acquisition_setting_outside_the_identification_rules_is_refused() {
    let short = InputsSection {
        acquisition: Some(settings(0.5)),
        ..InputsSection::default()
    };
    assert!(short.check().is_err(), "a duration under one second");
    let valid = InputsSection {
        acquisition: Some(settings(30.0)),
        ..InputsSection::default()
    };
    assert!(valid.check().is_ok());
}

#[test]
fn a_refused_acquisition_setting_names_the_rule_it_breaks() {
    // A relay without its declared direction names the sentence that the method's applicability gives (B3.2).
    let relay = InputsSection {
        acquisition: Some(AcquisitionSettings {
            method: AcquisitionMethod::Relay,
            ..settings(30.0)
        }),
        ..InputsSection::default()
    };
    let relay_failure = relay.check().expect_err("a relay names no direction");
    assert_eq!(
        relay_failure.message,
        method_refusal_text(MethodRefusal::RelayDirection)
    );
    assert_eq!(relay_failure.next_action, NextAction::None);
    // Another rule names its own sentence.
    let short = InputsSection {
        acquisition: Some(settings(0.5)),
        ..InputsSection::default()
    };
    let short_failure = short.check().expect_err("a duration under one second");
    assert_eq!(
        short_failure.message,
        "The experiment duration must be between 1 s and 24 h."
    );
}

#[test]
fn only_the_manual_method_without_its_settings_needs_them() {
    let manual_without = InputsSection {
        method: Some(TuneMethod::Manual),
        ..InputsSection::default()
    };
    assert!(manual_without.needs_manual_settings());
    let manual_with = InputsSection {
        method: Some(TuneMethod::Manual),
        settings: Some(BTreeMap::from([("ProportionalGain".to_owned(), 2.5)])),
        ..InputsSection::default()
    };
    assert!(!manual_with.needs_manual_settings());
    let formula = InputsSection {
        method: Some(TuneMethod::Simc),
        ..InputsSection::default()
    };
    assert!(
        !formula.needs_manual_settings(),
        "a formula method takes no entered settings"
    );
    let stale = InputsSection {
        method: Some(TuneMethod::Simc),
        settings: Some(BTreeMap::from([("ProportionalGain".to_owned(), 2.5)])),
        ..InputsSection::default()
    };
    assert_eq!(
        stale.manual_settings(),
        None,
        "settings entered for the manual method are not given to another method"
    );
    assert!(manual_with.manual_settings().is_some());
}

#[test]
fn the_validations_are_invalidated_only_when_the_held_requirements_change() {
    let held = requirements();
    let mut changed = requirements();
    changed.settling_time_seconds = 50.0;
    let none = held_test(None, &[]);
    assert!(
        !invalidation(&none, &none).validation,
        "nothing held, nothing changed"
    );
    assert!(
        !invalidation(&held_test(Some(&held), &[]), &held_test(Some(&held), &[])).validation,
        "the same requirements invalidate nothing"
    );
    assert!(
        invalidation(
            &held_test(Some(&held), &[]),
            &held_test(Some(&changed), &[])
        )
        .validation,
        "changed requirements invalidate the validations (A8.4)"
    );
    assert!(
        invalidation(&none, &held_test(Some(&held), &[])).validation,
        "requirements recorded where none were held are a change"
    );
}

/// The test a study holds, for the requirements and the scenarios given.
fn held_test<'a>(
    requirements: Option<&'a TuningRequirements>,
    scenarios: &'a [EngineerScenario],
) -> HeldTest<'a> {
    HeldTest {
        requirements,
        scenarios,
    }
}

/// The commissioning example's loop, which declares no scenarios.
fn commissioning_loop() -> LoopDefinition {
    LoopDefinition::decode(include_str!(
        "../../../../examples/control-pid-commissioning/control-loop.json"
    ))
    .expect("the commissioning example's loop")
}

/// One declared scenario that the campaign accepts, named `id`.
fn scenario_named(id: &str) -> EngineerScenario {
    serde_json::from_value(serde_json::json!({
        "id": id,
        "initialValues": { "Main.PV": 2 },
        "events": [{ "atMs": 100, "symbol": "Main.SP", "value": 3 }],
        "requirements": [{ "id": "r", "kind": "bounds", "channel": "Main.PV",
            "window": { "start": 0, "end": 1 }, "minimum": 0, "maximum": 5 }]
    }))
    .expect("a scenario in the stored form")
}

#[test]
fn a_changed_scenario_list_invalidates_the_validations_as_changed_requirements_do() {
    let held = requirements();
    let first = vec![scenario_named("load-step")];
    let second = vec![scenario_named("noise")];
    assert!(
        !invalidation(
            &held_test(Some(&held), &first),
            &held_test(Some(&held), &first)
        )
        .validation,
        "the same scenarios invalidate nothing"
    );
    assert!(
        invalidation(
            &held_test(Some(&held), &[]),
            &held_test(Some(&held), &first)
        )
        .validation,
        "a scenario declared where none was is a change (A8.4)"
    );
    assert!(
        invalidation(
            &held_test(Some(&held), &first),
            &held_test(Some(&held), &second)
        )
        .validation,
        "another scenario list is a change (A8.4)"
    );
}

#[test]
fn the_held_scenarios_are_the_recorded_list_and_the_loops_own_list_otherwise() {
    let mut loop_definition = commissioning_loop();
    assert!(
        held_scenarios(&loop_definition, None).is_empty(),
        "a loop that declares none, and an inputs section that records none"
    );
    loop_definition.verification_scenarios = Some(serde_json::json!([scenario_named("own")]));
    assert_eq!(
        held_scenarios(&loop_definition, None),
        vec![scenario_named("own")],
        "without a recorded list, the loop's own list stands"
    );
    let recorded = InputsSection {
        scenarios: Some(vec![scenario_named("recorded")]),
        ..InputsSection::default()
    };
    assert_eq!(
        held_scenarios(&loop_definition, Some(&recorded)),
        vec![scenario_named("recorded")],
        "the recorded list is held over the loop's own (B4.3)"
    );
    let declared_none = InputsSection {
        scenarios: Some(Vec::new()),
        ..InputsSection::default()
    };
    assert!(
        held_scenarios(&loop_definition, Some(&declared_none)).is_empty(),
        "an engineer's empty list declares that there are none"
    );
}

#[test]
fn the_loop_that_validate_runs_declares_the_recorded_scenarios_to_the_campaign() {
    let loop_definition = commissioning_loop();
    let recorded = InputsSection {
        scenarios: Some(vec![scenario_named("recorded")]),
        ..InputsSection::default()
    };
    let held = held_loop(&loop_definition, Some(&recorded)).expect("the loop with its scenarios");
    assert_eq!(
        crate::validation::engineer_verification_campaign::declared_scenarios(&held)
            .expect("the campaign reads them"),
        vec![scenario_named("recorded")],
        "the campaign runs the list the study declares"
    );
    let untouched =
        held_loop(&loop_definition, Some(&InputsSection::default())).expect("the loop as it is");
    assert_eq!(
        untouched, loop_definition,
        "without a recorded list, the loop stands"
    );
}

#[test]
fn the_declared_response_is_the_requirements_declared_objective_unless_automatic_generated() {
    assert_eq!(
        InputsSection::default().declared_response(),
        None,
        "no requirements, no declared response"
    );
    let declared = |objective: Option<TuningObjective>, generated: bool| InputsSection {
        requirements: Some(TuningRequirements {
            tuning_objective: objective,
            automatic_criteria: generated.then(|| AutomaticCriteria {
                model_revision: "sha256:model".to_owned(),
                fields: vec!["tuningObjective".to_owned()],
            }),
            ..requirements()
        }),
        ..InputsSection::default()
    };
    assert_eq!(
        declared(Some(TuningObjective::Fast), false).declared_response(),
        Some(TuneChoice::Faster)
    );
    assert_eq!(
        declared(Some(TuningObjective::Balanced), false).declared_response(),
        Some(TuneChoice::Balanced)
    );
    assert_eq!(
        declared(Some(TuningObjective::Robust), false).declared_response(),
        Some(TuneChoice::MoreRobust)
    );
    assert_eq!(
        declared(Some(TuningObjective::Robust), true).declared_response(),
        None,
        "a response Automatic generated is no declaration (B4.3)"
    );
    assert_eq!(declared(None, false).declared_response(), None);
}

#[test]
fn the_requirements_keep_no_tuning_method_of_their_own() {
    let mut with_method = requirements();
    with_method.tuning_method = Some(TuningMethod::Simc);
    let section = InputsSection {
        method: Some(TuneMethod::Simc),
        requirements: Some(with_method),
        ..InputsSection::default()
    }
    .without_requirement_method();
    assert_eq!(
        section.method,
        Some(TuneMethod::Simc),
        "the method input is kept"
    );
    assert_eq!(
        section
            .requirements
            .as_ref()
            .and_then(|value| value.tuning_method),
        None,
        "the requirement's copy of the method is dropped (A8.1)"
    );
    assert_eq!(
        section
            .requirements
            .as_ref()
            .map(|value| value.settling_time_seconds),
        Some(requirements().settling_time_seconds),
        "every other requirement is kept"
    );
}

#[test]
fn the_held_requirements_are_the_recorded_ones_and_none_without_a_record() {
    let recorded = InputsSection {
        requirements: Some(requirements()),
        ..InputsSection::default()
    };
    assert_eq!(
        held_requirements(Some(&recorded), None),
        Some(&requirements())
    );
    assert_eq!(
        held_requirements(Some(&InputsSection::default()), None),
        None
    );
    assert_eq!(held_requirements(None, None), None);
}
