//! Tests of the independent model check (`independentModelValidation.ts`).

#![allow(clippy::cast_precision_loss)] // Sample indices are small counts, far below 2^53.

use super::{
    decode_independent_validation, recording_fingerprint, validate_frozen_model, ChannelIdentity,
    IndependentModelValidation, ModelValidationChannels, RequirementStatus,
};
use crate::identification::record_fixtures::{fitted_model, setpoint_record, Row, Verdict};

const CHANNELS: &[(&str, &str)] = &[("MV", "%"), ("PV", "degC")];

fn channels() -> ModelValidationChannels {
    ModelValidationChannels {
        inputs: vec![ChannelIdentity {
            id: "MV".to_owned(),
            symbol: "MV".to_owned(),
            unit: "%".to_owned(),
        }],
        outputs: vec![ChannelIdentity {
            id: "PV".to_owned(),
            symbol: "PV".to_owned(),
            unit: "degC".to_owned(),
        }],
    }
}

/// The response of `y' = −y + u` to a unit step of the input at 0.5 s, sampled every 0.1 s (40 samples). The input holds
/// its value between samples, so the response is exact at the samples.
fn first_order_rows() -> Vec<Row> {
    (0..40_u64)
        .map(|index| {
            let time = index as f64 * 0.1;
            let (input, output) = if index < 5 {
                (0.0, 0.0)
            } else {
                (1.0, 1.0 - (-(time - 0.5)).exp())
            };
            (time, index, vec![Some(input), Some(output)])
        })
        .collect()
}

/// A short record of zeros that the model was fitted from.
fn estimation_rows() -> Vec<Row> {
    (0..40_u64)
        .map(|index| (index as f64 * 0.1, index, vec![Some(0.0), Some(0.0)]))
        .collect()
}

#[test]
fn the_true_model_passes_on_an_independent_record() {
    let estimation = setpoint_record(
        "estimation",
        CHANNELS,
        &estimation_rows(),
        RequirementStatus::Pass,
    );
    let validation = setpoint_record(
        "validation",
        CHANNELS,
        &first_order_rows(),
        RequirementStatus::Pass,
    );
    let model = fitted_model(
        vec![recording_fingerprint(&estimation.run)],
        vec![1.0],
        vec![1.0, 1.0],
    );
    let report = validate_frozen_model(&model, Some(&estimation), &validation, &channels(), 0.15);
    assert_eq!(report.status, RequirementStatus::Pass);
    assert_eq!(report.rows.len(), 1);
    let row = &report.rows[0];
    assert_eq!(row.status, RequirementStatus::Pass);
    let whole = row.normalized_error.expect("the check was made");
    let final_part = row.final_normalized_error.expect("the check was made");
    assert!(
        whole <= 1e-6 && final_part <= 1e-6,
        "whole {whole} final {final_part}"
    );
    assert_eq!(report.predictions.len(), 1);
    assert_eq!(report.predictions[0].recording_id, "validation");
    assert_eq!(
        report.estimation_recording_id.as_deref(),
        Some("estimation")
    );
}

#[test]
fn a_model_with_the_wrong_gain_fails_the_whole_response() {
    let estimation = setpoint_record(
        "estimation",
        CHANNELS,
        &estimation_rows(),
        RequirementStatus::Pass,
    );
    let validation = setpoint_record(
        "validation",
        CHANNELS,
        &first_order_rows(),
        RequirementStatus::Pass,
    );
    // A gain of two predicts twice the measured response, so the whole-response error is far above 0.15.
    let model = fitted_model(
        vec![recording_fingerprint(&estimation.run)],
        vec![2.0],
        vec![1.0, 1.0],
    );
    let report = validate_frozen_model(&model, Some(&estimation), &validation, &channels(), 0.15);
    assert_eq!(report.status, RequirementStatus::Fail);
    assert_eq!(report.rows[0].status, RequirementStatus::Fail);
    assert!(
        report.rows[0]
            .reason
            .starts_with("The fitted model does not predict"),
        "{}",
        report.rows[0].reason
    );
}

#[test]
fn a_check_on_the_same_record_is_inconclusive_with_its_reason() {
    let estimation = setpoint_record(
        "same",
        CHANNELS,
        &estimation_rows(),
        RequirementStatus::Pass,
    );
    let model = fitted_model(
        vec![recording_fingerprint(&estimation.run)],
        vec![1.0],
        vec![1.0, 1.0],
    );
    let report = validate_frozen_model(&model, Some(&estimation), &estimation, &channels(), 0.15);
    assert_eq!(report.status, RequirementStatus::Inconclusive);
    assert_eq!(
        report.predictions,
        [] as [crate::identification::independent_validation::ValidationPredictionSeries; 0]
    );
    assert_eq!(report.rows.len(), 1);
    assert_eq!(report.rows[0].normalized_error, None);
    assert_eq!(
        report.rows[0].reason,
        "Model validation needs a distinct recording from the same plant/program and the original estimation evidence."
    );
}

#[test]
fn a_record_that_did_not_pass_its_evaluation_is_inconclusive() {
    let estimation = setpoint_record(
        "estimation",
        CHANNELS,
        &estimation_rows(),
        RequirementStatus::Pass,
    );
    let validation = setpoint_record(
        "validation",
        CHANNELS,
        &first_order_rows(),
        RequirementStatus::Fail,
    );
    let model = fitted_model(
        vec![recording_fingerprint(&estimation.run)],
        vec![1.0],
        vec![1.0, 1.0],
    );
    let report = validate_frozen_model(&model, Some(&estimation), &validation, &channels(), 0.15);
    assert_eq!(report.status, RequirementStatus::Inconclusive);
    assert_eq!(
        report.rows[0].reason,
        "Model validation requires a declared error limit and a complete bounded plant recording."
    );
}

#[test]
fn a_stored_check_is_kept_only_when_it_recomputes_exactly() {
    let estimation = setpoint_record(
        "estimation",
        CHANNELS,
        &estimation_rows(),
        RequirementStatus::Pass,
    );
    let validation = setpoint_record(
        "validation",
        CHANNELS,
        &first_order_rows(),
        RequirementStatus::Pass,
    );
    let model = fitted_model(
        vec![recording_fingerprint(&estimation.run)],
        vec![1.0],
        vec![1.0, 1.0],
    );
    let stored: IndependentModelValidation<Verdict> =
        validate_frozen_model(&model, Some(&estimation), &validation, &channels(), 0.15);
    let records = vec![estimation.clone(), validation.clone()];
    assert_eq!(
        decode_independent_validation(&stored, &model, &records),
        Some(stored.clone())
    );
    let mut forged = stored;
    forged.status = RequirementStatus::Fail;
    assert_eq!(
        decode_independent_validation(&forged, &model, &records),
        None,
        "a forged verdict is refused"
    );
}

#[test]
fn the_fingerprint_is_a_sha256_digest_of_the_run() {
    let record = setpoint_record(
        "fingerprint",
        CHANNELS,
        &estimation_rows(),
        RequirementStatus::Pass,
    );
    let first = recording_fingerprint(&record.run);
    assert!(first.starts_with("sha256:"));
    assert_eq!(first.len(), "sha256:".len() + 64);
    assert_eq!(
        first,
        recording_fingerprint(&record.run),
        "the same run has the same fingerprint"
    );
}
