use crate::io::{IoAddress, IoSize, IoSnapshot, IoSnapshotValue};
use crate::value::Value;
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::time::{SystemTime, UNIX_EPOCH};

#[path = "recording/period.rs"]
mod period;

use period::RecordPeriod;

use super::DebugControl;
use crate::memory::Generation;
use crate::runtime::ProgramRead;

/// Most channels one recording holds (specification 35, A7).
const MAX_CHANNELS: usize = 32;
/// Most samples one recording keeps per channel (A7). A sample is one record period (A4.2).
const MAX_SAMPLES: usize = 100_000;
/// Most channel samples one recording holds (A7).
const MAX_CHANNEL_SAMPLES: usize = 1_600_000;
/// Longest recording, in process time (A7): 24 h.
const MAX_DURATION_MS: u64 = 86_400_000;
/// Most executions one sample averages (A4.2).
const MAX_RECORD_EVERY: u32 = 100_000;
/// Recordings one runtime holds at a time (A7).
const MAX_RECORDINGS: usize = 2;
/// Most samples one `control.recording.status` response carries.
const STATUS_PAGE_SAMPLES: usize = 2_000;

pub type RecordingBinding = super::signals::SignalBinding;
#[derive(Debug, Clone, Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
pub struct IoRecordingSpec {
    pub recording_id: String,
    pub loop_revision: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub program_revision: Option<String>,
    pub bindings: Vec<RecordingBinding>,
    pub maximum_samples: usize,
    pub maximum_duration_ms: u64,
    /// The task whose executions the recording samples: a loop's own task, whose program
    /// changes the loop's values. Without it every published scan is a sample.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub task: Option<String>,
    /// The executions one sample averages (A4.2). One keeps every execution.
    #[serde(
        default = "default_record_every",
        skip_serializing_if = "is_default_record_every"
    )]
    pub record_every: u32,
}
fn default_record_every() -> u32 {
    1
}
fn is_default_record_every(record_every: &u32) -> bool {
    *record_every == 1
}
#[derive(Debug, Clone, Serialize)]
pub struct IoRecordingSample {
    pub scan: u64,
    pub runtime_time_ns: Option<i64>,
    pub captured_at_ms: u64,
    pub values: Vec<Option<f64>>,
    pub forced: Vec<bool>,
    pub quality: Vec<super::signals::Quality>,
    /// A missed execution fell inside this sample's record period (A4.4).
    #[serde(skip_serializing_if = "is_false")]
    pub gap: bool,
}
fn is_false(value: &bool) -> bool {
    !*value
}
#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ScanRange {
    pub first: u64,
    pub last: u64,
}
#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct RecordingAcquisition {
    pub method: &'static str,
    pub completeness: &'static str,
    pub observed_sample_count: usize,
    pub scan_range: Option<ScanRange>,
    pub missing_scan_ranges: Vec<ScanRange>,
    pub dropped_observed_samples: usize,
    pub actual_cycle_timing_available: bool,
    pub freshness_available: bool,
    pub reason: String,
}
#[derive(Debug, Clone, Serialize)]
pub struct IoRecordingResult {
    pub recording_id: String,
    pub loop_revision: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub program_revision: Option<String>,
    pub bindings: Vec<RecordingBinding>,
    pub stop_reason: String,
    pub samples: Vec<IoRecordingSample>,
    pub acquisition: RecordingAcquisition,
}
/// The state of a retained recording and one page of its samples (`control.recording.status`).
#[derive(Debug, Clone, Serialize)]
pub struct IoRecordingStatus {
    pub recording_id: String,
    /// `recording` while captured, `failed` when the runtime ended it, otherwise `stopped`.
    pub state: &'static str,
    /// Process time from the first captured execution to the last one.
    pub elapsed_ms: u64,
    pub sample_count: usize,
    pub complete: bool,
    pub reason: Option<String>,
    pub samples: Vec<IoRecordingSample>,
    /// The index the next request should start from.
    pub next_sample: usize,
}
#[derive(Debug)]
struct Recording {
    spec: IoRecordingSpec,
    targets: Vec<super::signals::Target>,
    /// Per channel, whether it is BOOL: a BOOL sample is the last of its executions (A4.2).
    boolean: Vec<bool>,
    samples: Vec<IoRecordingSample>,
    /// The record period being filled: the executions captured since the last sample.
    period: RecordPeriod,
    expected_scan: Option<u64>,
    /// For a task recording: the executions of the task counted so far, the sequence its samples
    /// carry, and the runtime scan of the last one.
    task_runs: u64,
    last_scan: Option<u64>,
    missing: Vec<ScanRange>,
    incomplete: Option<String>,
    stop_reason: Option<String>,
    /// Process time (A3.2) of the first and of the last captured execution. The duration limit
    /// counts from the first (A7).
    first_time_ns: Option<i64>,
    last_time_ns: Option<i64>,
}
#[derive(Debug, Default)]
pub(super) struct IoRecordingRegistry {
    records: HashMap<String, Recording>,
}
impl IoRecordingRegistry {
    pub fn start(
        &mut self,
        spec: IoRecordingSpec,
        snapshot: Option<&IoSnapshot>,
        program: Option<&dyn ProgramRead>,
    ) -> Result<(), String> {
        if spec.recording_id.is_empty()
            || spec.recording_id.len() > 128
            || spec.loop_revision.is_empty()
            || spec.bindings.is_empty()
            || spec.maximum_samples < 2
            || spec.maximum_duration_ms < 10
            || spec
                .task
                .as_ref()
                .is_some_and(|task| task.is_empty() || task.len() > 128)
        {
            return Err("Invalid recording identity or bounds.".into());
        }
        check_limits(&spec)?;
        if self.records.contains_key(&spec.recording_id) {
            return Err("Recording identity already exists.".into());
        }
        if self.records.len() >= MAX_RECORDINGS {
            return Err(format!(
                "Release a retained recording before starting another; the limit is {MAX_RECORDINGS} recordings at a time."
            ));
        }
        let snapshot = snapshot.ok_or("A current runtime I/O snapshot is required.")?;
        let targets = super::signals::resolve_bindings(&spec.bindings, snapshot, program)?;
        // A task's executions are not consecutive scans; its missed intervals mark the gaps.
        let expected_scan = if spec.task.is_some() {
            None
        } else {
            snapshot.scan.and_then(|scan| scan.checked_add(1))
        };
        let boolean: Vec<bool> = spec
            .bindings
            .iter()
            .map(|binding| binding.iec_type.eq_ignore_ascii_case("BOOL"))
            .collect();
        self.records.insert(
            spec.recording_id.clone(),
            Recording {
                spec,
                targets,
                boolean,
                samples: Vec::new(),
                period: RecordPeriod::default(),
                expected_scan,
                task_runs: 0,
                last_scan: None,
                missing: Vec::new(),
                incomplete: None,
                stop_reason: None,
                first_time_ns: None,
                last_time_ns: None,
            },
        );
        Ok(())
    }
    /// Captures a published scan, `program` being the program as the scan left it in the program
    /// generation `generation`; `tasks` are the tasks that ran in it, each with the intervals it
    /// missed before it.
    pub fn capture_coherent(
        &mut self,
        snapshot: &IoSnapshot,
        runtime_time_ns: Option<i64>,
        program: &dyn ProgramRead,
        forced_vars: &[super::ForcedVar],
        generation: Generation,
        tasks: &[(smol_str::SmolStr, u64)],
    ) {
        let captured_at_ms = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .ok()
            .and_then(|duration| u64::try_from(duration.as_millis()).ok());
        for record in self
            .records
            .values_mut()
            .filter(|record| record.stop_reason.is_none())
        {
            if captured_at_ms.is_none() {
                record.incomplete = Some("Runtime capture wall clock is unavailable.".into());
            }
            if record.duration_reached(runtime_time_ns) {
                record.stop_reason = Some("duration_limit".into());
                continue;
            }
            let Some(scan) = snapshot.scan else {
                record.incomplete = Some("Runtime scan sequence is unavailable.".into());
                continue;
            };
            let sequence = if let Some(task) = record.spec.task.as_deref() {
                // Only the scans in which the loop's task ran. Their samples count the task's
                // executions, so intervals it missed leave a gap in the sequence.
                let Some((_, missed)) = tasks
                    .iter()
                    .find(|(name, _)| name.eq_ignore_ascii_case(task))
                else {
                    continue;
                };
                if record.last_scan.is_some_and(|last| scan <= last) {
                    record.incomplete = Some("Runtime scan sequence restarted or repeated.".into());
                    record.stop_reason = Some("scan_reset".into());
                    continue;
                }
                record.last_scan = Some(scan);
                if *missed > 0 && record.task_runs > 0 {
                    record.missing.push(ScanRange {
                        first: record.task_runs + 1,
                        last: record.task_runs + missed,
                    });
                    record.period.mark_gap();
                }
                record.task_runs += if record.task_runs > 0 { missed + 1 } else { 1 };
                record.task_runs
            } else if let Some(expected) = record.expected_scan {
                if scan < expected {
                    record.incomplete = Some("Runtime scan sequence restarted or repeated.".into());
                    record.stop_reason = Some("scan_reset".into());
                    continue;
                }
                if scan > expected {
                    record.missing.push(ScanRange {
                        first: expected,
                        last: scan - 1,
                    });
                    record.period.mark_gap();
                }
                record.expected_scan = scan.checked_add(1);
                scan
            } else {
                record.incomplete = Some("Recording start scan is unavailable.".into());
                record.expected_scan = scan.checked_add(1);
                scan
            };
            record.capture_execution(
                sequence,
                runtime_time_ns,
                snapshot,
                program,
                (forced_vars, generation),
                captured_at_ms.unwrap_or(0),
            );
            if record.samples.len() == record.spec.maximum_samples {
                record.stop_reason = Some("sample_limit".into());
            }
        }
    }
    pub fn stop(&mut self, id: &str, revision: &str) -> Result<IoRecordingResult, String> {
        let record = self
            .records
            .get_mut(id)
            .ok_or("Recording identity is unavailable.")?;
        if record.spec.loop_revision != revision {
            return Err("Recording loop revision changed.".into());
        }
        record.stop_reason.get_or_insert_with(|| "stopped".into());
        // The last record period may hold fewer executions than record_every. It is kept, so the
        // recording covers its whole interval up to the stop (specification 35, A4.2).
        if record.period.executions() > 0 && record.samples.len() < record.spec.maximum_samples {
            let sequence = if record.spec.task.is_some() {
                record.task_runs
            } else {
                record
                    .expected_scan
                    .map_or(0, |next| next.saturating_sub(1))
            };
            let period = std::mem::take(&mut record.period);
            record
                .samples
                .push(period.finish(sequence, &record.boolean));
        }
        let complete = record.is_complete();
        let scan_range = record
            .samples
            .first()
            .zip(record.samples.last())
            .map(|(first, last)| ScanRange {
                first: first.scan,
                last: last.scan,
            });
        let timing = !record.samples.is_empty()
            && record
                .samples
                .iter()
                .all(|sample| sample.runtime_time_ns.is_some());
        Ok(IoRecordingResult {
            recording_id: id.into(),
            loop_revision: revision.into(),
            program_revision: record.spec.program_revision.clone(),
            bindings: record.spec.bindings.clone(),
            stop_reason: record.stop_reason.clone().unwrap_or_default(),
            samples: record.samples.clone(),
            acquisition: RecordingAcquisition {
                method: "runtime_scan_buffer",
                completeness: if complete { "complete" } else { "incomplete" },
                observed_sample_count: record.samples.len(),
                scan_range,
                missing_scan_ranges: record.missing.clone(),
                dropped_observed_samples: 0,
                actual_cycle_timing_available: timing,
                freshness_available: false,
                reason: record.completion_reason(complete),
            },
        })
    }
    /// The state and one page of samples of a recording. It never stops or releases the recording.
    pub fn status(
        &self,
        id: &str,
        revision: &str,
        from_sample: Option<usize>,
    ) -> Result<IoRecordingStatus, String> {
        let record = self
            .records
            .get(id)
            .ok_or("Recording identity is unavailable.")?;
        if record.spec.loop_revision != revision {
            return Err("Recording loop revision changed.".into());
        }
        let complete = record.is_complete();
        let sample_count = record.samples.len();
        // Without a start index the page is empty; `next_sample` tells the caller where to begin.
        let start = from_sample.unwrap_or(sample_count).min(sample_count);
        let end = start.saturating_add(STATUS_PAGE_SAMPLES).min(sample_count);
        Ok(IoRecordingStatus {
            recording_id: id.into(),
            state: record.state(),
            elapsed_ms: record.elapsed_ms(),
            sample_count,
            complete,
            reason: if record.stop_reason.is_none() {
                record.incomplete.clone()
            } else {
                Some(record.completion_reason(complete))
            },
            samples: record.samples[start..end].to_vec(),
            next_sample: end,
        })
    }
    pub fn release(&mut self, id: &str, revision: &str) -> Result<(), String> {
        if let Some(record) = self.records.get(id) {
            if record.spec.loop_revision != revision {
                return Err("Recording loop revision changed.".into());
            }
            if record.stop_reason.is_none() {
                return Err("Stop the recording before releasing it.".into());
            }
        }
        self.records.remove(id);
        Ok(())
    }
    pub fn interrupt(&mut self) {
        for record in self
            .records
            .values_mut()
            .filter(|record| record.stop_reason.is_none())
        {
            record.stop_reason = Some("runtime_boundary".into());
            record.incomplete =
                Some("The runtime stopped, restarted or entered its safe state.".into());
        }
    }
}
impl Recording {
    /// Whether the recording holds every execution (or record period) of its interval (A4.4).
    fn is_complete(&self) -> bool {
        self.samples.len() >= 2 && self.incomplete.is_none() && self.missing.is_empty()
    }
    fn completion_reason(&self, complete: bool) -> String {
        self.incomplete
            .clone()
            .unwrap_or_else(|| match (complete, self.spec.task.as_deref()) {
                (true, Some(task)) => {
                    format!("Every execution of {task} in the recording interval was captured.")
                }
                (true, None) => "Every runtime scan in the recording interval was captured.".into(),
                (false, Some(task)) => {
                    format!("The recording is too short or {task} missed executions in it.")
                }
                (false, None) => "The recording is too short or contains missing scans.".into(),
            })
    }
    /// `failed` when the runtime ended the recording, `stopped` when a request or a limit did.
    fn state(&self) -> &'static str {
        match self.stop_reason.as_deref() {
            None => "recording",
            Some("scan_reset" | "runtime_boundary") => "failed",
            Some(_) => "stopped",
        }
    }
    fn elapsed_ms(&self) -> u64 {
        match (self.first_time_ns, self.last_time_ns) {
            (Some(first), Some(last)) if last >= first => {
                u64::try_from((i128::from(last) - i128::from(first)) / 1_000_000)
                    .unwrap_or(u64::MAX)
            }
            _ => 0,
        }
    }
    /// Whether process time has run `maximum_duration_ms` past the first captured execution. A
    /// scan without timing cannot be measured, so the sample limit alone bounds that recording.
    fn duration_reached(&self, now_ns: Option<i64>) -> bool {
        match (self.first_time_ns, now_ns) {
            (Some(first), Some(now)) => {
                i128::from(now) - i128::from(first)
                    >= i128::from(self.spec.maximum_duration_ms) * 1_000_000
            }
            _ => false,
        }
    }
    /// Folds one captured execution into the record period, closing a sample once the period holds
    /// `record_every` executions.
    fn capture_execution(
        &mut self,
        sequence: u64,
        time_ns: Option<i64>,
        snapshot: &IoSnapshot,
        program: &dyn ProgramRead,
        forces: (&[super::ForcedVar], Generation),
        captured_at_ms: u64,
    ) {
        if !time_ns
            .is_some_and(|time| time >= 0 && self.last_time_ns.is_none_or(|last| time > last))
        {
            self.incomplete = Some("Runtime sample timing is unavailable or nonmonotonic.".into());
        }
        if let Some(time) = time_ns {
            if self.first_time_ns.is_none() {
                self.first_time_ns = Some(time);
            }
            self.last_time_ns = Some(time);
        }
        let captured = super::signals::capture_values(
            &self.targets,
            &self.spec.bindings,
            snapshot,
            Some(program),
            forces,
        );
        self.period.add(time_ns, captured, captured_at_ms);
        if self.period.executions() == self.spec.record_every {
            let period = std::mem::take(&mut self.period);
            self.samples.push(period.finish(sequence, &self.boolean));
        }
    }
}
/// Refuses a recording beyond the limits of specification 35 (A7), naming the limit.
fn check_limits(spec: &IoRecordingSpec) -> Result<(), String> {
    if spec.bindings.len() > MAX_CHANNELS {
        return Err(format!("Recording channels are limited to {MAX_CHANNELS}."));
    }
    if spec.maximum_samples > MAX_SAMPLES {
        return Err(format!(
            "Recording samples per channel are limited to {MAX_SAMPLES}; record_every averages executions into one sample."
        ));
    }
    if spec.maximum_duration_ms > MAX_DURATION_MS {
        return Err(format!(
            "Recording duration is limited to {MAX_DURATION_MS} ms (24 h) of process time."
        ));
    }
    if spec.bindings.len().saturating_mul(spec.maximum_samples) > MAX_CHANNEL_SAMPLES {
        return Err(format!(
            "Recording channels × samples are limited to {MAX_CHANNEL_SAMPLES}."
        ));
    }
    if !(1..=MAX_RECORD_EVERY).contains(&spec.record_every) {
        return Err(format!(
            "Recording record_every must be 1 to {MAX_RECORD_EVERY} executions."
        ));
    }
    Ok(())
}
impl DebugControl {
    /// The state and one page of samples of a retained recording. It never stops or releases it.
    pub fn io_recording_status(
        &self,
        id: &str,
        revision: &str,
        from_sample: Option<usize>,
    ) -> Result<IoRecordingStatus, String> {
        self.lock().recordings.status(id, revision, from_sample)
    }
}
pub(super) fn snapshot_value<'a>(
    snapshot: &'a IoSnapshot,
    address: &IoAddress,
) -> Option<&'a Value> {
    snapshot
        .inputs
        .iter()
        .chain(&snapshot.outputs)
        .chain(&snapshot.memory)
        .find(|entry| entry.address == *address)
        .and_then(|entry| match &entry.value {
            IoSnapshotValue::Value(value) => Some(value),
            _ => None,
        })
}
pub(crate) fn numeric(value: &Value) -> Option<f64> {
    let value = match value {
        Value::Bool(value) => {
            if *value {
                1.0
            } else {
                0.0
            }
        }
        Value::SInt(value) => f64::from(*value),
        Value::Int(value) => f64::from(*value),
        Value::DInt(value) => f64::from(*value),
        Value::USInt(value) => f64::from(*value),
        Value::UInt(value) => f64::from(*value),
        Value::UDInt(value) => f64::from(*value),
        Value::LInt(value) if value.unsigned_abs() <= 9_007_199_254_740_991 => *value as f64,
        Value::ULInt(value) if *value <= 9_007_199_254_740_991 => *value as f64,
        Value::Real(value) => f64::from(*value),
        Value::LReal(value) => *value,
        // An enumeration reads as its numeric value (Specification 35, control signal reads).
        Value::Enum(value) if value.numeric_value().unsigned_abs() <= 9_007_199_254_740_991 => {
            value.numeric_value() as f64
        }
        _ => return None,
    };
    value.is_finite().then_some(value)
}
pub(super) fn overlap(a: &IoAddress, b: &IoAddress) -> bool {
    let range = |address: &IoAddress| {
        let bits = match address.size {
            IoSize::Bit => 1,
            IoSize::Byte => 8,
            IoSize::Word => 16,
            IoSize::DWord => 32,
            IoSize::LWord => 64,
            IoSize::Bytes(bytes) => u64::from(bytes) * 8,
        };
        let start = u64::from(address.byte) * 8 + u64::from(address.bit);
        (start, start + bits)
    };
    let (a0, a1) = range(a);
    let (b0, b1) = range(b);
    a.area == b.area && a0 < b1 && b0 < a1
}
