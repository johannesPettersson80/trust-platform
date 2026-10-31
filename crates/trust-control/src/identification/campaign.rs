//! The campaign of an identification on a live runtime (`identificationCampaign.ts`; specification 35, A3.2, A3.4, A3.6, A7
//! and B3.4): the limits it runs within, the sentences it says, the watch that keeps process time, and the supervision of
//! a silent runtime. The baseline and the hold are the trackers in `baseline` and `hold`, and the rest of a loop found in
//! Auto before a Manual method's mode change is the tracker in `rest`; the record it journals before each dispatch is in
//! `journal`.
//!
//! Process time decides every phase; the wall clock only supervises and paces (A3.2). Each function takes the wall time
//! it needs as a number and reads no clock, so the driver in `trust-lsp` owns the clock and the runtime calls.

#![allow(clippy::module_name_repetitions)] // The campaign's names state their subject, as the specification does.

use trust_control_math::sampling::statistics::TimedValue;

use crate::experiment::number_text::{js_number, js_round, js_to_fixed, js_to_precision};
use crate::experiment::{experiment_recording_window_ms, RestorationState};
use crate::failure::{Cause, Failure, NextAction};
use crate::identification::acquisition_settings::{AcquisitionError, DURATION_FIELD};
use crate::identification::connected_run::recording_settling_ms;
use crate::identification::policy::project_scripted;
use crate::identification::project_baseline::takes_the_declared_baseline;
use crate::identification::settle::{capped_baseline_window_seconds, observed_time_scale};
use crate::limits::{
    connection_loss_timeout_ms, record_period_executions, MAXIMUM_EXPERIMENT_DURATION_MS,
    MINIMUM_EXPERIMENT_DURATION_MS, RECORDING_DURATION_LIMIT_MS, RECORDING_SAMPLE_LIMIT,
};
use crate::loop_definition::{LoopDefinition, Range};

pub mod baseline;
pub mod hold;
pub mod journal;
pub mod rest;

/// The interval between two polls of the runtime, in ms (`POLL_MILLISECONDS`).
pub const POLL_MILLISECONDS: u64 = 100;
/// A progress text is reported at most once per this many seconds of process time (`PROGRESS_INTERVAL_SECONDS`).
pub const PROGRESS_INTERVAL_SECONDS: f64 = 10.0;
/// The sample spacing is the median of at most this many of the newest samples (`RATE_WINDOW_SAMPLES`).
pub const RATE_WINDOW_SAMPLES: usize = 200;
/// The baseline may wait at least five minutes, in seconds (`BASELINE_BUDGET_MINIMUM_SECONDS`, B3.4 step 2).
pub const BASELINE_BUDGET_MINIMUM_SECONDS: f64 = 300.0;

/// The stop reasons of an experiment that reached its planned end: the runtime's duration end, or the early finish of
/// B3.4 (`NORMAL_STOP_REASONS`).
const NORMAL_STOP_REASONS: [&str; 3] = ["duration_reached", "duration_complete", "finished_early"];
/// The largest whole number JSON carries exactly, 2⁵³ − 1, which bounds a whole number of milliseconds (E0.3).
const MAXIMUM_WHOLE_MS: f64 = 9_007_199_254_740_991.0;
/// The sentence of a campaign that is cancelled between its steps.
pub const CANCELLED_TEXT: &str = "Identification cancelled.";
/// The sentence of a campaign whose runtime sends no new sample within the connection-loss timeout (A3.4, A7).
pub const SILENT_RUNTIME_TEXT: &str = "The runtime stopped sending samples.";

/// One sample of a followed page, as the campaign reads it: its process time, whether its quality is good, and the
/// measured output when the sample carries one (`rawSamples` of `RecordingFollowPage`).
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct FollowSample {
    /// The process time of the sample, in seconds from the recording's first sample (A3.2).
    pub elapsed_seconds: f64,
    /// Whether the sample's quality is good.
    pub good: bool,
    /// The measured output of the sample, when it has one.
    pub output: Option<f64>,
    /// The plant input of the sample, when it has one: in a closed-loop step it is part of the response (B3.4 step 4).
    pub input: Option<f64>,
}

/// The good, finite plant inputs of `samples` at their process times, as [`Watch::observe`] takes the outputs.
#[must_use]
pub fn good_inputs(samples: &[FollowSample]) -> Vec<TimedValue> {
    samples
        .iter()
        .filter(|sample| sample.good)
        .filter_map(|sample| {
            sample
                .input
                .filter(|input| input.is_finite())
                .map(|value| TimedValue {
                    seconds: sample.elapsed_seconds,
                    value,
                })
        })
        .collect()
}

/// The first sample a campaign saw, with the wall time it arrived at (A3.6).
#[derive(Debug, Clone, Copy, PartialEq)]
struct FirstSample {
    wall_ms: f64,
    seconds: f64,
}

/// What a campaign has seen of its recording, shared by its phases: the good outputs at their process times are returned to
/// the trackers, and the newest process time is the step's reference and the campaign's elapsed time (`observe`).
#[derive(Debug, Clone, Copy, PartialEq, Default)]
pub struct Watch {
    first: Option<FirstSample>,
    newest_seconds: f64,
}

impl Watch {
    /// A watch that has seen no sample.
    #[must_use]
    pub fn new() -> Self {
        Self::default()
    }

    /// Keeps the good, finite outputs of `samples` at their process times, and notes the newest sample time. The first
    /// sample ever seen fixes the wall time that the time scale is measured from (`observe`, A3.6).
    pub fn observe(&mut self, samples: &[FollowSample], wall_ms: f64) -> Vec<TimedValue> {
        let mut values = Vec::new();
        for sample in samples {
            if self.first.is_none() {
                self.first = Some(FirstSample {
                    wall_ms,
                    seconds: sample.elapsed_seconds,
                });
            }
            self.newest_seconds = self.newest_seconds.max(sample.elapsed_seconds);
            let good_output = sample
                .output
                .filter(|output| sample.good && output.is_finite());
            if let Some(output) = good_output {
                values.push(TimedValue {
                    seconds: sample.elapsed_seconds,
                    value: output,
                });
            }
        }
        values
    }

    /// The process time of the newest sample seen so far, in seconds.
    #[must_use]
    pub fn newest_seconds(&self) -> f64 {
        self.newest_seconds
    }

    /// The ratio of process time to wall time since the first sample, when it differs from 1 by more than 5 % (A3.6), and
    /// `None` otherwise (`timeScale`).
    #[must_use]
    pub fn time_scale(&self, wall_ms: f64) -> Option<f64> {
        let first = self.first?;
        observed_time_scale(self.newest_seconds - first.seconds, wall_ms - first.wall_ms)
            .filter(|scale| (scale - 1.0).abs() > 0.05)
    }
}

/// The wall-clock supervision of the followed pages: a page with no new sample for longer than the connection-loss timeout
/// stops the campaign (A3.4, A7). The clock only supervises; it never decides a phase (A3.2).
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Silence {
    last_sample_ms: f64,
    limit_ms: u64,
}

impl Silence {
    /// A supervision that starts at `wall_ms`, with the connection-loss timeout `limit_ms` (see [`silence_limit_ms`]).
    #[must_use]
    pub fn starting_at(wall_ms: f64, limit_ms: u64) -> Self {
        Self {
            last_sample_ms: wall_ms,
            limit_ms,
        }
    }

    /// Records one page of `sample_count` samples read at `wall_ms`.
    ///
    /// # Errors
    /// The page holds no sample, and no sample has come for longer than the connection-loss timeout.
    pub fn observe_page(&mut self, sample_count: usize, wall_ms: f64) -> Result<(), Failure> {
        if sample_count > 0 {
            self.last_sample_ms = wall_ms;
            return Ok(());
        }
        if wall_ms - self.last_sample_ms > ms_as_f64(self.limit_ms) {
            return Err(Failure::new(
                SILENT_RUNTIME_TEXT,
                Cause::Runtime,
                NextAction::ConnectRuntime,
            ));
        }
        Ok(())
    }
}

/// The maximum duration of a campaign in ms: the declared maximum, within the limits of A7 (`campaignMaximumMs`).
///
/// # Errors
/// The loop declares no positive duration, which is the limits-required refusal; or the duration is outside 1 s to 24 h.
pub fn campaign_maximum_ms(loop_definition: &LoopDefinition) -> Result<u64, Failure> {
    let seconds = loop_definition
        .experiment
        .as_ref()
        .and_then(|experiment| experiment.duration_seconds)
        .filter(|seconds| seconds.is_finite() && *seconds > 0.0);
    let Some(seconds) = seconds else {
        return Err(AcquisitionError::LimitsRequired.failure());
    };
    let milliseconds = (seconds * 1_000.0).round();
    let limits =
        ms_as_f64(MINIMUM_EXPERIMENT_DURATION_MS)..=ms_as_f64(MAXIMUM_EXPERIMENT_DURATION_MS);
    if !limits.contains(&milliseconds) {
        return Err(Failure::new(
            "The maximum experiment duration must be between 1 s and 24 h.",
            Cause::Settings,
            NextAction::OpenLimits,
        )
        .with_field(DURATION_FIELD));
    }
    Ok(to_whole_ms(milliseconds))
}

/// How long a campaign waits for a steady baseline, in ms: the longest of five minutes, a fifth of the maximum duration and two
/// baseline windows without their twenty-sample floor (`baselineBudgetMs`, B3.4 step 2). The window takes the maximum duration
/// and the declared settling time, as [`capped_baseline_window_seconds`] does.
#[must_use]
pub fn baseline_budget_ms(maximum_ms: u64, expected_settling_seconds: Option<f64>) -> f64 {
    let maximum_seconds = ms_as_f64(maximum_ms) / 1_000.0;
    let window_ms =
        1_000.0 * capped_baseline_window_seconds(maximum_seconds, expected_settling_seconds);
    (BASELINE_BUDGET_MINIMUM_SECONDS * 1_000.0)
        .max(0.2 * ms_as_f64(maximum_ms))
        .max(2.0 * window_ms)
}

/// The planned duration of a campaign's recording in ms: the baseline wait and then the maximum duration of the step, within
/// the recording limit (`campaignRecordingMs`, B3.4). The wait is [`baseline_budget_ms`] for the expected settling time, when
/// the study declares one. The record period follows from the duration (A4.2). The runtime takes whole milliseconds (E4.2), so
/// a fraction of a millisecond is rounded up and the recording still covers both parts.
///
/// A project-scripted experiment that declares a baseline waits for no steady baseline (C12), so its recording plans no wait: it
/// starts with the plan and covers the declared duration, whose last instant is the restoration sample. A project that declares no
/// baseline waits for the steady baseline as a campaign does ([`takes_the_declared_baseline`]), and its recording plans that wait. The runtime ends a recording that has run its planned
/// duration, before the sample at that instant, so the plan is the declared duration with the restoration margin of a Test recording
/// ([`experiment_recording_window_ms`]); the run stops the recording as soon as the experiment has ended.
///
/// # Errors
/// The maximum duration cannot be read (see [`campaign_maximum_ms`]).
pub fn campaign_recording_ms(
    loop_definition: &LoopDefinition,
    expected_settling_seconds: Option<f64>,
) -> Result<u64, Failure> {
    let maximum_ms = campaign_maximum_ms(loop_definition)?;
    if takes_the_declared_baseline(loop_definition) {
        return Ok(experiment_recording_window_ms(maximum_ms));
    }
    let budget_ms = baseline_budget_ms(maximum_ms, expected_settling_seconds);
    let planned = (budget_ms + ms_as_f64(maximum_ms))
        .min(ms_as_f64(RECORDING_DURATION_LIMIT_MS))
        .ceil();
    Ok(to_whole_ms(planned))
}

/// The expected settling time the recording of a loop's run sets its record period by, in whole ms (A4.2): the declared one, or a
/// tenth of the maximum duration ([`recording_settling_ms`]). A project-scripted experiment records every execution of its hold
/// (C12): the project owns its scenario and Identify steps no plant input for it, so its recording takes no settling time, and its
/// record period is the smallest that fits the sample limit.
#[must_use]
pub fn campaign_settling_ms(
    loop_definition: &LoopDefinition,
    maximum_ms: u64,
    expected_settling_seconds: Option<f64>,
) -> Option<u64> {
    if project_scripted(loop_definition).is_some() {
        None
    } else {
        recording_settling_ms(maximum_ms, expected_settling_seconds)
    }
}

/// The connection-loss timeout of a campaign in ms: five seconds, or three recorded sample intervals when those are longer
/// (`silenceLimitMs`, A3.4, A7). A sample interval is the loop's task period times the record period of the campaign's
/// recording (A4.2). That record period takes the same expected settling time as the recording's start ([`campaign_settling_ms`]):
/// the declared one, or a tenth of the maximum duration (B3.4 step 2), and none for a project-scripted experiment (C12). The
/// interval is in process time, which never runs slower than the wall clock.
///
/// # Errors
/// The loop's task period is not a whole number of milliseconds, or the campaign's recording cannot be planned.
pub fn silence_limit_ms(
    loop_definition: &LoopDefinition,
    expected_settling_seconds: Option<f64>,
) -> Result<u64, Failure> {
    let task_ms = task_period_whole_ms(loop_definition)?;
    let maximum_ms = campaign_maximum_ms(loop_definition)?;
    let recording_ms = campaign_recording_ms(loop_definition, expected_settling_seconds)?;
    let settling_ms = campaign_settling_ms(loop_definition, maximum_ms, expected_settling_seconds);
    let record_every =
        record_period_executions(recording_ms, task_ms, RECORDING_SAMPLE_LIMIT, settling_ms);
    Ok(connection_loss_timeout_ms(Some(task_ms * record_every)))
}

/// The loop's task period in whole ms.
///
/// # Errors
/// The task period is not a whole, non-negative number of milliseconds that the runtime can count exactly.
pub fn task_period_whole_ms(loop_definition: &LoopDefinition) -> Result<u64, Failure> {
    let period = loop_definition.controller.task_period_ms;
    let whole =
        period.is_finite() && period >= 0.0 && period.fract() == 0.0 && period <= MAXIMUM_WHOLE_MS;
    if !whole {
        return Err(Failure::invalid_request(&format!(
            "The loop's task period {} ms is not a whole number of milliseconds.",
            js_number(period)
        )));
    }
    Ok(to_whole_ms(period))
}

/// The plant input range the campaign runs within: the output limits, which must both be set and ordered (B3.4 steps 1 and 6).
/// A refusal names the limit that is missing, and Limits opens there (B3.9).
///
/// # Errors
/// An output limit is missing, or the two are not finite and ordered.
pub fn plant_input_range(loop_definition: &LoopDefinition) -> Result<Range, Failure> {
    let limits = &loop_definition.limits;
    let missing = match (limits.output_minimum, limits.output_maximum) {
        (Some(minimum), Some(maximum))
            if minimum.is_finite() && maximum.is_finite() && minimum < maximum =>
        {
            return Ok(Range { minimum, maximum });
        }
        (None, Some(_)) => "limits.outputMinimum",
        (Some(_), None) => "limits.outputMaximum",
        _ => "limits.outputMinimum and limits.outputMaximum",
    };
    Err(Failure::new(
        &format!(
            "The plant input limits are missing: {missing} must be set, with the minimum below the maximum."
        ),
        Cause::Settings,
        NextAction::OpenLimits,
    ))
}

/// The limits and the permitted range must be set before a campaign starts (B3.4 step 1). Each refusal names the field that
/// is missing, and Limits opens there (B3.9).
///
/// # Errors
/// The plant input limits are missing or not ordered, the maximum experiment duration is missing, or the output's permitted
/// range is missing or not ordered.
pub fn require_campaign_limits(loop_definition: &LoopDefinition) -> Result<(), Failure> {
    plant_input_range(loop_definition)?;
    let declared = loop_definition
        .experiment
        .as_ref()
        .and_then(|experiment| experiment.duration_seconds);
    if declared.is_none() {
        return Err(Failure::new(
            "The maximum experiment duration is missing: experiment.durationSeconds must be set, between 1 s and 24 h.",
            Cause::Settings,
            NextAction::OpenLimits,
        )
        .with_field(DURATION_FIELD));
    }
    let output = &loop_definition.signals.process_variable;
    let range = output.range;
    if !(range.minimum.is_finite() && range.maximum.is_finite() && range.minimum < range.maximum) {
        return Err(Failure::new(
            &format!(
                "The permitted range of {} is missing: signals.processVariable.range must be set.",
                output.symbol
            ),
            Cause::Settings,
            NextAction::OpenLimits,
        ));
    }
    Ok(())
}

/// Why a finished experiment does not qualify its recording, or `None` for a normal end (B3.4 step 6). `completed` says
/// whether the runtime reports the experiment as completed, and `stop_reason` is its stop reason, when it has one.
#[must_use]
pub fn experiment_stop_failure(completed: bool, stop_reason: Option<&str>) -> Option<String> {
    if completed && stop_reason.is_some_and(|reason| NORMAL_STOP_REASONS.contains(&reason)) {
        return None;
    }
    let reason = stop_reason.unwrap_or("unknown");
    Some(format!(
        "The experiment stopped before its hold ended: {}. The recording is kept and is not qualified for a fit.",
        stop_reason_text(reason)
    ))
}

fn stop_reason_text(reason: &str) -> String {
    match reason {
        "user_abort" => "it was aborted".to_string(),
        "mode_changed" => "the loop left the mode the experiment requires".to_string(),
        "physical_approval_required" => "physical approval was not given".to_string(),
        "stale_scan" => "runtime data went stale".to_string(),
        other => format!("stop reason {other}"),
    }
}

/// Whether a restoration is confirmed: only the runtime's restored or not-required state confirms it (`restorationConfirmed`).
#[must_use]
pub fn restoration_confirmed(restoration: RestorationState) -> bool {
    matches!(
        restoration,
        RestorationState::Restored | RestorationState::NotRequired
    )
}

/// A duration in plain words: `45 s`, `12 min`, `3 h 50 min` (`formatDuration`). The seconds are rounded to whole seconds,
/// and the minutes to whole minutes, as JavaScript's `Math.round` rounds them.
#[must_use]
pub fn format_duration(seconds: f64) -> String {
    if seconds < 60.0 {
        return format!("{} s", js_number(js_round(seconds)));
    }
    let minutes = js_round(seconds / 60.0);
    if minutes < 60.0 {
        return format!("{} min", js_number(minutes));
    }
    let hours = (minutes / 60.0).floor();
    let rest = minutes % 60.0;
    if rest == 0.0 {
        format!("{} h", js_number(hours))
    } else {
        format!("{} h {} min", js_number(hours), js_number(rest))
    }
}

/// A rate of change per minute with two significant figures and its unit: `0.6 degC/min` (`formatRate`). A rate is shown
/// without its sign.
#[must_use]
pub fn format_rate(per_second: f64, unit: &str) -> String {
    let per_minute = js_to_precision(per_second.abs() * 60.0, 2);
    let unit_text = if unit.is_empty() {
        String::new()
    } else {
        format!(" {unit}")
    };
    format!("{}{unit_text}/min", js_number(per_minute))
}

/// Whether a progress text is due: one is reported at most every [`PROGRESS_INTERVAL_SECONDS`] of process time.
#[must_use]
pub(crate) fn progress_due(now_seconds: f64, reported_seconds: f64) -> bool {
    now_seconds - reported_seconds >= PROGRESS_INTERVAL_SECONDS
}

/// The text of a time scale to one decimal place, as `toFixed(1)` prints it (`(time scale 2.0×)`).
#[must_use]
pub(crate) fn scale_text(scale: f64) -> String {
    format!("{:.1}", js_to_fixed(scale, 1))
}

/// A whole number of milliseconds as the float the limits are compared in. The limits lie far below 2⁵³, so the conversion is
/// exact.
#[allow(clippy::cast_precision_loss)] // Every limit and every whole duration here is far below 2⁵³.
pub(crate) fn ms_as_f64(value: u64) -> f64 {
    value as f64
}

/// A non-negative whole number of milliseconds, already checked by the caller to lie in the `u64` range.
#[allow(clippy::cast_possible_truncation, clippy::cast_sign_loss)] // The callers check the range first.
fn to_whole_ms(value: f64) -> u64 {
    value as u64
}

#[cfg(test)]
#[path = "campaign_tests.rs"]
mod tests;
