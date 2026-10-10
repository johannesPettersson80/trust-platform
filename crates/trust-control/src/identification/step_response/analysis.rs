//! The bounded fit of a held step (`analyzeStepResponse`; N2.2 to N2.6, N3.2 and A10.3): the grid search of the
//! first-order form with delay, the acceptance rules of the fitted held step, the measurement-consistent set and the
//! assembled analysis.

use trust_control_math::identify::response::reached_fraction;
use trust_control_math::identify::step_response::{
    best_first_order_fit, consistent_parameter_set, gain_bounds, lattice, normalized_rms,
    ConsistencySearch, ConsistentSet, FirstOrderFit, HeldStep, StepResponseWindow,
};
use trust_control_math::identify::ParameterInterval;
use trust_control_math::sampling::statistics::baseline_noise_rms;

use crate::experiment::number_text::{js_number, js_to_precision};
use crate::identification::data::IdentificationSample;
use crate::identification::refusal::Refusal;
use crate::identification::texts::{
    noise_refusal, partial_response_refusal, ExcitationWord, MINIMUM_REACHED_FRACTION,
};

use super::measurement::{
    measured_step, onset, onset_direction, timestamp_shift_warnings, MeasuredStep,
};
use super::preparation::record_span;
use super::{
    valid_step_response_settings, SearchGrid, StepBestFit, StepResponseAnalysis, StepResponsePoint,
    StepResponseSettings, EPSILON, INVALID_SETTINGS,
};

/// The refusal of a grid too large to search: a lattice beyond its point limit, or a fit beyond its evaluation limit.
const GRID_TOO_LARGE: &str =
    "Step-response identification grid is too large; refine the requested bounds or spacing.";

/// The warning of a best fit on a search boundary; search-boundary findings are retained (N2.4).
const BOUNDARY_WARNING: &str =
    "The best grid fit is on a search boundary; inspect or extend the justified range.";

/// The warning of an empty measurement-consistent set, which is reported explicitly (N2.4).
const NO_CONSISTENT_SET_WARNING: &str =
    "No consistent parameter combination satisfies the declared measurement bound across the held response.";

/// Fits the first-order-with-delay candidates of the settings' grids to a held step (`analyzeStepResponse`; N2.2 to
/// N2.6, N3.2, A10.3).
///
/// Each candidate is simulated on the recorded sample times with its least-squares gain from the steady baseline
/// (N2.2); the smallest squared error wins, the first of equal errors. The fit is accepted only when the response is
/// large against the baseline noise (N2.6), its normalized error is within `maximum_normalized_error` over the whole
/// hold and its final 10 % (N3.2), and it has reached 63 % of its fitted final change (A10.3). The consistent set keeps
/// every grid combination within the measurement bound of each held sample (N2.4); its gains span what the windows
/// allow when the final window is steady, which then also bounds the steady value, and otherwise twice the bound over
/// `|Δu|` around the fitted gain. The onset direction is that of the mean of the hold's last 10 % from the baseline mean
/// (N2.5), not that of the fitted final change.
///
/// # Errors
/// Refuses as [`measure_step_response`](super::measure_step_response) does, then: an input change of `10⁻⁹` or less; a grid too large to search; no
/// finite gain; a fitted final change of `10⁻⁹` or less; a response below twenty times the baseline noise; a
/// normalized error above the limit; and a response below 63 % of its fitted final change, with the suggested hold.
pub fn analyze_step_response(
    samples: &[IdentificationSample],
    settings: &StepResponseSettings,
    maximum_normalized_error: f64,
) -> Result<StepResponseAnalysis, Refusal> {
    if !valid_step_response_settings(settings) {
        return Err(Refusal::new(INVALID_SETTINGS));
    }
    let step = measured_step(samples, settings)?;
    let input_change = step.prepared.input_change;
    if input_change.abs() <= EPSILON {
        return Err(Refusal::new(
            "Step-response identification requires a nonzero measured input change.",
        ));
    }
    let deviations: Vec<f64> = step
        .prepared
        .inputs
        .iter()
        .map(|input| input - step.prepared.input_before)
        .collect();
    let held = HeldStep {
        times: &step.prepared.times,
        deviations: &deviations,
        hold: &step.hold,
        hold_outputs: &step.hold_outputs,
        baseline: step.windows.baseline.mean,
    };
    let delays = grid_values(&settings.delay)?;
    let time_constants = grid_values(&settings.time_constant)?;
    let best = best_first_order_fit(
        &held,
        &delays,
        &time_constants,
        settings.timestamp_shift_seconds,
    )
    .map_err(|_| Refusal::new(GRID_TOO_LARGE))?
    .ok_or_else(|| Refusal::new("Step-response identification gain is not finite."))?;
    let residuals = hold_residuals(&step, &best);
    let fraction = check_fit(
        samples,
        settings,
        &step,
        &best,
        &residuals,
        maximum_normalized_error,
    )?;
    // The uncertainty grid keeps the final window as a constraint only where that window is steady (N2.4).
    let steady = final_steady(&step.windows.final_window, settings);
    let gain_interval = if steady {
        gain_bounds(
            &step.windows.baseline,
            &step.windows.final_window,
            input_change,
            settings.measurement_bound,
        )
    } else {
        let spread = 2.0 * settings.measurement_bound / input_change.abs();
        ParameterInterval {
            minimum: best.gain - spread,
            maximum: best.gain + spread,
        }
    };
    let gains = lattice(
        gain_interval.minimum,
        gain_interval.maximum,
        settings.gain_step,
    )
    .map_err(|_| Refusal::new(GRID_TOO_LARGE))?;
    let consistency = consistent_parameter_set(&ConsistencySearch {
        held,
        timestamp_shift_seconds: settings.timestamp_shift_seconds,
        input_change,
        measurement_bound: settings.measurement_bound,
        baseline_step: settings.baseline_step,
        gains: &gains,
        delays: &delays,
        time_constants: &time_constants,
        final_target: steady.then_some(step.windows.final_window.mean),
    });
    Ok(finish_fit(
        step,
        settings,
        &best,
        &residuals,
        fraction,
        consistency,
    ))
}

/// The residuals `y_k − (y0 + K·s_k)` of the best fit at the held samples.
fn hold_residuals(step: &MeasuredStep, best: &FirstOrderFit) -> Vec<f64> {
    let baseline = step.windows.baseline.mean;
    step.hold
        .iter()
        .map(|&index| step.prepared.outputs[index] - (baseline + best.gain * best.response[index]))
        .collect()
}

/// The acceptance rules of a fitted held step, in their order, and the fraction of the fitted final change reached.
///
/// The fitted final change `K·Δu` exceeds `10⁻⁹` and twenty times the RMS noise of the recorded baseline outputs
/// (N2.6); the normalized error over the hold and over its final `max(3, ⌈0.1·N⌉)` samples, against the span of the
/// fit record (N3.2), is within the limit; and the response has reached 63 % of its fitted final change (A10.3).
fn check_fit(
    samples: &[IdentificationSample],
    settings: &StepResponseSettings,
    step: &MeasuredStep,
    best: &FirstOrderFit,
    residuals: &[f64],
    maximum_normalized_error: f64,
) -> Result<f64, Refusal> {
    let baseline = step.windows.baseline.mean;
    let final_change = best.gain * step.prepared.input_change;
    if final_change.is_nan() || final_change.abs() <= EPSILON {
        return Err(Refusal::new(
            "The recorded step has no measurable final change.",
        ));
    }
    // The response must be large against the baseline noise before its shape is judged (N2.6).
    let unit = samples
        .first()
        .and_then(|sample| sample.output_unit.as_deref())
        .unwrap_or("");
    let baseline_times = (step.prepared.step_time - settings.baseline_duration_seconds - EPSILON)
        ..(step.prepared.step_time - EPSILON);
    let baseline_outputs: Vec<f64> = samples
        .iter()
        .filter(|sample| baseline_times.contains(&sample.time_seconds))
        .map(|sample| sample.output)
        .collect();
    let noise = baseline_noise_rms(&baseline_outputs);
    if let Some(sentence) = noise_refusal(final_change.abs(), noise, unit, ExcitationWord::Step) {
        return Err(Refusal(sentence));
    }
    // The normalized errors divide by the span of the fit record (N1.1, N3.2), the same span that the bounded review
    // divides its normalized errors by. A flat record has no span, so it is refused before any division.
    let span = record_span(&step.prepared, settings)?;
    // The normalized error over the whole held response and over its final 10 % must each meet the limit (N3.2).
    let whole_error = normalized_rms(residuals, span);
    let final_count = ceil_share(residuals.len(), 0.1).max(3);
    let final_error = normalized_rms(
        &residuals[residuals.len().saturating_sub(final_count)..],
        span,
    );
    let within_limit =
        whole_error <= maximum_normalized_error && final_error <= maximum_normalized_error;
    if !within_limit {
        let error = engineering_text(js_max(whole_error, final_error));
        let limit = engineering_text(maximum_normalized_error);
        return Err(Refusal(format!(
            "The first-order step fit has normalized error {error} against the limit {limit}. Choose another method, a larger step or a longer duration."
        )));
    }
    // A held step is accepted only once it has reached 63 % of its fitted final change (A10.3).
    let fraction = reached_fraction(&step.hold_outputs, baseline, final_change);
    if fraction.is_nan() || fraction < MINIMUM_REACHED_FRACTION {
        return Err(Refusal(partial_response_refusal(
            fraction,
            step.prepared.hold_end - step.prepared.step_time,
            best.delay_seconds,
            best.time_constant_seconds,
        )));
    }
    Ok(fraction)
}

/// The analysis of an accepted fit: its warnings, onset, fitted points and checks.
fn finish_fit(
    step: MeasuredStep,
    settings: &StepResponseSettings,
    best: &FirstOrderFit,
    residuals: &[f64],
    fraction: f64,
    consistency: ConsistentSet,
) -> StepResponseAnalysis {
    let baseline = step.windows.baseline.mean;
    let final_change = best.gain * step.prepared.input_change;
    let boundary_limited = on_search_boundary(best, settings);
    let mut warnings = Vec::new();
    if boundary_limited {
        warnings.push(BOUNDARY_WARNING.to_string());
    }
    if consistency.count == 0 {
        warnings.push(NO_CONSISTENT_SET_WARNING.to_string());
    }
    warnings.extend(timestamp_shift_warnings(settings));
    let direction = onset_direction(&step.hold_outputs, baseline);
    let (onset_threshold, detectable_onset_seconds) = onset(&step, settings, direction);
    let points = step
        .hold
        .iter()
        .zip(residuals)
        .map(|(&index, &residual)| StepResponsePoint {
            time_seconds: step.prepared.times[index],
            measured: step.prepared.outputs[index],
            predicted: baseline + best.gain * best.response[index],
            residual,
        })
        .collect();
    let final_window_difference = step.windows.final_window.mean - (baseline + final_change);
    StepResponseAnalysis {
        settings: settings.clone(),
        input_change: step.prepared.input_change,
        step_time_seconds: step.prepared.step_time,
        hold_end_seconds: step.prepared.hold_end,
        sample_time_seconds: step.prepared.sample_time,
        detectable_onset_seconds,
        onset_threshold,
        level63: baseline + 0.632 * final_change,
        best: StepBestFit {
            baseline,
            gain: best.gain,
            delay_seconds: best.delay_seconds,
            time_constant_seconds: best.time_constant_seconds,
            squared_error: best.squared_error,
        },
        consistency,
        boundary_limited,
        warnings,
        points,
        final_change_fraction: Some(fraction),
        final_window_difference: Some(final_window_difference),
        baseline: step.windows.baseline,
        final_window: step.windows.final_window,
    }
}

/// Whether the final window is steady within the range and slope limits (`finalSteady`; N2.4).
fn final_steady(final_window: &StepResponseWindow, settings: &StepResponseSettings) -> bool {
    final_window.range <= settings.maximum_window_range + EPSILON
        && final_window.slope_per_second.abs() <= settings.maximum_window_slope_per_second + EPSILON
}

/// Whether the best fit lies on a search boundary (N2.4): the delay at its positive minimum or at its positive maximum, or
/// the time constant at its positive minimum or at its maximum. Every fit here is first order with delay, so its delay is
/// always searched on the settings' grid (N2.2). A best delay of zero is one-sided, because a delay is never negative: it
/// is kept with the result and is no search boundary, so it refuses nothing.
#[allow(clippy::float_cmp)] // Grid values are compared with the declared bounds exactly.
pub(super) fn on_search_boundary(best: &FirstOrderFit, settings: &StepResponseSettings) -> bool {
    let delay = &settings.delay;
    let time_constant = &settings.time_constant;
    (best.delay_seconds == delay.minimum && delay.minimum > 0.0)
        || (best.delay_seconds == delay.maximum && delay.maximum > 0.0)
        || (best.time_constant_seconds == time_constant.minimum && time_constant.minimum > 0.0)
        || best.time_constant_seconds == time_constant.maximum
}

/// The values of a search grid (`gridValues`).
fn grid_values(grid: &SearchGrid) -> Result<Vec<f64>, Refusal> {
    lattice(grid.minimum, grid.maximum, grid.step).map_err(|_| Refusal::new(GRID_TOO_LARGE))
}

/// The larger of two values as `Math.max` gives it: `NaN` when either is `NaN`.
fn js_max(left: f64, right: f64) -> f64 {
    if left.is_nan() || right.is_nan() {
        f64::NAN
    } else {
        left.max(right)
    }
}

/// A number of the step sentences to three significant digits (`engineeringText`:
/// `Number(value.toPrecision(3)).toString()`).
fn engineering_text(value: f64) -> String {
    js_number(js_to_precision(value, 3))
}

/// `Math.ceil(count · share)` in floating point, as the `TypeScript` computes it: a sample count.
#[allow(
    clippy::cast_possible_truncation,
    clippy::cast_sign_loss,
    clippy::cast_precision_loss
)]
fn ceil_share(count: usize, share: f64) -> usize {
    (count as f64 * share).ceil() as usize
}
