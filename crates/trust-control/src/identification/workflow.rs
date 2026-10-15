//! Identify's review workflow (`identificationWorkflow.ts`; specification 35, B3.5, A8.4 and A8.6; the workflow
//! specification, "Identifiability and independent model validation"): the estimation and the validation recordings
//! stay separate until the engineer accepts the model. The first fit gives a provisional model from the estimation
//! recording; a second, distinct recording then scores that model with its coefficients frozen; only a model that
//! passed this independent validation can be accepted. A saved state is restored by replaying these steps on a fresh
//! workflow, which replaces this one only when the replay reaches the saved stage.
//!
//! The `TypeScript` could also fit in a worker (`fitUsing`, `restoreUsing`) and counted generations to drop late
//! results; here a fit is a plain call that takes its calculation (`fit_with`), so neither is needed. The choice of a
//! sensor-lag structure (`chooseStructure`, `structureChoice`) is `choose_structure`: the preview is prepared again with it.

#![allow(clippy::module_name_repetitions)] // `IdentificationWorkflow` and `WorkflowState` are the `TypeScript` names.

use schemars::JsonSchema;
use serde::{Deserialize, Serialize};
use serde_json::Value;

use crate::identification::data::{
    qualify_identification_data, DataSource, ExcludedInterval, IdentificationSample,
    QualifiedIdentificationData,
};
use crate::identification::form_draft::IdentificationFormDraft;
use crate::identification::plant::IdentifiedModel;
use crate::identification::refusal::Refusal;
use crate::identification::review::{
    prepare_identification_review_with_structure, AcceptanceReview, AcceptedIdentificationModel,
    IdentificationModelSelection, IdentificationReview, IdentificationReviewInput,
};
use crate::identification::sensor_lag_comparison::StructureChoice;
use crate::identification::state::{
    decode_identification_state, restore_identification_input, same_samples,
    save_identification_input, IdentificationPhase, IdentificationRecording,
    SavedIdentificationState, IDENTIFICATION_STATE_SCHEMA_VERSION,
};
use crate::identification::step_response::{
    measure_step_response, qualify_step_response, StepResponseMeasurement, StepResponseSettings,
};
use crate::loop_definition::LoopDefinition;

/// The error limit of a workflow created without one (`new IdentificationWorkflow()` in the `TypeScript`): the default
/// of the identification policy's limit (N3.2).
pub const DEFAULT_WORKFLOW_MAXIMUM_NORMALIZED_ERROR: f64 = 0.15;

const QUALIFY_FIRST: &str = "Qualify a suitable recording before fitting a model.";
const VALIDATION_REQUIRED: &str = "Independent validation is required before acceptance.";
const VALIDATION_FAILED: &str = "Independent validation failed.";
const REVIEW_REQUIRED: &str =
    "Independent validation and model review are required before acceptance.";
const MALFORMED_STATE: &str = "Saved identification inputs are malformed or incompatible.";
const ESTIMATION_NO_LONGER_QUALIFIES: &str = "Saved estimation no longer qualifies.";
const QUALIFICATION_MISMATCH: &str =
    "Saved identification qualification does not match its inputs.";
const STAGE_MISMATCH: &str = "Saved identification review stage does not match its inputs.";
const BOUNDED_STEP_REQUIREMENT: &str = "Bounded step analysis requires a first-order or two-lag model and the complete unexcluded recording.";
const CHOOSE_FIRST: &str = "Calculate the comparison before choosing a provisional model.";

mod manual_answer;

/// The role of the next fit (`nextFitRole`; B3.5): the first fit estimates the model, every later one validates it.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "lowercase")]
pub enum FitRole {
    /// The fit gives a provisional model from the estimation recording.
    Estimation,
    /// The fit scores the frozen model against a distinct validation recording.
    Validation,
}

/// The inputs of a check of a recording (the arguments of `qualify` and `selectRecording`; B3.5): the samples and the
/// excluded intervals, the evidence source, the requested model, the recording's identity, the bounded step analysis's
/// settings, the original recording, and the first-order reference settings of a two-lag request.
#[derive(Debug, Clone, PartialEq)]
pub struct QualifyRequest {
    /// The samples to check.
    pub samples: Vec<IdentificationSample>,
    /// The intervals excluded from the fit.
    pub excluded: Vec<ExcludedInterval>,
    /// Whether the samples come from a simulated or a physical plant.
    pub source: DataSource,
    /// The model the engineer asks for.
    pub requested_kind: IdentificationModelSelection,
    /// The identifier of the recording the samples come from; an empty one is not kept.
    pub recording_id: Option<String>,
    /// The bounded step analysis's settings, when it is on.
    pub step_response_settings: Option<StepResponseSettings>,
    /// The original recording the samples were taken from.
    pub recording: Option<IdentificationRecording>,
    /// The first-order reference settings of a two-lag request.
    pub first_order_step_response_settings: Option<StepResponseSettings>,
}

/// What the workflow shows (`currentState`; B3.8): the stage, the latest qualification, the latest review and why the
/// fit or the validation failed.
#[derive(Debug, Clone, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct WorkflowState {
    /// The stage.
    pub phase: IdentificationPhase,
    /// The qualification of the latest checked recording, until a recording is only selected.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub qualification: Option<QualifiedIdentificationData>,
    /// The latest review: the provisional model, or the validated one.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub review: Option<IdentificationReview>,
    /// Why the fit or the independent validation failed.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub failure: Option<String>,
}

/// Identify's review workflow (`IdentificationWorkflow`; B3.5): it keeps the estimation and the validation recordings
/// separate until the engineer accepts the model.
#[derive(Debug, Clone)]
pub struct IdentificationWorkflow {
    /// The error limit the inputs record.
    maximum_normalized_error: f64,
    /// The current input, while it qualifies.
    qualified: Option<IdentificationReviewInput>,
    /// The input the provisional model was fitted from.
    estimation: Option<IdentificationReviewInput>,
    /// The validated review that waits for acceptance.
    pending: Option<AcceptanceReview>,
    /// The accepted model.
    accepted: Option<AcceptedIdentificationModel>,
    /// The input of the latest check, qualified or not.
    current: Option<IdentificationReviewInput>,
    /// The stage.
    phase: IdentificationPhase,
    /// The qualification of the latest check.
    qualification: Option<QualifiedIdentificationData>,
    /// The latest review.
    review: Option<IdentificationReview>,
    /// Why the latest fit or validation failed.
    failure: Option<String>,
    /// The raw form, while it is edited or pending.
    form: Option<IdentificationFormDraft>,
    /// The sensor-lag structure the engineer kept at the preview (`structureChoice`).
    structure_choice: Option<StructureChoice>,
}

impl IdentificationWorkflow {
    /// A workflow whose inputs record `maximum_normalized_error`, the limit of N3.2. It starts empty, at the qualified
    /// stage, as the `TypeScript` did.
    #[must_use]
    pub fn new(maximum_normalized_error: f64) -> Self {
        Self {
            maximum_normalized_error,
            qualified: None,
            estimation: None,
            pending: None,
            accepted: None,
            current: None,
            phase: IdentificationPhase::Qualified,
            qualification: None,
            review: None,
            failure: None,
            form: None,
            structure_choice: None,
        }
    }

    /// The accepted model (`acceptedModel`; N3.4), once the engineer accepted one.
    #[must_use]
    pub fn accepted_model(&self) -> Option<&IdentifiedModel> {
        self.accepted.as_ref().map(|accepted| &accepted.model)
    }

    /// The role of the next fit (`nextFitRole`; N3.4): validation once a provisional model keeps its estimation
    /// recording.
    #[must_use]
    pub fn next_fit_role(&self) -> FitRole {
        if self.estimation.is_some() {
            FitRole::Validation
        } else {
            FitRole::Estimation
        }
    }

    /// What the workflow shows (`currentState`; B3.8), as a copy.
    #[must_use]
    pub fn current_state(&self) -> WorkflowState {
        WorkflowState {
            phase: self.phase,
            qualification: self.qualification.clone(),
            review: self.review.clone(),
            failure: self.failure.clone(),
        }
    }

    /// The saved state (`snapshot`; A8.2): the inputs, the stage and the form, or `None` when there is neither a
    /// current input nor a form. Reviews are not saved; [`IdentificationWorkflow::restore`] replays them.
    #[must_use]
    pub fn snapshot(&self) -> Option<SavedIdentificationState> {
        if self.current.is_none() && self.form.is_none() {
            return None;
        }
        Some(SavedIdentificationState {
            schema_version: IDENTIFICATION_STATE_SCHEMA_VERSION,
            maximum_normalized_error: self.maximum_normalized_error,
            phase: self.phase,
            current: self.current.as_ref().map(save_identification_input),
            estimation: self.estimation.as_ref().map(save_identification_input),
            form: self.form.clone(),
            structure_choice: self.structure_choice,
        })
    }

    /// Restores a saved state (`restore`; A8.6: reopening starts no test). The state is decoded and checked
    /// ([`decode_identification_state`]), then replayed on a fresh workflow with the existing checks and fits: the
    /// estimation is qualified and fitted again, then the current input is qualified, fitted and accepted as far as the
    /// saved stage says. This workflow is replaced only when the replay reaches the saved stage; otherwise it is left
    /// unchanged.
    ///
    /// # Errors
    /// The saved state is malformed or incompatible; the saved estimation no longer qualifies; the current input's
    /// qualification differs from the saved stage; a fit fails (unless the saved stage is the failure); or the replay
    /// ends at another stage.
    pub fn restore(
        &mut self,
        value: &Value,
        loop_definition: &LoopDefinition,
    ) -> Result<(), Refusal> {
        let saved = decode_identification_state(value, loop_definition)
            .ok_or_else(|| Refusal::new(MALFORMED_STATE))?;
        self.restore_saved(&saved)
    }

    /// Restores a state that is already decoded, or read from the study, by the replay of [`IdentificationWorkflow::restore`].
    /// This workflow is replaced only when the replay reaches the saved stage.
    ///
    /// # Errors
    /// As [`IdentificationWorkflow::restore`], without the decoder's refusal: a saved estimation no longer qualifies, a check
    /// or a fit fails, or the replay ends at another stage.
    pub fn restore_saved(&mut self, saved: &SavedIdentificationState) -> Result<(), Refusal> {
        *self = Self::replay(saved)?;
        Ok(())
    }

    /// The workflow of a saved state's inputs and stage, without replaying its checks and fits. It serves an edit of the form and a
    /// check, which clear every review and need only the inputs; an accepted model is not restored this way, because its acceptance
    /// is replayed by [`IdentificationWorkflow::restore_saved`].
    #[must_use]
    pub fn from_inputs(saved: &SavedIdentificationState) -> Self {
        Self {
            current: saved.current.as_ref().map(restore_identification_input),
            estimation: saved.estimation.as_ref().map(restore_identification_input),
            form: saved.form.clone(),
            phase: saved.phase,
            structure_choice: saved.structure_choice,
            ..Self::new(saved.maximum_normalized_error)
        }
    }

    /// Checks a recording (`qualify`; B3.5) and makes it the current input. Any form and any accepted model are
    /// cleared. The estimation is kept only while the requested model and the step-response settings stay the same
    /// and no model was accepted; with an estimation, a recording that qualifies is a validation recording.
    ///
    /// With step-response settings, the bounded step analysis also measures the held step; it needs a first-order
    /// with delay (`fopdt`) or two-lag (`sopdt`) request and no excluded interval. A two-lag request keeps first-order
    /// reference settings: the request's own, or else, for the same estimation recording, those the previous input
    /// kept, or its step-response settings, or else the request's step-response settings.
    pub fn qualify(&mut self, request: QualifyRequest) -> QualifiedIdentificationData {
        self.form = None;
        let previous = self.estimation.as_ref().or(self.current.as_ref());
        let same_estimation = previous.is_some_and(|previous| {
            previous.estimation_recording_id == request.recording_id
                && previous.source == request.source
                && same_samples(&previous.estimation, &request.samples)
        });
        let reference = reference_settings(&request, previous.filter(|_| same_estimation));
        let kind_changed = self
            .estimation
            .as_ref()
            .is_none_or(|estimation| estimation.requested_kind != request.requested_kind);
        let settings_changed = !same_json(
            &self
                .estimation
                .as_ref()
                .and_then(|estimation| estimation.step_response_settings.as_ref()),
            &request.step_response_settings.as_ref(),
        );
        if self.accepted.is_some() || kind_changed || settings_changed {
            self.estimation = None;
        }
        // A check of a validation recording keeps the structure chosen for the estimation it is checked against, because the
        // chosen structure is what the validation fit tests. Any other check clears the choice, as the `TypeScript` did.
        if self.estimation.is_none() {
            self.structure_choice = None;
        }
        let qualification = qualification_of(&request);
        let current = IdentificationReviewInput {
            recording: request.recording,
            step_response_settings: request.step_response_settings,
            first_order_step_response_settings: reference,
            maximum_normalized_error: Some(self.maximum_normalized_error),
            estimation: request.samples,
            validation: None,
            estimation_recording_id: request.recording_id.filter(|id| !id.is_empty()),
            validation_recording_id: None,
            excluded: request.excluded,
            source: request.source,
            requested_kind: request.requested_kind,
            declared_process: None,
            closed_loop: None,
        };
        self.qualified = qualification.suitable.then(|| current.clone());
        self.current = Some(current);
        self.qualification = Some(qualification.clone());
        self.review = None;
        self.failure = None;
        self.phase = if qualification.suitable {
            if self.estimation.is_some() {
                IdentificationPhase::ValidationQualified
            } else {
                IdentificationPhase::Qualified
            }
        } else {
            IdentificationPhase::Rejected
        };
        self.pending = None;
        self.accepted = None;
        qualification
    }

    /// Selects a recording (`selectRecording`; B3.6): it becomes the current input as
    /// [`IdentificationWorkflow::qualify`] makes it, but waits for the engineer's check (the selected stage, with no
    /// qualification), and a pending form is kept. Returns the qualification the check found.
    pub fn select_recording(&mut self, request: QualifyRequest) -> QualifiedIdentificationData {
        let pending_form = self.form.take();
        let qualification = self.qualify(request);
        self.form = pending_form;
        self.await_qualification();
        qualification
    }

    /// Edits the raw form (`editForm`; A8.4): the qualification, the review and an accepted model are cleared, and the
    /// workflow is at the editing stage. The current input stays, or else the qualified one or the estimation; an
    /// estimation stays unless a model had been accepted. The caller decodes the form first
    /// ([`decode_identification_form_draft`](crate::identification::form_draft::decode_identification_form_draft)),
    /// which refused a malformed draft in the `TypeScript`.
    pub fn edit_form(&mut self, form: IdentificationFormDraft) {
        let was_accepted = self.accepted.is_some();
        let qualified = self.qualified.take();
        let preserved = self
            .current
            .take()
            .or(qualified)
            .or_else(|| self.estimation.clone());
        self.pending = None;
        self.accepted = None;
        self.qualification = None;
        self.review = None;
        self.failure = None;
        self.structure_choice = None;
        if was_accepted {
            self.estimation = None;
        }
        self.current = preserved;
        self.form = Some(form);
        self.phase = IdentificationPhase::Editing;
    }

    /// Fits the qualified recording with the review of `review.rs` (`fit`; B3.5): see
    /// [`IdentificationWorkflow::fit_with`].
    ///
    /// # Errors
    /// As [`IdentificationWorkflow::fit_with`].
    pub fn fit(&mut self) -> Result<&IdentificationReview, Refusal> {
        let choice = self.structure_choice;
        self.fit_with(|input| prepare_identification_review_with_structure(input, choice))
    }

    /// Keeps the sensor-lag structure the engineer chose at the preview (`chooseStructure`): the preview is prepared again from
    /// the estimation with the choice, and the review accepts it only when it is the structure the comparison recommends. The
    /// stage stays the preview. A choice is cleared by an edit, by a failed fit, or by a check that does not keep the estimation.
    ///
    /// # Errors
    /// The stage is not the preview of an estimation, or the review refuses the choice (the state is then unchanged).
    pub fn choose_structure(
        &mut self,
        choice: StructureChoice,
    ) -> Result<&IdentificationReview, Refusal> {
        let review = match (&self.estimation, self.phase) {
            (Some(estimation), IdentificationPhase::Preview) => {
                prepare_identification_review_with_structure(estimation, Some(choice))?
            }
            _ => return Err(Refusal::new(CHOOSE_FIRST)),
        };
        self.structure_choice = Some(choice);
        Ok(&*self.review.insert(review))
    }

    /// Fits the qualified recording with `calculate` (`fit`, `fitUsing`; B3.5). Without an estimation the qualified
    /// input is fitted, and the result is the provisional model of the preview stage; its recording becomes the
    /// estimation. With an estimation, the frozen estimation is scored against the qualified recording as its
    /// validation; the review then waits for acceptance, or, when the validation recording does not qualify, the
    /// workflow fails with its reason and keeps the review for inspection. A refused fit leaves the workflow at the
    /// failed stage with the refusal's sentence.
    ///
    /// # Errors
    /// No recording qualifies; `calculate` refuses; or a validation fit returns no independently validated review.
    pub fn fit_with<F>(&mut self, calculate: F) -> Result<&IdentificationReview, Refusal>
    where
        F: FnOnce(&IdentificationReviewInput) -> Result<IdentificationReview, Refusal>,
    {
        let qualified = self
            .qualified
            .as_ref()
            .ok_or_else(|| Refusal::new(QUALIFY_FIRST))?;
        let calculated = match &self.estimation {
            None => calculate(qualified),
            Some(estimation) => calculate(&validation_input(estimation, qualified)),
        };
        if let Err(refusal) = calculated.and_then(|review| self.complete_fit(review)) {
            self.fail_fit(&refusal);
            return Err(refusal);
        }
        // A completed fit keeps its review, so the refusal below is never returned.
        self.review
            .as_ref()
            .ok_or_else(|| Refusal::new(QUALIFY_FIRST))
    }

    /// Accepts the independently validated model (`accept`; N3.4, N3.5): its model, its fit evidence, its evidence
    /// source and its review.
    ///
    /// # Errors
    /// No review waits for acceptance, or its validation did not qualify: the refusal is the failure's sentence when
    /// there is one.
    pub fn accept(&mut self) -> Result<AcceptedIdentificationModel, Refusal> {
        let Some(pending) = self
            .pending
            .as_ref()
            .filter(|pending| pending.validation_qualification.suitable)
        else {
            let sentence = self
                .failure
                .clone()
                .unwrap_or_else(|| REVIEW_REQUIRED.to_string());
            return Err(Refusal::new(sentence));
        };
        let accepted = AcceptedIdentificationModel {
            model: pending.selected.clone(),
            fit: pending.fit.clone(),
            evidence: pending.result.source,
            review: pending.clone(),
        };
        self.accepted = Some(accepted.clone());
        self.phase = IdentificationPhase::Accepted;
        Ok(accepted)
    }

    /// The workflow a saved state replays to (`replay`). An editing state takes its inputs and form as saved; any other
    /// stage is reached through the checks and fits ([`IdentificationWorkflow::replay_review`]).
    fn replay(saved: &SavedIdentificationState) -> Result<Self, Refusal> {
        let mut next = Self::new(saved.maximum_normalized_error);
        if saved.phase == IdentificationPhase::Editing {
            next.current = saved.current.as_ref().map(restore_identification_input);
            next.estimation = saved.estimation.as_ref().map(restore_identification_input);
            next.form.clone_from(&saved.form);
            next.phase = IdentificationPhase::Editing;
        } else {
            next.replay_review(saved)?;
            if let (Some(form), Some(_)) = (&saved.form, &saved.current) {
                next.form = Some(form.clone());
            }
        }
        if next.phase != saved.phase {
            return Err(Refusal::new(STAGE_MISMATCH));
        }
        Ok(next)
    }

    /// The steps of a saved stage other than editing, in the order of the `TypeScript`: the estimation is qualified and
    /// fitted again; a form without a current input is edited; the current input of any stage but the preview is
    /// qualified (and left selected when it was), fitted again for a review, an accepted model or a failure, and
    /// accepted again for an accepted model. A fit that fails is kept only when the saved stage is the failure.
    fn replay_review(&mut self, saved: &SavedIdentificationState) -> Result<(), Refusal> {
        if let Some(estimation) = &saved.estimation {
            if !self.replay_qualify(restore_identification_input(estimation), false) {
                return Err(Refusal::new(ESTIMATION_NO_LONGER_QUALIFIES));
            }
            self.refit()?;
            // The choice comes before any validation, which tests the chosen structure.
            if let Some(choice) = saved.structure_choice {
                self.choose_structure(choice)?;
            }
        }
        if let (Some(form), None) = (&saved.form, &saved.current) {
            self.edit_form(form.clone());
        }
        let Some(current) = &saved.current else {
            return Ok(());
        };
        if saved.phase == IdentificationPhase::Preview {
            return Ok(());
        }
        let selected = saved.phase == IdentificationPhase::Selected;
        let suitable = self.replay_qualify(restore_identification_input(current), selected);
        let expected = saved.phase != IdentificationPhase::Rejected;
        if !selected && suitable != expected {
            return Err(Refusal::new(QUALIFICATION_MISMATCH));
        }
        if matches!(
            saved.phase,
            IdentificationPhase::Review
                | IdentificationPhase::Accepted
                | IdentificationPhase::Failed
        ) {
            let refitted = self.refit();
            if saved.phase != IdentificationPhase::Failed {
                refitted?;
            }
        }
        if saved.phase == IdentificationPhase::Accepted {
            self.accept()?;
        }
        Ok(())
    }

    /// Checks a restored input again, as the replay's `qualify` did, and leaves it waiting for its check when it was
    /// only selected. Returns whether it qualifies.
    fn replay_qualify(&mut self, input: IdentificationReviewInput, selected: bool) -> bool {
        let IdentificationReviewInput {
            recording,
            step_response_settings,
            first_order_step_response_settings,
            estimation,
            estimation_recording_id,
            excluded,
            source,
            requested_kind,
            ..
        } = input;
        let qualification = self.qualify(QualifyRequest {
            samples: estimation,
            excluded,
            source,
            requested_kind,
            recording_id: estimation_recording_id,
            step_response_settings,
            recording,
            first_order_step_response_settings,
        });
        if selected {
            self.await_qualification();
        }
        qualification.suitable
    }

    /// Fits again during a replay; the review stays in the workflow.
    fn refit(&mut self) -> Result<(), Refusal> {
        self.fit().map(|_| ())
    }

    /// Leaves the current input waiting for the engineer's check (`awaitQualification`).
    fn await_qualification(&mut self) {
        self.phase = IdentificationPhase::Selected;
        self.qualified = None;
        self.qualification = None;
    }

    /// Takes a calculated review (`completeFit`): the provisional model of the estimation, or the validated review.
    fn complete_fit(&mut self, review: IdentificationReview) -> Result<(), Refusal> {
        let qualified = self
            .qualified
            .as_ref()
            .ok_or_else(|| Refusal::new(QUALIFY_FIRST))?;
        if self.estimation.is_none() {
            let estimation = IdentificationReviewInput {
                estimation: review_estimation(&review).to_vec(),
                ..qualified.clone()
            };
            self.estimation = Some(estimation);
            self.phase = IdentificationPhase::Preview;
            self.review = Some(review);
            return Ok(());
        }
        let IdentificationReview::AcceptanceRequired(acceptance) = &review else {
            return Err(Refusal::new(VALIDATION_REQUIRED));
        };
        if acceptance.validation_qualification.suitable {
            self.pending = Some(acceptance.clone());
            self.phase = IdentificationPhase::Review;
        } else {
            self.pending = None;
            self.accepted = None;
            self.phase = IdentificationPhase::Failed;
            self.failure = Some(
                acceptance
                    .validation_qualification
                    .reason
                    .clone()
                    .unwrap_or_else(|| VALIDATION_FAILED.to_string()),
            );
        }
        self.review = Some(review);
        Ok(())
    }

    /// Records a refused fit (`failFit`): the failed stage, no review, and the refusal's sentence.
    fn fail_fit(&mut self, refusal: &Refusal) {
        self.structure_choice = None;
        self.phase = IdentificationPhase::Failed;
        self.review = None;
        self.failure = Some(refusal.0.clone());
    }
}

/// The input of a validation fit (`fitInput` with an estimation; N3.4): the frozen estimation recording with its
/// excluded intervals, settings and error limit, the qualified recording as the validation, and the model the engineer
/// asks for now. The independent check applies the workflow's error limit (N3.2, N3.5); the `TypeScript` left the
/// limit out here, so that its validation fits fell back to the review's defaults (0.35 for a low-order fit).
fn validation_input(
    estimation: &IdentificationReviewInput,
    qualified: &IdentificationReviewInput,
) -> IdentificationReviewInput {
    IdentificationReviewInput {
        recording: None,
        step_response_settings: estimation.step_response_settings.clone(),
        first_order_step_response_settings: estimation.first_order_step_response_settings.clone(),
        maximum_normalized_error: estimation.maximum_normalized_error,
        estimation: estimation.estimation.clone(),
        validation: Some(qualified.estimation.clone()),
        estimation_recording_id: estimation.estimation_recording_id.clone(),
        validation_recording_id: qualified.estimation_recording_id.clone(),
        excluded: estimation.excluded.clone(),
        source: estimation.source,
        requested_kind: qualified.requested_kind,
        declared_process: None,
        closed_loop: None,
    }
}

/// The samples a review was fitted from.
fn review_estimation(review: &IdentificationReview) -> &[IdentificationSample] {
    match review {
        IdentificationReview::ValidationRequired(validation) => &validation.estimation,
        IdentificationReview::AcceptanceRequired(acceptance) => &acceptance.estimation,
    }
}

/// The qualification of a request: the data checks, and with step-response settings the bounded step analysis's.
fn qualification_of(request: &QualifyRequest) -> QualifiedIdentificationData {
    let mut qualification = qualify_identification_data(&request.samples, &request.excluded, false);
    if let Some(settings) = &request.step_response_settings {
        match bounded_step_measurement(request, settings) {
            Ok(measurement) => qualification.step_response = Some(measurement),
            Err(refusal) => {
                qualification.suitable = false;
                qualification.suitable_for_design = false;
                qualification.reason = Some(refusal.0);
            }
        }
    }
    qualification
}

/// The measured held step of a bounded step analysis (`qualifyStepResponse`, then `measureStepResponse`). The analysis
/// needs a first-order with delay or a two-lag request and the complete, unexcluded recording. The measurement repeats
/// the check's steps, so it does not refuse a recording the check passed; if it did, the recording would not qualify,
/// where the `TypeScript` would have thrown.
fn bounded_step_measurement(
    request: &QualifyRequest,
    settings: &StepResponseSettings,
) -> Result<StepResponseMeasurement, Refusal> {
    let bounded_model = matches!(
        request.requested_kind,
        IdentificationModelSelection::Fopdt | IdentificationModelSelection::Sopdt
    );
    if !request.excluded.is_empty() || !bounded_model {
        return Err(Refusal::new(BOUNDED_STEP_REQUIREMENT));
    }
    qualify_step_response(&request.samples, settings)?;
    measure_step_response(&request.samples, settings)
}

/// The first-order reference settings of a request (`referenceSettings`): only a two-lag request with step-response
/// settings has them. `same_estimation` is the previous input when the request checks the same estimation recording.
fn reference_settings(
    request: &QualifyRequest,
    same_estimation: Option<&IdentificationReviewInput>,
) -> Option<StepResponseSettings> {
    if request.requested_kind != IdentificationModelSelection::Sopdt {
        return None;
    }
    let settings = request.step_response_settings.as_ref()?;
    let retained = same_estimation.and_then(|previous| {
        previous
            .first_order_step_response_settings
            .as_ref()
            .or(previous.step_response_settings.as_ref())
    });
    Some(
        request
            .first_order_step_response_settings
            .as_ref()
            .or(retained)
            .unwrap_or(settings)
            .clone(),
    )
}

/// Whether two values print as the same JSON, as the `TypeScript` compared them with `JSON.stringify`: NaN prints as
/// `null`, and an absent value differs from a present one.
fn same_json<T: Serialize>(left: &T, right: &T) -> bool {
    match (serde_json::to_value(left), serde_json::to_value(right)) {
        (Ok(left), Ok(right)) => left == right,
        _ => false,
    }
}

#[cfg(test)]
#[path = "workflow_fixtures.rs"]
mod fixtures;

#[cfg(test)]
#[path = "workflow_tests.rs"]
mod tests;

#[cfg(test)]
#[path = "workflow_fit_tests.rs"]
mod fit_tests;
