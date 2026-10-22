//! Tests of the inputs a study records (specification 35, A8.4, B4.2, B4.3 and B8).

use super::*;
use crate::failure::{Cause, NextAction};
use crate::identification::acquisition_settings::AcquisitionMethod;
use crate::identification::method_applicability::MethodRefusal;
use crate::identification::texts::method_refusal_text;
use crate::study::StudySection;
use crate::tuning::automatic_criteria::AutomaticCriteria;
use crate::tuning::objective::{TuningMethod, TuningObjective};
use crate::tuning::output_requirements::OutputRequirements;

fn requirements() -> TuningRequirements {
    TuningRequirements {
        channel_requirements: None,
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
fn the_overshoot_limits_of_the_task_and_of_each_output_survive_a_round_trip() {
    let output = |reference_change: f64, overshoot: f64| OutputRequirements {
        reference_change,
        maximum_error: 0.4,
        settling_band: 0.1,
        settling_time_seconds: 45.0,
        maximum_overshoot_percent: Some(overshoot),
    };
    let section = InputsSection {
        requirements: Some(TuningRequirements {
            maximum_overshoot_percent: Some(5.0),
            channel_requirements: Some(std::collections::BTreeMap::from([
                ("P1.PV".to_owned(), output(2.0, 5.0)),
                ("P2.PV".to_owned(), output(1.0, 12.5)),
            ])),
            ..requirements()
        }),
        ..InputsSection::default()
    };
    let decoded = InputsSection::decode(&section.encode().expect("encoded")).expect("decoded");
    let requirements = decoded.requirements.expect("the requirements come back");
    assert_eq!(
        requirements.maximum_overshoot_percent,
        Some(5.0),
        "the task's limit"
    );
    let outputs = requirements
        .channel_requirements
        .expect("each output's requirements come back");
    assert_eq!(outputs["P1.PV"].maximum_overshoot_percent, Some(5.0));
    assert_eq!(
        outputs["P2.PV"].maximum_overshoot_percent,
        Some(12.5),
        "each output keeps its own limit"
    );
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
    assert_eq!(refused.field.as_deref(), Some("maximumError"));
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

#[test]
fn an_invalidation_states_its_sentence() {
    // A8.4, B9.9: the Validate page says why a result went away in the engine's words, and the page writes none. A change that leaves the
    // validations current says nothing, and the wire form of that answer carries no `reason` key at all.
    let held = requirements();
    let mut changed = requirements();
    changed.settling_time_seconds = 50.0;
    let none = held_test(None, &[]);
    let invalidated = invalidation(
        &held_test(Some(&held), &[]),
        &held_test(Some(&changed), &[]),
    );
    assert!(invalidated.validation);
    assert_eq!(
        invalidated.reason,
        Some("Inputs changed. Tune and verify before applying.")
    );
    assert_eq!(
        serde_json::to_value(invalidated).expect("the invalidation encodes"),
        serde_json::json!({
            "validation": true,
            "reason": "Inputs changed. Tune and verify before applying."
        })
    );
    let unchanged = invalidation(&none, &none);
    assert!(!unchanged.validation);
    assert_eq!(unchanged.reason, None);
    assert_eq!(
        serde_json::to_value(unchanged).expect("the invalidation encodes"),
        serde_json::json!({ "validation": false })
    );
    // A sentence without an invalidation, or an invalidation without a sentence, would leave the page to invent one.
    assert_eq!(Invalidation::of(true).reason, Some(INPUTS_CHANGED));
    assert_eq!(Invalidation::of(false).reason, None);
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

#[test]
fn a_scenario_list_that_no_scenario_reads_is_refused_under_the_scenarios_field() {
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
    assert_eq!(refused.field.as_deref(), Some("scenarios"));
}

#[test]
fn a_scenario_event_after_the_validation_duration_is_refused_under_the_scenarios_field() {
    let mut test = requirements();
    test.duration_seconds = 2.0;
    let late: EngineerScenario = serde_json::from_value(serde_json::json!({
        "id": "late",
        "initialValues": {},
        "events": [{ "atMs": 3000, "symbol": "Main.SP", "value": 3 }],
        "requirements": [{ "id": "r", "kind": "bounds", "channel": "Main.PV",
            "window": { "start": 0, "end": 1 }, "minimum": 0, "maximum": 5 }]
    }))
    .expect("a scenario in the stored form");
    let refused =
        check_within_duration(&test, &[late]).expect_err("the event is written after the run");
    assert_eq!(refused.field.as_deref(), Some("scenarios"));
    assert_eq!(
        refused.message,
        "Scenario late writes an event after the validation duration of 2 s."
    );
    assert!(check_within_duration(&test, &[scenario_named("early")]).is_ok());
}

/// A declared scenario that writes one event at `at_ms`.
fn scenario_with_event_at(id: &str, at_ms: f64) -> EngineerScenario {
    serde_json::from_value(serde_json::json!({
        "id": id,
        "initialValues": {},
        "events": [{ "atMs": at_ms, "symbol": "Main.SP", "value": 3 }],
        "requirements": [{ "id": "r", "kind": "bounds", "channel": "Main.PV",
            "window": { "start": 0, "end": 1 }, "minimum": 0, "maximum": 5 }]
    }))
    .expect("a scenario in the stored form")
}

#[test]
fn a_scenario_event_between_two_task_samples_is_refused_under_the_scenarios_field() {
    // B4.3: Validate writes an event at a sample of the loop's task, so the change that gives the event is refused when its time is
    // not a whole number of task periods.
    let refused = check_events_on_grid(&[scenario_with_event_at("late-event", 150.0)], 100.0)
        .expect_err("150 ms lies between the samples of a 100 ms task");
    assert_eq!(refused.field.as_deref(), Some("scenarios"));
    assert_eq!(
        refused.message,
        "Scenario late-event writes an event at 150 ms, which is not a multiple of the 100 ms task period."
    );
    for on_the_grid in [0.0, 100.0, 1_500.0] {
        assert!(
            check_events_on_grid(&[scenario_with_event_at("on", on_the_grid)], 100.0).is_ok(),
            "{on_the_grid} ms is a sample"
        );
    }
    let second = check_events_on_grid(
        &[
            scenario_with_event_at("first", 200.0),
            scenario_with_event_at("second", 250.0),
        ],
        100.0,
    )
    .expect_err("the second scenario's event is off the grid");
    assert!(
        second.message.starts_with("Scenario second "),
        "the refusal names the scenario that holds the event"
    );
}

#[test]
fn no_event_is_checked_against_a_task_period_that_validate_refuses() {
    // The campaign refuses a task period that is not a whole number of milliseconds with its own sentence (B4.3), so the scenarios'
    // events are not judged against it.
    let odd = [scenario_with_event_at("odd", 150.0)];
    for period in [2.5, 0.0, -100.0, f64::NAN] {
        assert!(check_events_on_grid(&odd, period).is_ok(), "{period}");
    }
}

#[test]
fn a_change_that_invalidates_the_validation_leaves_no_current_result_in_the_evidence() {
    let passed = StageEvidence {
        loop_bound: true,
        has_model: true,
        has_declared_analysis: false,
        has_manual_parameters: false,
        has_candidate: true,
        has_recording: true,
        identification_failed: false,
        validation_has_result: true,
        validation_passed: true,
        validation_inconclusive: false,
        controller_stages_apply: true,
    };
    let invalidated = evidence_after_invalidation(passed, Invalidation::of(true));
    assert!(
        !invalidated.validation_has_result,
        "the result the change left is not current"
    );
    assert!(!invalidated.validation_passed);
    assert!(!invalidated.validation_inconclusive);
    assert!(
        invalidated.has_model && invalidated.has_candidate,
        "the other evidence is kept"
    );
    assert_eq!(
        evidence_after_invalidation(passed, Invalidation::of(false)),
        passed
    );
}
