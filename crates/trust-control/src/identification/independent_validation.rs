//! The independent model check (`independentModelValidation.ts`; specification 35, B5.2 and B5.5; numerics N3.5). The
//! frozen model is run over a second recording, never refitted, and its free-running prediction is scored against the
//! measured outputs: the error over the whole response and over its final 10 % must both be within the limit.
//!
//! The record of the check is a `RuntimeVerificationResult` (`connected_record`). Its evaluation is the caller's
//! (stage V3), so the check reads only the evaluation's verdict through [`RunEvaluation`]. The per-record reason
//! sentences are in `texts`; the arithmetic of the score is the library's.

use std::fmt::Write as _;

use schemars::JsonSchema;
use serde::{Deserialize, Serialize};
use serde_json::Value;
use sha2::{Digest, Sha256};
use trust_control_math::identify::score::{score_independent_response, ResponsePoint, ScoreStatus};
use trust_control_math::simulate::FrozenPrediction;

use crate::identification::connected_record::{
    RuntimeVerificationResult, ValidationChannel, ValidationRun, ValidationValue,
};
use crate::identification::evidence::ModelFitPoint;
use crate::identification::refusal::Refusal;
use crate::identification::texts::{even_sampling_text, independent_response_reason};
use crate::model::{frozen_model_prediction, GeneralModel};
use crate::recording::channels::ChannelQuality;
use crate::recording::even_sampling::{
    evenly_sampled, ChannelReading, EvenlySampled, TimedRecordedScan,
};
use crate::signals::Quality;
use crate::study::canonical_json;

/// The verdict of a requirement or a check (`RequirementStatus` in `runValidation.ts`; moves with V3).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
pub enum RequirementStatus {
    /// The requirement holds.
    Pass,
    /// The requirement does not hold.
    Fail,
    /// The requirement could not be checked.
    Inconclusive,
}

/// The evaluation of a connected run, as the independent check reads it: its verdict (`evaluation.status`). The
/// evaluation itself is the caller's; the production one is the limits check of `recorded_limits`.
pub trait RunEvaluation {
    /// The verdict of the evaluation.
    fn status(&self) -> RequirementStatus;

    /// What the evaluation found against the run, in plain words, when it does not pass (A11.1): the signal, the time and
    /// the values involved. An evaluation that gives no finding keeps the default, `None`.
    fn finding(&self) -> Option<&str> {
        None
    }
}

/// A channel of a model check: its identifier in the model, the signal's symbol in the record, and its unit.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "camelCase")]
pub struct ChannelIdentity {
    /// The channel's identifier in the model.
    pub id: String,
    /// The signal's symbol in the record.
    pub symbol: String,
    /// The channel's unit.
    pub unit: String,
}

/// The inputs and outputs a model check compares, in the model's order (`ModelValidationChannels`).
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, JsonSchema)]
pub struct ModelValidationChannels {
    /// The plant inputs, in the model's order.
    pub inputs: Vec<ChannelIdentity>,
    /// The measured outputs, in the model's order.
    pub outputs: Vec<ChannelIdentity>,
}

/// One measured output's check (`rows` of the `TypeScript`): its errors over the whole response and over its final 10 %,
/// the limit, the verdict and its reason. An inconclusive check has no errors.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct IndependentRow {
    /// The signal's symbol.
    pub channel: String,
    /// The error over the whole response, absent when the check could not be made.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub normalized_error: Option<f64>,
    /// The error over the final 10 % of the response, absent when the check could not be made.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub final_normalized_error: Option<f64>,
    /// The limit both errors must be within (B5.2).
    pub limit: f64,
    /// Pass when both errors are within the limit.
    pub status: RequirementStatus,
    /// The sentence that says what passed or what to change.
    pub reason: String,
}

/// The predicted and measured response of one output on the independent recording (`ValidationPredictionSeries`; from
/// `identifiedStateModel.ts`; moves with V7).
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "camelCase")]
pub struct ValidationPredictionSeries {
    /// The output's channel identifier in the model.
    pub channel_id: String,
    /// The output's unit.
    pub unit: String,
    /// The identifier of the independent recording.
    pub recording_id: String,
    /// The revision of the model that predicted the response.
    pub model_revision: String,
    /// The measured and predicted values at each sample, in time order.
    pub points: Vec<ModelFitPoint>,
}

/// The result of an independent model check (`IndependentModelValidation`). The record is kept with the result, so a
/// stored check can be recomputed and compared (see [`decode_independent_validation`]).
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct IndependentModelValidation<E> {
    /// The revision of the model that was checked.
    pub model_revision: String,
    /// The identifier of the recording the model was estimated from; absent for a supplied model, which has none (B5.2).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub estimation_recording_id: Option<String>,
    /// The channels the check compares.
    pub channels: ModelValidationChannels,
    /// The error limit of the check, above zero and below one (B5.2).
    pub maximum_normalized_error: f64,
    /// The independent record the check ran on.
    pub record: RuntimeVerificationResult<E>,
    /// Pass when every output passes; Inconclusive when the check could not be made.
    pub status: RequirementStatus,
    /// One row per measured output.
    pub rows: Vec<IndependentRow>,
    /// The predictions of the frozen model, one series per output.
    pub predictions: Vec<ValidationPredictionSeries>,
}

/// The SHA-256 digest of a value's canonical JSON, as `sha256:` and its hex (`digest` in the `TypeScript`). Canonical JSON
/// sorts the keys, so the digest does not depend on how the value was built.
#[must_use]
pub(crate) fn digest(value: &Value) -> String {
    let digest = Sha256::digest(canonical_json(value).as_bytes());
    let mut text = String::from("sha256:");
    for byte in &digest {
        let _ = write!(text, "{byte:02x}");
    }
    text
}

/// The fingerprint of a recording (`recordingFingerprint`): the digest of the run. A model keeps the fingerprints of the
/// recordings it was fitted from, so the same run always gives the same evidence.
#[must_use]
pub fn recording_fingerprint(run: &ValidationRun) -> String {
    digest(&serde_json::to_value(run).unwrap_or(Value::Null))
}

/// Checks a frozen model on an independent record (`validateFrozenModel`). The model's coefficients and parameter pins
/// never change (B5.2). `estimation` is the recording a fitted model was estimated from, and `None` for a supplied model, which
/// has none. A refused check is returned as an inconclusive result whose rows carry the reason, never as an error (B5.5).
#[must_use]
pub fn validate_frozen_model<E: RunEvaluation + Clone>(
    model: &GeneralModel,
    estimation: Option<&RuntimeVerificationResult<E>>,
    record: &RuntimeVerificationResult<E>,
    channels: &ModelValidationChannels,
    maximum_normalized_error: f64,
) -> IndependentModelValidation<E> {
    let model_revision = model.identity().revision.clone();
    let outcome = run_check(
        model,
        estimation,
        record,
        channels,
        maximum_normalized_error,
    );
    let (status, rows, predictions) = match outcome {
        Ok(passed) => {
            let status = if passed
                .rows
                .iter()
                .all(|row| row.status == RequirementStatus::Pass)
            {
                RequirementStatus::Pass
            } else {
                RequirementStatus::Fail
            };
            (status, passed.rows, passed.predictions)
        }
        Err(refusal) => (
            RequirementStatus::Inconclusive,
            channels
                .outputs
                .iter()
                .map(|channel| IndependentRow {
                    channel: channel.symbol.clone(),
                    normalized_error: None,
                    final_normalized_error: None,
                    limit: maximum_normalized_error,
                    status: RequirementStatus::Inconclusive,
                    reason: refusal.0.clone(),
                })
                .collect(),
            Vec::new(),
        ),
    };
    IndependentModelValidation {
        model_revision,
        estimation_recording_id: estimation.map(|estimation| estimation.run.id.clone()),
        channels: channels.clone(),
        maximum_normalized_error,
        record: record.clone(),
        status,
        rows,
        predictions,
    }
}

/// Recomputes a stored check from the model and the recordings it names, and returns the stored check only when the
/// recomputation gives exactly the same result (`decodeIndependentModelValidation`). A forged verdict is refused. The
/// record's own decoding (its structure and evaluation) is the caller's, through `decode_connected_identification_record`.
#[must_use]
pub fn decode_independent_validation<E: RunEvaluation + Clone + PartialEq>(
    stored: &IndependentModelValidation<E>,
    model: &GeneralModel,
    records: &[RuntimeVerificationResult<E>],
) -> Option<IndependentModelValidation<E>> {
    let estimation = match &stored.estimation_recording_id {
        Some(id) => Some(records.iter().find(|item| item.run.id == *id)?),
        None => None,
    };
    let recomputed = validate_frozen_model(
        model,
        estimation,
        &stored.record,
        &stored.channels,
        stored.maximum_normalized_error,
    );
    (recomputed == *stored).then(|| stored.clone())
}

/// The checked parts of a check that ran to its end.
struct Passed {
    rows: Vec<IndependentRow>,
    predictions: Vec<ValidationPredictionSeries>,
}

/// The check itself. Each refusal is the sentence the `TypeScript` threw.
fn run_check<E: RunEvaluation>(
    model: &GeneralModel,
    estimation: Option<&RuntimeVerificationResult<E>>,
    record: &RuntimeVerificationResult<E>,
    channels: &ModelValidationChannels,
    limit: f64,
) -> Result<Passed, Refusal> {
    // A fitted model is checked against the recording it was estimated from, which must be distinct, from the same program, and
    // named by the model's evidence. A supplied model has no estimation recording, so none of these rules applies to it (B5.2).
    if let Some(estimation) = estimation {
        let evidence = &model.identity().evidence;
        if !evidence.contains(&recording_fingerprint(&estimation.run))
            || estimation.run.id == record.run.id
            || estimation.project_revision != record.project_revision
            || estimation.run.conditions.model_revision != record.run.conditions.model_revision
        {
            return Err(Refusal::new(
                "Model validation needs a distinct recording from the same plant/program and the original estimation evidence.",
            ));
        }
    }
    if !limit.is_finite()
        || limit <= 0.0
        || limit >= 1.0
        || record.evaluation.status() != RequirementStatus::Pass
    {
        return Err(Refusal::new(
            "Model validation requires a declared error limit and a complete bounded plant recording.",
        ));
    }
    check_channels(model, record, channels)?;
    // The comparison grid holds the plant inputs and interpolates the measured outputs between scans (A4.3).
    let sampled = even_scans(&record.run, channels)?;
    let inputs = qualified_values(&sampled, &record.run.channels, &channels.inputs)?;
    let outputs = qualified_values(&sampled, &record.run.channels, &channels.outputs)?;
    if channels.inputs.iter().enumerate().any(|(index, _)| {
        let column: Vec<f64> = inputs
            .iter()
            .filter_map(|row| row.get(index).copied())
            .collect();
        let largest = column.iter().copied().fold(f64::NEG_INFINITY, f64::max);
        let smallest = column.iter().copied().fold(f64::INFINITY, f64::min);
        largest - smallest <= 1e-9
    }) {
        return Err(Refusal::new(
            "Every selected input needs fresh excitation for independent model validation.",
        ));
    }
    let predicted = frozen_model_prediction(model, &inputs, &outputs, sampled.step_seconds)
        .map_err(|error| Refusal::new(error.to_string()))?;
    let predictions = predicted_series(model, record, channels, &sampled, &outputs, &predicted);
    let rows = predictions
        .iter()
        .zip(&channels.outputs)
        .map(|(series, channel)| {
            let response: Vec<ResponsePoint> = series
                .points
                .iter()
                .map(|point| ResponsePoint {
                    measured: point.measured,
                    residual: point.residual,
                })
                .collect();
            let score = score_independent_response(&response, limit);
            IndependentRow {
                channel: channel.symbol.clone(),
                normalized_error: Some(score.normalized_error),
                final_normalized_error: Some(score.final_normalized_error),
                limit,
                status: if score.status == ScoreStatus::Pass {
                    RequirementStatus::Pass
                } else {
                    RequirementStatus::Fail
                },
                reason: independent_response_reason(&score).to_owned(),
            }
        })
        .collect();
    Ok(Passed { rows, predictions })
}

/// The channels of the check must be the model's, with the same identifiers and units, and each must be in the record.
fn check_channels<E>(
    model: &GeneralModel,
    record: &RuntimeVerificationResult<E>,
    channels: &ModelValidationChannels,
) -> Result<(), Refusal> {
    let identity = model.identity();
    let pairs = [
        (&channels.inputs, &identity.inputs),
        (&channels.outputs, &identity.outputs),
    ];
    for (selected, model_channels) in pairs {
        let matches = selected.len() == model_channels.len()
            && selected.iter().zip(model_channels).all(|(channel, own)| {
                channel.id == own.id
                    && channel.unit == own.unit
                    && record
                        .run
                        .channels
                        .iter()
                        .any(|entry| entry.id == channel.symbol && entry.unit == channel.unit)
            });
        if !matches {
            return Err(Refusal::new(
                "Model validation channel identities or units differ.",
            ));
        }
    }
    Ok(())
}

/// The record's scans on their even grid, with the outputs interpolated between scans (A4.3). Each scan holds one reading
/// per channel of the run, in the run's order.
fn even_scans(
    run: &ValidationRun,
    channels: &ModelValidationChannels,
) -> Result<EvenlySampled, Refusal> {
    let scans: Vec<TimedRecordedScan> = run
        .samples
        .iter()
        .map(|sample| TimedRecordedScan {
            time_seconds: sample.time_seconds,
            sequence: sample.sequence,
            readings: run
                .channels
                .iter()
                .map(|channel| reading(sample.values.get(&channel.id)))
                .collect(),
        })
        .collect();
    let linear: Vec<usize> = channels
        .outputs
        .iter()
        .filter_map(|output| {
            run.channels
                .iter()
                .position(|entry| entry.id == output.symbol)
        })
        .collect();
    evenly_sampled(&scans, None, &linear).map_err(|error| Refusal::new(even_sampling_text(error)))
}

/// One channel's reading of a sample. A missing value is a bad reading.
fn reading(value: Option<&ValidationValue>) -> ChannelReading {
    match value {
        Some(value) => ChannelReading {
            value: value.value,
            quality: match value.quality {
                ChannelQuality::Good => Quality::Good,
                ChannelQuality::Forced => Quality::Forced,
                ChannelQuality::Bad => Quality::Bad,
            },
        },
        None => ChannelReading {
            value: None,
            quality: Quality::Bad,
        },
    }
}

/// The values of the selected channels at every scan, scan by scan. Each must be good and finite, or the record is
/// unqualified (the sentence the `TypeScript` threw).
fn qualified_values(
    sampled: &EvenlySampled,
    channels: &[ValidationChannel],
    selected: &[ChannelIdentity],
) -> Result<Vec<Vec<f64>>, Refusal> {
    let indices: Vec<usize> = selected
        .iter()
        .filter_map(|channel| channels.iter().position(|entry| entry.id == channel.symbol))
        .collect();
    sampled
        .scans
        .iter()
        .map(|scan| {
            indices
                .iter()
                .map(|&index| match scan.readings.get(index) {
                    Some(ChannelReading {
                        value: Some(value),
                        quality: Quality::Good,
                    }) if value.is_finite() => Ok(*value),
                    _ => Err(Refusal::new(
                        "The independent recording contains unqualified measurements.",
                    )),
                })
                .collect::<Result<Vec<f64>, Refusal>>()
        })
        .collect()
}

/// The predicted response of each output, from the first predicted sample to the end of the record.
fn predicted_series<E>(
    model: &GeneralModel,
    record: &RuntimeVerificationResult<E>,
    channels: &ModelValidationChannels,
    sampled: &EvenlySampled,
    outputs: &[Vec<f64>],
    predicted: &FrozenPrediction,
) -> Vec<ValidationPredictionSeries> {
    channels
        .outputs
        .iter()
        .enumerate()
        .map(|(column, channel)| ValidationPredictionSeries {
            channel_id: channel.id.clone(),
            unit: channel.unit.clone(),
            recording_id: record.run.id.clone(),
            model_revision: model.identity().revision.clone(),
            // The prediction covers the record from its start to the end (`frozen_model_prediction`), so every index exists.
            points: predicted
                .values
                .iter()
                .enumerate()
                .filter_map(|(offset, row)| {
                    let index = offset + predicted.start;
                    let time_seconds = sampled.scans.get(index)?.time_seconds;
                    let measured = *outputs.get(index)?.get(column)?;
                    let predicted_value = *row.get(column)?;
                    Some(ModelFitPoint {
                        time_seconds,
                        measured,
                        predicted: predicted_value,
                        residual: measured - predicted_value,
                    })
                })
                .collect(),
        })
        .collect()
}

#[cfg(test)]
#[path = "independent_validation_tests.rs"]
mod tests;
