//! The page's view of the manual identification (`identificationPresentationState` of `identificationPresentationState.ts`, and
//! the primary action and its label of `threePageIdentification.ts`; specification 35, B3.3 and §0.1a). The engine decides the
//! stage, the status, the primary action and its label, and whether the review may be accepted, so the page renders them and
//! keeps no copy of the rules (AGENTS.md, "Logic in Rust").

use crate::identification::form_draft::IdentificationFormDraft;
use crate::identification::presentation::identified_model_presentation;
use crate::identification::review::{IdentificationModelSelection, IdentificationReview};
use crate::identification::sensor_lag_comparison::{StructureChoice, StructureRecommendation};
use crate::identification::state::IdentificationPhase;
use crate::loop_definition::LoopDefinition;
use crate::messages::{ManualAction, ManualIdentificationAnswer, ManualStage, ManualStatus};

use super::IdentificationWorkflow;

/// The sentence of a review whose fit or validation failed without a sentence of its own (`presentation` of the page).
const VALIDATION_FAILED: &str = "Independent validation failed.";
/// The status sentences the page showed under each stage (`threePageIdentification.ts`).
const EDITED: &str = "Identification settings changed. Check the recording before fitting.";
const RECORDING_OPENED: &str = "Recording opened. Check it before fitting a model.";
const PREVIEW_READY: &str =
    "The estimation preview is ready. Open a distinct validation recording.";
const FIRST_ORDER_MEETS: &str = "First order meets the limits; the extra state is not justified.";
const ADDITIONAL_LAG_MEETS: &str =
    "The additional lag meets the limits. Its physical origin remains unresolved.";
const NEITHER_MEETS: &str =
    "Neither model meets the declared requirements. Review the comparison and settings.";
const ACCEPTANCE_READY: &str =
    "Independent validation is ready. Review it before using the fitted model.";
const ACCEPTED: &str = "Model accepted for design.";
/// The sentence of a failure with no review (the page's "Review the retained recording before fitting.").
const REVIEW_BEFORE_FIT: &str = "Review the retained recording before fitting.";

impl IdentificationWorkflow {
    /// The page's view of the manual identification (`identificationPresentationState`). `form` is the form the page holds; the
    /// units of the loop's plant input and measured output name the presentation's units. The study keeps a recording to check
    /// when the workflow holds an input.
    #[must_use]
    pub fn manual_answer(
        &self,
        loop_definition: &LoopDefinition,
        form: Option<IdentificationFormDraft>,
    ) -> ManualIdentificationAnswer {
        let has_recording = self.current.is_some() || self.estimation.is_some();
        let mut answer = ManualIdentificationAnswer {
            status: ManualStatus::Complete,
            reason: None,
            phase: None,
            next_fit_role: self.next_fit_role(),
            has_recording,
            form,
            qualification: None,
            estimation_qualification: None,
            validation_qualification: None,
            source: None,
            model: None,
            fit: None,
            presentation: None,
            primary_action: ManualAction::OpenRecording,
            primary_action_label: String::new(),
            acceptance_ready: false,
            structure_choice: None,
            recording_id: None,
            message: None,
            // The workflow holds no study: the studio fills the study's evidence and stages once the action is saved.
            stage_evidence: None,
            stages: None,
        };
        self.fill_stage(&mut answer, loop_definition);
        answer.acceptance_ready = self.acceptance_ready();
        let (action, structure) = self.primary_action(has_recording);
        answer.primary_action_label = action_label(action, structure, answer.acceptance_ready);
        answer.primary_action = action;
        answer.structure_choice = structure;
        answer.recording_id = self.next_recording_id();
        answer.message = self.stage_message(&answer);
        answer
    }

    /// The recording the next check reads: the recording the workflow holds as its current input. A preview has none to read yet,
    /// because the independent recording is still to be selected.
    fn next_recording_id(&self) -> Option<String> {
        if self.phase == IdentificationPhase::Preview {
            return None;
        }
        self.current
            .as_ref()
            .and_then(|current| current.estimation_recording_id.clone())
    }

    /// The status sentence under the stage: a failure's reason, or the sentence the page showed for the stage (a check names the
    /// action it offers next).
    fn stage_message(&self, answer: &ManualIdentificationAnswer) -> Option<String> {
        if answer.status == ManualStatus::Failed {
            return answer.reason.clone();
        }
        let text = match answer.phase? {
            ManualStage::Editing => EDITED,
            ManualStage::RecordingSelected => RECORDING_OPENED,
            ManualStage::Qualified => {
                return Some(format!(
                    "Recording qualified. {}.",
                    answer.primary_action_label
                ))
            }
            ManualStage::ValidationRequired => self.preview_message(),
            ManualStage::AcceptanceRequired => ACCEPTANCE_READY,
            ManualStage::Accepted => ACCEPTED,
        };
        Some(text.to_string())
    }

    /// The status sentence of a preview: the comparison's recommendation while no structure is kept, and the validation request
    /// otherwise.
    fn preview_message(&self) -> &'static str {
        let recommended = match &self.review {
            Some(IdentificationReview::ValidationRequired(preview))
                if self.structure_choice.is_none() =>
            {
                preview
                    .fit
                    .model_structure
                    .as_ref()
                    .map(|comparison| comparison.recommended)
            }
            _ => None,
        };
        match recommended {
            Some(StructureRecommendation::FirstOrder) => FIRST_ORDER_MEETS,
            Some(StructureRecommendation::AdditionalLag) => ADDITIONAL_LAG_MEETS,
            Some(StructureRecommendation::Neither) => NEITHER_MEETS,
            None => PREVIEW_READY,
        }
    }

    /// Fills the stage, the status and the reason, and what the stage shows (`identificationPresentationState`).
    fn fill_stage(
        &self,
        answer: &mut ManualIdentificationAnswer,
        loop_definition: &LoopDefinition,
    ) {
        match self.phase {
            IdentificationPhase::Editing => answer.phase = Some(ManualStage::Editing),
            IdentificationPhase::Selected => answer.phase = Some(ManualStage::RecordingSelected),
            IdentificationPhase::Qualified
            | IdentificationPhase::ValidationQualified
            | IdentificationPhase::Rejected => self.fill_check(answer),
            IdentificationPhase::Preview
            | IdentificationPhase::Review
            | IdentificationPhase::Accepted
            | IdentificationPhase::Failed => {
                if let Some(review) = &self.review {
                    self.fill_review(answer, review, loop_definition);
                } else {
                    answer.status = ManualStatus::Failed;
                    answer.reason = Some(
                        self.failure
                            .clone()
                            .unwrap_or_else(|| REVIEW_BEFORE_FIT.to_string()),
                    );
                }
            }
        }
    }

    /// The stage after a check: a suitable recording is qualified, and a rejected one fails with its reason.
    fn fill_check(&self, answer: &mut ManualIdentificationAnswer) {
        let Some(qualification) = &self.qualification else {
            return;
        };
        answer.phase = Some(ManualStage::Qualified);
        if !qualification.suitable {
            answer.status = ManualStatus::Failed;
            answer.reason.clone_from(&qualification.reason);
        }
        answer.qualification = Some(qualification.clone());
    }

    /// The stage of a review: the preview, the validated review or the accepted model, with the model and its presentation.
    fn fill_review(
        &self,
        answer: &mut ManualIdentificationAnswer,
        review: &IdentificationReview,
        loop_definition: &LoopDefinition,
    ) {
        let (stage, source, selected, fit) = match review {
            IdentificationReview::ValidationRequired(preview) => (
                ManualStage::ValidationRequired,
                preview.preview.source,
                &preview.selected,
                &preview.fit,
            ),
            IdentificationReview::AcceptanceRequired(acceptance) => (
                ManualStage::AcceptanceRequired,
                acceptance.result.source,
                &acceptance.selected,
                &acceptance.fit,
            ),
        };
        answer.phase = Some(if self.phase == IdentificationPhase::Accepted {
            ManualStage::Accepted
        } else {
            stage
        });
        answer.source = Some(source);
        answer.model = Some(selected.clone());
        answer.fit = Some(fit.clone());
        answer.presentation = Some(identified_model_presentation(
            selected,
            fit,
            &loop_definition.signals.manipulated_variable.unit,
            &loop_definition.signals.process_variable.unit,
        ));
        match review {
            IdentificationReview::ValidationRequired(preview) => {
                answer.estimation_qualification = Some(preview.estimation_qualification.clone());
            }
            IdentificationReview::AcceptanceRequired(acceptance) => {
                answer.estimation_qualification = Some(acceptance.estimation_qualification.clone());
                answer.validation_qualification = Some(acceptance.validation_qualification.clone());
            }
        }
        if self.phase == IdentificationPhase::Failed {
            answer.status = ManualStatus::Failed;
            answer.reason = Some(
                self.failure
                    .clone()
                    .unwrap_or_else(|| VALIDATION_FAILED.to_string()),
            );
        }
    }

    /// Whether the review may be accepted (the page's acceptance gate, `acceptanceEvidenceIsReady`): the stage is the validated
    /// review, its validation qualifies, and a sensor-lag comparison has its structure kept, which the review accepts only when
    /// it is the recommended one.
    #[must_use]
    pub fn acceptance_ready(&self) -> bool {
        let Some(IdentificationReview::AcceptanceRequired(acceptance)) = &self.review else {
            return false;
        };
        self.phase == IdentificationPhase::Review
            && acceptance.validation_qualification.suitable
            && (!self.sensor_lag_requested() || self.structure_choice.is_some())
    }

    /// Whether the estimation was asked for the second-order-with-dead-time model with held-step settings, so that the sensor-lag
    /// comparison applies to it.
    fn sensor_lag_requested(&self) -> bool {
        self.estimation.as_ref().is_some_and(|estimation| {
            estimation.requested_kind == IdentificationModelSelection::Sopdt
                && estimation.step_response_settings.is_some()
        })
    }

    /// The primary action the page offers and the structure it keeps, if any. A recording is checked, fitted, compared or
    /// accepted in turn; a preview with a sensor-lag comparison first keeps the recommended structure.
    fn primary_action(&self, has_recording: bool) -> (ManualAction, Option<StructureChoice>) {
        match self.phase {
            IdentificationPhase::Editing
            | IdentificationPhase::Rejected
            | IdentificationPhase::Failed => (recording_action(has_recording), None),
            IdentificationPhase::Selected => (ManualAction::CheckRecording, None),
            // A workflow with no check yet is the empty study: it asks for a recording first.
            IdentificationPhase::Qualified if self.qualification.is_none() => {
                (recording_action(has_recording), None)
            }
            IdentificationPhase::Qualified => (ManualAction::FitPlant, None),
            IdentificationPhase::ValidationQualified => (ManualAction::CompareWithModel, None),
            IdentificationPhase::Preview => self.preview_action(),
            IdentificationPhase::Review => (ManualAction::Accept, None),
            IdentificationPhase::Accepted => (ManualAction::ModelReady, None),
        }
    }

    /// The primary action of a preview: a comparison without a kept structure asks for the recommended one, and a comparison
    /// that recommends neither structure asks for the settings to be reviewed.
    fn preview_action(&self) -> (ManualAction, Option<StructureChoice>) {
        let Some(IdentificationReview::ValidationRequired(preview)) = &self.review else {
            return (ManualAction::OpenValidationRecording, None);
        };
        let Some(comparison) = &preview.fit.model_structure else {
            return (ManualAction::OpenValidationRecording, None);
        };
        if self.structure_choice.is_some() {
            return (ManualAction::OpenValidationRecording, None);
        }
        match comparison.recommended {
            StructureRecommendation::FirstOrder => (
                ManualAction::ChooseStructure,
                Some(StructureChoice::FirstOrder),
            ),
            StructureRecommendation::AdditionalLag => (
                ManualAction::ChooseStructure,
                Some(StructureChoice::AdditionalLag),
            ),
            StructureRecommendation::Neither => (ManualAction::ReviewSettings, None),
        }
    }
}

/// The action that checks the study's recording, or opens one when the study keeps none.
fn recording_action(has_recording: bool) -> ManualAction {
    if has_recording {
        ManualAction::CheckRecording
    } else {
        ManualAction::OpenRecording
    }
}

/// The label of the primary action, as the page showed it (`primaryActionLabel`).
fn action_label(
    action: ManualAction,
    structure: Option<StructureChoice>,
    acceptance_ready: bool,
) -> String {
    let label = match action {
        ManualAction::OpenRecording => "Open recording…",
        ManualAction::CheckRecording => "Check recording",
        ManualAction::FitPlant => "Fit plant model",
        ManualAction::CompareWithModel => "Compare with model",
        ManualAction::ChooseStructure => match structure {
            Some(StructureChoice::FirstOrder) => "Keep first-order model",
            Some(StructureChoice::AdditionalLag) => "Keep additional-lag model",
            None => "Review settings",
        },
        ManualAction::ReviewSettings => "Review settings",
        ManualAction::OpenValidationRecording => "Open validation recording",
        ManualAction::Accept if acceptance_ready => "Use fitted model",
        ManualAction::Accept => "Validation not qualified",
        ManualAction::ModelReady => "Model ready",
    };
    label.to_string()
}
