//! The records an automatic identification returns (`automaticIdentification.ts`), and the noise that a requirement reads
//! from a recording (`automaticRequirementEvidence.ts`; specification 35, B3.4, B3.9 and A6.4).
//!
//! A campaign keeps its recordings; an automatic identification gives either an accepted model (two independent records,
//! B5.2) or a provisional model from one record, with the fit that the page shows (`GeneralIdentificationFit`, a pick of
//! `IdentifiedStateModelResult` from `identifiedStateModel.ts`, which moves with V7).

use serde::{Deserialize, Serialize};

use crate::experiment::experiment_start_seconds;
use crate::identification::connected_record::{
    RuntimeVerificationResult, ValidationSample, ValidationValue,
};
use crate::identification::independent_validation::ValidationPredictionSeries;
use crate::identification::model_comparison::ComparisonPresentation;
use crate::identification::review::AcceptedIdentificationModel;
use crate::model::GeneralModel;
use crate::recording::channels::ChannelQuality;

/// The status of an identification campaign (`IdentificationCampaignRecord.status`).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum CampaignStatus {
    /// The campaign is still recording.
    Running,
    /// The campaign finished and its recordings are complete.
    Completed,
    /// The campaign failed; its reason says why.
    Failed,
    /// The campaign was cancelled by the engineer.
    Cancelled,
}

/// A campaign and its recordings (`IdentificationCampaignRecord`). Only the fields an automatic identification reads.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct IdentificationCampaignRecord<E> {
    /// The status of the campaign.
    pub status: CampaignStatus,
    /// Why the campaign failed or was cancelled, when it did.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub reason: Option<String>,
    /// The recordings the campaign took, in order.
    pub runs: Vec<RuntimeVerificationResult<E>>,
}

/// One comparison of a fit (`comparisons` of `IdentifiedStateModelResult`; from `identifiedStateModel.ts`; moves with V7).
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct FitComparison {
    /// The order of the model that was compared (its denominator's degree).
    pub order: usize,
    /// The normalised error of each output, when the fit gave one.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub normalized_errors: Option<Vec<f64>>,
    /// The sentence that says why the comparison is or is not adequate.
    pub reason: String,
    /// Whether the comparison meets the error limit.
    pub adequate: bool,
}

/// The fit of a recorded model that the page shows (`GeneralIdentificationFit`, a pick of `IdentifiedStateModelResult`
/// from `identifiedStateModel.ts`). This layer produces the single-record fits; the state fit (V7) fills the same record.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct GeneralIdentificationFit {
    /// The comparisons the fit made.
    pub comparisons: Vec<FitComparison>,
    /// The independent predictions, empty until an independent record exists.
    pub validation_predictions: Vec<ValidationPredictionSeries>,
    /// The predictions on the estimation record.
    pub estimation_predictions: Vec<ValidationPredictionSeries>,
}

/// The result of an automatic identification. Either the model is accepted on two independent records, or it is a model
/// from one record (provisional, with its fit), kept with the records it came from.
#[derive(Debug, Clone, PartialEq)]
pub enum AutomaticIdentificationResult<E> {
    /// The model is accepted: the estimation and the independent validation both support it (B5.2).
    Accepted {
        /// The accepted model with its evidence and review.
        accepted: Box<AcceptedIdentificationModel>,
        /// The records the model was fitted from.
        runs: Vec<RuntimeVerificationResult<E>>,
        /// What the page shows of the model, with the traces of the validation record (B3.8).
        presentation: Box<ComparisonPresentation>,
    },
    /// A model from the recordings, not yet validated independently when it came from one record.
    Model {
        /// The model.
        model: Box<GeneralModel>,
        /// The records the model was fitted from.
        runs: Vec<RuntimeVerificationResult<E>>,
        /// The fit, when the model was fitted from a recording here.
        fit: Option<GeneralIdentificationFit>,
        /// What the page shows of the model, with the traces of the record it was fitted from (B3.8).
        presentation: Box<ComparisonPresentation>,
    },
    /// A state-space model of several plant inputs or measured outputs (C7, C8), kept with its records and without a
    /// comparison presentation (B10.2).
    StateModel {
        /// The state-space model.
        model: Box<GeneralModel>,
        /// The records the model was fitted from.
        runs: Vec<RuntimeVerificationResult<E>>,
        /// The fit the model was made with.
        fit: GeneralIdentificationFit,
    },
}

impl<E> AutomaticIdentificationResult<E> {
    /// What the page shows of the model (B3.8), however it was identified: the study keeps this with the model. A state-space
    /// model has none (B10.2).
    #[must_use]
    pub fn presentation(&self) -> Option<&ComparisonPresentation> {
        match self {
            Self::Accepted { presentation, .. } | Self::Model { presentation, .. } => {
                Some(presentation.as_ref())
            }
            Self::StateModel { .. } => None,
        }
    }
}

/// The good, finite value a channel holds at a sample: good quality and a number (the rule `recordedValues` and the noise
/// read).
pub(crate) fn good_value(value: &ValidationValue) -> Option<f64> {
    let number = value.value?;
    (matches!(value.quality, ChannelQuality::Good) && number.is_finite()).then_some(number)
}

/// The good, finite values a recording holds for one signal, in time order (`recordedValues`). Empty when there is no
/// recording.
#[must_use]
pub fn recorded_values<E>(record: Option<&RuntimeVerificationResult<E>>, symbol: &str) -> Vec<f64> {
    record.map_or_else(Vec::new, |record| {
        record
            .run
            .samples
            .iter()
            .filter_map(|sample| good_value(sample.values.get(symbol)?))
            .collect()
    })
}

/// The noise of an output over its steady baseline (`baselineNoise`, B3.4): three standard deviations of the output about
/// a least-squares line, over the samples before the first input change. The plant transient and the wait before the
/// steady window never count. Zero when the recording has too few baseline samples or no input change to bound them.
#[must_use]
pub fn baseline_noise<E>(
    record: Option<&RuntimeVerificationResult<E>>,
    input_symbols: &[&str],
    output_symbol: &str,
) -> f64 {
    let Some(record) = record else {
        return 0.0;
    };
    let start = experiment_start_seconds(record.request.baseline_start_seconds);
    let samples: Vec<&ValidationSample> = record
        .run
        .samples
        .iter()
        .filter(|sample| sample.time_seconds >= start - 1e-9)
        .collect();
    if samples.len() < 8 {
        return 0.0;
    }
    let first = samples[0];
    let Some(changed) = samples.iter().position(|sample| {
        input_symbols
            .iter()
            .any(|symbol| input_changed(first, sample, symbol))
    }) else {
        return 0.0;
    };
    if changed < 8 {
        return 0.0;
    }
    let outputs: Option<Vec<f64>> = samples[..changed]
        .iter()
        .map(|sample| sample.values.get(output_symbol).and_then(good_value))
        .collect();
    let Some(outputs) = outputs else {
        return 0.0;
    };
    let count = count_f64(outputs.len());
    let mean_x = (count - 1.0) / 2.0;
    let mean_y = outputs.iter().sum::<f64>() / count;
    let slope_numerator: f64 = outputs
        .iter()
        .enumerate()
        .map(|(index, y)| (count_f64(index) - mean_x) * (y - mean_y))
        .sum();
    let slope_denominator: f64 = (0..outputs.len())
        .map(|index| (count_f64(index) - mean_x).powi(2))
        .sum();
    let slope = slope_numerator / slope_denominator;
    let residual: f64 = outputs
        .iter()
        .enumerate()
        .map(|(index, y)| (y - mean_y - slope * (count_f64(index) - mean_x)).powi(2))
        .sum();
    let divisor = count_f64(outputs.len().saturating_sub(2).max(1));
    3.0 * (residual / divisor).sqrt()
}

/// Whether an input has moved from its first value at a sample (`baselineNoise`): a value that is missing or not good
/// counts as a change, and a difference above the larger of `10⁻⁹` and `10⁻⁷` of the first value does too.
fn input_changed(first: &ValidationSample, sample: &ValidationSample, symbol: &str) -> bool {
    let (Some(start), Some(current)) = (first.values.get(symbol), sample.values.get(symbol)) else {
        return true;
    };
    if !matches!(start.quality, ChannelQuality::Good)
        || !matches!(current.quality, ChannelQuality::Good)
    {
        return true;
    }
    let (Some(start_value), Some(current_value)) = (start.value, current.value) else {
        return true;
    };
    (current_value - start_value).abs() > (start_value.abs() * 1e-7).max(1e-9)
}

/// A count as a float. The counts here are sample counts, far below 2^53, so the conversion is exact.
#[allow(clippy::cast_precision_loss)]
fn count_f64(count: usize) -> f64 {
    count as f64
}

#[cfg(test)]
#[path = "automatic_tests.rs"]
mod tests;
