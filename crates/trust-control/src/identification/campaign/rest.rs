//! The rest of a loop that Identify finds in Auto, before a Manual method's mode change (specification 35, B3.4 step 1). The
//! tracker is a pure state machine: the driver feeds it each reading of the measured output and of the plant input that the
//! transfer will hold, at the reading's process time, and it answers that the loop rests, that the wait goes on (with the
//! progress text when one is due), or that the loop still moves after the budget of the steady baseline (B3.4 step 2).
//!
//! A transfer that held an output the controller was still moving would let the plant settle open loop away from the
//! reference, and Apply would then wait for an operating point the controller never holds (B6.2).

use trust_control_math::sampling::statistics::{noise_sigma, trend_slope, TimedValue};

use super::{baseline_budget_ms, format_rate, ms_as_f64, progress_due, RATE_WINDOW_SAMPLES};
use crate::failure::{Cause, Failure, NextAction};
use crate::identification::settle::{
    baseline_window_seconds, is_steady, median_spacing, trailing_window, MINIMUM_WINDOW_SAMPLES,
};
use crate::loop_definition::LoopSignal;

/// What a rest step says (B3.4 step 1).
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum RestStep {
    /// The measured output and the plant input have both been steady over one baseline window: the transfer may hold the
    /// output.
    Rested,
    /// The loop still moves and the wait goes on; `progress` is the text to report now, when one is due.
    Waiting {
        /// The progress text, when one is due (at most one every ten seconds of process time).
        progress: Option<String>,
    },
}

/// One signal the rest watches: its name, its unit, its permitted range and its readings at their process times.
#[derive(Debug, Clone, PartialEq)]
struct Watched {
    symbol: String,
    unit: String,
    range: f64,
    points: Vec<TimedValue>,
}

impl Watched {
    fn new(signal: &LoopSignal) -> Self {
        Self {
            symbol: signal.symbol.clone(),
            unit: signal.unit.clone(),
            range: signal.range.maximum - signal.range.minimum,
            points: Vec::new(),
        }
    }

    /// Whether the readings within the trailing `window` are steady by the rule of B3.4 step 2.
    fn steady(&self, window: f64) -> bool {
        let recent = trailing_window(&self.points, window);
        let values: Vec<f64> = recent.iter().map(|point| point.value).collect();
        is_steady(recent, noise_sigma(&values), self.range)
    }

    /// The rate of change over the trailing `window`, as the progress and the refusal name it.
    fn rate(&self, window: f64) -> String {
        let recent = trailing_window(&self.points, window);
        let points = if recent.len() >= 2 {
            recent
        } else {
            &self.points
        };
        format_rate(trend_slope(points), &self.unit)
    }
}

/// The rest tracker: the measured output and the plant input seen so far, the limits of the steady baseline, and the
/// progress last reported.
#[derive(Debug, Clone, PartialEq)]
pub struct RestTracker {
    output: Watched,
    input: Watched,
    maximum_seconds: f64,
    budget_seconds: f64,
    nominal_seconds: f64,
    expected_settling_seconds: Option<f64>,
    offer_setpoint_step: bool,
    reported_seconds: f64,
}

impl RestTracker {
    /// A tracker for `output`, the measured output, and `input`, the plant input the transfer will hold. The campaign's
    /// maximum duration is `maximum_ms`, the loop's task period is `task_period_ms`, and `expected_settling_seconds` is the
    /// settling time the study declares, when it does: the window and the budget are those of the steady baseline (B3.4
    /// step 2). `offer_setpoint_step` says whether the refusal offers a setpoint step, as the baseline's does for a loop in
    /// Auto with a declared setpoint.
    #[must_use]
    pub fn new(
        output: &LoopSignal,
        input: &LoopSignal,
        maximum_ms: u64,
        task_period_ms: f64,
        expected_settling_seconds: Option<f64>,
        offer_setpoint_step: bool,
    ) -> Self {
        Self {
            output: Watched::new(output),
            input: Watched::new(input),
            maximum_seconds: ms_as_f64(maximum_ms) / 1_000.0,
            budget_seconds: baseline_budget_ms(maximum_ms, expected_settling_seconds) / 1_000.0,
            nominal_seconds: task_period_ms / 1_000.0,
            expected_settling_seconds,
            offer_setpoint_step,
            reported_seconds: f64::NEG_INFINITY,
        }
    }

    /// Feeds one reading at process time `seconds`: the measured output and the plant input. A reading that is not finite or
    /// is not later than the one before is left out.
    ///
    /// # Errors
    /// The loop still moves after the baseline budget: the refusal names the signal that moves and its rate and, when a
    /// setpoint step is offered, says so (B3.4 step 2).
    pub fn feed(&mut self, seconds: f64, output: f64, input: f64) -> Result<RestStep, Failure> {
        let later = self
            .output
            .points
            .last()
            .is_none_or(|last| seconds > last.seconds);
        if seconds.is_finite() && output.is_finite() && input.is_finite() && later {
            self.output.points.push(TimedValue {
                seconds,
                value: output,
            });
            self.input.points.push(TimedValue {
                seconds,
                value: input,
            });
        }
        let points = &self.output.points;
        let recent_spacing = &points[points.len().saturating_sub(RATE_WINDOW_SAMPLES)..];
        let spacing = median_spacing(recent_spacing, self.nominal_seconds);
        let window = baseline_window_seconds(
            self.maximum_seconds,
            spacing,
            self.expected_settling_seconds,
        );
        let span = match (points.first(), points.last()) {
            (Some(first), Some(last)) => last.seconds - first.seconds,
            _ => 0.0,
        };
        let enough =
            span >= window && trailing_window(points, window).len() >= MINIMUM_WINDOW_SAMPLES;
        let output_steady = enough && self.output.steady(window);
        let input_steady = enough && self.input.steady(window);
        if output_steady && input_steady {
            return Ok(RestStep::Rested);
        }
        // The signal named is the measured output while it moves, and otherwise the plant input the controller still moves.
        let moving = if enough && output_steady {
            &self.input
        } else {
            &self.output
        };
        let rate = moving.rate(window);
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
                    "{} is still changing by {rate} in Auto. Wait for the loop to settle{offer}.",
                    moving.symbol
                ),
                Cause::Settings,
                next_action,
            ));
        }
        let newest = points.last().map_or(0.0, |last| last.seconds);
        let progress = if progress_due(newest, self.reported_seconds) {
            self.reported_seconds = newest;
            Some(format!(
                "Waiting for the loop to settle in Auto ({} changing {rate})…",
                moving.symbol
            ))
        } else {
            None
        };
        Ok(RestStep::Waiting { progress })
    }
}

#[cfg(test)]
#[path = "rest_tests.rs"]
mod tests;
