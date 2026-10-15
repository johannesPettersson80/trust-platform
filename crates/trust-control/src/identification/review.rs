//! The review of an identification (specification 35, B3.5 and B5.2; numerical annex N1.1 and N3.4;
//! `prepareIdentificationReview` in `identification.ts`). The first fit of an estimation recording is a same-run
//! preview that waits for an independent validation recording; a fit scored on a distinct immutable validation
//! recording is a review that waits for the engineer's acceptance. Suitability for a preview and qualification for
//! controller design are separate states (N1.1).

#![allow(clippy::module_name_repetitions)] // The review records keep the names of the `TypeScript` records.

use schemars::JsonSchema;
use serde::{Deserialize, Serialize};
use trust_control_math::identify::LowOrderModelKind;

use crate::identification::bounded_step_review::prepare_bounded_step_review;
use crate::identification::data::{
    qualify_identification_data, same_recording, DataSource, ExcludedInterval,
    IdentificationSample, QualifiedIdentificationData,
};
use crate::identification::evidence::{model_fit_evidence, ModelFitEvidence};
use crate::identification::plant::{
    identify_plant, js_max, js_min, IdentificationResult, IdentifiedModel, IdentifyPlantOptions,
    ResultSource, DEFAULT_MAXIMUM_NORMALIZED_ERROR,
};
use crate::identification::refusal::Refusal;
use crate::identification::sensor_lag_comparison::StructureChoice;
use crate::identification::sensor_lag_review::prepare_sensor_lag_comparison;
use crate::identification::sensor_lag_texts::CALCULATE_FIRST;
use crate::identification::state::IdentificationRecording;
use crate::identification::step_response::{StepResponseAnalysis, StepResponseSettings};

/// The refusal of a validation recording that holds the estimation samples under no distinct recording identity.
const SAME_RECORDING_REFUSAL: &str =
    "A distinct immutable validation run is required before controller design.";

/// The adequacy limit of a requested model is never above this normalised validation error.
const LOOSEST_ADEQUACY_LIMIT: f64 = 0.35;

/// The adequacy limit of a requested model is never below this normalised validation error.
const TIGHTEST_ADEQUACY_LIMIT: f64 = 0.03;

/// A requested model may have up to this multiple of the recommended model's validation error.
const ADEQUACY_MARGIN: f64 = 1.25;

/// The model the engineer asks Identify for (B3.5; `IdentificationModelSelection`): the recommended model, or one
/// named form.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "snake_case")]
pub enum IdentificationModelSelection {
    /// The model the fit recommends.
    Automatic,
    /// The integrating form.
    Integrating,
    /// The first-order form.
    FirstOrder,
    /// The first-order form with dead time (FOPDT).
    Fopdt,
    /// The second-order form.
    SecondOrder,
    /// The second-order form with dead time (SOPDT).
    Sopdt,
}

/// What a review is prepared from (B3.5, N3.4; `IdentificationReviewInput`): the estimation recording and, for the
/// acceptance stage, a validation recording, with their immutable recording identities, the excluded intervals, the
/// evidence source, the requested model and what the caller states about the process.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct IdentificationReviewInput {
    /// The recording the estimation samples were taken from.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub recording: Option<IdentificationRecording>,
    /// The settings of a bounded step analysis; with them, the review is a bounded step review.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub step_response_settings: Option<StepResponseSettings>,
    /// The original first-order search on this same immutable estimation record.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub first_order_step_response_settings: Option<StepResponseSettings>,
    /// The normalised error limit of the fit; without one, the fit applies its default.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub maximum_normalized_error: Option<f64>,
    /// The estimation samples.
    pub estimation: Vec<IdentificationSample>,
    /// The validation samples, for the acceptance stage.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub validation: Option<Vec<IdentificationSample>>,
    /// The immutable identity of the estimation recording.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub estimation_recording_id: Option<String>,
    /// The immutable identity of the validation recording: distinct identities establish independent validation even
    /// for the equal samples of a deterministic simulation (N3.4).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub validation_recording_id: Option<String>,
    /// The intervals excluded from the estimation fit.
    pub excluded: Vec<ExcludedInterval>,
    /// Whether the recordings come from a simulated or a physical plant.
    pub source: DataSource,
    /// The model the engineer asks for.
    pub requested_kind: IdentificationModelSelection,
    /// The process the loop's identification policy declares; `integrating` accepts an integrating form directly
    /// (N2.3a).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub declared_process: Option<String>,
    /// Whether the fit is closed loop (a setpoint step in Auto), which the integrator rule does not apply to (N2.3a).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub closed_loop: Option<bool>,
}

/// The preview of an estimation recording (N1.1): suitable for review, not qualified for controller design until an
/// independent validation recording is fitted (stage `validation_required`).
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ValidationReview {
    /// The bounded step analysis of a bounded step review.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub step_response: Option<StepResponseAnalysis>,
    /// The qualification of the estimation recording.
    pub estimation_qualification: QualifiedIdentificationData,
    /// The single-run identification.
    pub preview: IdentificationResult,
    /// The requested model of the preview.
    pub selected: IdentifiedModel,
    /// The fit evidence of the selected model on the estimation recording.
    pub fit: ModelFitEvidence,
    /// The estimation samples.
    pub estimation: Vec<IdentificationSample>,
}

/// The independently validated result (N3.4, B5.2), waiting for the engineer's acceptance (stage
/// `acceptance_required`).
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct AcceptanceReview {
    /// The bounded step analysis of a bounded step review.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub step_response: Option<StepResponseAnalysis>,
    /// The qualification of the estimation recording.
    pub estimation_qualification: QualifiedIdentificationData,
    /// The qualification of the validation recording, with independent validation available.
    pub validation_qualification: QualifiedIdentificationData,
    /// The independently validated identification.
    pub result: IdentificationResult,
    /// The requested model of the result.
    pub selected: IdentifiedModel,
    /// The fit evidence of the selected model on the validation recording.
    pub fit: ModelFitEvidence,
    /// The estimation samples.
    pub estimation: Vec<IdentificationSample>,
    /// The validation samples.
    pub validation: Vec<IdentificationSample>,
}

/// A prepared review, tagged by its `stage` (`IdentificationReview`).
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(tag = "stage", rename_all = "snake_case")]
#[allow(clippy::large_enum_variant)] // One review per workflow; the variants keep the `TypeScript` union's shape.
pub enum IdentificationReview {
    /// A same-run preview, waiting for an independent validation recording.
    ValidationRequired(ValidationReview),
    /// An independently validated result, waiting for the engineer's acceptance.
    AcceptanceRequired(AcceptanceReview),
}

/// A model the engineer accepted from an acceptance review (`AcceptedIdentificationModel`): the model, its fit
/// evidence, the evidence source and the review it came from.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct AcceptedIdentificationModel {
    /// The accepted model.
    pub model: IdentifiedModel,
    /// Its fit evidence on the validation recording.
    pub fit: ModelFitEvidence,
    /// Whether it was identified from simulated or physical data.
    pub evidence: ResultSource,
    /// The review it was accepted from.
    pub review: AcceptanceReview,
}

/// Prepares the review of an identification (B3.5, N3.4; `prepareIdentificationReview`).
///
/// With step-response settings, the review is a bounded step review, or for a second-order-with-delay request a sensor-lag
/// comparison (`prepare_sensor_lag_comparison`, N2.10). Otherwise the estimation recording must qualify (N1.1). Without
/// validation samples, the single-run preview is identified, the requested model is selected
/// and refused when a physical rule refuses the preview, and its fit evidence is taken on the estimation record
/// (`validation_required`). With validation samples, the validation recording must be distinct, with different
/// samples or distinct immutable recording identities (N3.4), and qualify on its own; the validated fit is identified,
/// the requested model is selected, and its fit evidence is taken on the validation record (`acceptance_required`).
///
/// # Errors
/// Returns the [`Refusal`] of the sensor-lag comparison, of the bounded step review, of an estimation or validation
/// recording that does not qualify, of a validation recording that is not distinct, of the identification, of a
/// requested model that is missing or failed independent validation, of a refused preview, and of the fit evidence.
pub fn prepare_identification_review(
    input: &IdentificationReviewInput,
) -> Result<IdentificationReview, Refusal> {
    prepare_identification_review_with_structure(input, None)
}

/// Prepares the review of an identification with the structure the engineer chose at a sensor-lag comparison (N2.10).
///
/// The choice is applied to the comparison's preview, or checked by the independent check of the chosen structure. It is
/// refused when no comparison exists, and when it is not the recommended structure.
///
/// # Errors
/// As [`prepare_identification_review`], and the refusals of the choice (`choose_structure`).
pub fn prepare_identification_review_with_structure(
    input: &IdentificationReviewInput,
    structure: Option<StructureChoice>,
) -> Result<IdentificationReview, Refusal> {
    if input.step_response_settings.is_some() {
        if input.requested_kind == IdentificationModelSelection::Sopdt {
            return prepare_sensor_lag_comparison(input, structure);
        }
        refuse_structure_without_comparison(structure)?;
        return prepare_bounded_step_review(input);
    }
    refuse_structure_without_comparison(structure)?;
    let estimation = input.estimation.clone();
    let estimation_qualification = qualify_identification_data(&estimation, &input.excluded, false);
    if !estimation_qualification.suitable {
        return Err(Refusal::new(
            estimation_qualification.reason.unwrap_or_default(),
        ));
    }
    match &input.validation {
        None => validation_required(input, estimation, estimation_qualification),
        Some(validation) => acceptance_required(
            input,
            estimation,
            estimation_qualification,
            validation.clone(),
        ),
    }
}

/// A structure can be chosen only from a sensor-lag comparison (`chooseSensorLagModel`): without one, the choice is refused.
fn refuse_structure_without_comparison(structure: Option<StructureChoice>) -> Result<(), Refusal> {
    match structure {
        Some(_) => Err(Refusal::new(CALCULATE_FIRST)),
        None => Ok(()),
    }
}

/// The preview stage: the single-run identification, the requested model unless a physical rule refuses the preview,
/// and its fit evidence on the estimation record.
fn validation_required(
    input: &IdentificationReviewInput,
    estimation: Vec<IdentificationSample>,
    estimation_qualification: QualifiedIdentificationData,
) -> Result<IdentificationReview, Refusal> {
    let preview = identify_plant(
        &estimation,
        &input.excluded,
        input.source,
        None,
        maximum_normalized_error(input),
        &identify_options(input),
    )?;
    let selected = select_requested_model(&preview, input.requested_kind)?;
    refuse_refused_preview(&preview, &selected)?;
    let fit = model_fit_evidence(&selected, &estimation_qualification.samples)?;
    Ok(IdentificationReview::ValidationRequired(ValidationReview {
        step_response: None,
        estimation_qualification,
        preview,
        selected,
        fit,
        estimation,
    }))
}

/// The acceptance stage: a distinct validation recording that qualifies on its own, the validated identification, the
/// requested model, and its fit evidence on the validation record.
fn acceptance_required(
    input: &IdentificationReviewInput,
    estimation: Vec<IdentificationSample>,
    estimation_qualification: QualifiedIdentificationData,
    validation: Vec<IdentificationSample>,
) -> Result<IdentificationReview, Refusal> {
    if same_recording(&estimation, &validation) && !distinct_recording_ids(input) {
        return Err(Refusal::new(SAME_RECORDING_REFUSAL));
    }
    let validation_qualification = qualify_identification_data(&validation, &[], true);
    if !validation_qualification.suitable {
        let reason = validation_qualification.reason.unwrap_or_default();
        return Err(Refusal::new(format!("Independent validation: {reason}")));
    }
    let result = identify_plant(
        &estimation,
        &input.excluded,
        input.source,
        Some(validation.as_slice()),
        maximum_normalized_error(input),
        &identify_options(input),
    )?;
    let selected = select_requested_model(&result, input.requested_kind)?;
    let fit = model_fit_evidence(&selected, &validation_qualification.samples)?;
    Ok(IdentificationReview::AcceptanceRequired(AcceptanceReview {
        step_response: None,
        estimation_qualification,
        validation_qualification,
        result,
        selected,
        fit,
        estimation,
        validation,
    }))
}

/// Whether the two recordings have distinct immutable identities (`distinctRecordingIds`): both present, neither
/// empty, and different.
fn distinct_recording_ids(input: &IdentificationReviewInput) -> bool {
    match (
        input.estimation_recording_id.as_deref(),
        input.validation_recording_id.as_deref(),
    ) {
        (Some(estimation), Some(validation)) => {
            !estimation.is_empty() && !validation.is_empty() && estimation != validation
        }
        _ => false,
    }
}

/// The requested model of a result (`selectRequestedModel`): the recommended model for Automatic, otherwise the model
/// of the named form. A model scored on a validation record is refused when its validation error is not finite or
/// above the adequacy limit `min(0.35, max(0.03, 1.25·e))`, where `e` is the recommended model's validation error.
fn select_requested_model(
    result: &IdentificationResult,
    requested: IdentificationModelSelection,
) -> Result<IdentifiedModel, Refusal> {
    let kind = match requested {
        IdentificationModelSelection::Automatic => return Ok(result.recommended.clone()),
        IdentificationModelSelection::Integrating => LowOrderModelKind::Integrating,
        IdentificationModelSelection::FirstOrder => LowOrderModelKind::FirstOrder,
        IdentificationModelSelection::Fopdt => LowOrderModelKind::FirstOrderDeadTime,
        IdentificationModelSelection::SecondOrder => LowOrderModelKind::SecondOrder,
        IdentificationModelSelection::Sopdt => LowOrderModelKind::SecondOrderDeadTime,
    };
    let word = selection_word(requested);
    let Some(selected) = result.models.iter().find(|model| model.kind == kind) else {
        return Err(Refusal::new(format!(
            "The {word} fitter is not available for this recording."
        )));
    };
    if let (Some(selected_error), Some(recommended_error)) = (
        selected.normalized_validation_error,
        result.recommended.normalized_validation_error,
    ) {
        let adequacy_limit = js_min(
            LOOSEST_ADEQUACY_LIMIT,
            js_max(TIGHTEST_ADEQUACY_LIMIT, recommended_error * ADEQUACY_MARGIN),
        );
        if !selected_error.is_finite() || selected_error > adequacy_limit {
            return Err(Refusal::new(format!(
                "The requested {word} model failed independent validation; choose another order or Automatic."
            )));
        }
    }
    Ok(selected.clone())
}

/// The JSON word of a requested model, as the refusals name it.
fn selection_word(selection: IdentificationModelSelection) -> &'static str {
    match selection {
        IdentificationModelSelection::Automatic => "automatic",
        IdentificationModelSelection::Integrating => "integrating",
        IdentificationModelSelection::FirstOrder => "first_order",
        IdentificationModelSelection::Fopdt => "fopdt",
        IdentificationModelSelection::SecondOrder => "second_order",
        IdentificationModelSelection::Sopdt => "sopdt",
    }
}

/// A preview is not offered when a physical rule refuses it (`refuseRefusedPreview`): the refusal of the recommended
/// form, or the integrator rule's refusal of a requested integrating model.
fn refuse_refused_preview(
    result: &IdentificationResult,
    selected: &IdentifiedModel,
) -> Result<(), Refusal> {
    if let Some(refusal) = &result.refusal {
        return Err(Refusal::new(refusal.clone()));
    }
    match &result.integrator_refusal {
        Some(refusal) if selected.kind == LowOrderModelKind::Integrating => {
            Err(Refusal::new(refusal.clone()))
        }
        _ => Ok(()),
    }
}

/// The normalised error limit of the fit: the input's, or the default of [`identify_plant`].
fn maximum_normalized_error(input: &IdentificationReviewInput) -> f64 {
    input
        .maximum_normalized_error
        .unwrap_or(DEFAULT_MAXIMUM_NORMALIZED_ERROR)
}

/// What the input states about the process (`identifyOptions`).
fn identify_options(input: &IdentificationReviewInput) -> IdentifyPlantOptions {
    IdentifyPlantOptions {
        declared_process: input.declared_process.clone(),
        closed_loop: input.closed_loop == Some(true),
        pulse: false,
    }
}

#[cfg(test)]
#[path = "review_tests.rs"]
mod tests;
