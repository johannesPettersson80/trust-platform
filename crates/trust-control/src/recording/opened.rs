//! The recording an opened file becomes when `importRecording` keeps it (specification 35, §0.1a, A9, B3.6 and B10.2), and the
//! experiment its fit reads (C12). A saved Control Studio result or recording file that `check_import` accepts keeps its
//! measured samples as one recording file: its original source, the opened marker, and the verdict the runtime's acquisition
//! gives it. Nothing was written to the plant for such a file, so its fit reads it as an observation (`opened_plan`).

use std::time::Duration;

use serde_json::Value;

use crate::experiment::{
    ControlExperimentKind, ControlExperimentPlan, ExperimentValue, PlanSource,
};
use crate::identification::recording_import::{check_import, ImportCheck};
use crate::identification::refusal::Refusal;
use crate::loop_definition::LoopDefinition;
use crate::recording::channels::{
    binding_value, ACTUATOR_BINDING, MANIPULATED_VARIABLE_BINDING, PROCESS_VARIABLE_BINDING,
    SETPOINT_BINDING,
};
use crate::recording::evidence::recording_evidence;
use crate::recording::{
    Acquisition, Completeness, EvidenceSource, RecordedRun, RecordedSample, RecordingFile,
    ScanRange, StopReason,
};
use crate::signals::{Quality, SignalBinding};

/// The sentence of a checked file whose recording cannot be read as the runtime's recording (A9).
const UNREADABLE_SENTENCE: &str =
    "The saved recording has invalid data or source metadata, so it cannot be kept as a recording.";

/// Checks a saved result or recording file against `loop_definition` (`check_import`) and turns its recording into the
/// recording file the study keeps: the samples the file holds, with the loop's bindings, the file's source and the opened
/// marker (B10.2, A9). Nothing is kept when the check refuses the file.
///
/// # Errors
/// The file is refused by `check_import` with the sentence it gives (B10.4, B3.6, B3.9, A9), or a value of its recording
/// cannot be read as a runtime sample.
pub fn opened_recording(
    json: &str,
    loop_definition: &LoopDefinition,
) -> Result<(ImportCheck, RecordingFile), Refusal> {
    let check = check_import(json, loop_definition)?;
    let value: Value = serde_json::from_str(json).map_err(|_| unreadable())?;
    let source = source_of(&check.source)?;
    let recording = &value["recording"];
    let acquisition = acquisition_of(&recording["acquisition"]).ok_or_else(unreadable)?;
    let actuator = loop_definition.signals.actuator.is_some();
    let mut bindings: Vec<(&str, &str)> = Vec::new();
    if let Some(setpoint) = &loop_definition.signals.setpoint {
        bindings.push((SETPOINT_BINDING, setpoint.symbol.as_str()));
    }
    bindings.push((
        PROCESS_VARIABLE_BINDING,
        loop_definition.signals.process_variable.symbol.as_str(),
    ));
    bindings.push((
        MANIPULATED_VARIABLE_BINDING,
        loop_definition.signals.manipulated_variable.symbol.as_str(),
    ));
    if let Some(position) = &loop_definition.signals.actuator {
        bindings.push((ACTUATOR_BINDING, position.symbol.as_str()));
    }
    let samples = recording["rawSamples"]
        .as_array()
        .ok_or_else(unreadable)?
        .iter()
        .map(|sample| recorded_sample(sample, &bindings, actuator))
        .collect::<Result<Vec<_>, _>>()?;
    let run = RecordedRun {
        recording_id: text(&recording["acquisition"]["recordingId"])
            .unwrap_or(check.result_id.as_str())
            .to_string(),
        loop_revision: text(&value["loop"]["revision"])
            .unwrap_or_default()
            .to_string(),
        program_revision: text(&recording["acquisition"]["programRevision"]).map(str::to_string),
        bindings: bindings
            .iter()
            .map(|(id, symbol)| SignalBinding {
                id: (*id).to_string(),
                address: None,
                symbol: Some((*symbol).to_string()),
                // The file names no IEC type; an analogue signal of the loop is read as REAL.
                iec_type: "REAL".to_string(),
                unit: None,
            })
            .collect(),
        stop_reason: StopReason::Stopped,
        samples,
        acquisition,
    };
    let evidence = recording_evidence(&run, None);
    let mut file = RecordingFile::new(&loop_definition.id, source, run, evidence);
    file.opened = true;
    Ok((check, file))
}

/// The experiment an opened recording is fitted as (B3.6, C12): an observation of the file's samples. Control Studio ran no
/// experiment on the plant for the file, so the plan writes nothing. It starts and ends at the first manipulated value the file
/// records, and its range is the range that value took in the recording.
#[must_use]
pub fn opened_plan(
    file: &RecordingFile,
    loop_definition: &LoopDefinition,
) -> ControlExperimentPlan {
    let run = &file.run;
    let signal = &loop_definition.signals.manipulated_variable;
    let values: Vec<f64> = run
        .samples
        .iter()
        .filter_map(|sample| binding_value(run, sample, MANIPULATED_VARIABLE_BINDING))
        .collect();
    let first = values.first().copied().unwrap_or(0.0);
    let minimum = values.iter().copied().reduce(f64::min).unwrap_or(first);
    let maximum = values.iter().copied().reduce(f64::max).unwrap_or(first);
    ControlExperimentPlan {
        id: format!("opened-recording:{}", run.recording_id),
        source: plan_source(file.source),
        kind: ControlExperimentKind::Observation,
        affected_signal: signal.symbol.clone(),
        address: None,
        value_type: "REAL".to_string(),
        from: ExperimentValue::Number(first),
        to: ExperimentValue::Number(first),
        unit: signal.unit.clone(),
        minimum,
        maximum,
        duration_ms: recorded_duration_ms(run),
        expected_scan: run.samples.first().map_or(0, |sample| sample.scan),
        task: None,
        mode_binding: None,
        stop_conditions: Vec::new(),
        restoration_value: ExperimentValue::Number(first),
        requires_physical_approval: false,
        trajectory: Vec::new(),
        abort_binding: None,
        relay_feedback: None,
        interval_ms: None,
        seed: None,
        frequencies_hz: None,
    }
}

/// One raw sample of the saved recording as a runtime sample: its scan, its time base and one value per binding (E4.4). Each
/// binding takes the saved field of its signal, as `recording::channels` reads them: the setpoint, the measurement, the controller's
/// command (the requested value where the loop has an actuator) and the actuator's position (the applied value).
fn recorded_sample(
    sample: &Value,
    bindings: &[(&str, &str)],
    actuator: bool,
) -> Result<RecordedSample, Refusal> {
    let scan = sample["scan"].as_u64().ok_or_else(unreadable)?;
    let runtime_time_ns = sample["elapsedSeconds"]
        .as_f64()
        .and_then(nanoseconds)
        .ok_or_else(unreadable)?;
    let captured_at_ms = sample["observedAtMs"]
        .as_f64()
        .and_then(milliseconds)
        .ok_or_else(unreadable)?;
    let values: Vec<Option<f64>> = bindings
        .iter()
        .map(|(id, _)| saved_value(sample, id, actuator))
        .collect();
    let quality: Vec<Quality> = values
        .iter()
        .map(|value| saved_quality(sample, *value))
        .collect();
    let forced = quality
        .iter()
        .map(|quality| *quality == Quality::Forced)
        .collect();
    Ok(RecordedSample {
        scan,
        runtime_time_ns: Some(runtime_time_ns),
        captured_at_ms,
        values,
        forced,
        quality,
        gap: false,
    })
}

/// The saved field a binding reads (`recording::channels`): the setpoint, the measurement, the requested value (the controller's
/// command) when the loop has an actuator and the applied value otherwise, and the applied value for the actuator's position.
fn saved_value(sample: &Value, binding: &str, actuator: bool) -> Option<f64> {
    let field = match binding {
        SETPOINT_BINDING => "sp",
        PROCESS_VARIABLE_BINDING => "pv",
        MANIPULATED_VARIABLE_BINDING if actuator => "requestedMv",
        _ => "appliedMv",
    };
    sample[field].as_f64()
}

/// The quality of a saved value: bad when it is missing, forced when the sample says so, and good otherwise (A4.2).
fn saved_quality(sample: &Value, value: Option<f64>) -> Quality {
    match (value, sample["quality"].as_str()) {
        (Some(_), Some("forced")) => Quality::Forced,
        (Some(_), Some("good")) => Quality::Good,
        _ => Quality::Bad,
    }
}

/// The acquisition of a saved recording (`WorkflowRecordingAcquisition`), as the runtime's acquisition evidence (E4.5).
fn acquisition_of(value: &Value) -> Option<Acquisition> {
    Some(Acquisition {
        method: value["method"].as_str()?.to_string(),
        completeness: if value["completeness"].as_str()? == "complete" {
            Completeness::Complete
        } else {
            Completeness::Incomplete
        },
        observed_sample_count: value["observedSampleCount"].as_u64()?,
        scan_range: scan_range(&value["scanRange"]),
        missing_scan_ranges: value["missingScanRanges"]
            .as_array()?
            .iter()
            .map(scan_range)
            .collect::<Option<Vec<_>>>()?,
        dropped_observed_samples: value["droppedObservedSamples"].as_u64()?,
        actual_cycle_timing_available: value["actualCycleTimingAvailable"].as_bool()?,
        freshness_available: value["freshnessAvailable"].as_bool()?,
        reason: value["reason"].as_str()?.to_string(),
    })
}

/// A range of sequence values, or `None` for a null or a malformed range.
fn scan_range(value: &Value) -> Option<ScanRange> {
    Some(ScanRange {
        first: value["first"].as_u64()?,
        last: value["last"].as_u64()?,
    })
}

/// The source a saved result names, as the recording file keeps it (A9).
fn source_of(word: &str) -> Result<EvidenceSource, Refusal> {
    match word {
        "Simulation" => Ok(EvidenceSource::SimulatedPlant),
        "Physical" => Ok(EvidenceSource::PhysicalPlant),
        _ => Err(unreadable()),
    }
}

/// The plan source of a recording's source (A9).
fn plan_source(source: EvidenceSource) -> PlanSource {
    match source {
        EvidenceSource::SimulatedPlant => PlanSource::Simulation,
        EvidenceSource::PhysicalPlant => PlanSource::Physical,
    }
}

/// The recorded span from the first sample to the last, in ms of runtime time; zero without two timed samples.
fn recorded_duration_ms(run: &RecordedRun) -> f64 {
    run.samples
        .last()
        .and_then(|last| last.runtime_time_ns)
        .zip(run.samples.first().and_then(|first| first.runtime_time_ns))
        .and_then(|(last, first)| last.checked_sub(first))
        .and_then(|span| u64::try_from(span).ok())
        .map_or(0.0, |span| {
            Duration::from_nanos(span).as_secs_f64() * 1000.0
        })
}

/// Nanoseconds of runtime time from the seconds a saved sample writes, or `None` when they are not a time.
pub(crate) fn nanoseconds(seconds: f64) -> Option<i64> {
    let duration = Duration::try_from_secs_f64(seconds).ok()?;
    i64::try_from(duration.as_nanos()).ok()
}

/// Whole milliseconds of wall-clock time from the milliseconds a saved sample writes, or `None` when they are not a time.
fn milliseconds(ms: f64) -> Option<u64> {
    let duration = Duration::try_from_secs_f64(ms / 1000.0).ok()?;
    u64::try_from(duration.as_millis()).ok()
}

/// A text field that is present and not empty.
fn text(value: &Value) -> Option<&str> {
    value.as_str().filter(|text| !text.is_empty())
}

fn unreadable() -> Refusal {
    Refusal::new(UNREADABLE_SENTENCE)
}

#[cfg(test)]
#[path = "opened_tests.rs"]
mod tests;
