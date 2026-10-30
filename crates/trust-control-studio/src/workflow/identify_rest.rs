//! The rest of a loop that Identify finds in Auto, before a Manual method's mode change (specification 35, B3.4 step 1): the loop
//! is read again and again until its measured output and the output that the transfer will hold have both been steady over one
//! baseline window. Each step's decision is trust-control's (`RestTracker`, which refuses a loop still moving after the baseline's
//! budget); this module reads the runtime, measures process time from the runtime's own clock, shows the progress, stops on a
//! cancel, and writes nothing.

use trust_control::commissioning::apply_texts;
use trust_control::commissioning::commissioning_result::TransferStatus;
use trust_control::commissioning::texts;
use trust_control::commissioning::transfer::TransferValue;
use trust_control::failure::{Cause, Failure, NextAction};
use trust_control::identification::campaign::rest::{RestStep, RestTracker};
use trust_control::identification::campaign::{CANCELLED_TEXT, POLL_MILLISECONDS};
use trust_control::limits::CONNECTION_LOSS_TIMEOUT_MS;
use trust_control::loop_definition::LoopDefinition;
use trust_control::signals::ReadingClock;

use crate::workflow::identify_campaign::{CampaignClock, CampaignReport};

/// The wait for a loop found in Auto to rest: the tracker that decides, the measured output it reads, and the operation's report
/// and clock.
pub struct RestWait<'a> {
    tracker: RestTracker,
    measured: String,
    report: &'a dyn CampaignReport,
    clock: &'a dyn CampaignClock,
}

impl<'a> RestWait<'a> {
    /// The wait for `loop_definition`, with the limits of its campaign's steady baseline: the maximum duration `maximum_ms` and the
    /// study's `expected_settling_seconds`. `offer_setpoint_step` says whether a refusal offers a setpoint step (B3.4 step 2).
    #[must_use]
    pub fn new(
        loop_definition: &LoopDefinition,
        maximum_ms: u64,
        expected_settling_seconds: Option<f64>,
        offer_setpoint_step: bool,
        report: &'a dyn CampaignReport,
        clock: &'a dyn CampaignClock,
    ) -> Self {
        let signals = &loop_definition.signals;
        Self {
            tracker: RestTracker::new(
                &signals.process_variable,
                &signals.manipulated_variable,
                maximum_ms,
                loop_definition.controller.task_period_ms,
                expected_settling_seconds,
                offer_setpoint_step,
            ),
            measured: signals.process_variable.symbol.clone(),
            report,
            clock,
        }
    }

    /// Reads the loop with `read` until it rests, and returns the reading at rest, which the transfer then holds the output of.
    /// `held` is the output the transfer holds. Each progress is shown once, when it changes.
    ///
    /// # Errors
    /// The operation was cancelled (nothing was written); a reading fails, carries no runtime clock or no finite value of the
    /// measured output or of `held`; the loop still moves after the baseline's budget; or no newer runtime value arrived within
    /// the connection-loss timeout (A3.4).
    pub fn wait(
        &mut self,
        held: &str,
        read: &mut dyn FnMut() -> Result<TransferStatus, Failure>,
    ) -> Result<TransferStatus, Failure> {
        let mut clock = ReadingClock::new(whole_ms(self.clock.now_ms()));
        let mut shown: Option<String> = None;
        loop {
            if self.report.is_cancelled() {
                return Err(Failure::new(
                    CANCELLED_TEXT,
                    Cause::Unclassified,
                    NextAction::None,
                ));
            }
            let observed = read()?;
            let runtime_time_ns = observed
                .runtime_time_ns
                .ok_or_else(apply_texts::runtime_clock_missing)?;
            let process_ms = clock.observe(runtime_time_ns, whole_ms(self.clock.now_ms()));
            let measured = value_of(&observed, &self.measured)?;
            let output = value_of(&observed, held)?;
            match self.tracker.feed(process_ms / 1_000.0, measured, output)? {
                RestStep::Rested => return Ok(observed),
                RestStep::Waiting { progress } => {
                    if let Some(text) = progress
                        && shown.as_deref() != Some(text.as_str())
                    {
                        self.report.progress(&text);
                        shown = Some(text);
                    }
                }
            }
            clock.ensure_fresh(whole_ms(self.clock.now_ms()), CONNECTION_LOSS_TIMEOUT_MS)?;
            self.clock.wait(POLL_MILLISECONDS);
        }
    }
}

/// The finite number that `status` reports for `symbol`.
fn value_of(status: &TransferStatus, symbol: &str) -> Result<f64, Failure> {
    status
        .values
        .get(symbol)
        .copied()
        .and_then(TransferValue::as_number)
        .filter(|value| value.is_finite())
        .ok_or_else(|| {
            Failure::new(
                &texts::start_value_missing(symbol),
                Cause::Runtime,
                NextAction::None,
            )
        })
}

/// A wall time of the campaign's clock in whole milliseconds, as the reading clock takes it.
#[allow(clippy::cast_possible_truncation, clippy::cast_sign_loss)] // The clock counts up from zero and never nears 2⁶⁴ ms.
fn whole_ms(now_ms: f64) -> u64 {
    now_ms.max(0.0) as u64
}

#[cfg(test)]
#[path = "identify_rest_tests.rs"]
mod tests;
