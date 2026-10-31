//! The rest of a loop that Identify finds in Auto, before a Manual method's mode change (specification 35, B3.4 step 1). The
//! tracker is a pure state machine: the driver feeds it each reading of the measured outputs and of the plant inputs that the
//! transfer will hold, at the reading's process time, and it answers that the loop rests, that the wait goes on (with the
//! progress text when one is due), or that the loop still moves after the budget of the steady baseline (B3.4 step 2). A PID
//! group's rest watches every member's process value and every output its transfer holds (C7).
//!
//! A transfer that held an output the controller was still moving would let the plant settle open loop away from the
//! reference, and Apply would then wait for an operating point the controller never holds (B6.2).

use std::iter;

use trust_control_math::sampling::statistics::{noise_sigma, trend_slope, TimedValue};

use super::{baseline_budget_ms, format_rate, ms_as_f64, progress_due, RATE_WINDOW_SAMPLES};
use crate::commissioning::declared_initialization::further_channels;
use crate::commissioning::transfer::{TransferBinding, TransferMember};
use crate::failure::{Cause, Failure, NextAction};
use crate::identification::settle::{
    baseline_window_seconds, capped_baseline_window_seconds, is_steady, median_spacing,
    trailing_window, MINIMUM_WINDOW_SAMPLES,
};
use crate::loop_definition::{LoopDefinition, LoopSignal};

/// What a rest step says (B3.4 step 1).
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum RestStep {
    /// Every measured output and every plant input the transfer holds has been steady over one baseline window: the transfer
    /// may hold the outputs.
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
    fn new(symbol: &str, unit: &str, minimum: f64, maximum: f64) -> Self {
        Self {
            symbol: symbol.to_owned(),
            unit: unit.to_owned(),
            range: maximum - minimum,
            points: Vec::new(),
        }
    }

    /// A signal of the loop, in its declared range.
    fn of_signal(signal: &LoopSignal) -> Self {
        Self::new(
            &signal.symbol,
            &signal.unit,
            signal.range.minimum,
            signal.range.maximum,
        )
    }

    /// A member of the transfer, in its bounds.
    fn of_member(member: &TransferMember) -> Self {
        Self::new(&member.symbol, &member.unit, member.minimum, member.maximum)
    }

    /// Whether the readings within the trailing `window` are steady by the rule of B3.4 step 2.
    fn steady(&self, window: f64) -> bool {
        let recent = trailing_window(&self.points, window);
        let values: Vec<f64> = recent.iter().map(|point| point.value).collect();
        is_steady(recent, noise_sigma(&values), self.range)
    }

    /// The rate of change over the trailing `window`, as the progress and the refusal name it; over the readings from `kept`
    /// on (`retained_start`) when the window holds fewer than two.
    fn rate(&self, window: f64, kept: usize) -> String {
        let recent = trailing_window(&self.points, window);
        let points = if recent.len() >= 2 {
            recent
        } else {
            &self.points[kept..]
        };
        format_rate(trend_slope(points), &self.unit)
    }
}

/// The rest tracker: the signals it watches with their readings so far, the limits of the steady baseline, and the progress
/// last reported.
#[derive(Debug, Clone, PartialEq)]
pub struct RestTracker {
    /// The measured outputs, the loop's own process variable first, then the plant inputs the transfer holds.
    signals: Vec<Watched>,
    /// The process time of the wait's first reading: the wait's span starts there, whichever readings are kept.
    first_seconds: Option<f64>,
    maximum_seconds: f64,
    capped_window_seconds: f64,
    budget_seconds: f64,
    nominal_seconds: f64,
    expected_settling_seconds: Option<f64>,
    offer_setpoint_step: bool,
    reported_seconds: f64,
}

impl RestTracker {
    /// A tracker for the loop of `loop_definition` before the mode transfer of `binding` (B3.4 step 1). It watches the loop's
    /// process variable and, for a PID group, every other member's process value (C7), and every output the transfer holds:
    /// `binding`'s output and its further outputs. The campaign's maximum duration is `maximum_ms`, and
    /// `expected_settling_seconds` is the settling time the study declares, when it does: the window and the budget are those of
    /// the steady baseline (B3.4 step 2). `offer_setpoint_step` says whether the refusal offers a setpoint step, as the
    /// baseline's does for a loop in Auto with a declared setpoint.
    ///
    /// # Errors
    /// A PID group's channels do not fit (see `further_channels`), or `binding` does not read a member's process value.
    pub fn for_transfer(
        loop_definition: &LoopDefinition,
        binding: &TransferBinding,
        maximum_ms: u64,
        expected_settling_seconds: Option<f64>,
        offer_setpoint_step: bool,
    ) -> Result<Self, Failure> {
        let process = &loop_definition.signals.process_variable;
        let mut signals = vec![Watched::of_signal(process)];
        if loop_definition.controller.group.is_some() {
            for symbol in further_channels(loop_definition)?.measurements {
                if symbol == process.symbol {
                    continue;
                }
                let member = binding
                    .guards
                    .iter()
                    .find(|member| member.symbol == symbol)
                    .ok_or_else(|| {
                        Failure::new(
                            &format!("The group's transfer does not read {symbol}, a member's process value."),
                            Cause::Implementation,
                            NextAction::OpenLoopSetup,
                        )
                    })?;
                signals.push(Watched::of_member(member));
            }
        }
        signals.extend(
            iter::once(&binding.output)
                .chain(&binding.additional_outputs)
                .map(Watched::of_member),
        );
        Ok(Self::new(
            signals,
            maximum_ms,
            loop_definition.controller.task_period_ms,
            expected_settling_seconds,
            offer_setpoint_step,
        ))
    }

    /// A tracker of `signals`, measured outputs first, with the loop's task period `task_period_ms` and the limits of
    /// [`Self::for_transfer`].
    fn new(
        signals: Vec<Watched>,
        maximum_ms: u64,
        task_period_ms: f64,
        expected_settling_seconds: Option<f64>,
        offer_setpoint_step: bool,
    ) -> Self {
        let maximum_seconds = ms_as_f64(maximum_ms) / 1_000.0;
        Self {
            signals,
            first_seconds: None,
            maximum_seconds,
            capped_window_seconds: capped_baseline_window_seconds(
                maximum_seconds,
                expected_settling_seconds,
            ),
            budget_seconds: baseline_budget_ms(maximum_ms, expected_settling_seconds) / 1_000.0,
            nominal_seconds: task_period_ms / 1_000.0,
            expected_settling_seconds,
            offer_setpoint_step,
            reported_seconds: f64::NEG_INFINITY,
        }
    }

    /// The symbols of the signals the tracker watches, in the order [`Self::feed`] takes their readings.
    pub fn symbols(&self) -> impl Iterator<Item = &str> {
        self.signals.iter().map(|signal| signal.symbol.as_str())
    }

    /// Forgets the readings that no later step can judge (`retained_start`) once at least [`RATE_WINDOW_SAMPLES`] of them
    /// have gathered, so that each reading is moved a bounded number of times. However long the wait, a signal keeps the
    /// readings within the capped window or the last [`RATE_WINDOW_SAMPLES`], whichever are more, and fewer than
    /// [`RATE_WINDOW_SAMPLES`] older ones (Codex's review of #147: a 24-hour campaign of a 16-member group kept 84 MiB).
    fn forget_old_readings(&mut self) {
        let start = self.signals.first().map_or(0, |lead| {
            retained_start(&lead.points, self.capped_window_seconds)
        });
        if start >= RATE_WINDOW_SAMPLES {
            for signal in &mut self.signals {
                signal.points.drain(..start);
            }
        }
    }

    /// Feeds one reading at process time `seconds`: `readings` holds the value of each of [`Self::symbols`], in that order. A
    /// reading with a value that is not finite, with a value too many or too few, or that is not later than the one before is
    /// left out.
    ///
    /// # Errors
    /// The loop still moves after the baseline budget: the refusal names the signal that moves and its rate and, when a
    /// setpoint step is offered, says so (B3.4 step 2).
    pub fn feed(&mut self, seconds: f64, readings: &[f64]) -> Result<RestStep, Failure> {
        let later = self
            .signals
            .first()
            .and_then(|lead| lead.points.last())
            .is_none_or(|last| seconds > last.seconds);
        if seconds.is_finite()
            && later
            && readings.len() == self.signals.len()
            && readings.iter().all(|value| value.is_finite())
        {
            for (signal, &value) in self.signals.iter_mut().zip(readings) {
                signal.points.push(TimedValue { seconds, value });
            }
            self.first_seconds.get_or_insert(seconds);
            self.forget_old_readings();
        }
        let Some(lead) = self.signals.first() else {
            return Ok(RestStep::Waiting { progress: None });
        };
        let points = &lead.points;
        let recent_spacing = &points[points.len().saturating_sub(RATE_WINDOW_SAMPLES)..];
        let spacing = median_spacing(recent_spacing, self.nominal_seconds);
        let window = baseline_window_seconds(
            self.maximum_seconds,
            spacing,
            self.expected_settling_seconds,
        );
        let span = match (self.first_seconds, points.last()) {
            (Some(first), Some(last)) => last.seconds - first,
            _ => 0.0,
        };
        let enough =
            span >= window && trailing_window(points, window).len() >= MINIMUM_WINDOW_SAMPLES;
        // The signal named is the first that still moves: a measured output before the plant inputs the controller still moves,
        // and the loop's own process variable until the window is full.
        let moving = if enough {
            match self.signals.iter().find(|signal| !signal.steady(window)) {
                Some(signal) => signal,
                None => return Ok(RestStep::Rested),
            }
        } else {
            lead
        };
        let rate = moving.rate(window, retained_start(points, self.capped_window_seconds));
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

/// The first of `points` that a later step can still judge: every reading within `capped_window_seconds` of the newest, and
/// the last [`RATE_WINDOW_SAMPLES`], whose median spacing sets the window. A window longer than its cap holds twenty readings
/// at that spacing, and they lie among the last [`RATE_WINDOW_SAMPLES`]: at least half of those readings' gaps are as long as
/// the median, so together they span a hundred such gaps or more.
fn retained_start(points: &[TimedValue], capped_window_seconds: f64) -> usize {
    let Some(newest) = points.last() else {
        return 0;
    };
    let within_cap =
        points.partition_point(|point| point.seconds < newest.seconds - capped_window_seconds);
    within_cap.min(points.len().saturating_sub(RATE_WINDOW_SAMPLES))
}

#[cfg(test)]
#[path = "rest_tests.rs"]
mod tests;
