//! The admission rules of the step-response settings (`stepResponseSettingsProblem` and
//! `validStepResponseSettings`; N2.2, N2.4): the first problem of the settings, with the field the editor points at, and
//! whether the settings are admissible at all.

// The public checks keep the names of the `TypeScript` rules.
#![allow(clippy::module_name_repetitions)]

use trust_control_math::identify::step_response::grid_point_count;

use super::{SearchGrid, SettingsProblem, StepResponseSettings, EPSILON};

/// The most delay and time-constant combinations the settings admit, counted as the product of the two grids'
/// interval counts (`stepResponseSettingsProblem`).
const MAXIMUM_COMBINATIONS: f64 = 100_000.0;

/// The first problem of the analysis settings, with the field the editor points at (`stepResponseSettingsProblem`);
/// `None` when the settings are admissible. In order: the durations, window limits, measurement bound and lattice
/// steps are finite and above zero, the onset threshold finite and at least zero, the timestamp shift finite, and the
/// onset count from 1 to 100. Each grid, the delay grid first, has finite bounds of at least zero (the time constant's
/// minimum above zero), a step above zero that divides the span exactly, and a maximum not below its minimum. The
/// product of the two grids' interval counts is at most 100 000.
#[must_use]
pub fn step_response_settings_problem(settings: &StepResponseSettings) -> Option<SettingsProblem> {
    let scalars = [
        (
            "baselineDurationSeconds",
            settings.baseline_duration_seconds,
        ),
        ("finalDurationSeconds", settings.final_duration_seconds),
        ("maximumWindowRange", settings.maximum_window_range),
        (
            "maximumWindowSlopePerSecond",
            settings.maximum_window_slope_per_second,
        ),
        ("measurementBound", settings.measurement_bound),
        ("onsetThreshold", settings.onset_threshold),
        ("baselineStep", settings.baseline_step),
        ("gainStep", settings.gain_step),
    ];
    for (field, value) in scalars {
        // The onset threshold is the one number that may be zero.
        if let Some(message) = number_problem(value, field != "onsetThreshold") {
            return Some(problem(field, message));
        }
    }
    if !settings.timestamp_shift_seconds.is_finite() {
        return Some(problem(
            "timestampShiftSeconds",
            "Enter a finite time offset.",
        ));
    }
    if !(1..=100).contains(&settings.onset_samples) {
        return Some(problem(
            "onsetSamples",
            "Enter a whole number from 1 to 100.",
        ));
    }
    let mut combinations = 1.0;
    for (group, grid, minimum_positive) in [
        ("delay", &settings.delay, false),
        ("timeConstant", &settings.time_constant, true),
    ] {
        if let Some(found) = grid_problem(group, grid, minimum_positive) {
            return Some(found);
        }
        combinations *= grid_point_count(grid.minimum, grid.maximum, grid.step)
            .map_or(f64::INFINITY, count_f64);
    }
    if combinations > MAXIMUM_COMBINATIONS {
        return Some(problem(
            "delay.step",
            "Use a coarser grid or narrower delay and time-constant bounds.",
        ));
    }
    None
}

/// Whether the analysis settings are admissible (`validStepResponseSettings`): they have no
/// [`step_response_settings_problem`].
#[must_use]
pub fn valid_step_response_settings(settings: &StepResponseSettings) -> bool {
    step_response_settings_problem(settings).is_none()
}

/// The settings problem of one field.
fn problem(field: impl Into<String>, message: &str) -> SettingsProblem {
    SettingsProblem {
        field: field.into(),
        message: message.to_string(),
    }
}

/// What to enter for one number: a finite one, above zero when `positive_required` and otherwise at least zero.
fn number_problem(value: f64, positive_required: bool) -> Option<&'static str> {
    if !value.is_finite() {
        return Some("Enter a finite number.");
    }
    if positive_required {
        (value <= 0.0).then_some("Enter a value greater than zero.")
    } else {
        (value < 0.0).then_some("Enter zero or a greater value.")
    }
}

/// The first problem of one search grid, its fields named under `group`: finite bounds of at least zero (above zero
/// for the minimum when `minimum_positive`), a step above zero, a maximum at least as large as the minimum, and a step
/// that divides the span exactly.
fn grid_problem(group: &str, grid: &SearchGrid, minimum_positive: bool) -> Option<SettingsProblem> {
    for (field, value, positive_required) in [
        ("minimum", grid.minimum, minimum_positive),
        ("maximum", grid.maximum, false),
        ("step", grid.step, true),
    ] {
        if let Some(message) = number_problem(value, positive_required) {
            return Some(problem(format!("{group}.{field}"), message));
        }
    }
    if grid.maximum < grid.minimum {
        return Some(problem(
            format!("{group}.maximum"),
            "Enter a maximum at least as large as the minimum.",
        ));
    }
    let intervals = (grid.maximum - grid.minimum) / grid.step;
    if !intervals.is_finite() || (intervals - intervals.round()).abs() > EPSILON {
        return Some(problem(
            format!("{group}.step"),
            "Choose a grid step that divides the minimum-to-maximum span exactly.",
        ));
    }
    None
}

/// A count as a float; a grid interval count beyond `2^53` is far above every limit it is compared with.
#[allow(clippy::cast_precision_loss)]
fn count_f64(count: usize) -> f64 {
    count as f64
}
