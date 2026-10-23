//! Stepped scans of a stopped hosted resource (`cycle.step`, specification 11 section 6.8.1).
//!
//! The stopped wait runs each step on the scan thread. Names resolve as the test harness resolves
//! them; a cold restart loads a program installed since the program was loaded, as `restart` loads
//! it after a stop, and every other restart is the runtime's restart in place; the scan is the
//! shared scheduler at the request's time without the hosted services, so a step runs as the
//! conformance runner's bytecode cycles run, or the body of the program instance the step names,
//! as the test runner runs a test program. The watchdog bounds the scan as it bounds every scan.

use trust_runtime_services::cycle_step::{restart_time, StepOutcome, StepRefusal, StepRequest};

use super::{arm_cycle_deadlines, restore_cycle_deadlines, Clock, ResourceRunner};
use crate::boundary::{resolve_read, resolve_write, BoundaryError};
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
    let runtime = &mut runner.runtime;
    for name in request
        .inputs
        .iter()
        .map(|(name, _)| name)
        .chain(&request.watch)
    {
        resolve_read(runtime, name).map_err(|error| StepRefusal::name(name, reason(&error)))?;
    }
    let run = request
        .run
        .as_ref()
        .map(|name| {
            runtime
                .programs()
                .values()
                .find(|program| program.name.eq_ignore_ascii_case(name))
                .cloned()
                .ok_or_else(|| StepRefusal::name(name, "names no program instance"))
        })
        .transpose()?;
    if let Some(mode) = restart {
        runtime.set_current_time(restart_at);
        runtime.restart(mode).map_err(StepRefusal::Restart)?;
    }
    for (name, value) in &request.inputs {
        resolve_write(runtime, name, value.clone())
            .map_err(|error| StepRefusal::write(name, &error))?;
    }
    for (text, address, value) in &request.direct_inputs {
        runtime
            .write_io(address, value.clone())
            .map_err(|error| StepRefusal::write(text, &error))?;
    }
    runtime.set_current_time(request.time);
    let watchdog = runtime.watchdog_policy();
    let deadlines = arm_cycle_deadlines(runtime, watchdog, std::time::Instant::now());
    let error = match &run {
        Some(program) => runtime.execute_stepped_program(program),
        None => runtime.execute_stepped_cycle(),
    }
    .err();
    restore_cycle_deadlines(runtime, deadlines);
    steps.last = Some(request.time);
    let values = request
        .watch
        .iter()
        .map(|name| {
            resolve_read(runtime, name)
                .map(|value| (name.clone(), value))
                .map_err(|error| StepRefusal::read(name, &error))
        })
        .collect::<Result<Vec<_>, _>>()?;
    let direct = request
        .watch_direct
        .iter()
        .map(|(text, address)| {
            runtime
                .io()
                .read(address)
                .map(|value| (text.clone(), value))
                .map_err(|error| StepRefusal::read(text, &error))
        })
        .collect::<Result<Vec<_>, _>>()?;
    Ok(StepOutcome {
        values,
        direct,
        error,
    })
}

/// Why a name names no single variable, in the words of the other runtimes.
fn reason(error: &BoundaryError) -> String {
    match error {
        BoundaryError::UnresolvedName { .. } => "names no variable of the program".to_string(),
        BoundaryError::AmbiguousName { .. } => "names a variable of several programs".to_string(),
        other => other.to_string(),
    }
}
