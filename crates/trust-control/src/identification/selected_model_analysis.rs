//! The plant model that identification hands to the rest of Control Studio (`selectedModelAnalysis.ts`; specification 35,
//! A10 and B5.2). An accepted identification becomes a validated transfer model with its independent record identities;
//! a provisional identification from one record becomes an unvalidated one. Both keep the exact transport delay and the
//! operating point of the fit, and both carry a revision that changes with any part of the model.
//!
//! `selectedModelAnalysis` (the connection to the declared controller's analysis) imports `declaredAnalysis.ts`, which
//! moves with V7, so it is not ported here.

use serde::Serialize;
use serde_json::{json, Value};

use crate::identification::data::{identification_operating_point, IdentificationSample};
use crate::identification::independent_validation::digest;
use crate::identification::plant::{IdentifiedModel, Partition};
use crate::identification::refusal::Refusal;
use crate::identification::review::AcceptedIdentificationModel;
use crate::model::{GeneralModel, ModelChannel, ModelIdentity, ModelSource, ModelValidation};
use trust_control_math::identify::LowOrderModelKind;
use trust_control_math::model::{OperatingPoint, TransferFunction};

/// The input and the output of a plant model (`{ inputs, outputs }` of `identifiedTransferModel`), each a channel with its
/// identifier, unit and operating range.
#[derive(Debug, Clone, PartialEq, Serialize)]
pub struct ChannelSet {
    /// The plant inputs, in the model's order.
    pub inputs: Vec<ModelChannel>,
    /// The measured outputs, in the model's order.
    pub outputs: Vec<ModelChannel>,
}

/// The operating values a fit gives its model: the input and the output at which the deviations are measured. A fit from
/// an excitation gives them over its steady baseline; otherwise they come from the samples (`identificationOperatingPoint`).
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct OperatingValues {
    /// The input at the operating point.
    pub input0: f64,
    /// The output at the operating point.
    pub output0: f64,
}

/// The validated plant model of an accepted identification (`acceptedIdentificationModel`). The estimation and the
/// validation records must be independent (B5.2), and the model keeps the digests of both as its evidence.
///
/// # Errors
/// The review is not an independent-run acceptance with suitable records, the two records are the same, or the channels
/// are not one input and one output.
pub fn accepted_identification_model(
    accepted: &AcceptedIdentificationModel,
    channels: &ChannelSet,
) -> Result<GeneralModel, Refusal> {
    let review = &accepted.review;
    if review.result.partition != Some(Partition::IndependentRuns)
        || !review.estimation_qualification.suitable
        || !review.validation_qualification.suitable
    {
        return Err(Refusal::new(
            "Independent identification validation is required before selecting a fitted model.",
        ));
    }
    let evidence = vec![
        samples_digest(&review.estimation),
        samples_digest(&review.validation),
    ];
    if evidence[0] == evidence[1] {
        return Err(Refusal::new(
            "Estimation and validation records must be independent.",
        ));
    }
    identified_transfer_model(
        &accepted.model,
        &review.estimation_qualification.samples,
        channels,
        ModelValidation::Validated,
        evidence,
        None,
    )
}

/// The unvalidated plant model of a provisional identification from one record (`provisionalIdentificationModel`). The
/// evidence is the fingerprint of the record the model was fitted from.
///
/// # Errors
/// The channels are not one input and one output, or the operating point cannot be taken from the samples.
pub fn provisional_identification_model(
    model: &IdentifiedModel,
    samples: &[IdentificationSample],
    channels: &ChannelSet,
    evidence: &str,
    operating: Option<OperatingValues>,
) -> Result<GeneralModel, Refusal> {
    identified_transfer_model(
        model,
        samples,
        channels,
        ModelValidation::Unvalidated,
        vec![evidence.to_owned()],
        operating,
    )
}

/// The transfer model of an identified low-order model (`identifiedTransferModel`): the numerator is the gain, and the
/// denominator is the product of the time constants (with the integrator when the model integrates). The delay is the
/// dead time, or zero. The revision is the digest of the model, its evidence and its channels.
fn identified_transfer_model(
    model: &IdentifiedModel,
    samples: &[IdentificationSample],
    channels: &ChannelSet,
    validation: ModelValidation,
    evidence: Vec<String>,
    operating: Option<OperatingValues>,
) -> Result<GeneralModel, Refusal> {
    if channels.inputs.len() != 1 || channels.outputs.len() != 1 {
        return Err(Refusal::new(
            "Low-order identification requires one input and one output channel.",
        ));
    }
    let (input0, output0) = if let Some(values) = operating {
        (values.input0, values.output0)
    } else {
        let point = identification_operating_point(samples)?;
        (point.input0, point.output0)
    };
    let revision = digest(&json!({ "model": model, "evidence": evidence, "channels": channels }));
    Ok(GeneralModel::TransferFunction {
        identity: ModelIdentity {
            revision,
            source: ModelSource::Fitted,
            validation,
            evidence,
            inputs: channels.inputs.clone(),
            outputs: channels.outputs.clone(),
            delay_seconds: model.dead_time_seconds.unwrap_or(0.0),
            operating_point: Some(OperatingPoint {
                inputs: vec![input0],
                outputs: vec![output0],
            }),
        },
        transfer: TransferFunction {
            numerator: vec![model.gain],
            denominator: denominator_of(model),
        },
    })
}

/// The denominator of an identified model in descending powers: the integrator (`s`) for an integrating model, times
/// `(τ₁s + 1)` and `(τ₂s + 1)` for each time constant the model has.
fn denominator_of(model: &IdentifiedModel) -> Vec<f64> {
    let mut denominator = if model.kind == LowOrderModelKind::Integrating {
        vec![1.0, 0.0]
    } else {
        vec![1.0]
    };
    for time in [
        model.time_constant_seconds,
        model.second_time_constant_seconds,
    ]
    .into_iter()
    .flatten()
    {
        let mut next = vec![0.0; denominator.len() + 1];
        for (index, coefficient) in denominator.iter().enumerate() {
            next[index] += coefficient * time;
            next[index + 1] += coefficient;
        }
        denominator = next;
    }
    denominator
}

/// The digest of a record's samples, the evidence of a validated model.
fn samples_digest(samples: &[IdentificationSample]) -> String {
    digest(&serde_json::to_value(samples).unwrap_or(Value::Null))
}

#[cfg(test)]
#[path = "selected_model_analysis_tests.rs"]
mod tests;
