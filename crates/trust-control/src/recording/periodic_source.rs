//! The retained periodic source of a recording (`periodicRecordingSource.ts`): the scans of a complete recording
//! that stepped at one steady record period, placed in process time and resampled onto their even grid for the fit
//! (A4.3, N1.2, N1.4). The grid, the gap rule and the block averaging are the library's
//! (`trust_control_math::sampling`).

use trust_control_math::identify::SampleQuality;
use trust_control_math::sampling::{
    block_averaged, first_gap, median_spacing, resampled_evenly, EvenSample,
};

use crate::limits::FIT_POINT_LIMIT;
use crate::loop_definition::LoopDefinition;
use crate::recording::channels::{applied_mv, process_value, sample_quality, ChannelQuality};
use crate::recording::samples::seconds_between;
use crate::recording::sequence::steady_record_period;
use crate::recording::{Completeness, RecordedRun, RecordedSample};

/// The fewest samples a periodic source keeps (`periodicRecordingSource.ts`).
const MINIMUM_SAMPLES: usize = 32;

/// One scan of a retained recording: its time in process seconds from the first scan, the manipulated input the
/// process received and the measured output (`TimedScan`).
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct TimedScan {
    /// The scan's time from the first scan, in seconds.
    pub time_seconds: f64,
    /// The manipulated input the process received (the applied value).
    pub input: f64,
    /// The measured output.
    pub output: f64,
}

/// The retained periodic source of a recording (`RetainedPeriodicRecordingSource`): the fit's scans on their even grid.
#[derive(Debug, Clone, PartialEq)]
pub struct RetainedPeriodicRecordingSource {
    /// The fit's scans in time order, on the even grid, block-averaged to at most [`FIT_POINT_LIMIT`] scans.
    pub scans: Vec<TimedScan>,
    /// The sample time of the fit: the median cycle, or the median spacing of the averaged grid when the averaging
    /// changed the number of scans.
    pub sample_time_seconds: f64,
    /// The recording's identifier.
    pub recording_id: String,
    /// The revision the recording was retained for.
    pub recording_revision: String,
}

/// The retained periodic source of `recording` (`retainedPeriodicRecordingSource`), placed on its even grid for the fit.
///
/// `provenance_revision` is the result revision of an imported recording, as text (its `provenance`); a live
/// recording has none and is held to the loop revision its runs carry. The recording must be complete, carry
/// `recording_id` and `recording_revision`, hold at least 32 samples, have only good samples whose applied input
/// and measurement are finite and not on an output limit, step at one steady record period, and have a positive
/// cycle between every two samples, none longer than one and a half median cycles. Otherwise it returns `None`.
#[must_use]
pub fn retained_periodic_recording_source(
    recording: Option<&RecordedRun>,
    loop_definition: &LoopDefinition,
    recording_id: &str,
    recording_revision: &str,
    provenance_revision: Option<&str>,
) -> Option<RetainedPeriodicRecordingSource> {
    let run = recording?;
    let samples = &run.samples;
    if samples.len() < MINIMUM_SAMPLES {
        return None;
    }
    let acquisition = &run.acquisition;
    if acquisition.completeness != Completeness::Complete
        || run.recording_id != recording_id
        || !acquisition.missing_scan_ranges.is_empty()
        || acquisition.dropped_observed_samples != 0
        || !acquisition.actual_cycle_timing_available
    {
        return None;
    }
    let revision = provenance_revision.unwrap_or(&run.loop_revision);
    if revision != recording_revision {
        return None;
    }
    // A saturated input is told apart from a usable one only against both output limits.
    let (Some(minimum), Some(maximum)) = (
        loop_definition.limits.output_minimum,
        loop_definition.limits.output_maximum,
    ) else {
        return None;
    };
    let mut inputs = Vec::with_capacity(samples.len());
    let mut outputs = Vec::with_capacity(samples.len());
    for sample in samples {
        if sample_quality(run, loop_definition, sample) != ChannelQuality::Good {
            return None;
        }
        let input = applied_mv(run, loop_definition, sample)?;
        let output = process_value(run, sample)?;
        if !input.is_finite()
            || !output.is_finite()
            || is_on_limit(input, minimum)
            || is_on_limit(input, maximum)
        {
            return None;
        }
        inputs.push(input);
        outputs.push(output);
    }
    let sequences: Vec<u64> = samples.iter().map(|sample| sample.scan).collect();
    if !steady_record_period(&sequences, true) {
        return None;
    }
    // The first scan has no predecessor, so each later scan carries the cycle from the one before it.
    let cycles = cycles_seconds(samples)?;
    let sample_time_seconds = median(&cycles)?;
    let mut time = 0.0;
    let mut times = Vec::with_capacity(samples.len());
    times.push(time);
    for cycle in &cycles {
        time += cycle;
        times.push(time);
    }
    // A cycle longer than one and a half sample times is a gap that a periodic fit cannot bridge (`firstGap`).
    if first_gap(&times, sample_time_seconds).is_some() {
        return None;
    }
    let scan_samples: Vec<EvenSample> = times
        .iter()
        .zip(inputs.iter().zip(&outputs))
        .map(|(time, (input, output))| EvenSample {
            time_seconds: *time,
            input: *input,
            output: *output,
            quality: SampleQuality::Good,
            clipped: false,
            saturated: false,
        })
        .collect();
    // The command is held between scans and the output interpolated onto the grid (`resampledEvenly`).
    let even = resampled_evenly(&scan_samples, Some(sample_time_seconds)).ok()?;
    // The fit holds at most FIT_POINT_LIMIT points (`blockAveraged`, A4.5).
    let limit = usize::try_from(FIT_POINT_LIMIT).ok()?;
    let fitted = block_averaged(&even, limit, &[]).ok()?;
    let sample_time_seconds = if fitted.len() == even.len() {
        sample_time_seconds
    } else {
        let fitted_times: Vec<f64> = fitted.iter().map(|sample| sample.time_seconds).collect();
        median_spacing(&fitted_times)
    };
    let scans = fitted
        .iter()
        .map(|sample| TimedScan {
            time_seconds: sample.time_seconds,
            input: sample.input,
            output: sample.output,
        })
        .collect();
    Some(RetainedPeriodicRecordingSource {
        scans,
        sample_time_seconds,
        recording_id: recording_id.to_string(),
        recording_revision: recording_revision.to_string(),
    })
}

/// Whether a sample sits exactly on an output limit: such a sample is saturated and is not used (A4.2).
#[allow(clippy::float_cmp)] // Saturation is an exact match of the output limit.
fn is_on_limit(value: f64, limit: f64) -> bool {
    value == limit
}

/// The seconds between each two consecutive samples, from their runtime times, in order. The first sample has no
/// predecessor, so there is one cycle fewer than samples. `None` when a runtime time is missing or a cycle does not
/// advance.
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

/// The median of positive values as `periodicRecordingSource.ts` takes it: the element at `floor(n / 2)` of the
/// sorted values, so the upper middle one for an even count.
fn median(values: &[f64]) -> Option<f64> {
    let mut sorted = values.to_vec();
    sorted.sort_by(f64::total_cmp);
    sorted.get(sorted.len() / 2).copied()
}

#[cfg(test)]
#[path = "periodic_source_tests.rs"]
mod tests;
