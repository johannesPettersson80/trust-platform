//! Tests of the task record's decoder (specification 35, B4.3, A11.2; `TypeScript` `engineerVerificationPlan.ts`): the required and the optional
//! values of the task, the sentence and the field of each refusal, the declared overshoot limit, and the record of each controlled output.
//! They were part of `engineer_verification_plan_tests.rs`, which keeps the plan.

#![allow(clippy::float_cmp)] // The fixtures are exact decimal inputs, and the requirements are compared as the TypeScript tests state them.

use serde_json::json;

use super::test_support::{base_task, text};
use super::*;
use crate::tuning::automatic_criteria::REQUIREMENT_KEYS;

/// The task record as the decoder reads it: the base values as JSON.
fn base_json() -> Value {
    json!({ "referenceChange": 1, "maximumError": 0.2, "settlingBand": 0.1,
        "settlingTimeSeconds": 15, "durationSeconds": 30 })
}

/// The task record with a declared overshoot limit, as the decoder reads it.
fn decoded_overshoot(value: Value) -> Result<Option<f64>, Failure> {
    let mut task = base_json();
    task["maximumOvershootPercent"] = value;
    engineer_requirements(&task).map(|task| task.maximum_overshoot_percent)
}

#[test]
fn the_task_decodes_its_required_values_and_its_optional_ones() {
    let mut value = base_json();
    value["transferTolerance"] = json!(0);
    value["tuningMethod"] = json!("SIMC");
    value["tuningObjective"] = json!("robust");
    value["constantInputDisturbance"] = json!(0.3);
    value["measurementNoiseBand"] = json!(0);
    value["gainUncertaintyFraction"] = json!(1);
    let task = engineer_requirements(&value).expect("a complete task");
    assert_eq!(
        (
            task.reference_change,
            task.maximum_error,
            task.settling_band
        ),
        (1.0, 0.2, 0.1)
    );
    assert_eq!(
        (task.settling_time_seconds, task.duration_seconds),
        (15.0, 30.0)
    );
    assert_eq!(
        (
            task.transfer_tolerance,
            task.constant_input_disturbance,
            task.measurement_noise_band,
            task.gain_uncertainty_fraction
        ),
        (Some(0.0), Some(0.3), Some(0.0), Some(1.0))
    );
    assert_eq!(
        (task.tuning_method, task.tuning_objective),
        (Some(TuningMethod::Simc), Some(TuningObjective::Robust))
    );
}

#[test]
fn a_missing_or_mistyped_required_value_is_refused() {
    let mut value = base_json();
    value
        .as_object_mut()
        .expect("an object")
        .remove("referenceChange");
    let failure = engineer_requirements(&value).expect_err("a missing target change");
    assert_eq!(text(&failure), "Enter finite task requirements.");
    value["referenceChange"] = json!("1");
    let failure = engineer_requirements(&value).expect_err("a text target change");
    assert_eq!(text(&failure), "Enter finite task requirements.");
    assert_eq!(
        (failure.cause, failure.next_action),
        (Cause::Settings, NextAction::OpenRequirements)
    );
}

#[test]
fn each_refused_task_value_names_its_own_sentence_and_field() {
    // The requirements page's sentences (B4.3): each refused value has its own sentence, shown under the field it names.
    let cases = [
        (
            "settlingBand",
            json!(0.2),
            "maximumError",
            "The error limit must be greater than the settling band.",
        ),
        (
            "referenceChange",
            json!(0),
            "referenceChange",
            "Enter a nonzero setpoint change.",
        ),
        (
            "durationSeconds",
            json!(86_401),
            "durationSeconds",
            "Use 86400 s or less.",
        ),
    ];
    for (key, value, field, sentence) in cases {
        let mut task = base_json();
        task[key] = value;
        let failure = engineer_requirements(&task).expect_err(key);
        assert_eq!(text(&failure), sentence, "{key}");
        assert_eq!(failure.field.as_deref(), Some(field), "{key}");
    }
}

#[test]
fn a_typed_validation_duration_reads_as_its_seconds() {
    let mut value = base_json();
    value["durationSeconds"] = json!("1 h 30 min");
    let task = engineer_requirements(&value).expect("a typed duration is read");
    assert_eq!(task.duration_seconds, 5400.0);
}

#[test]
fn an_unreadable_validation_duration_is_refused_under_its_field() {
    let mut value = base_json();
    value["durationSeconds"] = json!("12 m");
    let failure = engineer_requirements(&value).expect_err("no duration reads");
    assert_eq!(
        text(&failure),
        "The validation duration must be a time such as 90 s, 15 min or 1 h 30 min."
    );
    assert_eq!(failure.field.as_deref(), Some("durationSeconds"));
    assert_eq!(
        (failure.cause, failure.next_action),
        (Cause::Settings, NextAction::OpenRequirements)
    );
}

#[test]
fn an_invalid_optional_value_is_refused_with_its_own_sentence() {
    const BOUNDS: &str =
        "Enter finite disturbance/noise bounds and gain uncertainty between zero and one.";
    let cases = [
        (
            "transferTolerance",
            json!(-1),
            "Auto-entry tolerance must be blank for automatic or a finite nonnegative value in output units.",
        ),
        ("tuningMethod", json!("LAMBDA"), "Choose a supported tuning method."),
        ("tuningObjective", json!("best"), "Choose a supported tuning objective."),
        ("constantInputDisturbance", json!(null), BOUNDS),
        ("measurementNoiseBand", json!(-0.1), BOUNDS),
        ("gainUncertaintyFraction", json!(1.5), BOUNDS),
    ];
    for (key, value, expected) in cases {
        let mut task = base_json();
        task[key] = value;
        let failure = engineer_requirements(&task).expect_err(key);
        assert_eq!(text(&failure), expected, "{key}");
    }
}

#[test]
fn a_declared_overshoot_limit_is_kept_and_a_blank_one_declares_none() {
    // control-studio-overshoot-requirement.test.ts: "parses a declared overshoot limit and leaves a blank or absent one undeclared".
    assert_eq!(
        engineer_requirements(&base_json())
            .expect("an absent limit")
            .maximum_overshoot_percent,
        None
    );
    assert_eq!(decoded_overshoot(json!("  ")).expect("a blank limit"), None);
    assert_eq!(decoded_overshoot(json!(null)).expect("a null limit"), None);
    assert_eq!(decoded_overshoot(json!(5)).expect("a 5 % limit"), Some(5.0));
    assert_eq!(
        decoded_overshoot(json!(1000)).expect("the largest limit"),
        Some(1000.0),
        "1000 % is the largest permitted limit"
    );
}

#[test]
fn an_invalid_overshoot_limit_is_refused_with_its_permitted_range() {
    // control-studio-overshoot-requirement.test.ts: "refuses an invalid overshoot limit and names the field and its permitted range".
    const SENTENCE: &str = "Maximum overshoot must be blank for no limit, or a finite value above 0 and at most 1000 % of the setpoint change.";
    // A limit of zero or less is refused under its own field, as the requirements page names it (B4.3).
    const OVERSHOOT_LIMIT: &str =
        "Enter an overshoot limit above 0 %, or leave it blank for no limit.";
    for value in [json!(0), json!(-1)] {
        let failure = decoded_overshoot(value.clone()).expect_err(&value.to_string());
        assert_eq!(text(&failure), OVERSHOOT_LIMIT, "refuses {value}");
        assert_eq!(
            failure.field.as_deref(),
            Some("maximumOvershootPercent"),
            "names the field of {value}"
        );
    }
    for value in [json!(1000.5), json!("5")] {
        let failure = decoded_overshoot(value.clone()).expect_err(&value.to_string());
        assert_eq!(text(&failure), SENTENCE, "refuses {value}");
    }
    // NaN and the infinities cannot be written as JSON; the record's own check refuses them.
    let mut task = base_task();
    task.maximum_overshoot_percent = Some(f64::NAN);
    assert_eq!(task.validate().expect_err("NaN").0, SENTENCE);
    task.maximum_overshoot_percent = Some(f64::INFINITY);
    assert_eq!(task.validate().expect_err("infinity").0, SENTENCE);
}

#[test]
fn automatic_criteria_never_generate_an_overshoot_limit() {
    // control-studio-overshoot-requirement.test.ts: "automatic criteria never generate an overshoot limit".
    assert!(
        REQUIREMENT_KEYS
            .iter()
            .all(|key| !key.to_lowercase().contains("overshoot")),
        "automatic setup fills no overshoot limit"
    );
    let mut task = base_json();
    task["automaticCriteria"] =
        json!({ "modelRevision": "r1", "fields": ["maximumOvershootPercent"] });
    let failure = engineer_requirements(&task).expect_err("a stored overshoot claim");
    assert_eq!(text(&failure), "Invalid automatic test criteria.");
    task["automaticCriteria"] = json!({ "modelRevision": "r1", "fields": ["maximumError"] });
    assert_eq!(
        engineer_requirements(&task)
            .expect("a generated error limit")
            .automatic_criteria,
        Some(AutomaticCriteria {
            model_revision: "r1".to_owned(),
            fields: vec!["maximumError".to_owned()],
        })
    );
}

#[test]
fn each_controlled_output_keeps_its_own_record_in_the_task() {
    let mut value = base_json();
    value["channelRequirements"] = json!({
        "P1.Zone2C": { "referenceChange": 2.0, "maximumError": 0.5, "settlingBand": 0.2, "settlingTimeSeconds": 10.0 }
    });
    let task = engineer_requirements(&value).expect("the task with its second output");
    let own = task
        .channel_requirements
        .as_ref()
        .and_then(|channels| channels.get("P1.Zone2C"))
        .expect("the second output's record");
    assert_eq!(
        (
            own.reference_change,
            own.maximum_error,
            own.settling_band,
            own.settling_time_seconds
        ),
        (2.0, 0.5, 0.2, 10.0)
    );
}

#[test]
fn a_malformed_second_output_is_refused_under_its_field_and_channel() {
    let mut value = base_json();
    value["channelRequirements"] = json!({
        "P1.Zone2C": { "referenceChange": 2.0, "maximumError": 0.5, "settlingBand": 0.6, "settlingTimeSeconds": 10.0 }
    });
    let failure =
        engineer_requirements(&value).expect_err("the second band is above its error limit");
    assert_eq!(
        text(&failure),
        "The error limit must be greater than the settling band."
    );
    assert_eq!(failure.field.as_deref(), Some("maximumError:P1.Zone2C"));
    assert_eq!(failure.next_action, NextAction::OpenRequirements);
}

#[test]
fn a_second_output_record_that_is_not_numbers_is_refused_with_its_sentence() {
    let mut value = base_json();
    value["channelRequirements"] = json!({ "P1.Zone2C": { "referenceChange": "2" } });
    let failure = engineer_requirements(&value).expect_err("a text setpoint change");
    assert_eq!(
        text(&failure),
        "The control requirements of a controlled output are not valid."
    );
}
