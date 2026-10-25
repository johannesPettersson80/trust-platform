//! Stepped scans of a stopped hosted resource (`cycle.step`, specification 11 section 6.8.1).
//!
//! The stopped wait runs each step on the scan thread. A cold restart loads a program installed
//! since the program was loaded, as `restart` loads it after a stop, and every other restart is
//! the runtime's restart in place; the program steps through its compiled cycle, as a chip steps
//! it (specification 44 §8.8).

use trust_runtime_services::cycle_step::{restart_time, StepOutcome, StepRefusal, StepRequest};

use super::{Clock, ResourceRunner};
use crate::value::Duration;
use crate::RestartMode;

/// What the stopped wait keeps of its steps.
#[derive(Debug, Default)]
pub(super) struct Steps {
    /// The logical time of the program's last step since the resource stopped.
    last: Option<Duration>,
    /// Whether `program.install` installed a program since the program was loaded.
    pub(super) installed: bool,
}

/// Runs `request` on the stopped `runner`.
pub(super) fn step<C: Clock + Clone>(
    runner: &mut ResourceRunner<C>,
    request: &StepRequest,
    steps: &mut Steps,
) -> Result<StepOutcome, StepRefusal> {
    // A program installed since the program was loaded starts with its cold restart, and its
    // stepped time starts at zero.
    let loads = request.restart == Some(RestartMode::Cold) && steps.installed;
    let restart_at = restart_time(request, if loads { None } else { steps.last })?;
    let mut restart = request.restart;
    if loads
        && runner
            .load_installed(RestartMode::Cold)
            .map_err(StepRefusal::Restart)?
    {
        steps.installed = false;
        steps.last = None;
        restart = None;
    }
    // The program steps through its compiled cycle, as a chip steps it (specification 44 §8.8);
    // the cycle restarts it at `restart_at` when the request still asks for a restart.
    let mut request = request.clone();
    request.restart = restart;
    let outcome = runner.runtime.step(&request, restart_at)?;
    steps.last = Some(request.time);
    Ok(outcome)
}
