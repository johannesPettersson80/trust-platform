//! The merge of a page change into the study's inputs (A8.1, A8.4; B4.2 to B4.4): one test per rule of `apply_change`.

use std::collections::BTreeMap;

use serde_json::{json, Map, Value};

use super::{apply_change, NOT_NUMBERS, OUTPUTS_INVALID};
use crate::failure::{Cause, NextAction};
use crate::messages::TuneMethod;
use crate::study::inputs::InputsSection;
use crate::study::StudySection;
use crate::tuning::automatic_criteria::AutomaticCriteria;
use crate::tuning::objective::TuningObjective;
use crate::tuning::output_requirements::OutputRequirements;
use crate::tuning::requirements::TuningRequirements;
use crate::tuning::state_controller::StateControllerFamily;
use crate::tuning::structure::Structure;
use crate::tuning::texts;

/// The loop revision the pins of a change belong to.
const REVISION: &str = "sha256:loop-revision";

/// The page's change as the object the request carries.
fn change(value: Value) -> Map<String, Value> {
    match value {
        Value::Object(map) => map,
        other => panic!("a change is a JSON object, not {other}"),
    }
}

/// The requirements record of a change, with its scenario list when `scenarios` is given.
fn requirements(scenarios: Option<Value>) -> Map<String, Value> {
    let mut record = change(json!({
        "referenceChange": 2.0,
        "maximumError": 0.4,
        "settlingBand": 0.1,
        "settlingTimeSeconds": 45.0,
        "durationSeconds": 90.0,
    }));
    if let Some(scenarios) = scenarios {
        record.insert("scenarioDefinitions".to_owned(), scenarios);
    }
    record
}

#[test]
fn acquisition_replaces_the_recorded_settings_and_null_clears_them() {
    let settings = json!({
        "method": "automatic",
        "amplitude": 0.0,
        "durationSeconds": 30.0,
        "reuseRecordings": false,
    });
    let set = apply_change(
        &InputsSection::default(),
        &change(json!({ "acquisitionSettings": settings })),
        REVISION,
    )
    .expect("the page's settings are accepted");
    assert_eq!(
        set.acquisition
            .as_ref()
            .map(|acquisition| acquisition.duration_seconds),
        Some(30.0)
    );
    let cleared = apply_change(
        &set,
        &change(json!({ "acquisitionSettings": null })),
        REVISION,
    )
    .expect("null clears the settings");
    assert_eq!(cleared.acquisition, None);
}

#[test]
fn a_family_sets_the_structure_and_any_other_word_clears_it() {
    let set = apply_change(
        &InputsSection::default(),
        &change(json!({ "family": "PI" })),
        REVISION,
    )
    .expect("a structure is accepted");
    assert_eq!(set.structure, Some(Structure::PI));
    let automatic = apply_change(&set, &change(json!({ "family": "automatic" })), REVISION)
        .expect("Automatic clears the structure");
    assert_eq!(automatic.structure, None);
    let null = apply_change(&set, &change(json!({ "family": null })), REVISION)
        .expect("a null family clears the structure");
    assert_eq!(null.structure, None);
}

#[test]
fn a_state_law_family_sets_the_state_design_family_too() {
    let law = apply_change(
        &InputsSection::default(),
        &change(json!({ "family": "LQR" })),
        REVISION,
    )
    .expect("a state law is accepted");
    assert_eq!(law.structure, None);
    assert_eq!(
        law.state_design.and_then(|design| design.family),
        Some(StateControllerFamily::Lqr)
    );
}

#[test]
fn manual_settings_are_kept_only_with_the_manual_method() {
    let manual = apply_change(
        &InputsSection::default(),
        &change(
            json!({ "tuningPreferences": { "method": "manual", "values": { "Main.Kp": 1.2 } } }),
        ),
        REVISION,
    )
    .expect("the manual settings are accepted");
    assert_eq!(manual.method, Some(TuneMethod::Manual));
    assert_eq!(
        manual.settings,
        Some(BTreeMap::from([("Main.Kp".to_owned(), 1.2)]))
    );
    let simc = apply_change(
        &manual,
        &change(json!({ "tuningPreferences": { "method": "SIMC", "values": { "Main.Kp": 1.2 } } })),
        REVISION,
    )
    .expect("the formula method takes none");
    assert_eq!(simc.method, Some(TuneMethod::Simc));
    assert_eq!(simc.settings, None);
}

#[test]
fn the_state_draft_is_kept_only_with_the_state_space_method() {
    let draft = json!({ "desiredPoles": "[-1, -2]", "solverMode": "finite" });
    let formula = apply_change(
        &InputsSection::default(),
        &change(json!({ "tuningPreferences": { "method": "SIMC", "stateDesignDraft": draft } })),
        REVISION,
    )
    .expect("a formula method is accepted");
    assert_eq!(formula.state_design, None);
    let state = apply_change(
        &InputsSection::default(),
        &change(
            json!({ "tuningPreferences": { "method": "state-space", "stateDesignDraft": draft } }),
        ),
        REVISION,
    )
    .expect("the state draft is accepted");
    assert_eq!(
        state
            .state_design
            .and_then(|design| design.draft)
            .and_then(|draft| draft.desired_poles),
        Some("[-1, -2]".to_owned())
    );
}

#[test]
fn pins_take_the_family_of_the_structure_the_change_sets_or_the_recorded_one() {
    let fixed = apply_change(
        &InputsSection::default(),
        &change(json!({ "family": "PI", "pinnedValues": { "Main.Kp": 2.0 } })),
        REVISION,
    )
    .expect("pins with a family are accepted");
    let pins = fixed.pins.clone().expect("the pins are recorded");
    assert_eq!(pins.family, Structure::PI);
    assert_eq!(pins.loop_revision, REVISION);
    assert_eq!(pins.values, BTreeMap::from([("Main.Kp".to_owned(), 2.0)]));
    let later = apply_change(
        &fixed,
        &change(json!({ "pinnedValues": { "Main.Kp": 3.0 } })),
        REVISION,
    )
    .expect("pins without a family keep the recorded family");
    assert_eq!(later.pins.map(|pins| pins.family), Some(Structure::PI));
}

#[test]
fn null_pins_clear_the_pins() {
    let fixed = apply_change(
        &InputsSection::default(),
        &change(json!({ "family": "PID", "pinnedValues": { "Main.Kp": 2.0 } })),
        REVISION,
    )
    .expect("pins with a family are accepted");
    let cleared = apply_change(&fixed, &change(json!({ "pinnedValues": null })), REVISION)
        .expect("null clears the pins");
    assert_eq!(cleared.pins, None);
}

#[test]
fn pins_with_no_family_to_belong_to_record_nothing_and_keep_the_recorded_pins() {
    let recorded = apply_change(
        &InputsSection::default(),
        &change(json!({ "family": "PI", "pinnedValues": { "Main.Kp": 2.0 } })),
        REVISION,
    )
    .expect("pins with a family are accepted");
    let automatic = apply_change(
        &recorded,
        &change(json!({ "family": "automatic" })),
        REVISION,
    )
    .expect("Automatic clears the structure");
    let next = apply_change(
        &automatic,
        &change(json!({ "pinnedValues": { "Main.Kp": 5.0 } })),
        REVISION,
    )
    .expect("pins without a family are not refused");
    assert_eq!(next.structure, None);
    assert_eq!(next.pins, recorded.pins);
}

#[test]
fn state_space_pins_go_to_the_state_design_and_null_clears_them() {
    let state = apply_change(
        &InputsSection::default(),
        &change(json!({
            "tuningPreferences": { "method": "state-space" },
            "pinnedValues": { "K1": 0.5 },
        })),
        REVISION,
    )
    .expect("the state pins are accepted");
    assert_eq!(state.pins, None);
    assert_eq!(
        state
            .state_design
            .as_ref()
            .and_then(|design| design.pins.clone()),
        Some(BTreeMap::from([("K1".to_owned(), 0.5)]))
    );
    let cleared = apply_change(&state, &change(json!({ "pinnedValues": null })), REVISION)
        .expect("null clears the state pins");
    assert_eq!(cleared.state_design.and_then(|design| design.pins), None);
}

#[test]
fn requirements_with_scenarios_record_both_and_without_them_keep_the_scenarios() {
    let declared = apply_change(
        &InputsSection::default(),
        &change(Value::Object(requirements(Some(json!([]))))),
        REVISION,
    )
    .expect("the requirements and an empty scenario list are accepted");
    assert_eq!(
        declared
            .requirements
            .as_ref()
            .map(|record| record.reference_change),
        Some(2.0)
    );
    assert_eq!(declared.scenarios, Some(Vec::new()));
    let kept = apply_change(
        &declared,
        &change(Value::Object(requirements(None))),
        REVISION,
    )
    .expect("the requirements without scenarios are accepted");
    assert_eq!(kept.scenarios, Some(Vec::new()));
}

#[test]
fn a_typed_validation_duration_is_recorded_as_its_seconds_and_an_unreadable_one_is_refused() {
    let mut typed = requirements(None);
    typed.insert("durationSeconds".to_owned(), json!("1 h 30 min"));
    let recorded = apply_change(&InputsSection::default(), &typed, REVISION)
        .expect("a typed duration is read");
    assert_eq!(
        recorded
            .requirements
            .as_ref()
            .map(|record| record.duration_seconds),
        Some(5400.0)
    );
    let mut unreadable = requirements(None);
    unreadable.insert("durationSeconds".to_owned(), json!("12 m"));
    let refused = apply_change(&InputsSection::default(), &unreadable, REVISION)
        .expect_err("no duration reads");
    assert_eq!(refused.field.as_deref(), Some("durationSeconds"));
    assert_eq!(
        refused.message,
        "The validation duration must be a time such as 90 s, 15 min or 1 h 30 min."
    );
}

#[test]
fn a_channel_mapping_and_a_binding_change_change_no_input() {
    let recorded = apply_change(
        &InputsSection::default(),
        &change(json!({ "family": "PID", "tuningPreferences": { "method": "SIMC" } })),
        REVISION,
    )
    .expect("the inputs are accepted");
    for named in [
        json!({ "channelMapping": { "inputs": [] } }),
        json!({ "bindingInvalid": true, "bindingInputs": [], "bindingOutputs": [] }),
    ] {
        assert_eq!(
            apply_change(&recorded, &change(named), REVISION)
                .expect("a key that names no input is accepted"),
            recorded
        );
    }
}

#[test]
fn a_change_that_names_the_recorded_inputs_leaves_them_equal() {
    let recorded = apply_change(
        &InputsSection::default(),
        &change(json!({ "family": "P", "tuningPreferences": { "method": "AMIGO" } })),
        REVISION,
    )
    .expect("the inputs are accepted");
    let same = apply_change(
        &recorded,
        &change(json!({ "family": "P", "tuningPreferences": { "method": "AMIGO" } })),
        REVISION,
    )
    .expect("the same inputs are accepted");
    assert_eq!(same, recorded);
    assert_eq!(
        apply_change(&recorded, &Map::new(), REVISION).expect("an empty change is accepted"),
        recorded
    );
}

#[test]
fn an_unknown_tuning_method_is_refused_with_the_engine_sentence() {
    let refused = apply_change(
        &InputsSection::default(),
        &change(json!({ "tuningPreferences": { "method": "MRAC" } })),
        REVISION,
    )
    .expect_err("the engine runs no MRAC");
    assert_eq!(refused.message, texts::METHOD_NOT_RUN);
}

#[test]
fn an_unknown_key_is_refused_by_name() {
    let refused = apply_change(
        &InputsSection::default(),
        &change(json!({ "gainSchedule": 1 })),
        REVISION,
    )
    .expect_err("the study takes no gainSchedule input");
    assert_eq!(
        refused.message,
        "The change names an input that the study does not take: gainSchedule."
    );
}

#[test]
fn a_requirement_without_the_mark_of_its_record_is_refused_by_name() {
    let refused = apply_change(
        &InputsSection::default(),
        &change(json!({ "maximumError": 0.4 })),
        REVISION,
    )
    .expect_err("a requirement is recorded with its referenceChange");
    assert_eq!(
        refused.message,
        "maximumError belongs to the requirements, which the change marks with referenceChange."
    );
}

#[test]
fn clearing_the_state_pins_keeps_the_state_draft_as_typed() {
    let draft = json!({ "desiredPoles": "[0.9" });
    let recorded = apply_change(
        &InputsSection::default(),
        &change(json!({
            "family": "LQG",
            "tuningPreferences": { "method": "state-space", "stateDesignDraft": draft },
            "pinnedValues": { "P1.L1": 0.5 },
        })),
        REVISION,
    )
    .expect("the state law, its draft and its pins are accepted");
    let cleared = apply_change(
        &recorded,
        &change(json!({
            "family": "LQG",
            "tuningPreferences": { "method": "state-space", "stateDesignDraft": draft },
            "pinnedValues": null,
        })),
        REVISION,
    )
    .expect("null clears the state pins and keeps the draft");
    let design = cleared.state_design.expect("the state design is kept");
    assert_eq!(design.family, Some(StateControllerFamily::Lqg));
    assert_eq!(design.pins, None);
    assert_eq!(
        design.draft.and_then(|draft| draft.desired_poles),
        Some("[0.9".to_owned())
    );
}

/// The rows the requirements editor typed for one output: its channel and its values as the engineer wrote them.
fn typed_output(channel: &str, reference_change: &str, band: &str) -> Value {
    json!({
        "channel": channel,
        "referenceChange": reference_change,
        "maximumError": "0.4",
        "settlingBand": band,
        "settlingTimeSeconds": "45 s",
    })
}

/// The values of an output's rows, as the record holds them.
fn output_values(
    reference_change: f64,
    maximum_error: f64,
    settling_band: f64,
    settling_time_seconds: f64,
    maximum_overshoot_percent: Option<f64>,
) -> OutputRequirements {
    OutputRequirements {
        reference_change,
        maximum_error,
        settling_band,
        settling_time_seconds,
        maximum_overshoot_percent,
    }
}

#[test]
fn the_rows_typed_for_one_output_are_the_records_own_values_and_text_is_read_as_numbers() {
    let mut first = typed_output("output-1", "2", "0.1");
    first["maximumOvershootPercent"] = json!("10");
    let recorded = apply_change(
        &InputsSection::default(),
        &change(json!({
            "outputRequirements": [first],
            "durationSeconds": "1 min",
            "constantInputDisturbance": "3",
            "transferTolerance": "0.5",
        })),
        REVISION,
    )
    .expect("the typed rows are assembled");
    assert_eq!(
        recorded.requirements,
        Some(TuningRequirements {
            reference_change: 2.0,
            maximum_error: 0.4,
            settling_band: 0.1,
            settling_time_seconds: 45.0,
            duration_seconds: 60.0,
            constant_input_disturbance: Some(3.0),
            measurement_noise_band: None,
            gain_uncertainty_fraction: None,
            transfer_tolerance: Some(0.5),
            maximum_overshoot_percent: Some(10.0),
            tuning_method: None,
            tuning_objective: None,
            automatic_criteria: None,
            channel_requirements: None,
        }),
        "one output keeps no record under its channel"
    );
}

#[test]
fn the_rows_typed_for_several_outputs_make_each_outputs_record_the_firsts_copy_and_the_shared_values(
) {
    let second = json!({
        "channel": "output-2",
        "referenceChange": "-3",
        "maximumError": 0.8,
        "settlingBand": "0.2",
        "settlingTimeSeconds": 30,
        "maximumOvershootPercent": "5",
    });
    let recorded = apply_change(
        &InputsSection::default(),
        &change(json!({
            "outputRequirements": [typed_output("output-1", "2", "0.1"), second],
            "durationSeconds": "2 min",
            "measurementNoiseBand": "0.05",
            "gainUncertaintyFraction": "0.1",
            "tuningObjective": "balanced",
            "automaticCriteria": { "modelRevision": "model-1", "fields": ["settlingBand:output-2"] },
            "scenarioDefinitions": [],
        })),
        REVISION,
    )
    .expect("the typed rows are assembled");
    let first = output_values(2.0, 0.4, 0.1, 45.0, None);
    let second = output_values(-3.0, 0.8, 0.2, 30.0, Some(5.0));
    assert_eq!(
        recorded.requirements,
        Some(TuningRequirements {
            reference_change: 2.0,
            maximum_error: 0.4,
            settling_band: 0.1,
            settling_time_seconds: 45.0,
            duration_seconds: 120.0,
            constant_input_disturbance: None,
            measurement_noise_band: Some(0.05),
            gain_uncertainty_fraction: Some(0.1),
            transfer_tolerance: None,
            maximum_overshoot_percent: None,
            tuning_method: None,
            tuning_objective: Some(TuningObjective::Balanced),
            automatic_criteria: Some(AutomaticCriteria {
                model_revision: "model-1".to_owned(),
                fields: vec!["settlingBand:output-2".to_owned()],
            }),
            channel_requirements: Some(BTreeMap::from([
                ("output-1".to_owned(), first),
                ("output-2".to_owned(), second),
            ])),
        }),
        "the record's own values are the first output's copy, and each output's rows are kept under its channel"
    );
    assert_eq!(
        recorded
            .requirements
            .as_ref()
            .map(OutputRequirements::of_record),
        Some(first),
        "the first output's copy is its rows"
    );
    assert_eq!(recorded.scenarios, Some(Vec::new()));
    recorded
        .check()
        .expect("the assembled record holds its rules");
}

#[test]
fn a_value_of_an_output_is_refused_under_its_field_with_its_channel() {
    // The second output's band is negative: the record is assembled, and the section refuses it under that output's row.
    let several = apply_change(
        &InputsSection::default(),
        &change(json!({
            "outputRequirements": [
                typed_output("output-1", "2", "0.1"),
                typed_output("output-2", "3", "-0.1"),
            ],
            "durationSeconds": "2 min",
        })),
        REVISION,
    )
    .expect("the typed rows are assembled; the section refuses them when it is stored");
    let refused = several.check().expect_err("a band below zero");
    assert_eq!(refused.field.as_deref(), Some("settlingBand:output-2"));
    assert_eq!(refused.message, "Enter a positive settling band.");
    assert_eq!(refused.next_action, NextAction::OpenRequirements);
    // The first output's own refusal names its channel too, and a single output's names the plain field.
    let first = apply_change(
        &InputsSection::default(),
        &change(json!({
            "outputRequirements": [
                typed_output("output-1", "0", "0.1"),
                typed_output("output-2", "3", "0.1"),
            ],
            "durationSeconds": "2 min",
        })),
        REVISION,
    )
    .expect("the typed rows are assembled");
    assert_eq!(
        first
            .check()
            .expect_err("no setpoint change")
            .field
            .as_deref(),
        Some("referenceChange:output-1")
    );
    let single = apply_change(
        &InputsSection::default(),
        &change(json!({
            "outputRequirements": [typed_output("output-1", "2", "0")],
            "durationSeconds": "2 min",
        })),
        REVISION,
    )
    .expect("the typed rows are assembled");
    assert_eq!(
        single.check().expect_err("no band").field.as_deref(),
        Some("settlingBand")
    );
}

#[test]
fn a_blank_optional_row_declares_no_value() {
    let mut first = typed_output("output-1", "2", "0.1");
    first["maximumOvershootPercent"] = json!("  ");
    let recorded = apply_change(
        &InputsSection::default(),
        &change(json!({
            "outputRequirements": [first],
            "durationSeconds": 120,
            "constantInputDisturbance": "",
            "transferTolerance": null,
            "measurementNoiseBand": "0.05",
        })),
        REVISION,
    )
    .expect("the typed rows are assembled");
    let record = recorded.requirements.expect("the record");
    assert_eq!(record.maximum_overshoot_percent, None);
    assert_eq!(record.constant_input_disturbance, None);
    assert_eq!(record.transfer_tolerance, None);
    assert_eq!(record.measurement_noise_band, Some(0.05));
}

#[test]
fn typed_outputs_that_are_not_a_list_of_distinct_channels_with_their_rows_are_refused() {
    let output = || typed_output("output-1", "2", "0.1");
    let invalid = [
        json!({ "outputRequirements": {} }),
        json!({ "outputRequirements": [] }),
        json!({ "outputRequirements": ["output-1"] }),
        json!({ "outputRequirements": [{ "referenceChange": "2" }] }),
        json!({ "outputRequirements": [typed_output("", "2", "0.1")] }),
        json!({ "outputRequirements": [output(), output()] }),
        json!({ "outputRequirements": [{ "channel": "output-1", "gain": 1 }] }),
        json!({ "outputRequirements": [output()], "referenceChange": 2 }),
        json!({ "outputRequirements": [output()], "channelRequirements": {} }),
    ];
    for typed in invalid {
        let refused = apply_change(&InputsSection::default(), &change(typed.clone()), REVISION)
            .expect_err("the typed outputs break a rule");
        assert_eq!(refused.message, OUTPUTS_INVALID, "{typed}");
        assert_eq!(refused.cause, Cause::Settings, "{typed}");
        assert_eq!(refused.next_action, NextAction::OpenRequirements, "{typed}");
    }
}

#[test]
fn the_typed_rows_mark_the_requirements_record_and_carry_its_scenarios() {
    let recorded = apply_change(
        &InputsSection::default(),
        &change(json!({
            "outputRequirements": [typed_output("output-1", "2", "0.1")],
            "durationSeconds": 90,
            "scenarioDefinitions": [],
        })),
        REVISION,
    )
    .expect("the typed rows mark the record");
    assert_eq!(recorded.scenarios, Some(Vec::new()));
    // A shared value without either mark is still refused by name.
    let refused = apply_change(
        &InputsSection::default(),
        &change(json!({ "durationSeconds": 90 })),
        REVISION,
    )
    .expect_err("a requirement is recorded with its mark");
    assert_eq!(
        refused.message,
        "durationSeconds belongs to the requirements, which the change marks with referenceChange."
    );
}

#[test]
fn the_rows_of_the_record_are_read_as_text_too_and_a_text_no_number_reads_is_refused() {
    let numbers = apply_change(&InputsSection::default(), &requirements(None), REVISION)
        .expect("the numbers are accepted");
    let mut typed = requirements(None);
    for (key, text) in [
        ("referenceChange", "2"),
        ("maximumError", "0.4"),
        ("settlingBand", "0.1"),
        ("settlingTimeSeconds", "45 s"),
        ("durationSeconds", "90 s"),
        ("maximumOvershootPercent", ""),
    ] {
        typed.insert(key.to_owned(), json!(text));
    }
    let text = apply_change(&InputsSection::default(), &typed, REVISION)
        .expect("the same rows as text are accepted");
    assert_eq!(text, numbers, "text is read as the number it names");
    typed.insert("maximumError".to_owned(), json!("0,4"));
    let refused =
        apply_change(&InputsSection::default(), &typed, REVISION).expect_err("no number reads 0,4");
    assert_eq!(refused.cause, Cause::Settings);
    assert_eq!(refused.next_action, NextAction::OpenRequirements);
    assert_eq!(refused.field.as_deref(), Some("maximumError"));
    assert_eq!(refused.message, "Enter the error limit as a number.");
}

#[test]
fn a_row_of_plain_numbers_that_no_number_reads_is_refused_under_its_own_field() {
    // A blank required row, and each row that is typed but is no number: the record's own rows and the rows that the outputs share.
    for (row, text, sentence) in [
        ("maximumError", "", "Enter the error limit as a number."),
        (
            "referenceChange",
            "two",
            "Enter the setpoint change as a number.",
        ),
        (
            "maximumOvershootPercent",
            "5 %",
            "Enter the overshoot limit as a number, or leave it blank for no limit.",
        ),
        (
            "constantInputDisturbance",
            "a lot",
            "Enter the input disturbance as a number, or leave it blank for none.",
        ),
        (
            "transferTolerance",
            "0,5",
            "Enter the Auto-entry tolerance as a number, or leave it blank.",
        ),
    ] {
        let mut rows = requirements(None);
        rows.insert(row.to_owned(), json!(text));
        let refused = apply_change(&InputsSection::default(), &rows, REVISION).expect_err(row);
        assert_eq!(refused.field.as_deref(), Some(row), "{row}");
        assert_eq!(refused.message, sentence, "{row}");
        assert_eq!(refused.cause, Cause::Settings, "{row}");
        assert_eq!(refused.next_action, NextAction::OpenRequirements, "{row}");
    }
    // One of several outputs names its channel, as the requirements page names that output's row; a single output names the plain row.
    let several = apply_change(
        &InputsSection::default(),
        &change(json!({
            "outputRequirements": [
                typed_output("output-1", "2", "0.1"),
                typed_output("output-2", "3", ""),
            ],
            "durationSeconds": "2 min",
        })),
        REVISION,
    )
    .expect_err("the second output's band is blank");
    assert_eq!(several.field.as_deref(), Some("settlingBand:output-2"));
    assert_eq!(several.message, "Enter the settling band as a number.");
    let single = apply_change(
        &InputsSection::default(),
        &change(json!({
            "outputRequirements": [typed_output("output-1", "2", "")],
            "durationSeconds": "2 min",
        })),
        REVISION,
    )
    .expect_err("the only output's band is blank");
    assert_eq!(single.field.as_deref(), Some("settlingBand"));
}

#[test]
fn manual_settings_are_read_as_typed_and_an_entry_no_number_reads_declares_no_setting_yet() {
    let recorded = apply_change(
        &InputsSection::default(),
        &change(
            json!({ "tuningPreferences": { "method": "manual", "values": {
            "Main.Kp": "1.25", "Main.Ki": 0.5, "Main.Kd": "", "Main.Tf": "1,5",
        } } }),
        ),
        REVISION,
    )
    .expect("typed settings are accepted");
    assert_eq!(
        recorded.settings,
        Some(BTreeMap::from([
            ("Main.Kp".to_owned(), 1.25),
            ("Main.Ki".to_owned(), 0.5)
        ])),
        "text that a number reads is that number; a blank entry and text that no number reads are left out, and Tune refuses them by symbol"
    );
    let refused = apply_change(
        &InputsSection::default(),
        &change(json!({ "tuningPreferences": { "method": "manual", "values": [1.2] } })),
        REVISION,
    )
    .expect_err("the settings are an object of entries");
    assert_eq!(refused.message, NOT_NUMBERS);
    assert_eq!(refused.cause, Cause::Settings);
}
