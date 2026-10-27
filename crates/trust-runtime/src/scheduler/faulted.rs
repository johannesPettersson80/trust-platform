//! Faulted wait state of a hosted resource (specification 11 section 6.6, "Faulted resource
//! lifecycle (hosted runtime)"; specification 34 STATE-06).
//!
//! A halting fault leaves the scan thread alive and idle. It executes no task and commits no
//! outputs. It waits here for a recovery request (a restart, or an online change) and serves
//! read-only and configuration commands. A `shutdown` stops the faulted resource as it stops a
//! running one, and an exit request does the same and ends the thread (specification 34 STATE-08);
//! the scan loop runs both stops.

use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::mpsc::RecvTimeoutError;
use std::sync::{Arc, Mutex};
use std::thread;
use std::time::Duration as StdDuration;

use trust_runtime_services::resource::commands::{faulted_command_effect, FaultedCommandEffect};

use super::supervisor::ScanSupervision;
use super::{
    apply_runner_command, command_class, recover_mutex_lock, set_last_error, set_resource_state,
    AutomaticRestartLimiter, Clock, ResourceCommand, ResourceRunner, ResourceState, RuntimeError,
    StepRefusal,
};

/// Poll period of the faulted wait, the shared one. A command wakes the wait at once. A restart
/// request or a stop request is seen within this period, so the wait never spins.
pub(super) const FAULTED_WAIT_POLL: StdDuration =
    trust_runtime_services::resource::supervisor::FAULTED_WAIT_POLL;

/// How the faulted wait ended.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(super) enum FaultedWait {
    /// A recovery succeeded. The resource is `Running` again and the scan loop resumes.
    Resumed,
    /// A `shutdown` asked to stop the faulted resource (specification 34 STATE-08). The scan loop
    /// stops it and waits in the state the stop leaves.
    StopRequested,
    /// An exit request, the explicit exit or a termination signal, ends the thread. The scan loop
    /// stops the resource first, as a `shutdown` does.
    Exit,
}

/// Records a halting fault whose fault action has already run, then waits for recovery or stop.
/// `last_error` keeps this root fault until the next start.
pub(super) fn halt_and_wait<C: Clock + Clone>(
    runner: &mut ResourceRunner<C>,
    stop: &AtomicBool,
    state: &Arc<Mutex<ResourceState>>,
    last_error: &Arc<Mutex<Option<RuntimeError>>>,
    supervision: &ScanSupervision,
    automatic_restarts: &mut AutomaticRestartLimiter,
    fault: RuntimeError,
) -> FaultedWait {
    set_last_error(last_error, fault);
    set_resource_state(state, ResourceState::Faulted);
    wait_while_faulted(
        runner,
        stop,
        state,
        last_error,
        supervision,
        automatic_restarts,
    )
}

/// Waits in the faulted state until a recovery succeeds, a `shutdown` asks for a stop or an exit is
/// requested.
pub(super) fn wait_while_faulted<C: Clock + Clone>(
    runner: &mut ResourceRunner<C>,
    stop: &AtomicBool,
    state: &Arc<Mutex<ResourceState>>,
    last_error: &Arc<Mutex<Option<RuntimeError>>>,
    supervision: &ScanSupervision,
    automatic_restarts: &mut AutomaticRestartLimiter,
) -> FaultedWait {
    loop {
        // An exit request is checked first, so no recovery runs after it. The thread ends, so
        // identify ends and drives its output off.
        if stop.load(Ordering::SeqCst) {
            runner.runtime.end_identify();
            return FaultedWait::Exit;
        }
        // The program scans nothing, so identify blinks from here, except an output that the safe
        // state of the fault holds at its safe value (specification 11, "Identify").
        runner.runtime.drive_identify_while_idle();
        // A `shutdown` stops the faulted resource as it stops a running one (specification 34,
        // STATE-08). The start after the stop arms the debugger again, as after any stop.
        if supervision.take_program_stop() {
            return FaultedWait::StopRequested;
        }
        let online_change_applied = match next_command(runner) {
            Some(command) => serve_command(runner, command),
            None => false,
        };
        if online_change_applied {
            // The running program is the one the online change gave it, not a stopped one.
            runner.reload_required = false;
            return finish_recovery(state, last_error, supervision, automatic_restarts);
        }
        if let Some(mode) = take_pending_restart(runner) {
            match runner.restart_or_reload(mode) {
                Ok(()) => {
                    return finish_recovery(state, last_error, supervision, automatic_restarts);
                }
                // The resource stays faulted, and the new error replaces the root fault.
                Err(err) => set_last_error(last_error, err),
            }
        }
    }
}

/// Clears what a successful recovery retires: the hung flag and the stale root fault.
/// The automatic-restart budget is not touched here, because an automatic retry counts itself.
pub(super) fn clear_recovered_fault(
    supervision: &ScanSupervision,
    last_error: &Arc<Mutex<Option<RuntimeError>>>,
) {
    supervision.clear_hung();
    *recover_mutex_lock(last_error.lock()) = None;
}

/// Returns the resource to `Running` after a control restart or an online change.
fn finish_recovery(
    state: &Arc<Mutex<ResourceState>>,
    last_error: &Arc<Mutex<Option<RuntimeError>>>,
    supervision: &ScanSupervision,
    automatic_restarts: &mut AutomaticRestartLimiter,
) -> FaultedWait {
    automatic_restarts.reset();
    clear_recovered_fault(supervision, last_error);
    set_resource_state(state, ResourceState::Running);
    FaultedWait::Resumed
}

/// Takes the restart request that the control plane left in the restart signal, if any.
pub(super) fn take_pending_restart<C: Clock + Clone>(
    runner: &ResourceRunner<C>,
) -> Option<crate::RestartMode> {
    let signal = runner.restart_signal.as_ref()?;
    let mut guard = recover_mutex_lock(signal.lock());
    guard.take()
}

/// Waits up to one poll period for a command. A missing or closed channel still paces the wait.
pub(super) fn next_command<C: Clock + Clone>(
    runner: &ResourceRunner<C>,
) -> Option<ResourceCommand> {
    let Some(commands) = runner.command_rx.as_ref() else {
        thread::sleep(FAULTED_WAIT_POLL);
        return None;
    };
    match commands.recv_timeout(FAULTED_WAIT_POLL) {
        Ok(command) => Some(command),
        Err(RecvTimeoutError::Timeout) => None,
        Err(RecvTimeoutError::Disconnected) => {
            thread::sleep(FAULTED_WAIT_POLL);
            None
        }
    }
}

/// Serves one command while faulted, by the shared faulted-wait rules. Returns true when an
/// online change was applied.
pub(super) fn serve_command<C: Clock + Clone>(
    runner: &mut ResourceRunner<C>,
    command: ResourceCommand,
) -> bool {
    if let ResourceCommand::StepCycle { respond_to, .. } = command {
        // A faulted program takes no step (specification 11 section 6.8.1).
        let _ = respond_to.send(Err(StepRefusal::NotStopped(ResourceState::Faulted)));
        return false;
    }
    match faulted_command_effect(command_class(&command)) {
        // The control plane refuses pause and resume while faulted; they are ignored if they
        // arrive. Variable writes are dropped: fault handling is a clearing boundary (spec 11
        // section 6.9.2).
        FaultedCommandEffect::Ignore | FaultedCommandEffect::Discard => false,
        FaultedCommandEffect::Recover => apply_recovery_reload(runner, command),
        // Configuration updates and read-only reports answer exactly as they do while running.
        FaultedCommandEffect::Apply => {
            apply_runner_command(&mut runner.runtime, runner.refuse_mesh_catalogue, command);
            false
        }
    }
}

/// Applies a reload while faulted. Returns true when the online change was applied, which ends
/// the fault.
fn apply_recovery_reload<C: Clock + Clone>(
    runner: &mut ResourceRunner<C>,
    command: ResourceCommand,
) -> bool {
    let ResourceCommand::ReloadBytecode {
        bytes,
        claim,
        respond_to,
    } = command
    else {
        return false;
    };
    // A withdrawn reload is skipped: it neither commits nor ends the faulted wait.
    if !claim.take() {
        return false;
    }
    let result = runner.runtime.apply_online_change_bytes(&bytes);
    let applied = result.is_ok();
    let _ = respond_to.send(result);
    applied
}
