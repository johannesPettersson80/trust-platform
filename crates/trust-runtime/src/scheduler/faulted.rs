//! Faulted wait state of a hosted resource (specification 11 section 6.6, "Faulted resource
//! lifecycle (hosted runtime)"; specification 34 STATE-06).
//!
//! A halting fault leaves the scan thread alive and idle. It executes no task and commits no
//! outputs. It waits here for a recovery request (a restart, or an online change), serves
//! read-only and configuration commands, and ends only on a stop request.

use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::mpsc::RecvTimeoutError;
use std::sync::{Arc, Mutex};
use std::thread;
use std::time::Duration as StdDuration;

use super::supervisor::ScanSupervision;
use super::{
    apply_resource_command, recover_mutex_lock, set_last_error, set_resource_state,
    AutomaticRestartLimiter, Clock, ResourceCommand, ResourceRunner, ResourceState, RuntimeError,
};

/// Poll period of the faulted wait. A command wakes the wait at once. A restart request or a
/// stop request is seen within this period, so the wait never spins.
const FAULTED_WAIT_POLL: StdDuration = StdDuration::from_millis(10);

/// How the faulted wait ended.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(super) enum FaultedWait {
    /// A recovery succeeded. The resource is `Running` again and the scan loop resumes.
    Resumed,
    /// A stop request ended the thread. The resource stays `Faulted`. No second safe-state write
    /// and no retain save happen here, so the last periodic checkpoint stays authoritative.
    Stopped,
}

/// Records a halting fault whose fault action has already run, then waits for recovery or stop.
/// `last_error` keeps this root fault until a recovery succeeds.
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

/// Waits in the faulted state until a recovery succeeds or a stop is requested.
pub(super) fn wait_while_faulted<C: Clock + Clone>(
    runner: &mut ResourceRunner<C>,
    stop: &AtomicBool,
    state: &Arc<Mutex<ResourceState>>,
    last_error: &Arc<Mutex<Option<RuntimeError>>>,
    supervision: &ScanSupervision,
    automatic_restarts: &mut AutomaticRestartLimiter,
) -> FaultedWait {
    loop {
        // A stop request is checked first, so no recovery runs after a stop.
        if stop.load(Ordering::SeqCst) {
            return FaultedWait::Stopped;
        }
        let online_change_applied = match next_command(runner) {
            Some(command) => serve_command(runner, command),
            None => false,
        };
        if online_change_applied {
            return finish_recovery(state, last_error, supervision, automatic_restarts);
        }
        if let Some(mode) = take_pending_restart(runner) {
            match runner.restart_with_retain_store(mode) {
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
fn next_command<C: Clock + Clone>(runner: &ResourceRunner<C>) -> Option<ResourceCommand> {
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

/// Serves one command while faulted. Returns true when an online change was applied.
pub(super) fn serve_command<C: Clock + Clone>(
    runner: &mut ResourceRunner<C>,
    command: ResourceCommand,
) -> bool {
    match command {
        // The control plane refuses pause and resume while faulted. They are ignored if they arrive.
        ResourceCommand::Pause | ResourceCommand::Resume => false,
        // Variable writes are dropped: fault handling is a clearing boundary (spec 11 section 6.9.2).
        ResourceCommand::MeshApply { .. } => false,
        ResourceCommand::ReloadBytecode {
            bytes,
            claim,
            respond_to,
        } => {
            // A withdrawn reload is skipped: it neither commits nor ends the faulted wait.
            if !claim.take() {
                return false;
            }
            let result = runner.runtime.apply_online_change_bytes(&bytes, None);
            let applied = result.is_ok();
            let _ = respond_to.send(result);
            applied
        }
        // Configuration updates and read-only reports answer exactly as they do while running.
        other => {
            apply_resource_command(&mut runner.runtime, other);
            false
        }
    }
}
