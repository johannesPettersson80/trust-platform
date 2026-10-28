//! What the page shows of a fitted low-order model (`modelComparison.ts`; specification 35, B3.8 and A8.4; the Identify page
//! specification, "Identified model and plot"): the presentation of the fit with the traces of the experiment it was read
//! from. Trust-control builds it from the fit ([`comparison_presentation`]), and the study keeps it with its model. A stored
//! presentation is checked against its model when the study is read ([`presentation_describes_model`]).
//!
//! The pole-zero plane and the per-output fits of a state model (V7) are not held here, so a state model's presentation is not
//! described here.

use serde::{Deserialize, Serialize};

use crate::identification::evidence::{ModelFitEvidence, ModelFitPoint};
use crate::identification::experiment_trace::{
    decode_identification_experiment, IdentificationExperiment,
};
use crate::identification::plant::IdentifiedModel;
use crate::identification::presentation::{
    identified_model_presentation, IdentifiedModelPresentation,
};
use crate::model::{GeneralModel, ModelChannel, ModelIdentity, ModelSource};

/// The fewest points a predicted response may have.
const MINIMUM_PREDICTED_POINTS: usize = 2;

/// The most points a predicted response may have (`p.predictedResponse.length > 20000`).
const MAXIMUM_PREDICTED_POINTS: usize = 20_000;

/// A channel named by a presentation: its identifier and unit (`inputChannels`).
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct ChannelReference {
    /// The channel's identifier in the model.
    pub id: String,
    /// The channel's unit.
    pub unit: String,
}

/// What the page shows of a fitted model (`ComparisonPresentation`): the presentation of the fit, the experiment it was read
/// from, the output it shows, and the input of the model. The study stores it as the one presentation of its model.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ComparisonPresentation {
    /// The presentation of the fit.
    #[serde(flatten)]
    pub presentation: IdentifiedModelPresentation,
    /// The traces of the experiment the fit was read from, when it was read from one.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub experiment: Option<IdentificationExperiment>,
    /// The identifier of the output the presentation shows, when the model has several.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub output_id: Option<String>,
    /// The inputs of the model, when the presentation names them.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub input_channels: Option<Vec<ChannelReference>>,
}

/// The presentation of a single-input, single-output fit with the traces of the experiment it was read from (B3.8). The
/// presentation names the plant input and the measured output the page plots, with their units.
#[must_use]
pub fn comparison_presentation(
    experiment: IdentificationExperiment,
    input: &ModelChannel,
    output: &ModelChannel,
    low_order: &IdentifiedModel,
    fit: &ModelFitEvidence,
) -> ComparisonPresentation {
    ComparisonPresentation {
        presentation: identified_model_presentation(low_order, fit, &input.unit, &output.unit),
        experiment: Some(experiment),
        output_id: Some(output.id.clone()),
        input_channels: Some(vec![ChannelReference {
            id: input.id.clone(),
            unit: input.unit.clone(),
        }]),
    }
}

/// Whether a stored presentation describes `model` (`decodeModelComparison`): its units are the model's channels' units, its
/// time is in seconds, its delay and error metrics are not negative, its predicted response is a time series of 2 to 20,000
/// points, and its experiment's traces are the model's inputs and outputs, in order, with the same units. The model is
/// fitted (`ModelSource::Fitted`). The study keeps the presentation with its model, so no revision binds the two here.
#[must_use]
pub fn presentation_describes_model(
    presentation: &ComparisonPresentation,
    model: &GeneralModel,
) -> bool {
    let identity = model.identity();
    if identity.source != ModelSource::Fitted {
        return false;
    }
    let output = match presentation.output_id.as_deref() {
        Some(id) => identity.outputs.iter().find(|output| output.id == id),
        None => identity.outputs.first(),
    };
    let (Some(output), Some(input)) = (output, identity.inputs.first()) else {
        return false;
    };
    let shown = &presentation.presentation;
    shown.units.time == "s"
        && shown.units.input == input.unit
        && shown.units.output == output.unit
        && shown.delay_seconds >= 0.0
        && metrics_valid(shown.estimation.normalized_error)
        && metrics_valid(shown.validation.normalized_error)
        && predicted_response_valid(&shown.predicted_response)
        && presentation
            .experiment
            .as_ref()
            .is_none_or(|experiment| experiment_is_valid(experiment, identity))
}

/// The normalised error of a fit, when it has one, is zero or more. The lag-one correlation needs no check: the decoding
/// gives only finite numbers.
fn metrics_valid(normalized_error: Option<f64>) -> bool {
    normalized_error.is_none_or(|error| error >= 0.0)
}

/// Whether a predicted response is a series of 2 to 20,000 points with non-negative, strictly increasing times.
fn predicted_response_valid(points: &[ModelFitPoint]) -> bool {
    if !(MINIMUM_PREDICTED_POINTS..=MAXIMUM_PREDICTED_POINTS).contains(&points.len()) {
        return false;
    }
    let mut previous = f64::NEG_INFINITY;
    points.iter().all(|point| {
        let valid = point.time_seconds >= 0.0 && point.time_seconds > previous;
        previous = point.time_seconds;
        valid
    })
}

/// Whether an experiment's traces decode as the experiment format requires and are the model's channels.
fn experiment_is_valid(experiment: &IdentificationExperiment, identity: &ModelIdentity) -> bool {
    serde_json::to_value(experiment)
        .ok()
        .and_then(|value| decode_identification_experiment(&value))
        .is_some_and(|decoded| experiment_matches(&decoded, identity))
}

/// Whether the experiment's traces are the model's inputs and outputs, in order, with the same units.
fn experiment_matches(experiment: &IdentificationExperiment, identity: &ModelIdentity) -> bool {
    experiment.inputs.len() == identity.inputs.len()
        && experiment.outputs.len() == identity.outputs.len()
        && experiment
            .inputs
            .iter()
            .zip(&identity.inputs)
            .all(|(trace, channel)| trace.unit == channel.unit)
        && experiment
            .outputs
            .iter()
            .zip(&identity.outputs)
            .all(|(trace, channel)| trace.unit == channel.unit)
}

#[cfg(test)]
#[path = "model_comparison_tests.rs"]
mod tests;
