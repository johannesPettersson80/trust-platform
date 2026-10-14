//! The measured frequency response of a retained periodic recording (`measuredFrequency`, B9.9; N2.8, N4.3): the record's
//! applied input and measured output, read as the page's periodic rule reads them, and the tones estimated on them. The
//! estimate is `identification::frequency_identification`, and its rows are `identification::measured_frequency_rows`.

#![allow(clippy::module_name_repetitions)] // The result keeps the name of its tool.

use std::f64::consts::PI;

use schemars::JsonSchema;
use serde::{Deserialize, Serialize};
use trust_control_math::sampling::first_gap;

use crate::analysis::budget::Budget;
use crate::analysis::tool::input_failure;
use crate::failure::{Cause, Failure, NextAction};
use crate::identification::frequency_identification::estimate_periodic_frequency;
use crate::identification::frequency_texts::frequency_refusal;
use crate::identification::measured_frequency_rows::{
    measured_frequency_rows, MeasuredFrequencyRow,
};
use crate::identification::texts::gap_phrase;
use crate::limits::RECORDING_SAMPLE_LIMIT;
use crate::loop_definition::LoopDefinition;
use crate::messages::AnalysisInputs;
use crate::recording::channels::{applied_mv, process_value, sample_quality, ChannelQuality};
use crate::recording::samples::seconds_between;
use crate::recording::sequence::steady_record_period;
use crate::recording::{Completeness, RecordedRun, RecordedSample};
use crate::tuning::texts::engineering;

/// The fewest samples a periodic record keeps (`decodePeriodicRecording`).
const MINIMUM_SAMPLES: usize = 32;

/// The tones of a request that gives none (B9.9, C7): 1, 2 and 4 rad/s.
const DEFAULT_FREQUENCIES: [f64; 3] = [1.0, 2.0, 4.0];

/// The segments of a request that gives none (N2.8).
const DEFAULT_SEGMENTS: u32 = 4;

/// The minimum coherence of a request that gives none (N2.8).
const DEFAULT_MINIMUM_COHERENCE: f64 = 0.8;

/// The startup samples of a request that gives none (N2.8).
const DEFAULT_STARTUP_SAMPLES: u32 = 16;

const NOT_PERIODIC: &str =
    "Periodic measurement requires finite good-quality samples without applied-output clipping.";
const NO_RECORDING: &str = "recordingId must name the recording of the study.";
const NO_REVISION: &str = "recordingRevision must be the revision of that recording.";
const TONES: &str = "frequencies must be finite, positive tones below π/Ts (N2.8).";
const SEGMENTS: &str = "segmentCount must be an integer of at least 2 (N2.8).";
const COHERENCE: &str = "minimumCoherence must be from 0 to 1 (N2.8).";
const STARTUP: &str = "startupSamples must be at least 1 and below the number of samples (N2.8).";

/// The status of a measured frequency response: its tones were estimated, or the estimate refused them.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "kebab-case")]
pub enum MeasuredStatus {
    /// The tones were estimated.
    Qualified,
    /// The estimate refused, for the reason the result gives.
    Rejected,
}

/// One measured tone (N2.8): its frequency, its magnitude and that magnitude in dB, its unwrapped phase in degrees and its
/// coherence.
#[derive(Debug, Clone, Copy, PartialEq, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct MeasuredPoint {
    /// The tone's angular frequency in rad/s.
    pub frequency_rad_per_second: f64,
    /// The magnitude `|H(jω)|`.
    pub magnitude: f64,
    /// The magnitude in dB, with a magnitude of zero read as the smallest positive number.
    pub magnitude_db: f64,
    /// The unwrapped phase in degrees.
    pub phase_degrees: f64,
    /// The coherence `γ²(ω)`.
    pub coherence: f64,
}

/// A measured frequency response of a retained periodic recording (N2.8; B10.2 `periodicFrequency`): the recording it was
/// measured on, its settings, and its tones or the reason the estimate refused them.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct MeasuredFrequencyResult {
    /// The recording's identifier in the study: the identifier `importRecording` answers, which the study lists, and not the
    /// identifier the runtime gave the run.
    pub recording_id: String,
    /// The revision of the recording.
    pub recording_revision: String,
    /// The sample time of the record, read from its cycles.
    pub sample_time_seconds: f64,
    /// The tones requested, in rad/s.
    pub frequencies: Vec<f64>,
    /// The segments of the estimate.
    pub segment_count: u32,
    /// The minimum coherence of a tone.
    pub minimum_coherence: f64,
    /// The samples discarded from the start of the record.
    pub startup_samples: u32,
    /// Whether the tones were estimated.
    pub status: MeasuredStatus,
    /// The reason of a rejected estimate.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub reason: Option<String>,
    /// The tones in ascending frequency, with their phases unwrapped; empty when rejected.
    pub points: Vec<MeasuredPoint>,
    /// The sentences of the result area, warnings first (B9.9).
    pub summary: Vec<String>,
    /// The sentences of the details disclosure.
    pub details: Vec<String>,
}

/// The record a measured frequency response is estimated on: the applied input and the measured output of each scan, and the
/// sample time that the scans' cycles give.
#[derive(Debug, Clone, PartialEq)]
pub struct PeriodicRecord {
    /// The applied input of each scan.
    pub input: Vec<f64>,
    /// The measured output of each scan.
    pub output: Vec<f64>,
    /// The median cycle of the scans, in seconds.
    pub sample_time_seconds: f64,
}

/// Reads a retained recording as the page's periodic rule reads it (`decodePeriodicRecording`): the recording is complete,
/// with no missing scan and no dropped sample; every sample is good, finite and not on an output limit; the scans step at one
/// steady record period; and no cycle is a gap of more than one and a half sample times.
///
/// # Errors
/// Returns the failure that says the recording is not usable for a periodic measurement, or that names its gap.
pub fn periodic_record(
    recording: &RecordedRun,
    loop_definition: &LoopDefinition,
) -> Result<PeriodicRecord, Failure> {
    let refused = || Failure::new(NOT_PERIODIC, Cause::ModelOrData, NextAction::None);
    let acquisition = &recording.acquisition;
    if recording.samples.len() < MINIMUM_SAMPLES
        || acquisition.completeness != Completeness::Complete
        || !acquisition.missing_scan_ranges.is_empty()
        || acquisition.dropped_observed_samples != 0
        || !acquisition.actual_cycle_timing_available
    {
        return Err(refused());
    }
    let (Some(minimum), Some(maximum)) = (
        loop_definition.limits.output_minimum,
        loop_definition.limits.output_maximum,
    ) else {
        return Err(refused());
    };
    let mut input = Vec::with_capacity(recording.samples.len());
    let mut output = Vec::with_capacity(recording.samples.len());
    for sample in &recording.samples {
        if sample_quality(recording, loop_definition, sample) != ChannelQuality::Good {
            return Err(refused());
        }
        let (Some(applied), Some(measured)) = (
            applied_mv(recording, loop_definition, sample),
            process_value(recording, sample),
        ) else {
            return Err(refused());
        };
        if !applied.is_finite()
            || !measured.is_finite()
            || on_limit(applied, minimum)
            || on_limit(applied, maximum)
        {
            return Err(refused());
        }
        input.push(applied);
        output.push(measured);
    }
    let sequences: Vec<u64> = recording.samples.iter().map(|sample| sample.scan).collect();
    if !steady_record_period(&sequences, true) {
        return Err(refused());
    }
    let cycles = cycles_seconds(&recording.samples).ok_or_else(refused)?;
    let sample_time_seconds = median(&cycles).ok_or_else(refused)?;
    let mut times = vec![0.0];
    let mut time = 0.0;
    for cycle in &cycles {
        time += cycle;
        times.push(time);
    }
    if let Some(gap) = first_gap(&times, sample_time_seconds) {
        return Err(Failure::new(
            &format!("The recording has {}.", gap_phrase(gap)),
            Cause::ModelOrData,
            NextAction::None,
        ));
    }
    Ok(PeriodicRecord {
        input,
        output,
        sample_time_seconds,
    })
}

/// The sentence of a measured response for which the study keeps no recording that is a periodic record (B9.9).
const NO_PERIODIC_RECORDING: &str =
    "The study keeps no recording that is a periodic record to measure.";

/// The recording a measured response measures when its request names none (§0.1a, B9.9): the last of the study's kept `recordings`, in
/// order, that is a periodic record (`periodic_record`). It is returned with its identifier in the study, the entry of `recordings`,
/// which the answer names. `read` reads a recording by that identifier. A recording that is not a periodic record is passed over.
///
/// # Errors
/// A recording cannot be read, or the study keeps no recording that is a periodic record; the failure of the second names the
/// next action Identify.
pub fn last_periodic_recording(
    recordings: &[String],
    loop_definition: &LoopDefinition,
    mut read: impl FnMut(&str) -> Result<RecordedRun, Failure>,
) -> Result<(String, RecordedRun), Failure> {
    for id in recordings.iter().rev() {
        let recording = read(id.as_str())?;
        if periodic_record(&recording, loop_definition).is_ok() {
            return Ok((id.clone(), recording));
        }
    }
    Err(Failure::new(
        NO_PERIODIC_RECORDING,
        Cause::ModelOrData,
        NextAction::Identify,
    ))
}

/// The measured frequency response of a retained recording (`measuredFrequency`, B9.9; N2.8): the tones of the request,
/// estimated on the record after its startup samples. A tone that the estimate refuses gives a rejected result with its reason.
/// `recording_id` is the recording's identifier in the study (the identifier `importRecording` answers, which the study lists), and
/// the answer names it; `recording` is the run the study keeps under it. `budget` is checked before the record and before the
/// estimate (A7).
///
/// # Errors
/// Returns the input failure of a setting outside its limit or of a recording or revision that the request names but that is not
/// the given recording's, the failure of a recording that is not a periodic record, or the budget's failure.
#[allow(clippy::too_many_lines)] // The answer reads, checks, estimates and words its result in the order of N2.8 and A7.
pub fn measured_frequency_answer(
    recording_id: &str,
    recording: &RecordedRun,
    loop_definition: &LoopDefinition,
    inputs: &AnalysisInputs,
    budget: &Budget<'_>,
) -> Result<MeasuredFrequencyResult, Failure> {
    budget.step()?;
    // The request may name its recording and the revision of that recording (B9.9). Each one it names must be the given
    // recording's. The recording's identifier is the study's, never the identifier the runtime gave the run (§0.1a). A request
    // that names neither measures the recording the caller chose, and the answer names it.
    if inputs
        .recording_id
        .as_deref()
        .is_some_and(|id| id != recording_id)
    {
        return Err(input_failure(NO_RECORDING));
    }
    if inputs
        .recording_revision
        .as_deref()
        .is_some_and(|revision| revision != recording.loop_revision)
    {
        return Err(input_failure(NO_REVISION));
    }
    let recording_id = recording_id.to_string();
    let recording_revision = recording.loop_revision.clone();
    let record = periodic_record(recording, loop_definition)?;
    if u64::try_from(record.input.len()).unwrap_or(u64::MAX) > RECORDING_SAMPLE_LIMIT {
        return Err(input_failure(
            "The recording has more samples than the analysis takes (100,000).",
        ));
    }
    let frequencies = inputs
        .frequencies
        .clone()
        .unwrap_or_else(|| DEFAULT_FREQUENCIES.to_vec());
    let segment_count = inputs.segment_count.unwrap_or(DEFAULT_SEGMENTS);
    let minimum_coherence = inputs
        .minimum_coherence
        .unwrap_or(DEFAULT_MINIMUM_COHERENCE);
    let startup_samples = inputs.startup_samples.unwrap_or(DEFAULT_STARTUP_SAMPLES);
    check_settings(
        &frequencies,
        segment_count,
        minimum_coherence,
        startup_samples,
        record.sample_time_seconds,
        record.input.len(),
    )?;
    budget.step()?;
    let estimate = estimate_periodic_frequency(
        &record.input,
        &record.output,
        record.sample_time_seconds,
        &frequencies,
        usize::try_from(segment_count).unwrap_or(usize::MAX),
        minimum_coherence,
        usize::try_from(startup_samples).unwrap_or(usize::MAX),
    );
    let (status, reason, points): (MeasuredStatus, Option<String>, Vec<MeasuredPoint>) =
        match estimate {
            Ok(points) => (
                MeasuredStatus::Qualified,
                None,
                measured_frequency_rows(&points)
                    .into_iter()
                    .map(measured_point)
                    .collect(),
            ),
            Err(rejection) => (
                MeasuredStatus::Rejected,
                Some(frequency_refusal(rejection).to_string()),
                Vec::new(),
            ),
        };
    let (summary, details) = match (status, &reason) {
        (MeasuredStatus::Rejected, Some(reason)) => (
            vec![format!("Measured frequency response unavailable: {reason}")],
            Vec::new(),
        ),
        _ => (
            vec![format!(
                "Measured {} frequency {}.",
                points.len(),
                if points.len() == 1 { "point" } else { "points" }
            )],
            vec![format!(
                "Sample time {} s from the recording; {startup_samples} startup samples discarded; {segment_count} segments; minimum coherence {}.",
                engineering(record.sample_time_seconds, None),
                engineering(minimum_coherence, None)
            )],
        ),
    };
    Ok(MeasuredFrequencyResult {
        recording_id,
        recording_revision,
        sample_time_seconds: record.sample_time_seconds,
        frequencies,
        segment_count,
        minimum_coherence,
        startup_samples,
        status,
        reason,
        points,
        summary,
        details,
    })
}

/// The settings of a measured response that N2.8 and the table of A7 bound: the tones below `π/Ts`, at least two segments, a
/// coherence from 0 to 1, and startup samples fewer than the samples.
fn check_settings(
    frequencies: &[f64],
    segment_count: u32,
    minimum_coherence: f64,
    startup_samples: u32,
    sample_time_seconds: f64,
    samples: usize,
) -> Result<(), Failure> {
    let nyquist = PI / sample_time_seconds;
    if frequencies.is_empty()
        || !frequencies
            .iter()
            .all(|frequency| frequency.is_finite() && *frequency > 0.0 && *frequency < nyquist)
    {
        return Err(input_failure(TONES));
    }
    if segment_count < 2 {
        return Err(input_failure(SEGMENTS));
    }
    if !minimum_coherence.is_finite() || !(0.0..=1.0).contains(&minimum_coherence) {
        return Err(input_failure(COHERENCE));
    }
    if startup_samples < 1 || usize::try_from(startup_samples).unwrap_or(usize::MAX) >= samples {
        return Err(input_failure(STARTUP));
    }
    Ok(())
}

fn measured_point(row: MeasuredFrequencyRow) -> MeasuredPoint {
    MeasuredPoint {
        frequency_rad_per_second: row.frequency,
        magnitude: row.magnitude,
        magnitude_db: 20.0 * row.magnitude.max(f64::from_bits(1)).log10(),
        phase_degrees: row.phase_degrees,
        coherence: row.coherence,
    }
}

/// The seconds between each two consecutive samples, from their runtime times; `None` when a time is missing or a cycle does
/// not advance (`periodicRecordingSource.ts`).
fn cycles_seconds(samples: &[RecordedSample]) -> Option<Vec<f64>> {
    let mut cycles = Vec::with_capacity(samples.len().saturating_sub(1));
    for pair in samples.windows(2) {
        let cycle = seconds_between(pair[0].runtime_time_ns?, pair[1].runtime_time_ns?);
        if !cycle.is_finite() || cycle <= 0.0 {
            return None;
        }
        cycles.push(cycle);
    }
    Some(cycles)
}

/// The median of the cycles as the page takes it: the element at `floor(n / 2)` of the sorted values.
fn median(values: &[f64]) -> Option<f64> {
    let mut sorted = values.to_vec();
    sorted.sort_by(f64::total_cmp);
    sorted.get(sorted.len() / 2).copied()
}

/// Whether a sample sits exactly on an output limit: such a sample is saturated and is not used (A4.2).
#[allow(clippy::float_cmp)] // Saturation is an exact match of the output limit.
fn on_limit(value: f64, limit: f64) -> bool {
    value == limit
}

#[cfg(test)]
#[path = "measured_frequency_tests.rs"]
mod tests;
