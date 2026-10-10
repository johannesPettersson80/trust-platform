//! Tests of the model check's composition (`validate_model_check.rs`): when a check runs, the failure of a check that does not pass,
//! and the Identify result a passing check keeps. The live campaign is the identification's own, tested with its fakes, and the
//! real-runtime proof is the Validate test on lesson 30.

use serde_json::json;
use trust_control::experiment::ControlExperimentKind;
use trust_control::identification::model_comparison::ComparisonPresentation;
use trust_control::model::decode_general_model;
use trust_control::recording::EvidenceSource;
use trust_control::study::identify::{IdentifyMethod, IdentifySignals};
use trust_control::study::StudySection;

use super::*;

/// The estimation recording's identifier: `rec-` and 32 lowercase hex digits, as the study names recordings.
const ESTIMATION: &str = "rec-0123456789abcdef0123456789abcdef";
/// The check's recording identifier, distinct from the estimation recording's.
const VALIDATION: &str = "rec-fedcba9876543210fedcba9876543210";
/// The fingerprint a passing check adds to the model's evidence.
const FINGERPRINT: &str = "sha256:check";

/// The model of `storage_tests.rs` and `identify_action_tests.rs`: the first-order fit `1 / (s + 1)` from a command in V to a
/// position in m, not validated.
fn fitted_model() -> GeneralModel {
    let channel = |id: &str, unit: &str| json!({ "id": id, "unit": unit, "operatingRange": { "minimum": -10.0, "maximum": 10.0 } });
    decode_general_model(&json!({
        "kind": "transfer-function", "revision": "model:fitted:1", "source": "fitted",
        "validation": "unvalidated", "evidence": ["sha256:recording"], "delaySeconds": 0.0,
        "inputs": [channel("command", "V")], "outputs": [channel("position", "m")],
        "numerator": [1.0], "denominator": [1.0, 1.0]
    }))
    .expect("a fitted model")
}

/// What the page shows of the model: the presentation of `identify_action_tests.rs`, in the model's units.
fn presentation() -> ComparisonPresentation {
    trust_control::json::decode_value(json!({
        "equation": "G(s) = 1 / (1s + 1)",
        "units": { "input": "V", "output": "m", "gain": "m/V", "time": "s" },
        "parameters": [
            { "name": "Gain", "value": 1.0, "unit": "m/V" },
            { "name": "Time constant 1", "value": 1.0, "unit": "s" },
            { "name": "Dead time", "value": 0.0, "unit": "s" }
        ],
        "poles": [-1.0], "zeros": [], "delaySeconds": 0.0,
        "uncertainty": { "label": "Sample-resolution envelope from the estimation record; not statistical confidence" },
        "estimation": { "normalizedError": 0.1 }, "validation": {},
        "predictedResponse": [
            { "timeSeconds": 0.0, "measured": 0.0, "predicted": 0.0, "residual": 0.0 },
            { "timeSeconds": 2.0, "measured": 1.0, "predicted": 0.9, "residual": 0.1 }
        ],
        "validityRange": { "inputMinimum": 0.0, "inputMaximum": 1.0, "outputMinimum": 0.0, "outputMaximum": 1.0 },
        "warnings": []
    }))
    .expect("the presentation decodes")
}

fn signals() -> IdentifySignals {
    IdentifySignals {
        inputs: vec!["Main.Output".to_string()],
        outputs: vec!["Main.Measured".to_string()],
        setpoint: None,
    }
}

/// The Identify result of a live run on the simulated plant, which names `validation` as its validation recording when given one.
fn identified(validation: Option<&str>) -> IdentifySection {
    IdentifySection::new(
        IdentifyMethod::Live(ControlExperimentKind::SetpointStep),
        EvidenceSource::SimulatedPlant,
        signals(),
        ESTIMATION,
        validation,
        fitted_model(),
        presentation(),
    )
}

fn row(status: RequirementStatus, reason: &str) -> IndependentRow {
    IndependentRow {
        channel: "Main.Measured".to_string(),
        normalized_error: None,
        final_normalized_error: None,
        limit: 0.15,
        status,
        reason: reason.to_string(),
    }
}

fn whole_text(failure: &Failure) -> &str {
    failure.detail.as_deref().unwrap_or(&failure.message)
}

#[test]
fn a_provisional_model_is_checked_and_a_qualified_one_is_not() {
    assert!(check_is_needed(&identified(None)));
    assert!(!check_is_needed(&identified(Some(VALIDATION))));
}

#[test]
fn a_passing_check_validates_the_model_on_its_recording_and_keeps_the_rest_of_it() {
    let model = fitted_model();
    let qualified = qualified_model(&model, FINGERPRINT.to_string());
    let identity = qualified.identity();
    assert_eq!(identity.validation, ModelValidation::Validated);
    assert_eq!(
        identity.evidence,
        vec!["sha256:recording".to_string(), FINGERPRINT.to_string()]
    );
    assert_eq!(identity.revision, model.identity().revision);
    assert_eq!(identity.inputs, model.identity().inputs);
    assert_eq!(identity.outputs, model.identity().outputs);
}

#[test]
fn a_qualified_identify_result_names_its_check_recording_and_passes_the_study_check() {
    let qualified = qualified_section(&identified(None), VALIDATION, FINGERPRINT.to_string());
    assert_eq!(
        qualified.validation_recording_id.as_deref(),
        Some(VALIDATION)
    );
    assert!(!qualified.provisional);
    assert_eq!(qualified.estimation_recording_id, ESTIMATION);
    assert_eq!(qualified.check(), Ok(()));
}

#[test]
fn a_check_that_passes_gives_no_failure() {
    let rows = [row(RequirementStatus::Pass, "The response agrees.")];
    assert_eq!(check_failure(RequirementStatus::Pass, &rows), None);
}

#[test]
fn a_failing_check_stops_with_the_reason_of_its_first_row_that_did_not_pass() {
    let rows = [
        row(RequirementStatus::Pass, "The first output agrees."),
        row(RequirementStatus::Fail, "The model misses the response."),
        row(RequirementStatus::Fail, "The second output misses too."),
    ];
    let failure =
        check_failure(RequirementStatus::Fail, &rows).expect("a failed check stops Validate");
    assert_eq!(whole_text(&failure), "The model misses the response.");
    assert_eq!(failure.cause, Cause::ModelOrData);
    assert_eq!(failure.next_action, NextAction::Identify);
}

#[test]
fn an_inconclusive_check_stops_with_its_reason_too() {
    let rows = [row(
        RequirementStatus::Inconclusive,
        "The validation recording has no fresh excitation.",
    )];
    let failure = check_failure(RequirementStatus::Inconclusive, &rows)
        .expect("an inconclusive check stops Validate");
    assert_eq!(
        whole_text(&failure),
        "The validation recording has no fresh excitation."
    );
    assert_eq!(failure.next_action, NextAction::Identify);
}
