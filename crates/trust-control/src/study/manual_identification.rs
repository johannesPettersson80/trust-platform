//! The manual identification of a loop: the Recorded-data fit of specification 35 (B3.3, B3.6, B3.7 and A8.1), with its section
//! `identifyManual` in the study. The section keeps the identifiers of the recordings the engineer checked, the settings they were
//! checked with, the stage, the form the page holds and the sensor-lag structure kept; it keeps no samples. The samples are read
//! from the kept recording files each time the workflow is replayed (`IdentificationWorkflow::restore_saved`), and a fit is never
//! stored, as the workflow's rules say.

use serde::{Deserialize, Serialize};

use crate::experiment::{
    retained_identification_samples_from_runtime_recording, ControlExperimentKind,
};
use crate::failure::{Cause, Failure, NextAction};
use crate::identification::data::{DataSource, ExcludedInterval};
use crate::identification::form_draft::{valid_form_draft, IdentificationFormDraft};
use crate::identification::identify_result::identify_signals;
use crate::identification::model_comparison::ComparisonPresentation;
use crate::identification::presentation::identified_model_presentation;
use crate::identification::recorded_model::channels_of;
use crate::identification::refusal::Refusal;
use crate::identification::review::{
    AcceptedIdentificationModel, IdentificationModelSelection, IdentificationReviewInput,
};
use crate::identification::selected_model_analysis::accepted_identification_model;
use crate::identification::sensor_lag_comparison::StructureChoice;
use crate::identification::state::{
    save_identification_input, IdentificationPhase, SavedIdentificationInput,
    SavedIdentificationState, IDENTIFICATION_STATE_SCHEMA_VERSION,
};
use crate::identification::step_response::{valid_step_response_settings, StepResponseSettings};
use crate::loop_definition::LoopDefinition;
use crate::recording::{EvidenceSource, RecordingFile};
use crate::study::identify::{IdentifyMethod, IdentifySection};
use crate::study::{is_recording_id, StudySection};

/// The format of the manual section this module reads and writes.
const MANUAL_SCHEMA_VERSION: u32 = 1;

/// The refusal of a saved input that names no recording.
const NO_RECORDING: &str = "A manual identification input names no recording.";
/// The refusal of a recording whose samples cannot be read for identification.
const UNREADABLE_SAMPLES: &str = "The recording's samples cannot be read for identification.";
/// The refusal of a section that breaks one of its rules.
const SECTION_BREAKS_RULE: &str =
    "The manual identification section breaks a rule: a recording, a setting, the form or the error limit is not valid.";

/// The manual identification's section of the study (`identifyManual`): the stage, the recordings it was checked with, the
/// settings, the form the page holds and the sensor-lag structure kept. It holds identifiers and settings, not samples.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct ManualIdentificationSection {
    /// Always 1.
    pub schema_version: u32,
    /// The error limit the inputs were checked with (N3.2), finite and positive.
    pub maximum_normalized_error: f64,
    /// The stage the review reached.
    pub phase: IdentificationPhase,
    /// The estimation recording, once a model was fitted from it.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub estimation: Option<ManualRecording>,
    /// The recording the stage works on: the estimation before a fit, the validation recording after it.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub current: Option<ManualRecording>,
    /// The form the page holds, as the page last sent it.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub form: Option<IdentificationFormDraft>,
    /// The sensor-lag structure the engineer kept at the preview.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub structure_choice: Option<StructureChoice>,
}

/// One recording of the manual identification: its identifier and the settings it was checked with. Its samples are not kept.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct ManualRecording {
    /// The identifier of the recording file, `rec-…` (annex E4).
    pub recording_id: String,
    /// The model the engineer asked for.
    pub requested_kind: IdentificationModelSelection,
    /// The held-step settings, when the held-step analysis was on.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub step_response_settings: Option<StepResponseSettings>,
    /// The first-order reference settings of a two-lag request.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub first_order_step_response_settings: Option<StepResponseSettings>,
    /// The excluded intervals of the fit.
    pub excluded: Vec<ExcludedInterval>,
}

impl StudySection for ManualIdentificationSection {
    const KEY: &'static str = "identifyManual";

    fn check(&self) -> Result<(), Failure> {
        let limit_ok =
            self.maximum_normalized_error.is_finite() && self.maximum_normalized_error > 0.0;
        let form_ok = self.form.as_ref().is_none_or(valid_form_draft);
        let recordings_ok = [&self.estimation, &self.current]
            .into_iter()
            .flatten()
            .all(valid_recording);
        if self.schema_version == MANUAL_SCHEMA_VERSION && limit_ok && form_ok && recordings_ok {
            Ok(())
        } else {
            Err(refusal(SECTION_BREAKS_RULE))
        }
    }

    fn recording_ids(&self) -> Vec<&str> {
        [&self.estimation, &self.current]
            .into_iter()
            .flatten()
            .map(|recording| recording.recording_id.as_str())
            .collect()
    }
}

impl ManualIdentificationSection {
    /// The section of a saved workflow state (`snapshot`), with the form the page holds. The samples of the saved inputs are left
    /// out: the section names their recordings and keeps their settings.
    ///
    /// # Errors
    /// An input names no recording.
    pub fn from_saved(
        saved: &SavedIdentificationState,
        form: Option<IdentificationFormDraft>,
    ) -> Result<Self, Refusal> {
        Ok(Self {
            schema_version: MANUAL_SCHEMA_VERSION,
            maximum_normalized_error: saved.maximum_normalized_error,
            phase: saved.phase,
            estimation: saved
                .estimation
                .as_ref()
                .map(ManualRecording::from_saved)
                .transpose()?,
            current: saved
                .current
                .as_ref()
                .map(ManualRecording::from_saved)
                .transpose()?,
            form,
            structure_choice: saved.structure_choice,
        })
    }

    /// The saved state to replay, from the inputs the caller read from the recording files (see [`manual_input`]). The stage, the
    /// form and the structure are the section's own.
    #[must_use]
    pub fn saved_state(
        &self,
        estimation: Option<SavedIdentificationInput>,
        current: Option<SavedIdentificationInput>,
    ) -> SavedIdentificationState {
        SavedIdentificationState {
            schema_version: IDENTIFICATION_STATE_SCHEMA_VERSION,
            maximum_normalized_error: self.maximum_normalized_error,
            phase: self.phase,
            current,
            estimation,
            form: self.form.clone(),
            structure_choice: self.structure_choice,
        }
    }
}

impl ManualRecording {
    /// The recording of a saved input, with the settings the input was checked with.
    ///
    /// # Errors
    /// The input names no recording.
    fn from_saved(input: &SavedIdentificationInput) -> Result<Self, Refusal> {
        let recording_id = input
            .estimation_recording_id
            .clone()
            .ok_or_else(|| Refusal::new(NO_RECORDING))?;
        Ok(Self {
            recording_id,
            requested_kind: input.requested_kind,
            step_response_settings: input.step_response_settings.clone(),
            first_order_step_response_settings: input.first_order_step_response_settings.clone(),
            excluded: input.excluded.clone(),
        })
    }
}

/// The saved input of one recording of the manual identification: the recording's samples as an observation (its plant input is
/// the applied output, B3.6), with the settings the recording was checked with and the error limit of the section.
///
/// # Errors
/// The recording's samples cannot be read for identification.
pub fn manual_input(
    recording: &ManualRecording,
    file: &RecordingFile,
    loop_definition: &LoopDefinition,
    maximum_normalized_error: f64,
) -> Result<SavedIdentificationInput, Refusal> {
    let estimation = retained_identification_samples_from_runtime_recording(
        ControlExperimentKind::Observation,
        &file.run,
        loop_definition,
    )
    .map_err(|_| Refusal::new(UNREADABLE_SAMPLES))?;
    let input = IdentificationReviewInput {
        recording: None,
        step_response_settings: recording.step_response_settings.clone(),
        first_order_step_response_settings: recording.first_order_step_response_settings.clone(),
        maximum_normalized_error: Some(maximum_normalized_error),
        estimation,
        validation: None,
        estimation_recording_id: Some(recording.recording_id.clone()),
        validation_recording_id: None,
        excluded: recording.excluded.clone(),
        source: data_source_of(file.source),
        requested_kind: recording.requested_kind,
        declared_process: None,
        closed_loop: None,
    };
    Ok(save_identification_input(&input))
}

/// The error limit a new manual identification is checked with: the loop's identification policy (N3.2), as the automatic
/// identification uses it.
#[must_use]
pub fn error_limit_of(loop_definition: &LoopDefinition) -> f64 {
    crate::identification::recorded_model::maximum_normalized_error(loop_definition)
}

/// The plant a recording file's values come from, as the identification names it (A9).
#[must_use]
pub fn data_source_of(source: EvidenceSource) -> DataSource {
    match source {
        EvidenceSource::PhysicalPlant => DataSource::Physical,
        EvidenceSource::SimulatedPlant => DataSource::Simulation,
    }
}

/// The Identify section that keeps an accepted manual model as the study's current model (B3.7): the model in the model contract,
/// with the presentation of the page's fit, the estimation and the validation recordings it was validated on (N3.4), and the
/// Recorded-data fit's method (an opened recording, read as an observation).
///
/// # Errors
/// The accepted review is not an independent validation, or its model is not one the model contract keeps.
pub fn accepted_manual_section(
    loop_definition: &LoopDefinition,
    accepted: &AcceptedIdentificationModel,
    source: EvidenceSource,
    estimation_recording_id: &str,
    validation_recording_id: &str,
) -> Result<IdentifySection, Refusal> {
    let channels = channels_of(loop_definition);
    let model = accepted_identification_model(accepted, &channels)?;
    let presentation = ComparisonPresentation {
        presentation: identified_model_presentation(
            &accepted.model,
            &accepted.fit,
            &loop_definition.signals.manipulated_variable.unit,
            &loop_definition.signals.process_variable.unit,
        ),
        experiment: None,
        output_id: None,
        input_channels: None,
    };
    let kind = ControlExperimentKind::Observation;
    Ok(IdentifySection::new(
        IdentifyMethod::Opened(kind),
        source,
        identify_signals(loop_definition, kind),
        estimation_recording_id,
        Some(validation_recording_id),
        model,
        presentation,
    ))
}

/// Whether one recording of the section is well formed: a recording identifier, finite excluded intervals that end after they
/// start, and settings that the admission rules accept.
fn valid_recording(recording: &ManualRecording) -> bool {
    is_recording_id(&recording.recording_id)
        && recording.excluded.iter().all(|interval| {
            interval.start_seconds.is_finite()
                && interval.end_seconds.is_finite()
                && interval.end_seconds >= interval.start_seconds
        })
        && recording
            .step_response_settings
            .as_ref()
            .is_none_or(valid_step_response_settings)
        && recording
            .first_order_step_response_settings
            .as_ref()
            .is_none_or(valid_step_response_settings)
}

/// A refusal of the section, as a failure of the settings.
fn refusal(text: &str) -> Failure {
    Failure::new(text, Cause::Settings, NextAction::None)
}

#[cfg(test)]
#[path = "manual_identification_tests.rs"]
mod tests;
