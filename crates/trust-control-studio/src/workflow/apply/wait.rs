//! The wait of an Apply for its starting point (specification 35, B6.2, A3.2, A3.4 and A10.4; `awaitCommissioningStartingPoint` of
//! `commissioningStartingPoint.ts`): the loop is read again and again until every measured output that does not rest has stayed in its
//! band for the settle dwell. Each step's decision is trust-control's (`StartingPoint::wait_step`, which refuses a plant input away from
//! its point at once and fails an output that has not settled within the process-time bound); this module reads the runtime, measures
//! process time from the runtime's own clock, hands each step the start of the dwell that the step before returned, shows the progress
//! when it changes, stops on a cancel, and writes nothing.

use std::thread::sleep;

use trust_control::commissioning::apply_texts;
use trust_control::commissioning::commissioning_result::TransferStatus;
use trust_control::commissioning::starting_point::{wait_cancelled, WaitStep};
use trust_control::failure::Failure;
use trust_control::limits::CONNECTION_LOSS_TIMEOUT_MS;
use trust_control::signals::ReadingClock;

use super::ApplyTiming;
use crate::workflow::identify_campaign::CampaignReport;

/// One step of the wait: the reading, the process time waited since the first reading in ms, and the start of the dwell that the step
/// before returned (`None` at the first reading).
pub(crate) type StartingPointStep<'a> =
    dyn Fn(&TransferStatus, f64, Option<f64>) -> Result<WaitStep, Failure> + 'a;

/// Waits until the outputs that `step` holds have stayed inside their bands for its settle dwell (B6.2), and returns the reading that
/// ends the wait. `step` is handed each reading with the process time waited since the first one, in ms, and the start of the dwell
/// that its previous step returned (`None` at the first reading). Each progress is shown once, when it changes, so a waiting page
/// receives only a changed reading (A10.7). The wait reads the runtime only; nothing is written while it waits.
///
/// # Errors
/// The engineer cancelled the wait (`wait_cancelled`: nothing was written); a reading fails, or carries no runtime clock; `step` refuses
/// the reading (a plant input away from its point, or an output that has not settled within the bound); or no newer runtime value
/// arrived within the connection-loss timeout (A3.4).
pub(crate) fn await_starting_point(
    read: &mut dyn FnMut() -> Result<TransferStatus, Failure>,
    step: &StartingPointStep<'_>,
    report: &dyn CampaignReport,
    timing: &ApplyTiming<'_>,
) -> Result<TransferStatus, Failure> {
    let mut clock = ReadingClock::new((timing.wall_ms)());
    let mut shown: Option<String> = None;
    let mut inside_since_ms: Option<f64> = None;
    loop {
        if report.is_cancelled() {
            return Err(wait_cancelled());
        }
        let observed = read()?;
        let runtime_time_ns = observed
            .runtime_time_ns
            .ok_or_else(apply_texts::runtime_clock_missing)?;
        let process_ms = clock.observe(runtime_time_ns, (timing.wall_ms)());
        match step(&observed, process_ms, inside_since_ms)? {
            WaitStep::Settled => return Ok(observed),
            WaitStep::Waiting {
                progress,
                inside_since_ms: since,
            } => {
                inside_since_ms = since;
                if shown.as_deref() != Some(progress.as_str()) {
                    report.progress(&progress);
                    shown = Some(progress);
                }
            }
        }
        clock.ensure_fresh((timing.wall_ms)(), CONNECTION_LOSS_TIMEOUT_MS)?;
        sleep(timing.wait_poll);
    }
}

#[cfg(test)]
#[path = "wait_tests.rs"]
mod tests;
