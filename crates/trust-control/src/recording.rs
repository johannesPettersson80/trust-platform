//! Recordings (runtime protocol annex, E4): the start request with the limits of A7, the status
//! page, the recorded run the runtime returns when a recording stops, and the file a study keeps it
//! in (specification 35, A8.3).
//!
//! A recording file holds the recorded run exactly as the runtime returned it, with the loop it
//! belongs to and its evidence source (A9). Its identifier is derived from its content, so the same
//! recording always maps to the same file and a recording file is never rewritten.

use std::fmt::Write as _;

use schemars::JsonSchema;
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};

use crate::failure::{Cause, Failure, NextAction};
use crate::limits::{
    RECORDING_CELL_LIMIT, RECORDING_CHANNEL_LIMIT, RECORDING_DURATION_LIMIT_MS,
    RECORDING_SAMPLE_LIMIT,
};
use crate::signals::{Quality, SignalBinding};
use crate::study::canonical_json;

/// The recording file format this crate reads and writes.
pub const RECORDING_FORMAT: u32 = 1;

/// The request of `control.recording.start` (E4.2).
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct RecordingRequest {
    /// The recording's identifier, 1 to 128 bytes, journaled before the start (E11.2).
    pub recording_id: String,
    /// The loop revision.
    pub loop_revision: String,
    /// The program revision; required when a binding is a symbol.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub program_revision: Option<String>,
    /// The signals, 1 to 32.
    pub bindings: Vec<SignalBinding>,
    /// The most samples per channel, 2 to 100,000.
    pub maximum_samples: u64,
    /// The longest recording in process time, 10 ms to 24 h.
    pub maximum_duration_ms: u64,
    /// The task whose executions are sampled; absent samples every resource cycle.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub task: Option<String>,
    /// The sequence values one sample covers, 1 to 100,000 (E4.4).
    #[serde(default = "one", skip_serializing_if = "is_one")]
    pub record_every: u64,
}

fn one() -> u64 {
    1
}

// serde passes the field by reference.
#[allow(clippy::trivially_copy_pass_by_ref)]
fn is_one(value: &u64) -> bool {
    *value == 1
}

impl RecordingRequest {
    /// Checks the request against the limits of A7 and the rules of E4.2 before it is sent.
    ///
    /// # Errors
    /// A limit or rule is broken; the failure names it.
    pub fn check(&self) -> Result<(), Failure> {
        let channels = self.bindings.len() as u64;
        let refuse = |text: String| Err(Failure::new(&text, Cause::Settings, NextAction::None));
        if self.recording_id.is_empty()
            || self.recording_id.len() > 128
            || self.loop_revision.is_empty()
        {
            return refuse(
                "A recording needs an identifier of 1 to 128 bytes and a loop revision.".into(),
            );
        }
        if channels == 0 || channels > RECORDING_CHANNEL_LIMIT {
            return refuse(format!(
                "Recording channels are limited to {RECORDING_CHANNEL_LIMIT}."
            ));
        }
        if !(2..=RECORDING_SAMPLE_LIMIT).contains(&self.maximum_samples) {
            return refuse(format!(
                "A recording keeps 2 to {RECORDING_SAMPLE_LIMIT} samples per channel."
            ));
        }
        if channels * self.maximum_samples > RECORDING_CELL_LIMIT {
            return refuse(format!(
                "{channels} channels of {} samples exceed the limit of {RECORDING_CELL_LIMIT} values in one recording.",
                self.maximum_samples
            ));
        }
        if !(10..=RECORDING_DURATION_LIMIT_MS).contains(&self.maximum_duration_ms) {
            return refuse("A recording lasts 10 ms to 24 h of process time.".into());
        }
        if !(1..=RECORDING_SAMPLE_LIMIT).contains(&self.record_every) {
            return refuse(format!(
                "A sample covers 1 to {RECORDING_SAMPLE_LIMIT} executions."
            ));
        }
        if self.bindings.iter().any(|binding| binding.symbol.is_some())
            && self.program_revision.as_deref().is_none_or(str::is_empty)
        {
            return refuse(
                "A recording of a variable by its name needs the program revision.".into(),
            );
        }
        Ok(())
    }
}

/// One sample of a recording (E4.4, E4.6).
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct RecordedSample {
    /// The last sequence value the sample covers.
    pub scan: u64,
    /// The mean runtime time of the executions it covers; null when the runtime gave none.
    pub runtime_time_ns: Option<i64>,
    /// The runtime's wall-clock time of capture, for display only.
    pub captured_at_ms: u64,
    /// One value per binding; null when bad or missing.
    pub values: Vec<Option<f64>>,
    /// One forced flag per binding.
    pub forced: Vec<bool>,
    /// One quality per binding: the worst of the executions it covers.
    pub quality: Vec<Quality>,
    /// A missed execution fell inside the sample's record period (A4.4).
    #[serde(default, skip_serializing_if = "std::ops::Not::not")]
    pub gap: bool,
}

/// The state of a recording (E4.6).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "lowercase")]
pub enum RecordingState {
    /// Still capturing.
    Recording,
    /// Ended by a stop or a limit.
    Stopped,
    /// Ended by a scan reset or a runtime boundary.
    Failed,
}

/// The answer of `control.recording.status` (E4.6): the state and one page of samples.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct RecordingStatus {
    /// The recording's identifier.
    pub recording_id: String,
    /// The state.
    pub state: RecordingState,
    /// Process time from the first captured execution to the last one.
    pub elapsed_ms: u64,
    /// The closed samples.
    pub sample_count: u64,
    /// Whether the recording is complete so far (E4.5).
    pub complete: bool,
    /// Why it ended or is incomplete.
    pub reason: Option<String>,
    /// At most 2,000 samples from the requested index on.
    pub samples: Vec<RecordedSample>,
    /// The index of the next page.
    pub next_sample: u64,
}

/// Why a recording ended (E4.7).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "snake_case")]
pub enum StopReason {
    /// A stop request.
    Stopped,
    /// `maximum_samples` was reached.
    SampleLimit,
    /// `maximum_duration_ms` was reached.
    DurationLimit,
    /// The scan sequence was reset or repeated.
    ScanReset,
    /// A reload, restart, fault or stop of execution ended it.
    RuntimeBoundary,
}

/// Whether a recording holds every execution of its interval (E4.5).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "lowercase")]
pub enum Completeness {
    /// Every execution, no gap, strictly increasing times.
    Complete,
    /// A gap, a non-increasing time, a missing timestamp or a boundary.
    Incomplete,
}

/// A range of sequence values.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct ScanRange {
    /// The first value.
    pub first: u64,
    /// The last value.
    pub last: u64,
}

/// How a recording was acquired (E4.5).
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct Acquisition {
    /// Always `runtime_scan_buffer`.
    pub method: String,
    /// Complete or incomplete.
    pub completeness: Completeness,
    /// The executions observed.
    pub observed_sample_count: u64,
    /// The sequence range covered.
    pub scan_range: Option<ScanRange>,
    /// The gaps.
    pub missing_scan_ranges: Vec<ScanRange>,
    /// Always 0.
    pub dropped_observed_samples: u64,
    /// Whether the actual cycle timing is known.
    pub actual_cycle_timing_available: bool,
    /// Always false.
    pub freshness_available: bool,
    /// Why the recording is incomplete, or empty.
    pub reason: String,
}

/// A recording as `control.recording.stop` returns it (E4.7): the evidence itself.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct RecordedRun {
    /// The recording's identifier.
    pub recording_id: String,
    /// The loop revision.
    pub loop_revision: String,
    /// The program revision, when the recording has one.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub program_revision: Option<String>,
    /// The signals, in the order of every sample's arrays.
    pub bindings: Vec<SignalBinding>,
    /// Why it ended.
    pub stop_reason: StopReason,
    /// All samples.
    pub samples: Vec<RecordedSample>,
    /// How it was acquired.
    pub acquisition: Acquisition,
}

impl RecordedRun {
    /// Checks that the run answers `request`: the same identity and bindings, and one value,
    /// forced flag and quality per binding in every sample.
    ///
    /// # Errors
    /// The run does not answer the request, or a sample is misaligned.
    pub fn check_answers(&self, request: &RecordingRequest) -> Result<(), Failure> {
        let aligned = self.samples.iter().all(|sample| {
            sample.values.len() == self.bindings.len()
                && sample.forced.len() == self.bindings.len()
                && sample.quality.len() == self.bindings.len()
                && sample
                    .values
                    .iter()
                    .flatten()
                    .all(|value| value.is_finite())
        });
        if self.recording_id != request.recording_id
            || self.loop_revision != request.loop_revision
            || self.bindings != request.bindings
            || (request.program_revision.is_some()
                && self.program_revision != request.program_revision)
            || !aligned
        {
            return Err(Failure::invalid_response("control.recording.stop"));
        }
        Ok(())
    }
}

/// Where a recording's values come from (A9). A live recording is labelled from the target fact
/// read when it starts (A2.4).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "kebab-case")]
pub enum EvidenceSource {
    /// Observed on a simulated target.
    SimulatedPlant,
    /// Observed on a physical target.
    PhysicalPlant,
}

/// One recording file of a study: `control-studio/<loop id>/recordings/<id>.json` (A8.3).
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct RecordingFile {
    /// Always [`RECORDING_FORMAT`].
    pub format: u32,
    /// The loop the recording belongs to.
    pub loop_id: String,
    /// Where its values come from.
    pub source: EvidenceSource,
    /// The run exactly as the runtime returned it.
    pub run: RecordedRun,
}

impl RecordingFile {
    /// The file of a run of loop `loop_id`.
    #[must_use]
    pub fn new(loop_id: &str, source: EvidenceSource, run: RecordedRun) -> Self {
        Self {
            format: RECORDING_FORMAT,
            loop_id: loop_id.to_string(),
            source,
            run,
        }
    }

    /// The file's text: canonical JSON.
    ///
    /// # Errors
    /// The run holds a value JSON cannot carry.
    pub fn encode(&self) -> Result<String, Failure> {
        let value = serde_json::to_value(self).map_err(|error| unreadable(&error.to_string()))?;
        Ok(canonical_json(&value))
    }

    /// The identifier the file is stored under: `rec-` and 32 hex digits of the SHA-256 of its text.
    ///
    /// # Errors
    /// As [`RecordingFile::encode`].
    pub fn id(&self) -> Result<String, Failure> {
        Ok(recording_id_for(&self.encode()?))
    }

    /// Reads the file stored as `id` for loop `loop_id`, and checks that its content has that
    /// identifier.
    ///
    /// # Errors
    /// The text is not a recording file of this format, of this loop, or of this identifier.
    pub fn decode(text: &str, loop_id: &str, id: &str) -> Result<Self, Failure> {
        let file: Self =
            serde_json::from_str(text).map_err(|error| unreadable(&error.to_string()))?;
        if file.format != RECORDING_FORMAT || file.loop_id != loop_id || file.id()? != id {
            return Err(unreadable(&format!("{id} is not a recording of loop {loop_id} in this format, or does not match its identifier")));
        }
        Ok(file)
    }
}

/// `rec-` and the first 32 hex digits of the SHA-256 of `text`.
#[must_use]
pub fn recording_id_for(text: &str) -> String {
    let digest = Sha256::digest(text.as_bytes());
    let mut id = String::from("rec-");
    for byte in &digest[..16] {
        let _ = write!(id, "{byte:02x}");
    }
    id
}

fn unreadable(reason: &str) -> Failure {
    Failure::new(
        &format!("The recording file cannot be read: {reason}."),
        Cause::Settings,
        NextAction::None,
    )
}

#[cfg(test)]
#[path = "recording_tests.rs"]
mod tests;
