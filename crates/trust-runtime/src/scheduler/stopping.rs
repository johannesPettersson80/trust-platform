//! The stops of a hosted resource (specification 34 STATE-08; specification 11 section 6.6): the
//! stop for an exit request, the stop for the control `shutdown` of a running, paused or faulted
//! resource, and the waits in the state a stop leaves.

use super::{
    faulted, recover_mutex_lock, set_last_error, set_resource_state, stop_outcome, stopped,
    AutomaticRestartLimiter, Clock, LoopShared, Next, ResourceRunner, ResourceState,
};

/// What the scan loop does when the faulted wait ends: it goes on after a recovery, stops the
/// resource and waits in the state the stop leaves after a `shutdown`, and stops it and ends after
/// an exit request (specification 34 STATE-08).
pub(super) fn follow_faulted_wait<C: Clock + Clone>(
    wait: faulted::FaultedWait,
    runner: &mut ResourceRunner<C>,
    shared: &LoopShared<'_>,
    restarts: &mut AutomaticRestartLimiter,
) -> Next {
    match wait {
        faulted::FaultedWait::Resumed => Next::Continue,
        faulted::FaultedWait::StopRequested => {
            stop_faulted(runner, shared);
            wait_after_stop(runner, shared, restarts)
        }
        faulted::FaultedWait::Exit => {
            stop_faulted(runner, shared);
            Next::Exit
        }
    }
}

/// Applies the safe state and saves the retain store for a stop request. A stop request ends the
/// thread at once, so a stop-time failure is recorded without entering the faulted wait.
pub(super) fn stop_resource<C: Clock + Clone>(
    runner: &mut ResourceRunner<C>,
    shared: &LoopShared<'_>,
) {
    let safe_state = runner.runtime.apply_io_safe_state();
    // The thread ends with the process, so identify ends and drives its output off
    // (specification 11, "Identify").
    runner.runtime.end_identify();
    let retain_save = runner.runtime.save_retain_store();
    let (next, stop_error) = stop_outcome(safe_state, retain_save);
    if let Some(err) = stop_error {
        set_last_error(shared.last_error, err);
    }
    set_resource_state(shared.state, next);
}

/// Stops the program for the control `shutdown`, as `stop_resource` does for an exit, and then
/// waits for a restart or an exit (specification 34, STATE-08).
///
/// A restart queued before the stop is dropped with the program it was meant for; one that arrives
/// once the state reads `Stopped` starts the program. The stop leaves the resource `Stopped`, or
/// `Faulted` when the stop outputs are unconfirmed or the save fails, and the wait follows the
/// state it left.
pub(super) fn stop_program<C: Clock + Clone>(
    runner: &mut ResourceRunner<C>,
    shared: &LoopShared<'_>,
    restarts: &mut AutomaticRestartLimiter,
) -> Next {
    stop_started_program(runner, shared);
    wait_after_stop(runner, shared, restarts)
}

/// The stop of a program that started: the safe state, the retain save and the state they leave,
/// `Stopped`, or `Faulted` with the first failure. A restart queued before the stop is dropped with
/// the program it was meant for, and the next start loads the installed program again. A stop that
/// succeeds keeps the root fault of a faulted program in `last_error` until the next start
/// (specification 34 STATE-08).
fn stop_started_program<C: Clock + Clone>(runner: &mut ResourceRunner<C>, shared: &LoopShared<'_>) {
    let safe_state = runner.runtime.apply_io_safe_state();
    let retain_save = runner.runtime.save_retain_store();
    let (next, stop_error) = stop_outcome(safe_state, retain_save);
    let _ = faulted::take_pending_restart(runner);
    if let Some(err) = stop_error {
        set_last_error(shared.last_error, err);
    }
    runner.reload_required = true;
    set_resource_state(shared.state, next);
}

/// The stop of a faulted resource, for a `shutdown` or an exit (specification 34 STATE-08). A
/// program that faulted while it ran is stopped as a running one is. A resource whose start failed
/// runs no program, and its outputs are as the stop before that start left them, so it is `Stopped`
/// at once. The root fault stays in `last_error` until the next start.
fn stop_faulted<C: Clock + Clone>(runner: &mut ResourceRunner<C>, shared: &LoopShared<'_>) {
    if runner.reload_required {
        let _ = faulted::take_pending_restart(runner);
        set_resource_state(shared.state, ResourceState::Stopped);
    } else {
        stop_started_program(runner, shared);
    }
}

/// Waits after a stop in the state it left: in the stopped wait while the resource is `Stopped` and
/// in the faulted wait while it is `Faulted`, until a start resumes the scan loop or an exit ends
/// the thread. A start that fails leaves the resource faulted, and a stop of the faulted resource
/// leaves it stopped again, so the two waits take turns without nesting.
fn wait_after_stop<C: Clock + Clone>(
    runner: &mut ResourceRunner<C>,
    shared: &LoopShared<'_>,
    restarts: &mut AutomaticRestartLimiter,
) -> Next {
    loop {
        if *recover_mutex_lock(shared.state.lock()) == ResourceState::Stopped {
            match stopped::wait_while_stopped(runner, shared, restarts) {
                stopped::StoppedWait::Started => return Next::Continue,
                stopped::StoppedWait::Exited => return Next::Exit,
                // The program did not start: the resource is faulted with the reason.
                stopped::StoppedWait::Failed => {}
            }
        }
        match faulted::wait_while_faulted(
            runner,
            shared.stop,
            shared.state,
            shared.last_error,
            shared.supervision,
            restarts,
        ) {
            faulted::FaultedWait::Resumed => return Next::Continue,
            faulted::FaultedWait::StopRequested => stop_faulted(runner, shared),
            faulted::FaultedWait::Exit => {
                stop_faulted(runner, shared);
                return Next::Exit;
            }
        }
    }
}
