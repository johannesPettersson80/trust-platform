use crate::io::{IoAddress, IoSize, IoSnapshot, IoSnapshotValue};
use crate::value::Value;
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::time::{Instant, SystemTime, UNIX_EPOCH};

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
}
#[derive(Debug, Clone, Serialize)]
pub struct IoRecordingSample {
    pub scan: u64,
    pub runtime_time_ns: Option<i64>,
    pub captured_at_ms: u64,
    pub values: Vec<Option<f64>>,
    pub forced: Vec<bool>,
    pub quality: Vec<super::signals::Quality>,
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
#[derive(Debug)]
struct Recording {
    spec: IoRecordingSpec,
    targets: Vec<super::signals::Target>,
    samples: Vec<IoRecordingSample>,
    expected_scan: Option<u64>,
    missing: Vec<ScanRange>,
    incomplete: Option<String>,
    stop_reason: Option<String>,
    started: Instant,
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
        storage: Option<&crate::memory::VariableStorage>,
    ) -> Result<(), String> {
        if spec.recording_id.is_empty()
            || spec.recording_id.len() > 128
            || spec.loop_revision.is_empty()
            || spec.bindings.is_empty()
            || spec.bindings.len() > 32
            || !(2..=12000).contains(&spec.maximum_samples)
            || !(10..=3_600_000).contains(&spec.maximum_duration_ms)
        {
            return Err("Invalid recording identity or bounds.".into());
        }
        if self.records.contains_key(&spec.recording_id) {
            return Err("Recording identity already exists.".into());
        }
        if self.records.len() >= 2 {
            return Err(
                "Release a retained recording before starting another; limit is two.".into(),
            );
        }
        let snapshot = snapshot.ok_or("A current runtime I/O snapshot is required.")?;
        let targets = super::signals::resolve_bindings(&spec.bindings, snapshot, storage)?;
        self.records.insert(
            spec.recording_id.clone(),
            Recording {
                spec,
                targets,
                samples: Vec::new(),
                expected_scan: snapshot.scan.and_then(|scan| scan.checked_add(1)),
                missing: Vec::new(),
                incomplete: None,
                stop_reason: None,
                started: Instant::now(),
            },
        );
        Ok(())
    }
    pub fn capture_coherent(
        &mut self,
        snapshot: &IoSnapshot,
        runtime_time_ns: Option<i64>,
        storage: Option<&crate::memory::VariableStorage>,
        forced_vars: &[super::ForcedVar],
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
            if record.started.elapsed().as_millis() >= u128::from(record.spec.maximum_duration_ms) {
                record.stop_reason = Some("duration_limit".into());
                continue;
            }
            let Some(scan) = snapshot.scan else {
                record.incomplete = Some("Runtime scan sequence is unavailable.".into());
                continue;
            };
            if let Some(expected) = record.expected_scan {
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
                }
            } else {
                record.incomplete = Some("Recording start scan is unavailable.".into());
            }
            record.expected_scan = scan.checked_add(1);
            if runtime_time_ns.is_none()
                || runtime_time_ns.is_some_and(|time| time < 0)
                || record
                    .samples
                    .last()
                    .is_some_and(|last| runtime_time_ns <= last.runtime_time_ns)
            {
                record.incomplete =
                    Some("Runtime sample timing is unavailable or nonmonotonic.".into());
            }
            let captured = super::signals::capture_values(
                &record.targets,
                &record.spec.bindings,
                snapshot,
                storage,
                forced_vars,
            );
            record.samples.push(IoRecordingSample {
                scan,
                runtime_time_ns,
                captured_at_ms: captured_at_ms.unwrap_or(0),
                values: captured.values,
                forced: captured.forced,
                quality: captured.quality,
            });
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
        let complete =
            record.samples.len() >= 2 && record.incomplete.is_none() && record.missing.is_empty();
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
                reason: record.incomplete.clone().unwrap_or_else(|| {
                    if complete {
                        "Every runtime scan in the recording interval was captured.".into()
                    } else {
                        "The recording is too short or contains missing scans.".into()
                    }
                }),
            },
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
