//! Reading a saved recording for identification (`identificationRecordingImport.ts`; specification 35, A8.6, A9 and B9.9).
//! A saved recording is measured data with its original identity. Importing it keeps the identity and the evidence, and
//! never adopts the design approval of the file it came from.
//!
//! The rules here are the checks of an import: the size limit, the acquisition and its steady record period, the
//! provenance (an aborted experiment or other units are refused), the display of units, and the routing of the scoped
//! import message. `check_import` reads a saved result through `read_saved_result` (`decodeWorkflowResult`, B10.4), then
//! applies those checks. The decoder lives in `saved_result`, with the limits of what it reads and the TypeScript decoder
//! sections it does not port yet. A step-response reference is refused here. Fitting an imported file and keeping it in the
//! study are not ported yet.

use serde::Serialize;
use serde_json::Value;

use crate::identification::refusal::Refusal;
use crate::identification::review::IdentificationModelSelection;
use crate::identification::saved_result::read_saved_result;
use crate::identification::state::{RecordingProvenance, SavedIdentificationInput};
use crate::limits::RECORDING_SAMPLE_LIMIT;
use crate::loop_definition::LoopDefinition;
use crate::recording::sequence::steady_record_period;
use crate::recording::{Acquisition, Completeness};

pub mod archive;

/// The largest recording file an import reads, in bytes of its text (`MAX_RECORDING_IMPORT_BYTES`).
pub const MAXIMUM_RECORDING_IMPORT_BYTES: usize = 32 * 1024 * 1024;

/// The recording of a saved result, as the import reads it: the runtime acquisition evidence and the scan of each raw
/// sample.
#[derive(Debug, Clone, PartialEq)]
pub struct SavedRecordingData {
    /// The acquisition evidence the recording was taken with.
    pub acquisition: Option<Acquisition>,
    /// The scan number of each raw sample, in order.
    pub scans: Vec<u64>,
}

/// The provenance of a saved recording, as the import reads it: the identity of the saved result, the units of its plant
/// input and measured output, and the status of its experiment when it had one.
#[derive(Debug, Clone, PartialEq)]
pub struct SavedProvenance {
    /// The identity of the saved result.
    pub identity: RecordingProvenance,
    /// The unit of the plant input (`units.mv`).
    pub input_unit: String,
    /// The unit of the measured output (`units.pv`).
    pub output_unit: String,
    /// The status of the experiment that produced the recording, when it had one.
    pub experiment_status: Option<String>,
}

/// The scope a file-import message must match (`LoopMessageScope`): the project, the loop, the editor session and the loop
/// revision it was sent for.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ImportScope {
    /// The project of the loop.
    pub project_id: String,
    /// The loop.
    pub loop_id: String,
    /// The editor session that sent the message.
    pub editor_session_id: String,
    /// The loop revision the message expects.
    pub loop_revision: String,
}

/// What a message is for the import (`routeIdentificationRecordingImport`).
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ImportRoute {
    /// The message is not a file import; another handler takes it.
    NotAnImport,
    /// A file import that does not match its scope or its shape: it is ignored and gets no reply.
    Ignored,
    /// A well-formed file import of the scope, with the file text.
    Import {
        /// The identifier of the message, which the reply carries.
        message_id: String,
        /// The text of the file.
        json: String,
    },
}

/// The reason an import is refused while another recording is active.
pub const BUSY_REASON: &str = "Stop the active recording before importing another recording.";

/// Refuses an import file whose text is larger than the limit (`readIdentificationRecordingImport`: the length in UTF-16
/// code units).
///
/// # Errors
/// The text is larger than `32 MiB`.
pub fn check_import_length(json: &str) -> Result<(), Refusal> {
    if json.encode_utf16().count() > MAXIMUM_RECORDING_IMPORT_BYTES {
        return Err(Refusal::new("Recording file exceeds 32 MiB."));
    }
    Ok(())
}

/// Refuses a file opened from a recording whose UTF-8 text is larger than the limit (`identifyOpenedRecording`).
///
/// # Errors
/// The file is larger than `32 MiB`.
pub fn check_opened_file_size(json: &str) -> Result<(), Refusal> {
    if json.len() > MAXIMUM_RECORDING_IMPORT_BYTES {
        return Err(Refusal::new(
            "Choose a Control Studio recording of at most 32 MiB.",
        ));
    }
    Ok(())
}

/// Whether a saved identification is the reference of an import: a second-order-with-dead-time fit that kept its step
/// response and its recording (`requestedKind === "fopdt"`, the settings and the recording present).
#[must_use]
pub fn is_import_reference(estimation: &SavedIdentificationInput) -> bool {
    estimation.requested_kind == IdentificationModelSelection::Fopdt
        && estimation.step_response_settings.is_some()
        && estimation.recording.is_some()
}

/// Checks the acquisition of a saved recording (`readIdentificationRecordingImport`): a complete runtime scan buffer with
/// no missing or dropped sample, its timing available, two to the sample limit of samples whose scans match the acquisition
/// range, and a steady record period.
///
/// # Errors
/// The recording is not a complete recording of runtime samples at a steady record period (A7, annex E4.5).
pub fn check_saved_acquisition(data: &SavedRecordingData) -> Result<(), Refusal> {
    let refused = || {
        Refusal::new(format!(
            "Identify requires a complete recording of 2–{RECORDING_SAMPLE_LIMIT} runtime samples at a steady record period."
        ))
    };
    let Some(acquisition) = &data.acquisition else {
        return Err(refused());
    };
    let samples = &data.scans;
    let count = u64::try_from(samples.len()).unwrap_or(u64::MAX);
    let range_matches = match (acquisition.scan_range, samples.first(), samples.last()) {
        (Some(range), Some(first), Some(last)) => range.first == *first && range.last == *last,
        _ => false,
    };
    let valid = acquisition.method == "runtime_scan_buffer"
        && acquisition.completeness == Completeness::Complete
        && acquisition.missing_scan_ranges.is_empty()
        && acquisition.dropped_observed_samples == 0
        && acquisition.actual_cycle_timing_available
        && (2..=RECORDING_SAMPLE_LIMIT).contains(&count)
        && acquisition.observed_sample_count == count
        && range_matches
        && steady_record_period(samples, true);
    if valid {
        Ok(())
    } else {
        Err(refused())
    }
}

/// Checks the provenance of a saved recording against the loop (`readIdentificationRecordingImport`): an experiment that
/// was not completed cannot supply an Identify recording, and the units must match the loop's, with no conversion.
///
/// # Errors
/// The experiment was aborted or failed, or the units differ from the loop's.
pub fn check_saved_provenance(
    provenance: &SavedProvenance,
    loop_definition: &LoopDefinition,
) -> Result<(), Refusal> {
    if provenance
        .experiment_status
        .as_deref()
        .is_some_and(|status| status != "completed")
    {
        return Err(Refusal::new(
            "An aborted or failed experiment cannot supply an Identify recording.",
        ));
    }
    let loop_input = display_engineering_unit(&loop_definition.signals.manipulated_variable.unit);
    let loop_output = display_engineering_unit(&loop_definition.signals.process_variable.unit);
    if display_engineering_unit(&provenance.output_unit) != loop_output
        || display_engineering_unit(&provenance.input_unit) != loop_input
    {
        return Err(Refusal::new(
            "Recording input and output units must match this loop; no unit conversion was applied.",
        ));
    }
    Ok(())
}

/// The notation a student reads for a stored unit (`displayEngineeringUnit`): `degC` becomes `°C`, the word `pp` becomes
/// `percentage points`, and `*` becomes `·`. The stored unit is never rewritten; the text is trimmed first.
#[must_use]
pub fn display_engineering_unit(unit: &str) -> String {
    let normalized = unit.trim();
    if normalized.is_empty() {
        return normalized.to_owned();
    }
    let degrees = replace_word(normalized, "degC", "°C");
    let percentage = replace_word(&degrees, "pp", "percentage points");
    percentage.replace('*', "·")
}

/// Replaces each whole word `word` of `text` (a word boundary on each side, as the ASCII `\b` of `JavaScript`).
fn replace_word(text: &str, word: &str, replacement: &str) -> String {
    let bytes = text.as_bytes();
    let is_word_byte = |byte: u8| byte.is_ascii_alphanumeric() || byte == b'_';
    let mut out = String::with_capacity(text.len());
    let mut last = 0;
    for (start, matched) in text.match_indices(word) {
        let end = start + matched.len();
        let before_ok = start == 0 || !is_word_byte(bytes[start - 1]);
        let after_ok = end >= bytes.len() || !is_word_byte(bytes[end]);
        if before_ok && after_ok {
            out.push_str(&text[last..start]);
            out.push_str(replacement);
            last = end;
        }
    }
    out.push_str(&text[last..]);
    out
}

/// Routes a message that may be a scoped file import (`routeIdentificationRecordingImport`). Only a message of the scope,
/// with the expected shape (schema 1, a message identifier, and a payload holding only the file text), is an import; any
/// other message of the kind is ignored, and gets no reply.
#[must_use]
pub fn route_import_message(message: &Value, scope: &ImportScope) -> ImportRoute {
    let Some(object) = message.as_object() else {
        return ImportRoute::NotAnImport;
    };
    if object.get("kind").and_then(Value::as_str) != Some("control-studio.identify.import") {
        return ImportRoute::NotAnImport;
    }
    let text = |key: &str| object.get(key).and_then(Value::as_str);
    let payload = object.get("payload").and_then(Value::as_object);
    let json = payload
        .filter(|payload| payload.len() == 1)
        .and_then(|payload| payload.get("json"))
        .and_then(Value::as_str);
    let message_id = text("messageId").filter(|id| !id.is_empty());
    let matches_scope = object.get("schemaVersion").and_then(Value::as_u64) == Some(1)
        && text("projectId") == Some(scope.project_id.as_str())
        && text("loopId") == Some(scope.loop_id.as_str())
        && text("editorSessionId") == Some(scope.editor_session_id.as_str())
        && text("expectedLoopRevision") == Some(scope.loop_revision.as_str());
    match (matches_scope, message_id, json) {
        (true, Some(message_id), Some(json)) => ImportRoute::Import {
            message_id: message_id.to_owned(),
            json: json.to_owned(),
        },
        _ => ImportRoute::Ignored,
    }
}

/// The reply of a refused import, in the shape the page expects (`respond` with `status: "failed"`).
#[must_use]
pub fn import_failure_reply(scope: &ImportScope, message_id: &str, reason: &str) -> Value {
    serde_json::json!({
        "schemaVersion": 1,
        "kind": "control-studio.identification-recording",
        "messageId": message_id,
        "projectId": scope.project_id,
        "loopId": scope.loop_id,
        "editorSessionId": scope.editor_session_id,
        "expectedLoopRevision": scope.loop_revision,
        "payload": { "status": "failed", "reason": reason },
    })
}

/// The sentence of a file that keeps a step-response reference, which the engine does not read yet.
pub(crate) const REFERENCE_SENTENCE: &str =
    "This recording keeps a step-response identification reference, which the engine cannot read yet.";

/// The answer of a checked import (`identify` with `importRecording`; specification 35, §0.1a): the identity of the saved result
/// and the number of its raw samples. Nothing is kept or fitted.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ImportCheck {
    /// Always `checked`.
    pub status: &'static str,
    /// The identifier of the saved result.
    pub result_id: String,
    /// The revision of the saved result.
    pub result_revision: u64,
    /// The project the result was saved in.
    pub project_id: String,
    /// The source of the recording as the file writes it: `Simulation` or `Physical`.
    pub source: String,
    /// The number of raw samples.
    pub samples: u64,
}

/// Checks a saved Control Studio result or recording file against `loop_definition` (`readIdentificationRecordingImport`;
/// B3.6, B3.9, B10.4 and A9). The text is read and its result decoded as a whole, then the recording is checked for its
/// acquisition, its provenance and its units. Nothing is kept or fitted.
///
/// # Errors
/// The text is larger than 32 MiB or is not JSON; the saved result does not decode (B10.4); it keeps a step-response
/// reference; its recording is not a complete runtime scan buffer at a steady record period; its experiment did not
/// complete; or its units differ from the loop's.
pub fn check_import(json: &str, loop_definition: &LoopDefinition) -> Result<ImportCheck, Refusal> {
    check_import_length(json)?;
    let saved = read_saved_result(json)?;
    if saved.step_response_reference {
        return Err(Refusal::new(REFERENCE_SENTENCE));
    }
    check_saved_acquisition(&saved.recording)?;
    check_saved_provenance(&saved.provenance, loop_definition)?;
    Ok(ImportCheck {
        status: "checked",
        result_id: saved.result_id,
        result_revision: saved.result_revision,
        project_id: saved.project_id,
        source: saved.source,
        samples: saved.samples,
    })
}

#[cfg(test)]
#[path = "recording_import_tests.rs"]
mod tests;
