//! The export of a loop's results (specification 35, §0.1a `controlStudio/export` and B10.5): the complete typed workflow result as
//! JSON, and the selected signal table as CSV with full-precision values. The engine writes the text; the extension asks the
//! engineer where to save it and writes it (B10.5). Neither form carries a credential: the JSON drops every key that names one, and
//! the CSV is built from the loop and the recording alone, which hold none.

#![allow(clippy::module_name_repetitions)] // The types name their concept: the export.

use schemars::JsonSchema;
use serde::{Deserialize, Serialize};
use serde_json::{Map, Value};

use crate::failure::{Cause, Failure, NextAction};
use crate::identification::identify_result::recorded_chart_samples;
use crate::loop_definition::LoopDefinition;
use crate::recording::channels::sample_controller_mode;
use crate::recording::context::{ExperimentContext, PlantContext};
use crate::recording::evidence::RecordingEvidence;
use crate::recording::samples::seconds_between;
use crate::recording::{EvidenceSource, RecordedRun, RecordedSample, RecordingFile};

/// The columns of the signal table that come before the additional channels (B10.5), in the order the CSV writes them.
pub const CSV_COLUMNS: [&str; 19] = [
    "elapsed_seconds",
    "observed_at_ms",
    "actual_cycle_seconds",
    "sp",
    "pv",
    "requested_mv",
    "applied_mv",
    "sp_unit",
    "pv_unit",
    "mv_unit",
    "quality",
    "controller_mode",
    "safety_state",
    "output_minimum",
    "output_maximum",
    "rate_up_per_second",
    "rate_down_per_second",
    "source",
    "events",
];

/// The words that name a secret in a key of the JSON export; a key that contains one is dropped, at any depth.
const SECRET_WORDS: [&str; 5] = ["token", "password", "credential", "secret", "authorization"];

/// The safety state a recording shows. The engine's recordings keep no safety channel, so the column says so.
const SAFETY_UNAVAILABLE: &str = "Unavailable";

/// The format of an export (B10.5).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "lowercase")]
pub enum ExportFormat {
    /// The complete typed workflow result.
    Json,
    /// The selected signal table, one row per recorded sample.
    Csv,
}

/// The arguments of `controlStudio/export` (specification 35, §0.1a; B10.5).
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct ExportRequest {
    /// The project folder.
    pub project_root: String,
    /// The runtime selected in VS Code; a loop of another runtime is refused, as `open` refuses it.
    pub runtime_id: String,
    /// The loop of the study.
    pub loop_id: String,
    /// The format to write.
    pub format: ExportFormat,
    /// The recording the CSV table holds, by its identifier in the study (`rec-…`). Absent, the table is the recording the study's
    /// current Identify result was fitted from. The JSON export ignores it.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub recording_id: Option<String>,
}

/// The answer of `controlStudio/export`: the text of the export and the file name the page suggests for it.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, JsonSchema)]
#[serde(rename_all = "camelCase")]
pub struct ExportResult {
    /// The format of `text`.
    pub format: ExportFormat,
    /// The file name the page suggests; the engineer may choose another.
    pub suggested_file_name: String,
    /// The export itself: a JSON document or CSV text with `\n` line ends.
    pub text: String,
}

/// The JSON export of a typed result: the result as it serializes, pretty-printed, without any key that names a secret (B10.5).
///
/// # Errors
/// The result cannot be written as JSON.
pub fn workflow_json<T: Serialize>(result: &T) -> Result<String, Failure> {
    workflow_json_with_recordings(result, &[])
}

/// The run context of one recording the study keeps, as the JSON export writes it under `recordings` (B10.2, B10.3, B10.5): the
/// recording's identity and verdict, the experiments its run wrote with how each one ended, and the plant it ran against.
#[derive(Debug, Clone, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct RecordingContextEntry {
    /// The recording's identifier in the study (`rec-…`).
    pub recording_id: String,
    /// Where the recording's values come from (A9).
    pub source: EvidenceSource,
    /// Whether the recording was opened from a saved result or recording file (A9).
    pub opened: bool,
    /// The verdict on the recording when it was kept: complete or incomplete, and the reason (E4.5). An aborted test's recording is
    /// incomplete evidence that stays available (B10.3).
    pub evidence: RecordingEvidence,
    /// The experiments of the run, each with its stop reason, its abort state and its restoration. Absent where the run wrote none.
    #[serde(skip_serializing_if = "Vec::is_empty")]
    pub experiments: Vec<ExperimentContext>,
    /// The plant the run ran against. Absent where the run had none to state.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub plant: Option<PlantContext>,
}

/// The entry of the recording `file`, kept in the study as `id`. `None` when the run has no context to state (an observation of a
/// physical target, a recording made before contexts were kept), so that the recording has no entry. The caller gives one file at a time,
/// so that a study with many recordings is never held in memory at once.
#[must_use]
pub fn recording_context(id: &str, file: &RecordingFile) -> Option<RecordingContextEntry> {
    let context = file.context.as_ref()?;
    Some(RecordingContextEntry {
        recording_id: id.to_owned(),
        source: file.source,
        opened: file.opened,
        evidence: file.evidence.clone(),
        experiments: context.experiments.clone(),
        plant: context.plant.clone(),
    })
}

/// The JSON export of a typed result with the run contexts of the recordings the study keeps (B10.2, B10.5): the result as it
/// serializes, and under `recordings` one entry per recording that has a context, pretty-printed, without any key that names a secret.
/// A result that is not a JSON object, or a study with no context to state, adds nothing.
///
/// # Errors
/// The result cannot be written as JSON.
pub fn workflow_json_with_recordings<T: Serialize>(
    result: &T,
    recordings: &[RecordingContextEntry],
) -> Result<String, Failure> {
    let mut value =
        serde_json::to_value(result).map_err(|error| export_failure(&error.to_string()))?;
    if !recordings.is_empty() {
        let entries =
            serde_json::to_value(recordings).map_err(|error| export_failure(&error.to_string()))?;
        if let Some(fields) = value.as_object_mut() {
            fields.insert("recordings".to_owned(), entries);
        }
    }
    serde_json::to_string_pretty(&without_secrets(value))
        .map_err(|error| export_failure(&error.to_string()))
}

/// The CSV export of one recording (B10.5): the columns of [`CSV_COLUMNS`], then the five columns of each additional channel of the
/// recording, in the order of their identifiers. Each number is written with the digits that round-trip it exactly, and a missing value
/// stays an empty cell.
#[must_use]
pub fn samples_csv(
    loop_definition: &LoopDefinition,
    source: EvidenceSource,
    run: &RecordedRun,
) -> String {
    let rows = recorded_chart_samples(loop_definition, run);
    let channels = additional_identifiers(&rows);
    let mut lines = vec![header(&channels)];
    for (index, row) in rows.iter().enumerate() {
        let raw = run.samples.get(index);
        let previous = index
            .checked_sub(1)
            .and_then(|before| run.samples.get(before));
        let mut cells = vec![
            number(Some(row.elapsed_seconds)),
            raw.map_or_else(String::new, |sample| sample.captured_at_ms.to_string()),
            number(cycle_seconds(raw, previous)),
            number(row.sp),
            number(row.pv),
            number(row.requested_mv),
            number(row.applied_mv),
            loop_definition
                .signals
                .setpoint
                .as_ref()
                .map_or_else(String::new, |signal| signal.unit.clone()),
            loop_definition.signals.process_variable.unit.clone(),
            loop_definition.signals.manipulated_variable.unit.clone(),
            word(&row.quality),
            controller_mode(run, loop_definition, raw),
            SAFETY_UNAVAILABLE.to_owned(),
            number(loop_definition.limits.output_minimum),
            number(loop_definition.limits.output_maximum),
            number(loop_definition.limits.rate_up_per_s),
            number(loop_definition.limits.rate_down_per_s),
            word(&source),
            String::new(),
        ];
        for id in &channels {
            let channel = row.additional.iter().find(|channel| &channel.id == id);
            cells.push(number(channel.and_then(|channel| channel.value)));
            cells.push(channel.map_or_else(String::new, |channel| channel.unit.clone()));
            cells.push(channel.map_or_else(String::new, |channel| word(&channel.quality)));
            cells.push(channel.map_or_else(String::new, |channel| channel.channel.clone()));
            cells.push(channel.map_or_else(String::new, |channel| word(&channel.role)));
        }
        lines.push(
            cells
                .iter()
                .map(|cell| csv_cell(cell.as_str()))
                .collect::<Vec<_>>()
                .join(","),
        );
    }
    let mut text = lines.join("\n");
    text.push('\n');
    text
}

/// The header line: the fixed columns, then the five columns of each additional channel identifier.
fn header(channels: &[String]) -> String {
    let mut columns: Vec<String> = CSV_COLUMNS
        .iter()
        .map(|column| (*column).to_owned())
        .collect();
    for id in channels {
        for field in ["value", "unit", "quality", "channel", "role"] {
            columns.push(format!("additional.{id}.{field}"));
        }
    }
    columns
        .iter()
        .map(|column| csv_cell(column.as_str()))
        .collect::<Vec<_>>()
        .join(",")
}

/// The identifiers of every additional channel the rows hold, once each, in order.
fn additional_identifiers(rows: &[crate::messages::RecordedChartSample]) -> Vec<String> {
    let mut identifiers: Vec<String> = rows
        .iter()
        .flat_map(|row| row.additional.iter().map(|channel| channel.id.clone()))
        .collect();
    identifiers.sort();
    identifiers.dedup();
    identifiers
}

/// The seconds from the previous sample to this one, in process time; absent for the first sample (B10.5, A3.2).
fn cycle_seconds(raw: Option<&RecordedSample>, previous: Option<&RecordedSample>) -> Option<f64> {
    let time = raw?.runtime_time_ns?;
    let before = previous?.runtime_time_ns?;
    Some(seconds_between(before, time))
}

/// The controller mode a sample shows, as the Tune page names it, or `Unavailable` when the loop's mode channel gives none (A4.2).
fn controller_mode(
    run: &RecordedRun,
    loop_definition: &LoopDefinition,
    raw: Option<&RecordedSample>,
) -> String {
    raw.and_then(|sample| sample_controller_mode(run, loop_definition, sample))
        .map_or_else(|| "Unavailable".to_owned(), |mode| word(&mode))
}

/// A number as the CSV writes it: the shortest text that reads back as the same value, or an empty cell when it is missing or
/// not finite (B10.3: a missing value stays missing).
fn number(value: Option<f64>) -> String {
    match value {
        Some(value) if value.is_finite() => value.to_string(),
        _ => String::new(),
    }
}

/// The lowercase or variant word a value serializes to (`good`, `simulated-plant`, `Auto`).
fn word<T: Serialize>(value: &T) -> String {
    serde_json::to_value(value)
        .ok()
        .and_then(|value| value.as_str().map(str::to_owned))
        .unwrap_or_default()
}

/// A CSV cell: quoted when it holds a comma, a quote or a line break, with its quotes doubled.
fn csv_cell(text: &str) -> String {
    if text.contains([',', '"', '\n', '\r']) {
        format!("\"{}\"", text.replace('"', "\"\""))
    } else {
        text.to_owned()
    }
}

/// The JSON value without the keys that name a secret, at any depth (the rule of the `TypeScript` export).
fn without_secrets(value: Value) -> Value {
    match value {
        Value::Object(map) => Value::Object(
            map.into_iter()
                .filter(|(key, _)| !names_a_secret(key))
                .map(|(key, value)| (key, without_secrets(value)))
                .collect::<Map<String, Value>>(),
        ),
        Value::Array(items) => Value::Array(items.into_iter().map(without_secrets).collect()),
        other => other,
    }
}

/// Whether a key names a secret: it contains one of the secret words, in any case.
fn names_a_secret(key: &str) -> bool {
    let lower = key.to_ascii_lowercase();
    SECRET_WORDS.iter().any(|word| lower.contains(word))
}

fn export_failure(detail: &str) -> Failure {
    Failure::new(
        &format!("The export cannot be written: {detail}"),
        Cause::Implementation,
        NextAction::None,
    )
}

#[cfg(test)]
#[path = "export_tests.rs"]
mod tests;
