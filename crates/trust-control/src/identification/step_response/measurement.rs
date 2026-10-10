//! The measurement of a held step without a fit (`qualifyStepResponse`, `measureStepResponse` and `inspectWindows`;
//! N2.5, N2.6): the windows, the step and its hold, the sustained onset in the direction of the hold's last tenth, and
//! the 63 % level of the measured change.

use trust_control_math::identify::score::final_tenth_start;
use trust_control_math::identify::step_response::{
    onset_time, window_statistics, StepResponseWindow, WindowError,
};
use trust_control_math::sampling::statistics::mean;

use crate::identification::data::IdentificationSample;
use crate::identification::refusal::Refusal;

use super::preparation::{prepare, Prepared};
use super::{
    valid_step_response_settings, StepResponseMeasurement, StepResponseSettings, EPSILON,
    INVALID_SETTINGS,
};

/// The warning of a timestamp shift: a sensitivity study, not nominal model evidence (N2.4).
const TIMESTAMP_SHIFT_WARNING: &str =
    "Timestamp-shift tolerance is retained for sensitivity analysis and is not nominal model evidence.";

/// The baseline window, which ends at the step, and the final window, which ends at the restoration.
pub(super) struct MeasuredWindows {
    pub(super) baseline: StepResponseWindow,
    pub(super) final_window: StepResponseWindow,
}

/// A prepared recording with its windows and its hold, the fit's samples from the step up to the restoration: their
/// indices, times and measured outputs.
pub(super) struct MeasuredStep {
    pub(super) prepared: Prepared,
    pub(super) windows: MeasuredWindows,
    pub(super) hold: Vec<usize>,
    pub(super) hold_times: Vec<f64>,
    pub(super) hold_outputs: Vec<f64>,
}

/// The two windows of a held step, which the refusals name.
#[derive(Debug, Clone, Copy)]
enum WindowLabel {
    Baseline,
    Final,
}

impl WindowLabel {
    /// The window's name at the start of a sentence.
    fn name(self) -> &'static str {
        match self {
            Self::Baseline => "Baseline",
            Self::Final => "Final",
        }
    }

    /// The window's name inside a sentence.
    fn word(self) -> &'static str {
        match self {
            Self::Baseline => "baseline",
            Self::Final => "final",
        }
    }
}

/// Qualifies a recording for a bounded step analysis (`qualifyStepResponse`; N2.6, B3.4): the samples are prepared
/// and both windows measured as the analysis will measure them, without a fit.
///
/// # Errors
/// Returns the first refusal, in this order: invalid settings; fewer than four samples, or one not good, not finite,
/// clipped, saturated or in other units than the first; times not strictly increasing; a gap (named); a timestamp
/// shift beyond one sample time; no step, no restoration, or another input change; a final window starting before
/// the step; a window with too few samples or without coverage; and a baseline outside the steadiness limits.
pub fn qualify_step_response(
    samples: &[IdentificationSample],
    settings: &StepResponseSettings,
) -> Result<(), Refusal> {
    let prepared = prepare(samples, settings)?;
    inspect_windows(&prepared, settings)?;
    Ok(())
}

/// Measures a held step without fitting it (`measureStepResponse`; N2.5, N2.6): the windows, the step and its hold,
/// the sustained onset in the direction of the mean of the hold's last 10 % from the baseline mean, and the 63 % level
/// `y0 + 0.632·(ȳ_final − y0)`. A timestamp shift is reported as a sensitivity study, not nominal evidence (N2.4).
///
/// # Errors
/// Refuses as [`qualify_step_response`] does.
pub fn measure_step_response(
    samples: &[IdentificationSample],
    settings: &StepResponseSettings,
) -> Result<StepResponseMeasurement, Refusal> {
    if !valid_step_response_settings(settings) {
        return Err(Refusal::new(INVALID_SETTINGS));
    }
    let step = measured_step(samples, settings)?;
    let baseline_mean = step.windows.baseline.mean;
    let change = step.windows.final_window.mean - baseline_mean;
    let direction = onset_direction(&step.hold_outputs, baseline_mean);
    let (onset_threshold, detectable_onset_seconds) = onset(&step, settings, direction);
    Ok(StepResponseMeasurement {
        settings: settings.clone(),
        input_change: step.prepared.input_change,
        step_time_seconds: step.prepared.step_time,
        hold_end_seconds: step.prepared.hold_end,
        sample_time_seconds: step.prepared.sample_time,
        detectable_onset_seconds,
        onset_threshold,
        level63: baseline_mean + 0.632 * change,
        warnings: timestamp_shift_warnings(settings),
        baseline: step.windows.baseline,
        final_window: step.windows.final_window,
    })
}

/// Measures the windows of a prepared recording (`inspectWindows`; N2.4, N2.6). The final window lies inside the hold.
/// Only the baseline must be steady; the final window is a reported check, so a partial response passes (A10.3).
fn inspect_windows(
    prepared: &Prepared,
    settings: &StepResponseSettings,
) -> Result<MeasuredWindows, Refusal> {
    let baseline_start = prepared.step_time - settings.baseline_duration_seconds;
    let final_start = prepared.hold_end - settings.final_duration_seconds;
    if final_start < prepared.step_time - EPSILON {
        return Err(Refusal::new(
            "Final window must lie within the held step before restoration. Shorten the final window.",
        ));
    }
    let baseline = window(
        prepared,
        baseline_start,
        prepared.step_time,
        WindowLabel::Baseline,
    )?;
    let final_window = window(prepared, final_start, prepared.hold_end, WindowLabel::Final)?;
    if baseline.range > settings.maximum_window_range + EPSILON
        || baseline.slope_per_second.abs() > settings.maximum_window_slope_per_second + EPSILON
    {
        return Err(Refusal::new(
            "Baseline window is not steady within the declared range and slope limits.",
        ));
    }
    Ok(MeasuredWindows {
        baseline,
        final_window,
    })
}

/// The statistics of one window of a prepared recording, with its refusals worded for the window (`window`; N2.6): a
/// window error tells too few samples from missing coverage.
fn window(
    prepared: &Prepared,
    start_seconds: f64,
    end_seconds: f64,
    label: WindowLabel,
) -> Result<StepResponseWindow, Refusal> {
    window_statistics(
        &prepared.times,
        &prepared.outputs,
        start_seconds,
        end_seconds,
        prepared.sample_time,
    )
    .map_err(|error| {
        let name = label.name();
        Refusal(match error {
            WindowError::TooFewSamples => format!(
                "{name} window lacks the required time coverage. Include at least two recorded samples."
            ),
            WindowError::Coverage => {
                let word = label.word();
                format!(
                    "{name} window lacks the required time coverage. Shorten the {word} window or import a longer complete recording."
                )
            }
        })
    })
}

/// Prepares a recording, measures its windows and collects its hold: the samples from the step up to the restoration.
pub(super) fn measured_step(
    samples: &[IdentificationSample],
    settings: &StepResponseSettings,
) -> Result<MeasuredStep, Refusal> {
    let prepared = prepare(samples, settings)?;
    let windows = inspect_windows(&prepared, settings)?;
    let interval = prepared.step_time..prepared.hold_end;
    let hold: Vec<usize> = prepared
        .times
        .iter()
        .enumerate()
        .filter(|&(_, time)| interval.contains(time))
        .map(|(index, _)| index)
        .collect();
    let hold_times = hold.iter().map(|&index| prepared.times[index]).collect();
    let hold_outputs = hold.iter().map(|&index| prepared.outputs[index]).collect();
    Ok(MeasuredStep {
        prepared,
        windows,
        hold,
        hold_times,
        hold_outputs,
    })
}

/// The direction of the onset (N2.5): the sign of the change from the baseline mean `y0` to the mean `yf` of the last 10 %
/// of the hold samples, at least three of them. It does not depend on the sign of the input step, and it is zero when the
/// hold does not change from the baseline.
pub(super) fn onset_direction(hold_outputs: &[f64], baseline_mean: f64) -> f64 {
    js_sign(mean(&hold_outputs[final_tenth_start(hold_outputs.len())..]) - baseline_mean)
}

/// The onset threshold `y0 ± h` and the detectable onset after the step (N2.5): the first run of the declared number of
/// held samples strictly beyond the threshold in `direction`. A zero direction puts it above `y0` and finds no onset.
pub(super) fn onset(
    step: &MeasuredStep,
    settings: &StepResponseSettings,
    direction: f64,
) -> (f64, Option<f64>) {
    let side = if direction < 0.0 { -1.0 } else { 1.0 };
    let threshold = step.windows.baseline.mean + side * settings.onset_threshold;
    let detectable = onset_time(
        &step.hold_times,
        &step.hold_outputs,
        threshold,
        settings.onset_samples,
        direction,
    )
    .map(|time| time - step.prepared.step_time);
    (threshold, detectable)
}

/// The timestamp-shift warning when the settings shift the timestamps (N2.4).
pub(super) fn timestamp_shift_warnings(settings: &StepResponseSettings) -> Vec<String> {
    if settings.timestamp_shift_seconds.abs() > 0.0 {
        vec![TIMESTAMP_SHIFT_WARNING.to_string()]
    } else {
        Vec::new()
    }
}

/// The direction of a change (`Math.sign`): one when it rises, minus one when it falls, and zero when it does not
/// change or is not a number, which no onset run satisfies either.
fn js_sign(value: f64) -> f64 {
    if value > 0.0 {
        1.0
    } else if value < 0.0 {
        -1.0
    } else {
        0.0
    }
}
