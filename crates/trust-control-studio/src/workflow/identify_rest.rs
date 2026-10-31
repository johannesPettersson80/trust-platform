//! The rest of a loop that Identify finds in Auto, before a Manual method's mode change (specification 35, B3.4 step 1): the loop
//! is read again and again until its measured outputs and the outputs that the transfer will hold have all been steady over one
//! baseline window (for a PID group, every member's, C7). Each step's decision is trust-control's (`RestTracker`, which chooses the
//! signals and refuses a loop still moving after the baseline's budget); this module reads the runtime, measures process time from
//! the runtime's own clock, shows the progress, stops on a cancel, and writes nothing.

use trust_control::commissioning::apply_texts;
use trust_control::commissioning::commissioning_result::TransferStatus;
use trust_control::commissioning::texts;
use trust_control::commissioning::transfer::{TransferBinding, TransferValue};
use trust_control::failure::{Cause, Failure, NextAction};
use trust_control::identification::campaign::rest::{RestStep, RestTracker};
use trust_control::identification::campaign::{CANCELLED_TEXT, POLL_MILLISECONDS};
use trust_control::limits::CONNECTION_LOSS_TIMEOUT_MS;
use trust_control::loop_definition::LoopDefinition;
use trust_control::signals::ReadingClock;

use crate::workflow::identify_campaign::{CampaignClock, CampaignReport};

/// The wait for a loop found in Auto to rest: the loop, the limits of its campaign's steady baseline, and the operation's report and
/// clock.
pub struct RestWait<'a> {
    loop_definition: &'a LoopDefinition,
    maximum_ms: u64,
    expected_settling_seconds: Option<f64>,
    offer_setpoint_step: bool,
    report: &'a dyn CampaignReport,
    clock: &'a dyn CampaignClock,
}

impl<'a> RestWait<'a> {
    /// The wait for `loop_definition`, with the limits of its campaign's steady baseline: the maximum duration `maximum_ms` and the
    /// study's `expected_settling_seconds`. `offer_setpoint_step` says whether a refusal offers a setpoint step (B3.4 step 2).
    #[must_use]
    pub fn new(
        loop_definition: &'a LoopDefinition,
        maximum_ms: u64,
        expected_settling_seconds: Option<f64>,
        offer_setpoint_step: bool,
        report: &'a dyn CampaignReport,
        clock: &'a dyn CampaignClock,
    ) -> Self {
        Self {
            loop_definition,
            maximum_ms,
            expected_settling_seconds,
            offer_setpoint_step,
            report,
            clock,
        }
    }

    /// Reads the loop with `read` until it rests before the mode transfer of `binding`, and returns the reading at rest, which the
    /// transfer then holds the outputs of. Each progress is shown once, when it changes.
    ///
    /// # Errors
    /// The signals to watch cannot be named (see `RestTracker::for_transfer`); the operation was cancelled (nothing was written); a
    /// reading fails, carries no runtime clock or no finite value of a watched signal; the loop still moves after the baseline's
    /// budget; or no newer runtime value arrived within the connection-loss timeout (A3.4).
    pub fn wait(
        &mut self,
        binding: &TransferBinding,
        read: &mut dyn FnMut() -> Result<TransferStatus, Failure>,
    ) -> Result<TransferStatus, Failure> {
        let mut tracker = RestTracker::for_transfer(
            self.loop_definition,
            binding,
            self.maximum_ms,
            self.expected_settling_seconds,
            self.offer_setpoint_step,
        )?;
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
            let readings = tracker
                .symbols()
                .map(|symbol| value_of(&observed, symbol))
                .collect::<Result<Vec<f64>, Failure>>()?;
            match tracker.feed(process_ms / 1_000.0, &readings)? {
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
