//! The data of an identification (specification 35, B3.4 and B3.5; numerical annex N1): the qualification summary of a
//! recording before any fit (N1.1), the operating point before its step (N1.3, N2.2), the held part of a bounded step
//! (B3.4 step 5), and the record a fit receives, block-averaged to at most 5,000 points (A4.5, N1.4). The rules are
//! those of `identification.ts` and `evenSampling.ts`; the step search and the statistics are the library's
//! (`trust_control_math`).

#![allow(clippy::module_name_repetitions)] // `QualifiedIdentificationData` is the name the `TypeScript` record has.

use schemars::JsonSchema;
use serde::{Deserialize, Serialize};
use trust_control_math::identify::single_run::largest_input_change;
use trust_control_math::sampling::statistics::{mean, noise_sigma};
use trust_control_math::sampling::{largest, smallest};

use crate::experiment::SampleQuality;
use crate::identification::refusal::Refusal;
use crate::identification::step_response::StepResponseMeasurement;

/// One sample Identify fits: the Test sample, under the name the identification rules use (`IdentificationSample`).
pub use crate::experiment::ExperimentIdentificationSample as IdentificationSample;

/// The most points a fit receives (A4.5, N1.4): a longer record is block-averaged to this many before fitting
/// (`MAXIMUM_FIT_SAMPLES`).
pub const MAXIMUM_FIT_SAMPLES: usize = 5_000;

/// The fewest finite good samples a recording needs before any fit (N1.1).
const MINIMUM_QUALIFIED_SAMPLES: usize = 20;

/// The span the input and the output must exceed to count as an excitation and as a response.
const MINIMUM_SPAN: f64 = 1e-9;

/// The baseline noise at or below which a baseline counts as noise-free in the signal-to-noise estimate.
const NOISE_FLOOR: f64 = 1e-12;

/// The signal-to-noise estimate, in dB, of a response over a noise-free baseline.
const NOISE_FREE_SIGNAL_TO_NOISE_DB: f64 = 300.0;

/// An interval longer than this many median intervals is a gap (A4.3, N1.2).
const GAP_INTERVALS: f64 = 1.5;

/// The unit a recording reports when no sample states one.
const DEFAULT_UNIT: &str = "unit";

/// The coherence of a time-domain excitation: it is never derived from a step response (N1.1).
const COHERENCE_NOT_EVALUATED: &str = "Not evaluated for time-domain excitation";

/// An interval the engineer excludes from a fit, in seconds of the recording's time, both ends included (the selected
/// intervals of B10.4; `ExcludedInterval`).
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "camelCase")]
pub struct ExcludedInterval {
    /// The first excluded time, in s.
    pub start_seconds: f64,
    /// The last excluded time, in s.
    pub end_seconds: f64,
}

/// Where the samples of an identification come from (B3.1): a simulated or a physical plant
/// (`"Simulation" | "Physical"`). The `TypeScript` default is a simulation.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
pub enum DataSource {
    /// A simulated plant.
    #[default]
    Simulation,
    /// A physical plant.
    Physical,
}

/// The excitation class of a recording (N1.1): a step when the input changes, insufficient when it never does.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "lowercase")]
pub enum Excitation {
    /// The input changes: the largest change is the step to fit.
    Step,
    /// The input never changes.
    Insufficient,
}

/// The qualification summary of a recording before any fit (N1.1; `IdentificationQualificationMetrics`). The fields
/// are in the order the `TypeScript` record has them.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "camelCase")]
pub struct IdentificationQualificationMetrics {
    /// The number of samples received.
    pub raw_sample_count: usize,
    /// The number of finite good samples outside the excluded intervals.
    pub retained_sample_count: usize,
    /// The time from the first to the last retained sample, in s; zero with fewer than two.
    pub duration_seconds: f64,
    /// The median positive interval between retained samples, in s.
    pub median_sample_time_seconds: f64,
    /// The largest deviation of an interval from the median interval, in percent of the median interval.
    pub maximum_sample_time_deviation_percent: f64,
    /// The number of intervals longer than one and a half median intervals (A4.3, A4.4).
    pub gap_count: usize,
    /// The number of received samples whose quality is not good.
    pub rejected_quality_sample_count: usize,
    /// The number of received samples reported as clipped.
    pub clipped_sample_count: usize,
    /// The number of received samples reported as saturated.
    pub saturation_sample_count: usize,
    /// The drift of the measured output across the baseline, per second.
    pub baseline_drift_per_second: f64,
    /// The span of the retained inputs: their maximum minus their minimum.
    pub input_span: f64,
    /// The span of the retained outputs.
    pub output_span: f64,
    /// The time-domain signal-to-noise estimate in dB: the output span over the baseline noise `σ` (N1.3, N2.6).
    pub signal_to_noise_db: f64,
    /// The excitation class.
    pub excitation: Excitation,
    /// The coherence, which is never evaluated for a time-domain excitation (N1.1).
    pub coherence: String,
    /// Whether an independent validation recording is available.
    pub independent_validation_available: bool,
}

/// A recording qualified for a fit (N1.1; `QualifiedIdentificationData`): the samples received, the samples a fit may
/// use, and whether they suffice. Suitability for a same-run preview and qualification for controller design are
/// separate states: a recording is suitable for design only with independent validation available.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "camelCase")]
pub struct QualifiedIdentificationData {
    /// Every sample received, unchanged; raw samples are never deleted (N1.4).
    pub raw_samples: Vec<IdentificationSample>,
    /// The finite good samples outside the excluded intervals.
    pub samples: Vec<IdentificationSample>,
    /// Whether the retained samples suffice for a fit.
    pub suitable: bool,
    /// Why they do not suffice, when they do not.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub reason: Option<String>,
    /// The first unit a sample states for the input, or `unit`.
    pub input_unit: String,
    /// The first unit a sample states for the output, or `unit`.
    pub output_unit: String,
    /// The median positive interval between retained samples, in s.
    pub sample_time_seconds: f64,
    /// Whether the recording qualifies for controller design: suitable, with independent validation available.
    pub suitable_for_design: bool,
    /// The qualification summary.
    pub metrics: IdentificationQualificationMetrics,
    /// The measured step response of a bounded step review; set by the caller that measured it.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub step_response: Option<StepResponseMeasurement>,
}

/// The operating point before the step (N1.3, N2.2; `identificationOperatingPoint`): the index of the largest input
/// change, and the means `u0` of the input and `y0` of the output over the samples before it.
#[derive(Debug, Clone, PartialEq)]
pub struct OperatingPoint {
    /// The index of the first sample after the largest input change; at least one.
    pub step_index: usize,
    /// The mean input before the step, `u0`.
    pub input0: f64,
    /// The mean output before the step, `y0`.
    pub output0: f64,
}

/// Qualifies a recording before any fit (N1.1; `qualifyIdentificationData`). A fit may use the samples whose quality
/// is good, whose time, input and output are finite, and whose time lies outside every excluded interval. They are
/// refused, with the first reason that applies, when fewer than twenty remain, when their times do not increase
/// strictly, when the input span is `10⁻⁹` or less, or when the output span is. The raw samples are kept unchanged.
#[must_use]
pub fn qualify_identification_data(
    samples: &[IdentificationSample],
    excluded: &[ExcludedInterval],
    independent_validation_available: bool,
) -> QualifiedIdentificationData {
    let usable: Vec<IdentificationSample> = samples
        .iter()
        .filter(|sample| {
            sample.quality == SampleQuality::Good
                && finite_sample(sample)
                && !excluded.iter().any(|interval| {
                    sample.time_seconds >= interval.start_seconds
                        && sample.time_seconds <= interval.end_seconds
                })
        })
        .cloned()
        .collect();
    let sample_time_seconds = median_interval(&usable);
    let metrics = qualification_metrics(
        samples,
        &usable,
        sample_time_seconds,
        independent_validation_available,
    );
    let reason = unsuitable_reason(&usable);
    let suitable = reason.is_none();
    QualifiedIdentificationData {
        raw_samples: samples.to_vec(),
        samples: usable,
        suitable,
        reason: reason.map(str::to_string),
        input_unit: first_input_unit(samples),
        output_unit: first_output_unit(samples),
        sample_time_seconds,
        suitable_for_design: suitable && independent_validation_available,
        metrics,
        step_response: None,
    }
}

/// The operating point before the step (N1.3, N2.2; `identificationOperatingPoint`): the largest input change, and the
/// means of the input and the output over the samples before it. Fit evidence and the selected models share it.
///
/// # Errors
/// Refuses fewer than two samples, or a sample whose time, input or output is not finite ("Model evidence requires
/// finite measured samples."), and an input that never changes ("Model evidence requires a measured input change.").
pub fn identification_operating_point(
    samples: &[IdentificationSample],
) -> Result<OperatingPoint, Refusal> {
    if samples.len() < 2 || !samples.iter().all(finite_sample) {
        return Err(Refusal::new(
            "Model evidence requires finite measured samples.",
        ));
    }
    let Some(step_index) = largest_change(samples) else {
        return Err(Refusal::new(
            "Model evidence requires a measured input change.",
        ));
    };
    let before = &samples[..step_index];
    Ok(OperatingPoint {
        step_index,
        input0: mean(&inputs_of(before)),
        output0: mean(&outputs_of(before)),
    })
}

/// The record a fit receives (A4.5, N1.4; `fitRecording`): at most [`MAXIMUM_FIT_SAMPLES`] points, with no block
/// straddling the largest input change. A recording within the limit is returned unchanged. A longer one is
/// block-averaged: the block size starts at `⌈n / 5,000⌉` and grows by one while the step's cut leaves more blocks than
/// the limit allows; a block takes the mean time, input and output of its members, the units of its first member, and
/// it is bad, clipped or saturated when any member is (bad also when any member is clipped or saturated).
#[must_use]
pub fn fit_recording(samples: &[IdentificationSample]) -> Vec<IdentificationSample> {
    if samples.len() <= MAXIMUM_FIT_SAMPLES {
        return samples.to_vec();
    }
    let cuts_seconds = match largest_change(samples) {
        Some(index) => vec![samples[index].time_seconds],
        None => Vec::new(),
    };
    // The block rule of `blockAveraged` is applied here rather than through `trust_control_math::sampling::
    // block_averaged`: the library's samples carry no units, and its blocks cannot be recovered from its result, so
    // the units of each block's first member would be lost.
    let starts = fit_block_starts(samples, &cuts_seconds);
    starts
        .iter()
        .enumerate()
        .filter_map(|(block, &start)| {
            let end = starts.get(block + 1).copied().unwrap_or(samples.len());
            averaged_block(&samples[start..end])
        })
        .collect()
}

/// The held part of a bounded step (B3.4 step 5; `heldStepSamples`): the samples before the first restoration of the
/// largest input change, that is, the first later change against the step's direction of at least half its size. The
/// whole recording when the input never changes or is never restored.
#[must_use]
pub fn held_step_samples(samples: &[IdentificationSample]) -> Vec<IdentificationSample> {
    let Some(step_index) = largest_change(samples) else {
        return samples.to_vec();
    };
    let change = samples[step_index].input - samples[step_index - 1].input;
    let magnitude = change.abs();
    let direction = change.signum();
    let restoration = (step_index + 1..samples.len()).find(|&index| {
        let difference = samples[index].input - samples[index - 1].input;
        difference * direction < 0.0 && difference.abs() >= magnitude * 0.5
    });
    match restoration {
        Some(index) => samples[..index].to_vec(),
        None => samples.to_vec(),
    }
}

/// Whether two recordings hold the same samples (`sameRecording`): the same count, and at each index the same time,
/// input, output and quality. A missing (`NaN`) value equals nothing, as in the `TypeScript`.
#[allow(clippy::float_cmp)] // The recordings are compared value for value, exactly, as the `TypeScript` compares them.
pub(crate) fn same_recording(
    left: &[IdentificationSample],
    right: &[IdentificationSample],
) -> bool {
    left.len() == right.len()
        && left.iter().zip(right).all(|(sample, other)| {
            sample.time_seconds == other.time_seconds
                && sample.input == other.input
                && sample.output == other.output
                && sample.quality == other.quality
        })
}

/// The median interval between samples (`medianIntervals`): of the finite positive intervals in time order, sorted,
/// the one at index `⌊n/2⌋`; zero when there is none.
pub(crate) fn median_interval(samples: &[IdentificationSample]) -> f64 {
    let mut intervals = positive_intervals(samples);
    intervals.sort_by(f64::total_cmp);
    intervals.get(intervals.len() / 2).copied().unwrap_or(0.0)
}

/// Whether a sample's time, input and output are all finite (`finiteSample`).
pub(crate) fn finite_sample(sample: &IdentificationSample) -> bool {
    sample.time_seconds.is_finite() && sample.input.is_finite() && sample.output.is_finite()
}

/// The first output unit a sample states, or `unit` (`samples.find(sample => sample.outputUnit)?.outputUnit ?? "unit"`).
pub(crate) fn first_output_unit(samples: &[IdentificationSample]) -> String {
    samples
        .iter()
        .filter_map(|sample| sample.output_unit.as_deref())
        .find(|unit| !unit.is_empty())
        .unwrap_or(DEFAULT_UNIT)
        .to_string()
}

/// The inputs of the samples, in order.
pub(crate) fn inputs_of(samples: &[IdentificationSample]) -> Vec<f64> {
    samples.iter().map(|sample| sample.input).collect()
}

/// The outputs of the samples, in order.
pub(crate) fn outputs_of(samples: &[IdentificationSample]) -> Vec<f64> {
    samples.iter().map(|sample| sample.output).collect()
}

/// The first input unit a sample states, or `unit` (`samples.find(sample => sample.inputUnit)?.inputUnit ?? "unit"`).
fn first_input_unit(samples: &[IdentificationSample]) -> String {
    samples
        .iter()
        .filter_map(|sample| sample.input_unit.as_deref())
        .find(|unit| !unit.is_empty())
        .unwrap_or(DEFAULT_UNIT)
        .to_string()
}

/// The index of the largest input change (`largestInputChange`): the first sample whose input differs most from the
/// input before it; `None` when the input never changes.
fn largest_change(samples: &[IdentificationSample]) -> Option<usize> {
    largest_input_change(&inputs_of(samples))
}

/// The first reason the retained samples do not suffice for a fit, in the order the `TypeScript` checks them.
fn unsuitable_reason(usable: &[IdentificationSample]) -> Option<&'static str> {
    if usable.len() < MINIMUM_QUALIFIED_SAMPLES {
        return Some("At least 20 finite good samples are required.");
    }
    if !strictly_increasing(usable) {
        return Some("Sample times must be finite and strictly increasing.");
    }
    if span(&inputs_of(usable)) <= MINIMUM_SPAN {
        return Some("Input excitation is too small to identify a model.");
    }
    if span(&outputs_of(usable)) <= MINIMUM_SPAN {
        return Some("Output response is too small to identify a model.");
    }
    None
}

/// The qualification summary (N1.1; `qualificationMetrics`). The baseline is the retained samples before the largest
/// input change when it comes after the second sample, and otherwise the first fifth of them (at least two). Its noise
/// `σ` comes from its first differences (N1.3), and the signal-to-noise estimate is `20·log₁₀(span / σ)`, or 300 dB
/// over a noise-free baseline when the output moves.
fn qualification_metrics(
    raw: &[IdentificationSample],
    usable: &[IdentificationSample],
    sample_time_seconds: f64,
    independent_validation_available: bool,
) -> IdentificationQualificationMetrics {
    let intervals = positive_intervals(usable);
    let maximum_deviation = if sample_time_seconds > 0.0 && !intervals.is_empty() {
        let deviations: Vec<f64> = intervals
            .iter()
            .map(|interval| (interval - sample_time_seconds).abs())
            .collect();
        largest(&deviations, f64::NEG_INFINITY) / sample_time_seconds * 100.0
    } else {
        0.0
    };
    let gap_count = if sample_time_seconds > 0.0 {
        intervals
            .iter()
            .filter(|&&interval| interval > sample_time_seconds * GAP_INTERVALS)
            .count()
    } else {
        0
    };
    let step_index = largest_change(usable);
    let baseline = baseline_window(usable, step_index);
    let noise = noise_sigma(&outputs_of(baseline));
    let output_span = safe_span(&outputs_of(usable));
    let signal_to_noise_db = if noise > NOISE_FLOOR && output_span > 0.0 {
        20.0 * (output_span / noise).log10()
    } else if output_span > 0.0 {
        NOISE_FREE_SIGNAL_TO_NOISE_DB
    } else {
        0.0
    };
    IdentificationQualificationMetrics {
        raw_sample_count: raw.len(),
        retained_sample_count: usable.len(),
        duration_seconds: duration_seconds(usable),
        median_sample_time_seconds: sample_time_seconds,
        maximum_sample_time_deviation_percent: maximum_deviation,
        gap_count,
        rejected_quality_sample_count: raw
            .iter()
            .filter(|sample| sample.quality != SampleQuality::Good)
            .count(),
        clipped_sample_count: raw
            .iter()
            .filter(|sample| sample.clipped == Some(true))
            .count(),
        saturation_sample_count: raw
            .iter()
            .filter(|sample| sample.saturated == Some(true))
            .count(),
        baseline_drift_per_second: baseline_drift_per_second(baseline),
        input_span: safe_span(&inputs_of(usable)),
        output_span,
        signal_to_noise_db,
        excitation: if step_index.is_some() {
            Excitation::Step
        } else {
            Excitation::Insufficient
        },
        coherence: COHERENCE_NOT_EVALUATED.to_string(),
        independent_validation_available,
    }
}

/// The baseline of the qualification summary: the samples before the step when the step comes after the second
/// sample, otherwise the first `max(2, ⌊n/5⌋)` samples (fewer when there are fewer).
fn baseline_window(
    usable: &[IdentificationSample],
    step_index: Option<usize>,
) -> &[IdentificationSample] {
    let end = match step_index {
        Some(index) if index > 1 => index,
        _ => (usable.len() / 5).max(2),
    };
    &usable[..end.min(usable.len())]
}

/// The drift of the baseline's output per second: its change from the first to the last sample over the time between
/// them; zero with fewer than two samples or no time between them.
fn baseline_drift_per_second(baseline: &[IdentificationSample]) -> f64 {
    match (baseline.first(), baseline.last()) {
        (Some(first), Some(last)) if baseline.len() > 1 => {
            let duration = last.time_seconds - first.time_seconds;
            if duration > 0.0 {
                (last.output - first.output) / duration
            } else {
                0.0
            }
        }
        _ => 0.0,
    }
}

/// The time from the first to the last sample; zero with fewer than two samples.
fn duration_seconds(samples: &[IdentificationSample]) -> f64 {
    match (samples.first(), samples.last()) {
        (Some(first), Some(last)) if samples.len() > 1 => last.time_seconds - first.time_seconds,
        _ => 0.0,
    }
}

/// The finite positive intervals between consecutive samples, in time order.
fn positive_intervals(samples: &[IdentificationSample]) -> Vec<f64> {
    samples
        .windows(2)
        .map(|pair| pair[1].time_seconds - pair[0].time_seconds)
        .filter(|interval| interval.is_finite() && *interval > 0.0)
        .collect()
}

/// Whether every sample's time is above the time before it.
fn strictly_increasing(samples: &[IdentificationSample]) -> bool {
    samples
        .windows(2)
        .all(|pair| pair[1].time_seconds > pair[0].time_seconds)
}

/// The span of values: their maximum minus their minimum.
fn span(values: &[f64]) -> f64 {
    largest(values, f64::NEG_INFINITY) - smallest(values)
}

/// The span of values, or zero for no values.
fn safe_span(values: &[f64]) -> f64 {
    if values.is_empty() {
        0.0
    } else {
        span(values)
    }
}

/// The first index of each fit block (`blockAveraged`): the block size starts at `⌈n / 5,000⌉` and grows by one while
/// the cuts leave more blocks than [`MAXIMUM_FIT_SAMPLES`].
fn fit_block_starts(samples: &[IdentificationSample], cuts_seconds: &[f64]) -> Vec<usize> {
    let mut size = samples.len().div_ceil(MAXIMUM_FIT_SAMPLES);
    let mut starts = block_starts(samples, size, cuts_seconds);
    while starts.len() > MAXIMUM_FIT_SAMPLES && size < samples.len() {
        size += 1;
        starts = block_starts(samples, size, cuts_seconds);
    }
    starts
}

/// The first index of each block of at most `size` samples. A sample's side is the number of cuts at or before its
/// time, and a block ends where the side changes, so no block holds samples from two sides of a cut.
fn block_starts(samples: &[IdentificationSample], size: usize, cuts_seconds: &[f64]) -> Vec<usize> {
    let cut_side = |time: f64| cuts_seconds.iter().filter(|&&cut| cut <= time).count();
    let mut starts = Vec::new();
    let mut start = 0;
    while start < samples.len() {
        starts.push(start);
        let block_side = cut_side(samples[start].time_seconds);
        let mut end = start + 1;
        while end < samples.len()
            && end - start < size
            && cut_side(samples[end].time_seconds) == block_side
        {
            end += 1;
        }
        start = end;
    }
    starts
}

/// One fit block (N1.4): the mean time, input and output of its members and the units of its first member. It is bad
/// when any member is bad, clipped or saturated, otherwise uncertain when any member is uncertain, otherwise good; it
/// is clipped or saturated when any member is. `None` for no members.
fn averaged_block(members: &[IdentificationSample]) -> Option<IdentificationSample> {
    let first = members.first()?;
    let clipped = members.iter().any(|sample| sample.clipped == Some(true));
    let saturated = members.iter().any(|sample| sample.saturated == Some(true));
    let quality = if clipped
        || saturated
        || members
            .iter()
            .any(|sample| sample.quality == SampleQuality::Bad)
    {
        SampleQuality::Bad
    } else if members
        .iter()
        .any(|sample| sample.quality == SampleQuality::Uncertain)
    {
        SampleQuality::Uncertain
    } else {
        SampleQuality::Good
    };
    let times: Vec<f64> = members.iter().map(|sample| sample.time_seconds).collect();
    Some(IdentificationSample {
        time_seconds: mean(&times),
        input: mean(&inputs_of(members)),
        output: mean(&outputs_of(members)),
        quality,
        input_unit: first.input_unit.clone(),
        output_unit: first.output_unit.clone(),
        clipped: Some(clipped),
        saturated: Some(saturated),
    })
}

#[cfg(test)]
#[path = "data_tests.rs"]
pub(crate) mod tests;
