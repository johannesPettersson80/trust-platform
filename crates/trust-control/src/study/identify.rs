//! The Identify section of a study (specification 35, A8.1, A9, B3.5, B3.8 and B10.2; Identify page specification, "Single
//! experiment and provisional model"): what one identification produced, and where it came from.
//!
//! The section holds the method and the signals the identification used, the model it produced (what Tune reads: the
//! `selectedModel` of the `TypeScript` engineer study) with the presentation that shows it (what the page shows: the equation
//! with its coefficients, the units, the measured response beside the prediction and the traces of the experiment; the
//! `modelComparison` of the engineer study), whether the model is provisional, and the recordings it came from. A fit is not
//! replayed when the study is reopened (A8.6): the model is stored with its presentation, and the recordings stay immutable
//! files (A8.1).
//!
//! A fit from one recording is provisional. The same-record fit is not independent validation and is never stored as
//! validated evidence: a model is validated only when a distinct validation recording is named (N3.4), and a validated model
//! names one. Evidence sources are never mixed (A9): both recordings come from the section's source. A recording that is
//! not complete evidence identifies nothing, so it cannot be a recording a model came from (B10.3).
//!
//! A live identification also keeps the request its estimation recording was taken with: the reviewed plan and the steady
//! baseline window, which the recording file does not hold, so the recording can be fitted again (B3.6). The model stays
//! current while the loop's signals are those it was identified with (A8.4); a change of the limits does not invalidate it.

#![allow(clippy::module_name_repetitions)] // The section keeps the name of its stage.

use std::collections::BTreeSet;

use schemars::JsonSchema;
use serde::{Deserialize, Serialize};

use crate::experiment::ControlExperimentKind;
use crate::failure::{Cause, Failure, NextAction};
use crate::identification::connected_record::ConnectedRequest;
use crate::identification::identify_result::identify_signals;
use crate::identification::model_comparison::{
    presentation_describes_model, ComparisonPresentation,
};
use crate::loop_definition::LoopDefinition;
use crate::model::general::js_text;
use crate::model::{GeneralModel, ModelSource, ModelValidation};
use crate::recording::{Completeness, EvidenceSource, RecordingFile};
use crate::study::{is_recording_id, is_valid_model, StudySection};

/// How the recording of an identification was obtained (B3.3, B3.6; A9).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum IdentifyMethod {
    /// A live campaign ran this experiment on the plant (B3.4). What Automatic chose is stored as the experiment it chose.
    Live(ControlExperimentKind),
    /// The recording was opened from a file and fitted without live excitation (B3.6). The kind is the experiment it was
    /// recorded with, and the recording keeps its original evidence source (A9).
    Opened(ControlExperimentKind),
}

/// The signals an identification used (B3.2): the plant inputs and the measured outputs, by the symbols of the loop's
/// signals, in the order of the model's channels.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct IdentifySignals {
    /// The plant inputs: the signals that drive the plant.
    pub inputs: Vec<String>,
    /// The measured outputs: the plant's responses.
    pub outputs: Vec<String>,
    /// The setpoint signal of a closed-loop identification.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub setpoint: Option<String>,
}

/// The result of one identification, kept in the study under the key `identify`.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct IdentifySection {
    /// How the recording was obtained.
    pub method: IdentifyMethod,
    /// Where the recordings' values come from (A9): the source of the recording files.
    pub source: EvidenceSource,
    /// The signals that were used.
    pub signals: IdentifySignals,
    /// The recording file the model was fitted on, by its identifier (A8.3).
    pub estimation_recording_id: String,
    /// The distinct recording file the model was validated on, when it was validated independently (N3.4).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub validation_recording_id: Option<String>,
    /// Whether the model is provisional: Tune may use it, but it cannot authorize Apply. A fit from one recording is
    /// provisional; so is every model without a validation recording.
    pub provisional: bool,
    /// The model: fitted from the recording, with the evidence it was fitted from.
    pub selected_model: GeneralModel,
    /// What the page shows of the model: its presentation, with the traces of the experiment it was read from.
    pub presentation: ComparisonPresentation,
    /// The request the estimation recording was taken with, when a live identification took it: what fitting the recording
    /// again needs (B3.6).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub record_request: Option<StoredRecordRequest>,
}

/// The request a live identification took its estimation recording with (B3.4 and B3.6): the program revision it ran on and
/// the connected request, with the reviewed plan and the steady baseline window the recording file does not hold.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct StoredRecordRequest {
    /// The program revision the recording was taken on.
    pub project_revision: String,
    /// The request of the connected record: the loop revision, the acquisition, the reviewed plan and the baseline window.
    pub request: ConnectedRequest,
}

impl IdentifySection {
    /// The section of an identification. The model is provisional unless a validation recording is named: a fit judged on
    /// the recording it was fitted on is never validated evidence.
    #[must_use]
    pub fn new(
        method: IdentifyMethod,
        source: EvidenceSource,
        signals: IdentifySignals,
        estimation_recording_id: &str,
        validation_recording_id: Option<&str>,
        selected_model: GeneralModel,
        presentation: ComparisonPresentation,
    ) -> Self {
        Self {
            method,
            source,
            signals,
            estimation_recording_id: estimation_recording_id.to_string(),
            validation_recording_id: validation_recording_id.map(str::to_string),
            provisional: validation_recording_id.is_none(),
            selected_model,
            presentation,
            record_request: None,
        }
    }

    /// The section with the request its estimation recording was taken with (B3.6).
    #[must_use]
    pub fn with_record_request(self, record_request: StoredRecordRequest) -> Self {
        Self {
            record_request: Some(record_request),
            ..self
        }
    }

    /// The request for fitting the recording `recording_id` again without live excitation (B3.6): only the section's
    /// estimation recording, and only when the section keeps the request it was taken with. A recording file holds no
    /// reviewed plan and no baseline window, so no other recording can be fitted again.
    ///
    /// # Errors
    /// `recording_id` is not the section's estimation recording, or the section keeps no request ([`refit_refusal`]).
    pub fn refit_request(&self, recording_id: &str) -> Result<&StoredRecordRequest, Failure> {
        if self.estimation_recording_id != recording_id {
            return Err(refit_refusal(recording_id));
        }
        self.record_request
            .as_ref()
            .ok_or_else(|| refit_refusal(recording_id))
    }

    /// Whether the model is current for `loop_definition` (A8.4): the plant inputs and the measured outputs are the loop's
    /// manipulated variable and measurement, and the setpoint of a closed-loop identification is the loop's setpoint, by
    /// their symbols ([`identify_signals`]). Only a change of these signals invalidates the model; the limits do not.
    #[must_use]
    pub fn is_current_for(&self, loop_definition: &LoopDefinition) -> bool {
        let current = identify_signals(loop_definition, ControlExperimentKind::SetpointStep);
        self.signals.inputs == current.inputs
            && self.signals.outputs == current.outputs
            && (self.signals.setpoint.is_none() || self.signals.setpoint == current.setpoint)
    }

    /// Checks the recording files the section names against it: each is the file of the identifier the section gives, comes
    /// from the section's evidence source (A9), and is complete evidence (B10.3; an incomplete recording stays labelled and
    /// identifies nothing). `validation` is given exactly when the section names a validation recording. A record request the
    /// section keeps is the request of its estimation recording: it names the runtime recording that file holds.
    ///
    /// # Errors
    /// A file is not the one the section names, comes from another evidence source, or is not complete evidence; the record
    /// request names another runtime recording; or a validation file is given or missing against the section.
    pub fn check_recordings(
        &self,
        estimation: &RecordingFile,
        validation: Option<&RecordingFile>,
    ) -> Result<(), Failure> {
        self.check_file("estimation", &self.estimation_recording_id, estimation)?;
        if let Some(stored) = &self.record_request {
            let requested = &stored.request.acquisition.recording_id;
            if *requested != estimation.run.recording_id {
                return Err(refuse(&format!(
                    "the record request it keeps was taken with runtime recording {requested}, but its estimation recording holds runtime recording {}",
                    estimation.run.recording_id
                )));
            }
        }
        match (&self.validation_recording_id, validation) {
            (Some(id), Some(file)) => self.check_file("validation", id, file),
            (None, None) => Ok(()),
            (Some(_), None) => Err(refuse("its validation recording is missing")),
            (None, Some(_)) => Err(refuse("it names no validation recording")),
        }
    }

    fn check_file(&self, role: &str, id: &str, file: &RecordingFile) -> Result<(), Failure> {
        if file.id()? != id {
            return Err(refuse(&format!("the {role} recording file is not {id}")));
        }
        if file.source != self.source {
            return Err(refuse(&format!(
                "the {role} recording comes from a {}, the result from a {} (A9)",
                source_name(file.source),
                source_name(self.source)
            )));
        }
        if file.evidence.completeness != Completeness::Complete {
            return Err(refuse(&format!(
                "the {role} recording is not complete evidence, so no model came from it (B10.3)"
            )));
        }
        Ok(())
    }

    /// The model is a fitted model of the model contract, with its evidence; it is validated exactly when the section is
    /// not provisional.
    fn check_model(&self) -> Result<(), Failure> {
        let identity = self.selected_model.identity();
        if !is_valid_model(&self.selected_model) {
            return Err(refuse(
                "the model breaks the model contract (revision, evidence, channels, delay, operating point or coefficients)",
            ));
        }
        if identity.source != ModelSource::Fitted {
            return Err(refuse("the model was not fitted from a recording"));
        }
        if identity.evidence.is_empty() {
            return Err(refuse("the model names no evidence it was fitted from"));
        }
        match (self.provisional, identity.validation) {
            (true, ModelValidation::Validated) => Err(refuse(
                "a provisional model is not validated: a fit judged on the recording it was fitted on is never validated evidence",
            )),
            (false, ModelValidation::Unvalidated) => Err(refuse(
                "a model that is not provisional must be validated on its validation recording",
            )),
            _ => Ok(()),
        }
    }

    /// The signals are as many as the model has channels, named, and each named once.
    fn check_signals(&self) -> Result<(), Failure> {
        let identity = self.selected_model.identity();
        let signals = &self.signals;
        if signals.inputs.len() != identity.inputs.len()
            || signals.outputs.len() != identity.outputs.len()
        {
            return Err(refuse(&format!(
                "it names {} plant input(s) and {} measured output(s), but the model has {} and {}",
                signals.inputs.len(),
                signals.outputs.len(),
                identity.inputs.len(),
                identity.outputs.len()
            )));
        }
        let mut seen = BTreeSet::new();
        for name in signals
            .inputs
            .iter()
            .chain(&signals.outputs)
            .chain(&signals.setpoint)
        {
            if !js_text(name) || !seen.insert(name.as_str()) {
                return Err(refuse(&format!(
                    "the signal name {name:?} is blank or named twice"
                )));
            }
        }
        Ok(())
    }

    /// The recording identifiers are recording identifiers; the validation recording is another recording than the
    /// estimation recording (N3.4) and is named exactly when the model is not provisional.
    fn check_recording_ids(&self) -> Result<(), Failure> {
        if !is_recording_id(&self.estimation_recording_id) {
            return Err(refuse(&format!(
                "{:?} is not a recording identifier",
                self.estimation_recording_id
            )));
        }
        match (&self.validation_recording_id, self.provisional) {
            (None, true) => Ok(()),
            (None, false) => Err(refuse(
                "a model that is not provisional names the recording it was validated on",
            )),
            (Some(_), true) => Err(refuse("a provisional model names no validation recording")),
            (Some(id), false) => {
                if !is_recording_id(id) {
                    return Err(refuse(&format!("{id:?} is not a recording identifier")));
                }
                if *id == self.estimation_recording_id {
                    return Err(refuse(
                        "the validation recording must be another recording than the estimation recording (N3.4)",
                    ));
                }
                Ok(())
            }
        }
    }

    /// The presentation describes this model: its units, delay, errors, predicted response and traces.
    fn check_presentation(&self) -> Result<(), Failure> {
        if !presentation_describes_model(&self.presentation, &self.selected_model) {
            return Err(refuse(
                "its presentation does not describe the model (units, delay, errors, predicted response or traces)",
            ));
        }
        Ok(())
    }
}

impl StudySection for IdentifySection {
    const KEY: &'static str = "identify";

    fn check(&self) -> Result<(), Failure> {
        self.check_model()?;
        self.check_signals()?;
        self.check_recording_ids()?;
        self.check_presentation()
    }

    fn recording_ids(&self) -> Vec<&str> {
        let mut ids = vec![self.estimation_recording_id.as_str()];
        ids.extend(self.validation_recording_id.as_deref());
        ids
    }
}

/// The refusal of fitting the recording `recording_id` again when the study keeps no request for it (B3.6): only the recording
/// of the study's Identify result can be fitted again, and Identify plant records a new one.
#[must_use]
pub fn refit_refusal(recording_id: &str) -> Failure {
    Failure::new(
        &format!(
            "Recording {recording_id} cannot be fitted again: the study keeps the experiment request only for the recording of its Identify result. Identify plant records a new one."
        ),
        Cause::ModelOrData,
        NextAction::Identify,
    )
}

/// The words A9 gives an evidence source.
fn source_name(source: EvidenceSource) -> &'static str {
    match source {
        EvidenceSource::SimulatedPlant => "simulated plant",
        EvidenceSource::PhysicalPlant => "physical plant",
    }
}

fn refuse(reason: &str) -> Failure {
    Failure::new(
        &format!("The Identify result of the study is not valid: {reason}."),
        Cause::ModelOrData,
        NextAction::None,
    )
}

#[cfg(test)]
#[path = "identify_tests.rs"]
mod tests;
