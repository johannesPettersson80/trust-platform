//! The steady baseline that a step is judged against (`IdentificationCampaign.baseline`; specification 35, B3.4 step 2). The
//! tracker is a pure state machine: the driver feeds it each page of the recording, and it answers that the output is steady,
//! that the wait goes on (with the progress text when one is due), or that the output is still changing after the budget.

#![allow(clippy::module_name_repetitions)] // The baseline's types name what the step is judged against.

use schemars::JsonSchema;
use serde::{Deserialize, Serialize};
use trust_control_math::sampling::statistics::{mean, noise_sigma, trend_slope, TimedValue};

use super::{
    baseline_budget_ms, format_rate, ms_as_f64, progress_due, FollowSample, Watch,
    RATE_WINDOW_SAMPLES,
};
use crate::failure::{Cause, Failure, NextAction};
use crate::identification::settle::{
    baseline_window_seconds, is_steady, median_spacing, trailing_window, Baseline,
    MINIMUM_WINDOW_SAMPLES,
};
use crate::loop_definition::LoopSignal;

/// The steady baseline (`BaselineSummary`; B3.4 step 2). A step is judged against its mean and its noise.
#[derive(Debug, Clone, Copy, PartialEq, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "camelCase")]
pub struct BaselineSummary {
    /// The mean of the steady baseline.
    pub mean: f64,
    /// The noise `σ` of the steady baseline (B3.4).
    pub noise: f64,
    /// Half the baseline's peak-to-peak range: the size of a relay's switching band and of the mode's response allowance.
    pub half_range: f64,
    /// The spacing of the baseline's samples, in seconds.
    pub spacing_seconds: f64,
    /// The length of the steady window, in seconds.
    pub window_seconds: f64,
}

impl BaselineSummary {
    /// The baseline as the settle rules read it: its mean, its noise and its sample spacing.
    #[must_use]
    pub fn settle_baseline(&self) -> Baseline {
        Baseline {
            mean: self.mean,
            noise: self.noise,
            spacing_seconds: self.spacing_seconds,
        }
    }
}

/// What a baseline step says (B3.4 step 2).
#[derive(Debug, Clone, PartialEq)]
pub enum BaselineStep {
    /// The output has been steady over one window: the baseline is known.
    Steady(BaselineSummary),
    /// The output is still moving and the wait goes on; `progress` is the text to report now, when one is due.
    Waiting {
        /// The progress text, when one is due (at most one every ten seconds of process time).
        progress: Option<String>,
    },
}

/// The baseline tracker: the good outputs seen so far at their process times, and the progress last reported.
#[derive(Debug, Clone, PartialEq)]
pub struct BaselineTracker {
    symbol: String,
    unit: String,
    range: f64,
    maximum_seconds: f64,
    budget_seconds: f64,
    nominal_seconds: f64,
    expected_settling_seconds: Option<f64>,
    offer_setpoint_step: bool,
    points: Vec<TimedValue>,
    reported_seconds: f64,
}

impl BaselineTracker {
    /// A tracker for `output`, the measured output the campaign watches (its symbol, unit and permitted range). The campaign's
    /// maximum duration is `maximum_ms`, the loop's task period is `task_period_ms`, and `expected_settling_seconds` is the
    /// settling time the study declares, when it does. `offer_setpoint_step` says whether a setpoint step is offered in the
    /// refusal, which a loop in Auto with a declared setpoint gets (B3.4 step 2).
    #[must_use]
    pub fn new(
        output: &LoopSignal,
        maximum_ms: u64,
        task_period_ms: f64,
        expected_settling_seconds: Option<f64>,
        offer_setpoint_step: bool,
    ) -> Self {
        Self {
            symbol: output.symbol.clone(),
            unit: output.unit.clone(),
            range: output.range.maximum - output.range.minimum,
            maximum_seconds: ms_as_f64(maximum_ms) / 1_000.0,
            budget_seconds: baseline_budget_ms(maximum_ms) / 1_000.0,
            nominal_seconds: task_period_ms / 1_000.0,
            expected_settling_seconds,
            offer_setpoint_step,
            points: Vec::new(),
            reported_seconds: f64::NEG_INFINITY,
        }
    }

    /// Feeds one page of the recording's samples, observed at wall time `wall_ms`, and answers what the baseline says now.
    ///
    /// # Errors
    /// The output is still changing after the baseline budget: the refusal of B3.4 step 2, which names the rate of change
    /// and, when a setpoint step is offered, says so.
    pub fn feed(
        &mut self,
        watch: &mut Watch,
        samples: &[FollowSample],
        wall_ms: f64,
    ) -> Result<BaselineStep, Failure> {
        let observed = watch.observe(samples, wall_ms);
        self.points.extend(observed);
        let spacing = median_spacing(
            last_points(&self.points, RATE_WINDOW_SAMPLES),
            self.nominal_seconds,
        );
        let window = baseline_window_seconds(
            self.maximum_seconds,
            spacing,
            self.expected_settling_seconds,
        );
        let recent = trailing_window(&self.points, window);
        let span = match (self.points.first(), self.points.last()) {
            (Some(first), Some(last)) => last.seconds - first.seconds,
            _ => 0.0,
        };
        if span >= window && recent.len() >= MINIMUM_WINDOW_SAMPLES {
            let values: Vec<f64> = recent.iter().map(|point| point.value).collect();
            let noise = noise_sigma(&values);
            if is_steady(recent, noise, self.range) {
                return Ok(BaselineStep::Steady(BaselineSummary {
                    mean: mean(&values),
                    noise,
                    half_range: half_range(&values),
                    spacing_seconds: spacing,
                    window_seconds: window,
                }));
            }
        }
        let rate_window = if recent.len() >= 2 {
            recent
        } else {
            &self.points
        };
        let rate = format_rate(trend_slope(rate_window), &self.unit);
        if span >= self.budget_seconds {
            let (offer, next_action) = if self.offer_setpoint_step {
                (
                    ", or identify with a setpoint step",
                    NextAction::ChooseMethod,
                )
            } else {
                ("", NextAction::Wait)
            };
            return Err(Failure::new(
                &format!(
                    "{} is still changing by {rate}. Wait for it to settle{offer}.",
                    self.symbol
                ),
                Cause::Settings,
                next_action,
            ));
        }
        let progress = if progress_due(watch.newest_seconds(), self.reported_seconds) {
            self.reported_seconds = watch.newest_seconds();
            Some(format!(
                "Waiting for {} to settle (changing {rate})…",
                self.symbol
            ))
        } else {
            None
        };
        Ok(BaselineStep::Waiting { progress })
    }
}

/// The last `count` points, in time order (`points.slice(-count)`).
fn last_points(points: &[TimedValue], count: usize) -> &[TimedValue] {
    &points[points.len().saturating_sub(count)..]
}

/// Half the range between the highest and the lowest value of a steady window (B3.4 step 2).
fn half_range(values: &[f64]) -> f64 {
    let high = values.iter().copied().fold(f64::NEG_INFINITY, f64::max);
    let low = values.iter().copied().fold(f64::INFINITY, f64::min);
    (high - low) / 2.0
}
