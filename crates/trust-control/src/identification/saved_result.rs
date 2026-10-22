//! The decoder of a saved Control Studio result (`decodeWorkflowResult`, `workflowResult.ts`; specification 35, B10.2 and
//! B10.4). The import reads a saved result or recording file through `read_saved_result`; the rules of the import are in
//! `recording_import`. The decoder checks every field it reads as the TypeScript decoder does: the schema version (3 and 4),
//! the identity, the loop, the controller, the software, the units and the source, every raw sample, the selected intervals,
//! the acquisition, the provenance of a copied recording (schema 4 only), the experiment and the plant for the fields the
//! import reads, and the absence of credentials. The rest of the experiment and of the plant is kept by the file the import
//! writes (`recording::context::saved_context`, B10.2), which leaves out a part it does not read and never refuses the file for
//! it. Parts of the TypeScript decoder are not ported yet, and the decoder does not
//! read them: the sections of later stages (the identification study beyond its step-response reference, the model, the
//! controller candidates, the tuning comparison, the analyses, the commissioning and the engineer's study); the additional
//! channels of a sample, which it keeps as written; and the optional `method`. The TypeScript decoder still checks those for
//! parked loops until V7.

use std::collections::BTreeMap;

use serde::Deserialize;
use serde_json::Value;

use crate::identification::recording_import::{SavedProvenance, SavedRecordingData};
use crate::identification::refusal::Refusal;
use crate::identification::state::RecordingProvenance;
use crate::recording::{Acquisition, Completeness, ScanRange};

/// The sentence of a file that is not JSON (`readIdentificationRecordingImport`).
pub(crate) const NOT_JSON_SENTENCE: &str =
    "Choose a valid Control Studio recording or result JSON file.";
/// The sentence of a file whose saved result does not decode (`readIdentificationRecordingImport`).
pub(crate) const INVALID_SENTENCE: &str =
    "The saved recording has invalid data or source metadata.";

/// A saved Control Studio result as the file writes it (`ControlStudioWorkflowResult`, schema 3 or 4), with the fields the
/// check reads. Keys the check does not read are ignored.
#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
struct SavedResultFile {
    schema_version: u64,
    result_id: String,
    result_revision: u64,
    created_at: String,
    project_id: String,
    source: String,
    #[serde(rename = "loop")]
    loop_record: SavedLoop,
    controller: SavedController,
    software: SavedSoftware,
    units: SavedUnits,
    recording: SavedRecording,
    evidence: SavedEvidence,
    conclusion: String,
    experiment: Option<Value>,
    plant: Option<Value>,
    identification_study: Option<Value>,
}

/// The loop a saved result or its provenance was taken from (`loop`).
#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
struct SavedLoop {
    id: String,
    revision: String,
    topology: Option<String>,
}

/// The controller a saved result was taken with.
#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
struct SavedController {
    instance: String,
    revision: String,
}

/// The software that wrote a saved result: the extension, the runtime when there was one, and the numerical adapters.
#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
struct SavedSoftware {
    extension_version: String,
    runtime_version: Option<String>,
    numerical_adapters: BTreeMap<String, String>,
}

/// The units of a saved result, of its provenance or of one of its samples: the setpoint, the measured output and the input.
#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
struct SavedUnits {
    sp: String,
    pv: String,
    mv: String,
}

/// The evidence a saved result carries (`evidence`): whether it was observed, and which model it has.
#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
struct SavedEvidence {
    observation: String,
    model: String,
}

/// The recording of a saved result: its raw samples, the selected intervals, the acquisition and the provenance it was taken
/// from when the result is a copy of another (schema 4).
#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
struct SavedRecording {
    raw_samples: Vec<SavedSample>,
    selected_intervals: Vec<SavedInterval>,
    acquisition: Option<SavedAcquisition>,
    provenance: Option<SavedResultProvenance>,
}

/// A selected interval of a recording, in seconds of process time.
#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
struct SavedInterval {
    start_seconds: f64,
    end_seconds: f64,
}

/// The limits of a sample's output (`outputLimits`).
#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
struct SavedLimits {
    minimum: f64,
    maximum: f64,
}

/// One raw sample of a recording (`WorkflowResultSample`). The nullable fields are read with `present_nullable`, so a missing
/// key is refused and a null is a value.
#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
struct SavedSample {
    elapsed_seconds: f64,
    observed_at_ms: f64,
    #[serde(deserialize_with = "present_nullable")]
    actual_cycle_seconds: Nullable<f64>,
    editor_observation_interval_seconds: Option<f64>,
    scan: u64,
    source: String,
    loop_revision: String,
    #[serde(deserialize_with = "present_nullable")]
    sp: Nullable<f64>,
    #[serde(deserialize_with = "present_nullable")]
    pv: Nullable<f64>,
    #[serde(deserialize_with = "present_nullable")]
    requested_mv: Nullable<f64>,
    #[serde(deserialize_with = "present_nullable")]
    applied_mv: Nullable<f64>,
    units: SavedUnits,
    quality: String,
    #[serde(deserialize_with = "present_nullable")]
    freshness_ms: Nullable<f64>,
    controller_mode: String,
    output_limits: SavedLimits,
    safety_state: String,
    events: Vec<String>,
}

/// The acquisition of a recording as the file writes it (`WorkflowRecordingAcquisition`).
#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
struct SavedAcquisition {
    method: String,
    completeness: String,
    observed_sample_count: u64,
    scan_range: Option<ScanRange>,
    missing_scan_ranges: Vec<ScanRange>,
    dropped_observed_samples: u64,
    actual_cycle_timing_available: bool,
    freshness_available: bool,
    reason: String,
    program_revision: Option<String>,
    recording_id: Option<String>,
}

/// The provenance of a recording copied from another saved result (`WorkflowRecordingProvenance`, schema 4). Its keys are
/// exactly the ones the TypeScript decoder allows.
#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
struct SavedResultProvenance {
    schema_version: u64,
    result_id: String,
    result_revision: u64,
    created_at: String,
    project_id: String,
    source: String,
    #[serde(rename = "loop")]
    loop_record: SavedLoop,
    controller: SavedController,
    software: SavedSoftware,
    units: SavedUnits,
    plant: Option<Value>,
    experiment: Option<Value>,
}

/// A value of a sample whose key must be present, though the value may be null (`nullableFinite`): `Null` is a null, and `Value`
/// is the number. A missing key is no `Nullable` at all: serde refuses a field with `deserialize_with` and no default before it
/// calls the function, so the result does not decode.
#[derive(Debug)]
enum Nullable<T> {
    /// The key is present and its value is null.
    Null,
    /// The key is present and holds a value.
    Value(T),
}

/// Reads a value whose key must be present, though its value may be null (`nullableFinite`): a null is `Nullable::Null`, a
/// number is `Nullable::Value`, and a missing key is a decode error.
fn present_nullable<'de, D, T>(deserializer: D) -> Result<Nullable<T>, D::Error>
where
    D: serde::Deserializer<'de>,
    T: Deserialize<'de>,
{
    Option::<T>::deserialize(deserializer)
        .map(|value| value.map_or(Nullable::Null, Nullable::Value))
}

/// Decodes a saved result (`decodeWorkflowResult`; B10.2, B10.4). A value with a credential key anywhere does not decode.
fn decode_saved_result(value: &Value) -> Option<SavedResultFile> {
    if contains_credential_key(value) {
        return None;
    }
    let file = SavedResultFile::deserialize(value).ok()?;
    (valid_header(&file) && valid_recording(&file)).then_some(file)
}

/// The rules of the result's own fields (`decodeWorkflowResult`): the schema version, the identity, the source, the loop, the
/// controller, the software, the units, the evidence, the conclusion, and the plant and experiment the import reads.
fn valid_header(file: &SavedResultFile) -> bool {
    let open_loop_sp = file.schema_version == 4 && is_open_loop(&file.loop_record);
    matches!(file.schema_version, 3 | 4)
        && valid_identity(
            &file.result_id,
            file.result_revision,
            &file.project_id,
            &file.created_at,
        )
        && is_source(&file.source)
        && valid_loop(&file.loop_record)
        && valid_controller(&file.controller)
        && valid_software(&file.software)
        && valid_units(&file.units, open_loop_sp)
        && valid_evidence(&file.evidence)
        && nonempty(&file.conclusion)
        && file
            .plant
            .as_ref()
            .is_none_or(|plant| plant_source(plant) == Some(file.source.as_str()))
        && file.experiment.as_ref().is_none_or(valid_experiment)
}

/// The rules of the result's recording (`recordingMatchesResult`, `recordingAcquisition`, `workflowSample`): the samples, the
/// selected intervals, the acquisition, the provenance (schema 4 only) and the order of the samples against the result.
fn valid_recording(file: &SavedResultFile) -> bool {
    let recording = &file.recording;
    let sample_count = recording.raw_samples.len();
    recording.raw_samples.iter().all(valid_sample)
        && recording.selected_intervals.iter().all(valid_interval)
        && recording
            .acquisition
            .as_ref()
            .is_none_or(|acquisition| valid_acquisition(acquisition, sample_count))
        && recording
            .provenance
            .as_ref()
            .is_none_or(|provenance| file.schema_version == 4 && valid_provenance(provenance))
        && recording_matches_result(file)
}

/// The rules of the provenance of a recording copied from another result (`recordingProvenance`).
fn valid_provenance(provenance: &SavedResultProvenance) -> bool {
    provenance.schema_version == 1
        && valid_identity(
            &provenance.result_id,
            provenance.result_revision,
            &provenance.project_id,
            &provenance.created_at,
        )
        && is_source(&provenance.source)
        && valid_loop(&provenance.loop_record)
        && valid_controller(&provenance.controller)
        && valid_software(&provenance.software)
        && valid_units(&provenance.units, is_open_loop(&provenance.loop_record))
        && provenance
            .plant
            .as_ref()
            .is_none_or(|plant| plant_source(plant) == Some(provenance.source.as_str()))
        && provenance.experiment.as_ref().is_none_or(valid_experiment)
}

/// The rules of one raw sample (`workflowSample`). An optional interval or channel is not required; a required nullable value
/// must be present, and a null is a missing value.
fn valid_sample(sample: &SavedSample) -> bool {
    sample.observed_at_ms >= 0.0
        && nullable(&sample.actual_cycle_seconds, |seconds| seconds > 0.0)
        && sample
            .editor_observation_interval_seconds
            .is_none_or(|seconds| seconds > 0.0)
        && [
            &sample.sp,
            &sample.pv,
            &sample.requested_mv,
            &sample.applied_mv,
        ]
        .iter()
        .all(|field| matches!(field, Nullable::Null | Nullable::Value(_)))
        && nullable(&sample.freshness_ms, |milliseconds| milliseconds >= 0.0)
        && is_source(&sample.source)
        && revision(&sample.loop_revision)
        && nonempty(&sample.units.pv)
        && nonempty(&sample.units.mv)
        && matches!(sample.quality.as_str(), "good" | "forced" | "bad")
        && matches!(
            sample.controller_mode.as_str(),
            "Auto" | "Manual" | "Track" | "Unavailable"
        )
        && sample.output_limits.minimum <= sample.output_limits.maximum
        && nonempty(&sample.safety_state)
        && sample.events.len() <= 128
        && sample.events.iter().all(|event| nonempty(event))
}

/// Whether a required nullable value holds `ok` when it is a number (`null` is accepted, a missing key is not).
fn nullable(field: &Nullable<f64>, ok: impl Fn(f64) -> bool) -> bool {
    match field {
        Nullable::Null => true,
        Nullable::Value(value) => ok(*value),
    }
}

/// The rules of one selected interval (`interval`).
fn valid_interval(interval: &SavedInterval) -> bool {
    interval.start_seconds >= 0.0 && interval.end_seconds >= interval.start_seconds
}

/// The rules of an acquisition (`recordingAcquisition`): its method and completeness, the samples it counts, its reason, its
/// sequence ranges and its identities, and an interface-only method that claims timing or freshness is refused.
fn valid_acquisition(acquisition: &SavedAcquisition, sample_count: usize) -> bool {
    let counted = u64::try_from(sample_count).unwrap_or(u64::MAX);
    matches!(
        acquisition.method.as_str(),
        "ui_observed_snapshots" | "runtime_scan_buffer"
    ) && matches!(acquisition.completeness.as_str(), "incomplete" | "complete")
        && acquisition.observed_sample_count >= counted
        && nonempty(&acquisition.reason)
        && acquisition.scan_range.as_ref().is_none_or(valid_scan_range)
        && acquisition.missing_scan_ranges.iter().all(valid_scan_range)
        && acquisition.program_revision.as_deref().is_none_or(nonempty)
        && acquisition.recording_id.as_deref().is_none_or(nonempty)
        && !(acquisition.method == "ui_observed_snapshots"
            && (acquisition.completeness != "incomplete"
                || acquisition.actual_cycle_timing_available
                || acquisition.freshness_available))
}

/// The rules of a sequence range: its last value is not before its first.
fn valid_scan_range(range: &ScanRange) -> bool {
    range.last >= range.first
}

/// The rules of the result's and the provenance's identity (`identity` of B10.2): a nonempty identifier and project, a
/// positive revision, and a creation time in the ISO 8601 form the extension writes.
fn valid_identity(result_id: &str, revision: u64, project_id: &str, created_at: &str) -> bool {
    nonempty(result_id) && revision > 0 && nonempty(project_id) && iso_timestamp(created_at)
}

/// The rules of a loop reference: an identifier, a `sha256:` revision, and no topology or one of the two topologies.
fn valid_loop(loop_record: &SavedLoop) -> bool {
    nonempty(&loop_record.id)
        && revision(&loop_record.revision)
        && matches!(
            loop_record.topology.as_deref(),
            None | Some("open-loop" | "closed-loop")
        )
}

/// The rules of a controller reference: an instance and a `sha256:` revision.
fn valid_controller(controller: &SavedController) -> bool {
    nonempty(&controller.instance) && revision(&controller.revision)
}

/// The rules of the software that wrote a result: the extension, an optional runtime, and named numerical adapters.
fn valid_software(software: &SavedSoftware) -> bool {
    nonempty(&software.extension_version)
        && software.runtime_version.as_deref().is_none_or(nonempty)
        && software
            .numerical_adapters
            .iter()
            .all(|(name, version)| nonempty(name) && nonempty(version))
}

/// The rules of the units of a result or provenance: the setpoint unit is empty for an open loop of schema 4, and the
/// measured output and the input units are named (`units` of `decodeWorkflowResult`).
fn valid_units(units: &SavedUnits, setpoint_is_empty: bool) -> bool {
    let setpoint_ok = if setpoint_is_empty {
        units.sp.is_empty()
    } else {
        nonempty(&units.sp)
    };
    setpoint_ok && nonempty(&units.pv) && nonempty(&units.mv)
}

/// The rules of the evidence a result carries: its observation and its model, as the TypeScript decoder names them.
fn valid_evidence(evidence: &SavedEvidence) -> bool {
    matches!(evidence.observation.as_str(), "Observed" | "Not observed")
        && matches!(
            evidence.model.as_str(),
            "Not evaluated"
                | "Exact simulation model"
                | "Identified from simulation data"
                | "Identified from physical data"
                | "Model-predicted"
        )
}

/// The rules of an experiment the import reads (`experimentContext`, checked for its status): an object whose status is one of
/// the four the experiment can end with. The import needs no other field of it.
fn valid_experiment(experiment: &Value) -> bool {
    experiment.is_object()
        && experiment
            .get("status")
            .and_then(Value::as_str)
            .is_some_and(|status| {
                matches!(
                    status,
                    "completed" | "aborted" | "failed" | "failed_to_restore"
                )
            })
}

/// The source of the plant an object describes, when it names one.
fn plant_source(plant: &Value) -> Option<&str> {
    plant.get("source").and_then(Value::as_str)
}

/// Whether the source is one a saved result names: `Simulation` or `Physical`.
fn is_source(text: &str) -> bool {
    matches!(text, "Simulation" | "Physical")
}

/// Whether the loop of a reference is open (`topology: "open-loop"`).
fn is_open_loop(loop_record: &SavedLoop) -> bool {
    loop_record.topology.as_deref() == Some("open-loop")
}

/// Whether the recording's samples agree with the result (`recordingMatchesResult`): the same source, loop revision and units
/// as the result, or as the provenance the recording was copied from; a strictly increasing elapsed time and scan; an observed
/// time that increases too, unless the acquisition is a runtime scan buffer of schema 4; no setpoint in an open loop of
/// schema 4; and every selected interval within the recording.
fn recording_matches_result(file: &SavedResultFile) -> bool {
    let recording = &file.recording;
    let origin = recording.provenance.as_ref();
    let loop_record = origin.map_or(&file.loop_record, |origin| &origin.loop_record);
    let units = origin.map_or(&file.units, |origin| &origin.units);
    if origin.is_some_and(|origin| {
        origin.source != file.source
            || origin.units.pv != file.units.pv
            || origin.units.mv != file.units.mv
    }) {
        return false;
    }
    let runtime_ordering = file.schema_version == 4
        && recording
            .acquisition
            .as_ref()
            .is_some_and(|acquisition| acquisition.method == "runtime_scan_buffer");
    let open_loop = file.schema_version == 4 && is_open_loop(loop_record);
    let mut previous: Option<&SavedSample> = None;
    for sample in &recording.raw_samples {
        let units_match = sample.units.sp == units.sp
            && sample.units.pv == units.pv
            && sample.units.mv == units.mv;
        if sample.source != file.source
            || sample.loop_revision != loop_record.revision
            || !units_match
            || (open_loop && matches!(sample.sp, Nullable::Value(_)))
            || sample.elapsed_seconds < 0.0
        {
            return false;
        }
        if let Some(before) = previous {
            if sample.elapsed_seconds <= before.elapsed_seconds
                || (!runtime_ordering && sample.observed_at_ms <= before.observed_at_ms)
                || sample.scan <= before.scan
            {
                return false;
            }
        }
        previous = Some(sample);
    }
    let last_elapsed = recording
        .raw_samples
        .last()
        .map(|sample| sample.elapsed_seconds);
    recording
        .selected_intervals
        .iter()
        .all(|interval| last_elapsed.is_some_and(|last| interval.end_seconds <= last))
}

/// Whether the result is a step-response reference: an identification whose estimation is first order with dead time, and
/// keeps its step-response settings and its recording (`reference` of `readIdentificationRecordingImport`).
fn has_step_response_reference(file: &SavedResultFile) -> bool {
    file.identification_study
        .as_ref()
        .and_then(|study| study.get("estimation"))
        .is_some_and(|estimation| {
            estimation.get("requestedKind").and_then(Value::as_str) == Some("fopdt")
                && present(estimation.get("stepResponseSettings"))
                && present(estimation.get("recording"))
        })
}

/// Whether a value exists and is not null.
fn present(value: Option<&Value>) -> bool {
    value.is_some_and(|value| !value.is_null())
}

/// The recording of a decoded result as the acquisition check reads it.
fn recording_data(file: &SavedResultFile) -> SavedRecordingData {
    SavedRecordingData {
        acquisition: file.recording.acquisition.as_ref().map(to_acquisition),
        scans: file
            .recording
            .raw_samples
            .iter()
            .map(|sample| sample.scan)
            .collect(),
    }
}

/// The acquisition as the rule layer reads it. The completeness is one of the two words the decoder allowed.
fn to_acquisition(saved: &SavedAcquisition) -> Acquisition {
    Acquisition {
        method: saved.method.clone(),
        completeness: if saved.completeness == "complete" {
            Completeness::Complete
        } else {
            Completeness::Incomplete
        },
        observed_sample_count: saved.observed_sample_count,
        scan_range: saved.scan_range,
        missing_scan_ranges: saved.missing_scan_ranges.clone(),
        dropped_observed_samples: saved.dropped_observed_samples,
        actual_cycle_timing_available: saved.actual_cycle_timing_available,
        freshness_available: saved.freshness_available,
        reason: saved.reason.clone(),
    }
}

/// The provenance the import checks (`readIdentificationRecordingImport`): the recording's own provenance when it has one, or
/// else the result's identity, units and experiment.
fn saved_provenance(file: &SavedResultFile) -> SavedProvenance {
    match &file.recording.provenance {
        Some(provenance) => SavedProvenance {
            identity: RecordingProvenance {
                project_id: provenance.project_id.clone(),
                result_id: provenance.result_id.clone(),
                result_revision: provenance.result_revision,
            },
            input_unit: provenance.units.mv.clone(),
            output_unit: provenance.units.pv.clone(),
            experiment_status: experiment_status(provenance.experiment.as_ref()),
        },
        None => SavedProvenance {
            identity: RecordingProvenance {
                project_id: file.project_id.clone(),
                result_id: file.result_id.clone(),
                result_revision: file.result_revision,
            },
            input_unit: file.units.mv.clone(),
            output_unit: file.units.pv.clone(),
            experiment_status: experiment_status(file.experiment.as_ref()),
        },
    }
}

/// The status an experiment ended with, when the experiment has one.
fn experiment_status(experiment: Option<&Value>) -> Option<String> {
    experiment
        .and_then(|value| value.get("status"))
        .and_then(Value::as_str)
        .map(str::to_owned)
}

/// Whether any key of the value names a credential (`containsSecretKey`): a key that contains `token`, `password`,
/// `credential`, `secret` or `authorization`, in any letter case.
fn contains_credential_key(value: &Value) -> bool {
    match value {
        Value::Object(map) => map
            .iter()
            .any(|(key, item)| credential_word(key) || contains_credential_key(item)),
        Value::Array(items) => items.iter().any(contains_credential_key),
        _ => false,
    }
}

/// Whether a key names a credential.
fn credential_word(key: &str) -> bool {
    let lower = key.to_ascii_lowercase();
    ["token", "password", "credential", "secret", "authorization"]
        .iter()
        .any(|word| lower.contains(*word))
}

/// Whether a text is nonempty once its spaces are trimmed (`nonempty`).
fn nonempty(text: &str) -> bool {
    !text.trim().is_empty()
}

/// Whether a text is a `sha256:` revision (`revision`).
fn revision(text: &str) -> bool {
    nonempty(text) && text.starts_with("sha256:")
}

/// Whether a text is an ISO 8601 date and time with a zone, the form the extension writes `createdAt` in: `YYYY-MM-DDTHH:MM:SS`,
/// an optional fraction of seconds, then `Z` or an offset `±HH:MM`.
fn iso_timestamp(text: &str) -> bool {
    let bytes = text.as_bytes();
    if bytes.len() < 20
        || bytes[4] != b'-'
        || bytes[7] != b'-'
        || bytes[10] != b'T'
        || bytes[13] != b':'
        || bytes[16] != b':'
    {
        return false;
    }
    let fields = [(0, 4), (5, 7), (8, 10), (11, 13), (14, 16), (17, 19)];
    if !fields
        .iter()
        .all(|&(start, end)| all_digits(&bytes[start..end]))
    {
        return false;
    }
    let number = |start: usize, end: usize| {
        bytes[start..end]
            .iter()
            .fold(0_u32, |value, digit| value * 10 + u32::from(*digit - b'0'))
    };
    let (month, day, hour, minute, second) = (
        number(5, 7),
        number(8, 10),
        number(11, 13),
        number(14, 16),
        number(17, 19),
    );
    if !(1..=12).contains(&month)
        || !(1..=31).contains(&day)
        || hour > 23
        || minute > 59
        || second > 60
    {
        return false;
    }
    let mut rest = &bytes[19..];
    if let [b'.', tail @ ..] = rest {
        let digits = tail.iter().take_while(|byte| byte.is_ascii_digit()).count();
        if digits == 0 {
            return false;
        }
        rest = &tail[digits..];
    }
    match rest {
        [b'Z'] => true,
        [sign, hours_tens, hours_units, b':', minutes_tens, minutes_units] => {
            (*sign == b'+' || *sign == b'-')
                && all_digits(&[*hours_tens, *hours_units])
                && all_digits(&[*minutes_tens, *minutes_units])
        }
        _ => false,
    }
}

/// Whether every byte of a slice is an ASCII digit.
fn all_digits(bytes: &[u8]) -> bool {
    bytes.iter().all(u8::is_ascii_digit)
}

/// What the import reads of a saved result: its identity, its recording as the acquisition check reads it, the provenance the
/// check reads, the sample count and whether it keeps a step-response reference.
pub(crate) struct SavedResult {
    /// The identifier of the saved result.
    pub(crate) result_id: String,
    /// The revision of the saved result.
    pub(crate) result_revision: u64,
    /// The project the result was saved in.
    pub(crate) project_id: String,
    /// The source of the recording as the file writes it.
    pub(crate) source: String,
    /// The number of raw samples.
    pub(crate) samples: u64,
    /// The recording's acquisition and sequence numbers.
    pub(crate) recording: SavedRecordingData,
    /// The provenance the import checks.
    pub(crate) provenance: SavedProvenance,
    /// Whether the result keeps a step-response reference (`identificationStudy`), which the import does not read.
    pub(crate) step_response_reference: bool,
}

/// Reads a saved result from the text of its file (`readIdentificationRecordingImport`): the text is JSON, and the result
/// decodes as a whole (B10.4). Nothing is checked against a loop here.
///
/// # Errors
/// The text is not JSON, or the saved result does not decode (a schema version other than 3 or 4, a malformed identity, a
/// nonfinite or out-of-order sample, inconsistent units or source, an invalid selected interval, or a credential key).
pub(crate) fn read_saved_result(json: &str) -> Result<SavedResult, Refusal> {
    let value: Value = serde_json::from_str(json).map_err(|_| Refusal::new(NOT_JSON_SENTENCE))?;
    let file = decode_saved_result(&value).ok_or_else(|| Refusal::new(INVALID_SENTENCE))?;
    let step_response_reference = has_step_response_reference(&file);
    let recording = recording_data(&file);
    let provenance = saved_provenance(&file);
    let samples = u64::try_from(file.recording.raw_samples.len()).unwrap_or(u64::MAX);
    Ok(SavedResult {
        result_id: file.result_id,
        result_revision: file.result_revision,
        project_id: file.project_id,
        source: file.source,
        samples,
        recording,
        provenance,
        step_response_reference,
    })
}
