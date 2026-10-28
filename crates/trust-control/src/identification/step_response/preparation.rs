//! The preparation of a recording for the step rules (`prepare`; A4.3, A4.5, N1.4): the samples checked, the step and
//! its restoration, the baseline input, the block averaging of the fit, and the output span of the fit record (N1.1,
//! N3.2).

use trust_control_math::identify::response::step_amplitude;
use trust_control_math::identify::SampleQuality;
use trust_control_math::sampling::statistics::mean;
use trust_control_math::sampling::{
    block_averaged, first_gap, largest, median_spacing, smallest, EvenSample,
};

use crate::identification::data::{IdentificationSample, MAXIMUM_FIT_SAMPLES};
use crate::identification::refusal::Refusal;
use crate::identification::texts::{gap_phrase, OUTPUT_TOO_SMALL};

use super::{valid_step_response_settings, StepResponseSettings, EPSILON, INVALID_SETTINGS};

/// A recording prepared for the step rules (`prepare`): the times, inputs and outputs of the fit's samples on their
/// recorded instants, block-averaged to at most `MAXIMUM_FIT_SAMPLES` points with no block across the step or its
/// restoration (A4.3, A4.5); the times of the step and of the restoration; the input before the step and the change
/// to the held input; and the sample time, the median spacing of the recorded samples, or of the averaged ones when
/// the averaging merged samples.
pub(super) struct Prepared {
    pub(super) times: Vec<f64>,
    pub(super) inputs: Vec<f64>,
    pub(super) outputs: Vec<f64>,
    pub(super) step_time: f64,
    pub(super) hold_end: f64,
    pub(super) input_before: f64,
    pub(super) input_change: f64,
    pub(super) sample_time: f64,
}

/// Checks the settings and the samples, finds the step and its restoration, and block-averages the fit (`prepare`).
pub(super) fn prepare(
    samples: &[IdentificationSample],
    settings: &StepResponseSettings,
) -> Result<Prepared, Refusal> {
    if !valid_step_response_settings(settings) {
        return Err(Refusal::new(INVALID_SETTINGS));
    }
    if !complete_samples(samples) {
        return Err(Refusal::new(
            "Step-response identification requires complete finite good-quality samples.",
        ));
    }
    let raw_times: Vec<f64> = samples.iter().map(|sample| sample.time_seconds).collect();
    if raw_times.windows(2).any(|pair| pair[1] <= pair[0]) {
        return Err(Refusal::new(
            "Step-response sample times must be strictly increasing.",
        ));
    }
    // Task jitter is normal; an interval longer than one and a half median spacings is a gap, refused with its time
    // (A4.3).
    let raw_sample_time = median_spacing(&raw_times);
    if let Some(gap) = first_gap(&raw_times, raw_sample_time) {
        return Err(Refusal(format!("The recording has {}.", gap_phrase(gap))));
    }
    if settings.timestamp_shift_seconds.abs() > raw_sample_time + EPSILON {
        return Err(Refusal::new(
            "Step-response timestamp shift must not exceed one sample time.",
        ));
    }
    let (step_index, restoration_index) = step_and_restoration(samples)?;
    let step_time = samples[step_index].time_seconds;
    let hold_end = samples[restoration_index].time_seconds;
    // u₀ is the mean plant input over the steady baseline window, which ends at the step (N1.3, N2.2).
    let baseline_start = step_time - settings.baseline_duration_seconds;
    let baseline_inputs: Vec<f64> = samples
        .iter()
        .filter(|sample| {
            sample.time_seconds >= baseline_start - EPSILON
                && sample.time_seconds < step_time - EPSILON
        })
        .map(|sample| sample.input)
        .collect();
    let input_before = mean(&baseline_inputs);
    // The fit keeps the recorded instants (A4.3). No block straddles the step or its restoration, so the input change
    // stays exact (A4.5).
    let recorded: Vec<EvenSample> = samples.iter().map(even_sample).collect();
    // The limit is a nonzero constant, which the averaging always accepts.
    let fitted =
        block_averaged(&recorded, MAXIMUM_FIT_SAMPLES, &[step_time, hold_end]).unwrap_or(recorded);
    let times: Vec<f64> = fitted.iter().map(|sample| sample.time_seconds).collect();
    let sample_time = if fitted.len() == samples.len() {
        raw_sample_time
    } else {
        median_spacing(&times)
    };
    let hold_inputs: Vec<f64> = fitted
        .iter()
        .filter(|sample| sample.time_seconds >= step_time && sample.time_seconds < hold_end)
        .map(|sample| sample.input)
        .collect();
    Ok(Prepared {
        inputs: fitted.iter().map(|sample| sample.input).collect(),
        outputs: fitted.iter().map(|sample| sample.output).collect(),
        times,
        step_time,
        hold_end,
        input_before,
        input_change: step_amplitude(input_before, &hold_inputs),
        sample_time,
    })
}

/// Whether there are at least four samples, each good, finite, neither clipped nor saturated, in the first's units.
fn complete_samples(samples: &[IdentificationSample]) -> bool {
    let Some(first) = samples.first() else {
        return false;
    };
    samples.len() >= 4
        && samples.iter().all(|sample| {
            sample.quality == SampleQuality::Good
                && sample.clipped != Some(true)
                && sample.saturated != Some(true)
                && sample.input_unit == first.input_unit
                && sample.output_unit == first.output_unit
                && sample.time_seconds.is_finite()
                && sample.input.is_finite()
                && sample.output.is_finite()
        })
}

/// The indices of the step (the first input change beyond `10⁻⁹`) and of its restoration (the first later sample off
/// the stepped level), when the input stays within `10⁻⁹` of its level before, during and after the hold.
fn step_and_restoration(samples: &[IdentificationSample]) -> Result<(usize, usize), Refusal> {
    let Some(step_index) = (1..samples.len())
        .find(|&index| (samples[index].input - samples[index - 1].input).abs() > EPSILON)
    else {
        return Err(Refusal::new(
            "Step-response identification requires one nonzero input step.",
        ));
    };
    let input_before = samples[step_index - 1].input;
    let input_after = samples[step_index].input;
    let Some(restoration_index) = (step_index + 1..samples.len())
        .find(|&index| (samples[index].input - input_after).abs() > EPSILON)
    else {
        return Err(Refusal::new(
            "Step-response recording must include one complete held step and restoration.",
        ));
    };
    let leaves = |range: &[IdentificationSample], level: f64| {
        range
            .iter()
            .any(|sample| (sample.input - level).abs() > EPSILON)
    };
    if leaves(&samples[..step_index], input_before)
        || leaves(&samples[step_index..restoration_index], input_after)
        || (samples[restoration_index].input - input_before).abs() > EPSILON
        || leaves(&samples[restoration_index..], input_before)
    {
        return Err(Refusal::new(
            "Step-response input contains ambiguous excitation before restoration.",
        ));
    }
    Ok((step_index, restoration_index))
}

/// A sample as the block averaging reads it.
fn even_sample(sample: &IdentificationSample) -> EvenSample {
    EvenSample {
        time_seconds: sample.time_seconds,
        input: sample.input,
        output: sample.output,
        quality: sample.quality,
        clipped: sample.clipped.unwrap_or(false),
        saturated: sample.saturated.unwrap_or(false),
    }
}

/// The output span of a prepared fit record (N1.1, N3.2): the largest less the smallest measured output from the start of
/// the steady baseline window to the end of the hold, the samples and span that the single-run and validated fits use
/// (B3.4). A flat record has no span, so it is refused before any error is divided by it.
///
/// # Errors
/// Refuses with the output-too-small sentence when the measured outputs do not change.
pub(super) fn record_span(
    prepared: &Prepared,
    settings: &StepResponseSettings,
) -> Result<f64, Refusal> {
    let baseline_start = prepared.step_time - settings.baseline_duration_seconds;
    let outputs: Vec<f64> = prepared
        .times
        .iter()
        .zip(&prepared.outputs)
        .filter(|(time, _)| **time >= baseline_start - EPSILON && **time < prepared.hold_end)
        .map(|(_, output)| *output)
        .collect();
    let span = largest(&outputs, f64::NEG_INFINITY) - smallest(&outputs);
    if span <= 0.0 {
        return Err(Refusal::new(OUTPUT_TOO_SMALL));
    }
    Ok(span)
}

/// The output span of the fit record of a recording (N3.2), which the bounded step review divides its normalized errors
/// by: the recording is prepared as the analysis prepares it, and [`record_span`] takes its span.
///
/// # Errors
/// Refuses as [`prepare`] does, and with the output-too-small sentence when the fit record is flat.
pub(crate) fn fit_record_span(
    samples: &[IdentificationSample],
    settings: &StepResponseSettings,
) -> Result<f64, Refusal> {
    let prepared = prepare(samples, settings)?;
    record_span(&prepared, settings)
}
