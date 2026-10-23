//! Stopped wait of a hosted resource (specification 34 STATE-08; specification 11 section 6.9).
//!
//! The control `shutdown` stops the program and leaves the scan thread alive. The thread executes
//! no task and commits no outputs. It serves the commands that act on a stopped runtime and waits
//! for a `restart`, which starts the installed program again, or for an exit request, the
//! termination signals and `shutdown` with `{"exit": true}`, which end it. The control server,
//! the web server and the other services of the process stay up around it.

use std::sync::atomic::Ordering;

use trust_runtime_services::resource::commands::CommandClass;

use super::faulted::{next_command, take_pending_restart};
use super::{
    apply_resource_command, command_class, set_last_error, set_resource_state, stepped,
    AutomaticRestartLimiter, Clock, LoopShared, ResourceCommand, ResourceRunner, ResourceState,
};

/// How the stopped wait ended.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(super) enum StoppedWait {
    /// A `restart` started the installed program. The resource is `Running` again.
    Started,
    /// The program did not start. The resource is `Faulted` with the reason, and the next
    /// `restart` loads the program again.
    Failed,
    /// An exit request ended the thread. The resource stays `Stopped`.
    Exited,
}

/// Waits in the stopped state until a `restart` starts the installed program or an exit request
/// ends the thread.
pub(super) fn wait_while_stopped<C: Clock + Clone>(
    runner: &mut ResourceRunner<C>,
    shared: &LoopShared<'_>,
    automatic_restarts: &mut AutomaticRestartLimiter,
) -> StoppedWait {
    // What the stepped scans since the resource stopped keep (specification 11 section 6.8.1);
    // every stop starts this wait again, so the steps after a new stop start afresh.
    let mut steps = stepped::Steps::default();
    loop {
        // An exit request is checked first, so no program starts after it.
        if shared.stop.load(Ordering::SeqCst) {
            return StoppedWait::Exited;
        }
        // A `shutdown` of a stopped program stops nothing more.
        let _ = shared.supervision.take_program_stop();
        if let Some(command) = next_command(runner) {
            serve_command(runner, command, &mut steps);
        }
        if let Some(mode) = take_pending_restart(runner) {
            return match runner.restart_or_reload(mode) {
                Ok(()) => {
                    automatic_restarts.reset();
                    super::faulted::clear_recovered_fault(shared.supervision, shared.last_error);
                    set_resource_state(shared.state, ResourceState::Running);
                    StoppedWait::Started
                }
                // The program stays faulted with the reason until a restart loads it again.
                Err(err) => {
                    set_last_error(shared.last_error, err);
                    set_resource_state(shared.state, ResourceState::Faulted);
                    StoppedWait::Failed
                }
            };
        }
    }
}

/// Serves one command while stopped. A stepped scan runs here, and only here (specification 11
/// section 6.8.1). A configuration update or a read-only report answers as it does while the
/// program runs. The other commands have nothing to act on: the control plane refuses a pause and
/// a resume while stopped, the stop cleared the queued variable writes (specification 11 section
/// 6.9.2), and an online change is refused while stopped (specification 11 section 6.7.1), so its
/// sender sees the request dropped.
fn serve_command<C: Clock + Clone>(
    runner: &mut ResourceRunner<C>,
    command: ResourceCommand,
    steps: &mut stepped::Steps,
) {
    match command {
        ResourceCommand::StepCycle {
            request,
            respond_to,
        } => {
            let _ = respond_to.send(stepped::step(runner, &request, steps));
            return;
        }
        ResourceCommand::ProgramInstalled => {
            steps.installed = true;
            return;
        }
        _ => {}
    }
    match command_class(&command) {
        CommandClass::Service => apply_resource_command(&mut runner.runtime, command),
        CommandClass::Pause
        | CommandClass::Resume
        | CommandClass::Reload
        | CommandClass::VariableWrite => {}
    }
}
