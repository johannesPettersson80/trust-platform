//! Identify's saved state (`identificationState.ts`; specification 35, A8.2, A8.6 and B3): the inputs of the estimation
//! and of the current recording, the stage the review reached and the raw form, as the study keeps them. Fits are never
//! stored: reopening replays them from these inputs (`workflow`), after the decoder here has checked every input,
//! including that a retained original recording still yields exactly the saved samples.

#![allow(clippy::module_name_repetitions)] // `SavedIdentificationState` is the name the `TypeScript` record has.

use schemars::JsonSchema;
use serde::{Deserialize, Deserializer, Serialize};
use serde_json::Value;

use crate::experiment::{
    retained_identification_samples_from_runtime_recording, ControlExperimentKind, SampleQuality,
};
use crate::identification::data::{DataSource, ExcludedInterval, IdentificationSample};
use crate::identification::form_draft::{valid_form_draft, IdentificationFormDraft};
use crate::identification::review::{IdentificationModelSelection, IdentificationReviewInput};
use crate::identification::step_response::{valid_step_response_settings, StepResponseSettings};
use crate::limits::RECORDING_SAMPLE_LIMIT;
use crate::loop_definition::LoopDefinition;
use crate::recording::RecordedRun;

/// The saved-state format this module reads and writes.
pub(crate) const IDENTIFICATION_STATE_SCHEMA_VERSION: u32 = 1;

/// The longest saved state the decoder reads: 32 MiB of JSON text, in UTF-16 code units as the `TypeScript` counted it.
const MAXIMUM_STATE_TEXT_LENGTH: usize = 32 * 1024 * 1024;

/// The longest recording identifier of a saved input, in UTF-16 code units.
const MAXIMUM_RECORDING_ID_LENGTH: usize = 1024;

/// The longest unit of a saved sample, in UTF-16 code units.
const MAXIMUM_UNIT_LENGTH: usize = 256;

/// The identity of a saved result that an imported recording came from (`WorkflowRecordingProvenance`, the fields its
/// identity needs; A8.3, A9). The other fields of the `TypeScript` provenance are not read and not kept.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "camelCase")]
pub struct RecordingProvenance {
    /// The project the result was saved in.
    pub project_id: String,
    /// The saved result that holds the recording.
    pub result_id: String,
    /// The revision of that result.
    pub result_revision: u64,
}

/// The identity of an imported saved recording (`importedRecordingId`; A8.3): `saved-recording:` followed by the JSON
/// array of its project, result and revision. It does not depend on the study that imports the recording.
#[must_use]
pub fn imported_recording_id(provenance: &RecordingProvenance) -> String {
    let identity = Value::Array(vec![
        Value::from(provenance.project_id.as_str()),
        Value::from(provenance.result_id.as_str()),
        Value::from(provenance.result_revision),
    ]);
    format!("saved-recording:{identity}")
}

/// The original recording an identification input was taken from (`IdentificationRecording`; A8.1: a recording is
/// immutable once it has ended): the experiment kind that selects the plant input, the recorded run exactly as the
/// runtime returned it, the selected intervals and, for an imported recording, the identity of its saved result.
///
/// It serialises as `{kind, run, selectedIntervals, provenance?}`. The `TypeScript` kept the editor's recording under
/// `data` instead (`rawSamples`, `selectedIntervals`, `acquisition`, `provenance`), so its saved recordings do not
/// decode here.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct IdentificationRecording {
    /// The experiment kind: it selects the plant input of every sample.
    pub kind: ControlExperimentKind,
    /// The recorded run.
    pub run: RecordedRun,
    /// The intervals selected in the recording.
    pub selected_intervals: Vec<ExcludedInterval>,
    /// The identity of the saved result an imported recording came from.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub provenance: Option<RecordingProvenance>,
}

/// The stage of Identify's review (`IdentificationPhase`; B3 and the workflow specification, "Identifiability and
/// independent model validation").
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "kebab-case")]
pub enum IdentificationPhase {
    /// The raw form is being edited; nothing is qualified.
    Editing,
    /// A recording is selected and waits for its check.
    Selected,
    /// The estimation recording qualifies; no model is fitted yet.
    Qualified,
    /// A provisional model is fitted from the estimation recording.
    Preview,
    /// A validation recording qualifies beside the estimation (`validation-qualified`).
    ValidationQualified,
    /// The independently validated model waits for acceptance.
    Review,
    /// The model is accepted.
    Accepted,
    /// The recording does not qualify.
    Rejected,
    /// The fit or the independent validation failed.
    Failed,
}

/// One sample of a saved input (`SavedSample`; A8.2): a missing time, input or output is `null`, where the live sample
/// holds NaN. The three numbers must be present, as numbers or `null`.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct SavedSample {
    /// The process time from the first sample, in s.
    #[serde(deserialize_with = "nullable")]
    pub time_seconds: Option<f64>,
    /// The plant input.
    #[serde(deserialize_with = "nullable")]
    pub input: Option<f64>,
    /// The measured output.
    #[serde(deserialize_with = "nullable")]
    pub output: Option<f64>,
    /// The sample's quality.
    pub quality: SampleQuality,
    /// The input's unit, at most 256 UTF-16 code units.
    #[serde(
        default,
        deserialize_with = "present",
        skip_serializing_if = "Option::is_none"
    )]
    pub input_unit: Option<String>,
    /// The output's unit, at most 256 UTF-16 code units.
    #[serde(
        default,
        deserialize_with = "present",
        skip_serializing_if = "Option::is_none"
    )]
    pub output_unit: Option<String>,
    /// Whether the input is clipped by its limits.
    #[serde(
        default,
        deserialize_with = "present",
        skip_serializing_if = "Option::is_none"
    )]
    pub clipped: Option<bool>,
    /// Whether the manipulated output sits on its limit.
    #[serde(
        default,
        deserialize_with = "present",
        skip_serializing_if = "Option::is_none"
    )]
    pub saturated: Option<bool>,
}

/// One saved identification input (`SavedIdentificationInput`; A8.2): the review input without its validation
/// recording, with the samples in their saved form.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct SavedIdentificationInput {
    /// The original recording the samples were taken from.
    #[serde(
        default,
        deserialize_with = "present",
        skip_serializing_if = "Option::is_none"
    )]
    pub recording: Option<IdentificationRecording>,
    /// The bounded step analysis's settings, when it is on.
    #[serde(
        default,
        deserialize_with = "present",
        skip_serializing_if = "Option::is_none"
    )]
    pub step_response_settings: Option<StepResponseSettings>,
    /// The first-order reference settings of a two-lag request (`sopdt` only).
    #[serde(
        default,
        deserialize_with = "present",
        skip_serializing_if = "Option::is_none"
    )]
    pub first_order_step_response_settings: Option<StepResponseSettings>,
    /// The error limit the input was checked with: the saved state's own.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub maximum_normalized_error: Option<f64>,
    /// The samples, at most the recording sample limit (A7).
    pub estimation: Vec<SavedSample>,
    /// The excluded intervals, at most the recording sample limit (A7).
    pub excluded: Vec<ExcludedInterval>,
    /// Whether the samples come from a simulated or a physical plant.
    pub source: DataSource,
    /// The model the engineer asks for.
    pub requested_kind: IdentificationModelSelection,
    /// The identifier of the recording the samples come from, 1 to 1,024 UTF-16 code units.
    #[serde(
        default,
        deserialize_with = "present",
        skip_serializing_if = "Option::is_none"
    )]
    pub estimation_recording_id: Option<String>,
    /// The process the loop's identification policy declares. A saved input never carries it: the decoder refuses it.
    #[serde(
        default,
        deserialize_with = "present",
        skip_serializing_if = "Option::is_none"
    )]
    pub declared_process: Option<String>,
    /// Whether the fit is closed-loop. A saved input never carries it: the decoder refuses it.
    #[serde(
        default,
        deserialize_with = "present",
        skip_serializing_if = "Option::is_none"
    )]
    pub closed_loop: Option<bool>,
}

/// Identify's saved state (`SavedIdentificationState`; A8.2): the inputs and the explicit review stage. The fits are
/// derived, and reopening replays them.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct SavedIdentificationState {
    /// Always 1.
    pub schema_version: u32,
    /// The workflow's error limit, finite and positive.
    pub maximum_normalized_error: f64,
    /// The stage the review reached.
    pub phase: IdentificationPhase,
    /// The current input; a state that holds only the form has none.
    #[serde(
        default,
        deserialize_with = "present",
        skip_serializing_if = "Option::is_none"
    )]
    pub current: Option<SavedIdentificationInput>,
    /// The estimation input, once a model was fitted from it.
    #[serde(
        default,
        deserialize_with = "present",
        skip_serializing_if = "Option::is_none"
    )]
    pub estimation: Option<SavedIdentificationInput>,
    /// The raw form, while it is edited or pending.
    #[serde(
        default,
        deserialize_with = "present",
        skip_serializing_if = "Option::is_none"
    )]
    pub form: Option<IdentificationFormDraft>,
}

/// The saved form of a review input (`saveIdentificationInput`; A8.2): its JSON copy, in which a missing number (NaN or
/// infinite) of a sample is `null`. The validation recording is not part of a saved input.
#[must_use]
pub fn save_identification_input(input: &IdentificationReviewInput) -> SavedIdentificationInput {
    let IdentificationReviewInput {
        recording,
        step_response_settings,
        first_order_step_response_settings,
        maximum_normalized_error,
        estimation,
        validation: _,
        estimation_recording_id,
        validation_recording_id: _,
        excluded,
        source,
        requested_kind,
        declared_process,
        closed_loop,
    } = input.clone();
    SavedIdentificationInput {
        recording,
        step_response_settings,
        first_order_step_response_settings,
        maximum_normalized_error,
        estimation: estimation.into_iter().map(saved_sample).collect(),
        excluded,
        source,
        requested_kind,
        estimation_recording_id,
        declared_process,
        closed_loop,
    }
}

/// The review input of a saved input (`restoreIdentificationInput`; A8.6): a `null` time, input or output becomes NaN.
#[must_use]
pub fn restore_identification_input(input: &SavedIdentificationInput) -> IdentificationReviewInput {
    let SavedIdentificationInput {
        recording,
        step_response_settings,
        first_order_step_response_settings,
        maximum_normalized_error,
        estimation,
        excluded,
        source,
        requested_kind,
        estimation_recording_id,
        declared_process,
        closed_loop,
    } = input.clone();
    IdentificationReviewInput {
        recording,
        step_response_settings,
        first_order_step_response_settings,
        maximum_normalized_error,
        estimation: estimation.into_iter().map(live_sample).collect(),
        validation: None,
        estimation_recording_id,
        validation_recording_id: None,
        excluded,
        source,
        requested_kind,
        declared_process,
        closed_loop,
    }
}

/// The original recording of a selected run (`identificationOriginalRecording`; A8.1), or `None` without a run: the
/// run with no selected interval, the experiment kind and the provenance of an imported recording. The `TypeScript`
/// defaulted the kind to the provenance's experiment kind, or `observation`; here the caller passes it.
#[must_use]
pub fn identification_original_recording(
    run: Option<&RecordedRun>,
    kind: ControlExperimentKind,
    provenance: Option<RecordingProvenance>,
) -> Option<IdentificationRecording> {
    run.map(|run| IdentificationRecording {
        kind,
        run: run.clone(),
        selected_intervals: Vec::new(),
        provenance,
    })
}

/// Decodes a saved state (`decodeIdentificationState`; A8.6), or `None` when it breaks a rule, checked in the order of
/// the `TypeScript`:
///
/// - schema version 1, a finite positive error limit and a known stage;
/// - each saved input: at most the recording sample limit of samples and of excluded intervals; every number of a
///   sample finite or `null`, every unit at most 256 UTF-16 code units; every excluded interval finite, its end not
///   before its start; a recording identifier of 1 to 1,024 code units; valid step-response settings; first-order
///   reference settings only for a two-lag (`sopdt`) request; a retained recording that matches the input; and no key
///   outside the saved input (`declaredProcess` and `closedLoop` included);
/// - a valid form draft;
/// - no key outside the saved state. A `structureChoice` of the sensor-lag comparison is such a key: that comparison is
///   parked with stage V7, so a state that holds it is refused;
/// - the stage rules: an editing state has a form, and an estimation only beside a current input; a preview, a
///   qualified validation, a review and an accepted model have an estimation; a qualified estimation has none yet; and
///   a preview's current input is its estimation;
/// - each saved input's error limit is the state's own;
/// - the state's JSON text is at most 32 MiB.
///
/// A retained recording matches its input when it has at most the recording sample limit of samples, its identity (the
/// imported identity of its provenance, or else the run's recording identifier) is the input's recording identifier,
/// and the samples it yields for `loop_definition` (`retained_identification_samples_from_runtime_recording`) are the
/// saved samples. The `TypeScript` also compared each editor sample's evidence source with the input's source; a
/// recorded run carries no source, so that comparison has no counterpart here.
#[must_use]
pub fn decode_identification_state(
    value: &Value,
    loop_definition: &LoopDefinition,
) -> Option<SavedIdentificationState> {
    let state = SavedIdentificationState::deserialize(value).ok()?;
    let valid = valid_header(&state)
        && state
            .current
            .as_ref()
            .is_none_or(|input| valid_input(input, loop_definition))
        && state
            .estimation
            .as_ref()
            .is_none_or(|input| valid_input(input, loop_definition))
        && state.form.as_ref().is_none_or(valid_form_draft)
        && valid_stage(&state)
        && same_error_limit(&state)
        && within_text_limit(value);
    valid.then_some(state)
}

/// Whether two sample lists print as the same JSON (`JSON.stringify(previous.estimation) === JSON.stringify(samples)`
/// in the `TypeScript`): a missing number (NaN or infinite) prints as `null`, so two missing numbers are the same.
pub(crate) fn same_samples(left: &[IdentificationSample], right: &[IdentificationSample]) -> bool {
    left.len() == right.len()
        && left
            .iter()
            .zip(right)
            .all(|(left, right)| same_sample(left, right))
}

/// Reads a number that must be present but may be `null` (`number | null` in the `TypeScript`).
fn nullable<'de, D>(deserializer: D) -> Result<Option<f64>, D::Error>
where
    D: Deserializer<'de>,
{
    Option::<f64>::deserialize(deserializer)
}

/// Reads an optional key that is not `null` when present: the `TypeScript` checked such keys against `undefined` and
/// refused `null`.
fn present<'de, D, T>(deserializer: D) -> Result<Option<T>, D::Error>
where
    D: Deserializer<'de>,
    T: Deserialize<'de>,
{
    T::deserialize(deserializer).map(Some)
}

/// A number as JSON prints it: `None` (`null`) when it is NaN or infinite.
fn json_number(value: f64) -> Option<f64> {
    value.is_finite().then_some(value)
}

fn saved_sample(sample: IdentificationSample) -> SavedSample {
    SavedSample {
        time_seconds: json_number(sample.time_seconds),
        input: json_number(sample.input),
        output: json_number(sample.output),
        quality: sample.quality,
        input_unit: sample.input_unit,
        output_unit: sample.output_unit,
        clipped: sample.clipped,
        saturated: sample.saturated,
    }
}

fn live_sample(sample: SavedSample) -> IdentificationSample {
    IdentificationSample {
        time_seconds: sample.time_seconds.unwrap_or(f64::NAN),
        input: sample.input.unwrap_or(f64::NAN),
        output: sample.output.unwrap_or(f64::NAN),
        quality: sample.quality,
        input_unit: sample.input_unit,
        output_unit: sample.output_unit,
        clipped: sample.clipped,
        saturated: sample.saturated,
    }
}

fn same_sample(left: &IdentificationSample, right: &IdentificationSample) -> bool {
    json_number(left.time_seconds) == json_number(right.time_seconds)
        && json_number(left.input) == json_number(right.input)
        && json_number(left.output) == json_number(right.output)
        && left.quality == right.quality
        && left.input_unit == right.input_unit
        && left.output_unit == right.output_unit
        && left.clipped == right.clipped
        && left.saturated == right.saturated
}

/// Whether a sample prints as a saved sample: a missing number prints as `null`.
fn matches_saved(sample: &IdentificationSample, saved: &SavedSample) -> bool {
    json_number(sample.time_seconds) == saved.time_seconds
        && json_number(sample.input) == saved.input
        && json_number(sample.output) == saved.output
        && sample.quality == saved.quality
        && sample.input_unit == saved.input_unit
        && sample.output_unit == saved.output_unit
        && sample.clipped == saved.clipped
        && sample.saturated == saved.saturated
}

fn valid_header(state: &SavedIdentificationState) -> bool {
    state.schema_version == IDENTIFICATION_STATE_SCHEMA_VERSION
        && state.maximum_normalized_error.is_finite()
        && state.maximum_normalized_error > 0.0
}

/// The checks of one saved input (`input` in the `TypeScript`), in its order; the key list is last.
fn valid_input(input: &SavedIdentificationInput, loop_definition: &LoopDefinition) -> bool {
    within_sample_limit(input.estimation.len())
        && input.estimation.iter().all(valid_sample)
        && within_sample_limit(input.excluded.len())
        && input.excluded.iter().all(valid_interval)
        && input
            .estimation_recording_id
            .as_deref()
            .is_none_or(valid_recording_id)
        && input
            .step_response_settings
            .as_ref()
            .is_none_or(valid_step_response_settings)
        && input
            .first_order_step_response_settings
            .as_ref()
            .is_none_or(|settings| {
                input.requested_kind == IdentificationModelSelection::Sopdt
                    && valid_step_response_settings(settings)
            })
        && recording_matches_input(input, loop_definition)
        && input.declared_process.is_none()
        && input.closed_loop.is_none()
}

/// The checks of one saved sample (`sample` in the `TypeScript`) that its type does not already hold.
fn valid_sample(sample: &SavedSample) -> bool {
    [sample.time_seconds, sample.input, sample.output]
        .into_iter()
        .all(|number| number.is_none_or(f64::is_finite))
        && [sample.input_unit.as_deref(), sample.output_unit.as_deref()]
            .into_iter()
            .all(|unit| unit.is_none_or(|text| utf16_length(text) <= MAXIMUM_UNIT_LENGTH))
}

fn valid_interval(interval: &ExcludedInterval) -> bool {
    interval.start_seconds.is_finite()
        && interval.end_seconds.is_finite()
        && interval.end_seconds >= interval.start_seconds
}

fn valid_recording_id(id: &str) -> bool {
    (1..=MAXIMUM_RECORDING_ID_LENGTH).contains(&utf16_length(id))
}

/// Whether a saved input's retained recording matches it (`recordingMatchesInput`); an input without a recording does.
fn recording_matches_input(
    input: &SavedIdentificationInput,
    loop_definition: &LoopDefinition,
) -> bool {
    let Some(recording) = &input.recording else {
        return true;
    };
    if !within_sample_limit(recording.run.samples.len()) {
        return false;
    }
    let id = recording
        .provenance
        .as_ref()
        .map_or_else(|| recording.run.recording_id.clone(), imported_recording_id);
    if id.is_empty() || input.estimation_recording_id.as_deref() != Some(id.as_str()) {
        return false;
    }
    retained_identification_samples_from_runtime_recording(
        recording.kind,
        &recording.run,
        loop_definition,
    )
    .is_ok_and(|projection| {
        projection.len() == input.estimation.len()
            && projection
                .iter()
                .zip(&input.estimation)
                .all(|(sample, saved)| matches_saved(sample, saved))
    })
}

/// The rules between the stage and the saved inputs.
fn valid_stage(state: &SavedIdentificationState) -> bool {
    let has_estimation = state.estimation.is_some();
    match state.phase {
        IdentificationPhase::Editing => {
            state.form.is_some() && (state.current.is_some() || !has_estimation)
        }
        // Decoded records hold only finite numbers, so comparing them is comparing their JSON, as the `TypeScript` did.
        IdentificationPhase::Preview => {
            state.current.is_some() && state.current == state.estimation
        }
        IdentificationPhase::ValidationQualified
        | IdentificationPhase::Review
        | IdentificationPhase::Accepted => has_estimation,
        IdentificationPhase::Qualified => !has_estimation,
        IdentificationPhase::Selected
        | IdentificationPhase::Rejected
        | IdentificationPhase::Failed => true,
    }
}

fn same_error_limit(state: &SavedIdentificationState) -> bool {
    [&state.current, &state.estimation]
        .into_iter()
        .flatten()
        .all(|input| input.maximum_normalized_error == Some(state.maximum_normalized_error))
}

fn within_text_limit(value: &Value) -> bool {
    serde_json::to_string(value).is_ok_and(|text| utf16_length(&text) <= MAXIMUM_STATE_TEXT_LENGTH)
}

/// Whether a count of samples or intervals stays within the recording sample limit (A7).
fn within_sample_limit(count: usize) -> bool {
    u64::try_from(count).is_ok_and(|count| count <= RECORDING_SAMPLE_LIMIT)
}

/// The length of a text in UTF-16 code units, as JavaScript counts a string's length.
fn utf16_length(text: &str) -> usize {
    text.encode_utf16().count()
}

#[cfg(test)]
#[path = "state_tests.rs"]
mod tests;
