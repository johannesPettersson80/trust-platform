//! The hold pass: what a device does while the debugger holds a scan (specification 13, "Pause",
//! "Continue", "Stepping Semantics" and "Safe Points"; specification 11 §6.9; specification 34,
//! STATE-07, STATE-08, PORT-05 and TIME-04).
//!
//! The device services' statement hook ([`DeviceServices`](crate::runner::engine::DeviceServices))
//! calls the loop at the first instruction of each statement through a [`HoldHandle`]. The loop
//! decides there whether the scan stops, and if it does, holds the scan inside the hook. A hold
//! repeats one pass until it is released:
//!
//! 1. it reads a wake token;
//! 2. it serves up to [`FRAMES_PER_PASS`](crate::runner::FRAMES_PER_PASS) requests through the
//!    loop's own path, so `status`, `health`, `pause`, `resume` and `shutdown` answer as between
//!    scans (the halted state is lent to the pass as a [`HeldView`], for the debug reads);
//! 3. it runs the platform's idle work, so communication drivers keep running as they do on
//!    trust-runtime;
//! 4. it feeds the hardware watchdog;
//! 5. it ends when the debugger no longer holds the scan, and otherwise waits until the earlier of
//!    one feed interval and, while the link still holds reply bytes, one millisecond.
//!
//! Step 2 also withdraws an online change that has waited five seconds for a boundary (specification 34
//! §11.3): a held scan reaches none, and the loop reads no request while the change's reply is owed.
//!
//! It never takes a boundary: no scan starts, no output is committed (the hold sits before the output
//! phase), and no retained checkpoint advances, so it runs neither the supervisor nor the persister
//! step. A hold lasts until a `resume`, a step or a stop request. Nothing else releases it: neither
//! time, nor the cycle watchdog, nor a new host session of the link (PORT-05 drops only the reply
//! being sent).
//!
//! The pass borrows what the loop shares through [`Shell`](super::shell::Shell) one leaf operation at
//! a time; see the borrow rules there.

use alloc::rc::Rc;
use alloc::vec::Vec;
use core::cell::Cell;

use smol_str::SmolStr;
use trust_runtime_core::error::RuntimeError;
use trust_runtime_core::memory::VariableStorage;
use trust_runtime_core::ports::{ControlLink, MonotonicClock, RunnerWait};
use trust_runtime_core::value;
use trust_runtime_core::vm::StatementFrame;

use super::debugger::{Boundary, StatementStart};
use super::host::ProgramView;
use super::shell::Shell;
use super::{RunnerPlatform, FRAMES_PER_PASS, REPLY_SERVICE_INTERVAL};
use crate::status::IoDriverStatus;

/// The halted state of a held scan, lent to the hold for the length of one hook call: the storage
/// and the active frame the engine passed to the hook, and the health of the process I/O the cycle
/// published before the program ran. Nothing in it is a copy (specification 11 §6.13.1: the value
/// of a held scan is the halted state, and the runtime makes no copy per statement).
#[cfg_attr(
    not(test),
    allow(
        dead_code,
        reason = "S3d-8's debug.stack, debug.scopes and debug.variables read the halted state here"
    )
)]
pub(super) struct HeldView<'a> {
    /// The scan's storage as the statements before the stop left it.
    pub(super) storage: &'a VariableStorage,
    /// The frame of the statement the scan stopped before, with the suspended frames below it.
    pub(super) frame: StatementFrame<'a>,
    /// The logical time of the scan.
    pub(super) logical_time: value::Duration,
    /// The health of the process I/O, which the cycle published after it read the inputs.
    pub(super) drivers: &'a Cell<Vec<IoDriverStatus>>,
}

/// What the device services ask of the resource loop at a scan and at a statement start. The
/// loop's shell implements it.
pub(super) trait HoldPass {
    /// A scan begins: the ordinary stops of the last one are forgotten. Returns whether the platform
    /// enables debugging for this scan.
    fn scan_begins(&self) -> bool;

    /// The decision at the statement start `at` (specification 13, "Runtime Debug-Control
    /// Execution").
    ///
    /// # Errors
    ///
    /// The [`RuntimeError`] that fails the scan when the loop's state was in use.
    fn at_statement(&self, at: &StatementStart) -> Result<Boundary, RuntimeError>;

    /// Holds the scan before the statement `at` until the debugger releases it.
    ///
    /// # Errors
    ///
    /// The [`RuntimeError`] that fails the scan when the hold cannot go on: a wait that failed, or
    /// state of the loop that was in use.
    fn hold(&self, at: &StatementStart, view: &HeldView<'_>) -> Result<(), RuntimeError>;

    /// Whether no cell of the loop is borrowed.
    #[cfg(test)]
    fn cells_free(&self) -> bool;
}

/// The resource loop as the device services hold it, to hold a scan at a statement start. The loop
/// hands one to the installed program's cycle when it starts ([`ResourceCycle::attach_hold`]).
///
/// [`ResourceCycle::attach_hold`]: crate::runner::ResourceCycle::attach_hold
#[derive(Clone)]
pub struct HoldHandle(pub(super) Rc<dyn HoldPass>);

impl HoldHandle {
    /// A handle on `shell`, which the loop keeps for the rest of the boot: its link, wait,
    /// platform and clock are `'static`, as a firmware keeps them.
    pub(super) fn new<L, W, P, C>(shell: &Rc<Shell<L, W, P, C>>) -> Self
    where
        L: ControlLink + 'static,
        W: RunnerWait + 'static,
        P: RunnerPlatform + 'static,
        C: MonotonicClock + 'static,
    {
        // Clone with the concrete type, then coerce: a target type on the clone itself would make
        // the compiler look for a clone of the trait object.
        let shell: Rc<Shell<L, W, P, C>> = Rc::clone(shell);
        Self(shell)
    }

    /// A scan begins; see [`HoldPass::scan_begins`].
    pub(super) fn scan_begins(&self) -> bool {
        self.0.scan_begins()
    }

    /// The decision at a statement start; see [`HoldPass::at_statement`].
    pub(super) fn at_statement(&self, at: &StatementStart) -> Result<Boundary, RuntimeError> {
        self.0.at_statement(at)
    }

    /// Holds the scan; see [`HoldPass::hold`].
    pub(super) fn hold(
        &self,
        at: &StatementStart,
        view: &HeldView<'_>,
    ) -> Result<(), RuntimeError> {
        self.0.hold(at, view)
    }

    /// Whether no cell of the loop is borrowed, for the tests that prove no borrow outlives an
    /// operation.
    #[cfg(test)]
    pub(crate) fn cells_free(&self) -> bool {
        self.0.cells_free()
    }
}

/// The installed program as a held scan shows it: installed, with the I/O health its cycle
/// published.
struct HeldProgram<'a> {
    drivers: &'a Cell<Vec<IoDriverStatus>>,
}

impl ProgramView for HeldProgram<'_> {
    /// A program that is held has run its scan, so it is installed.
    fn missing_program(&self) -> Option<&str> {
        None
    }

    fn driver_health(&self) -> Vec<IoDriverStatus> {
        let drivers = self.drivers.take();
        let health = drivers.clone();
        self.drivers.set(drivers);
        health
    }
}

impl<L, W, P, C> HoldPass for Shell<L, W, P, C>
where
    L: ControlLink,
    W: RunnerWait,
    P: RunnerPlatform,
    C: MonotonicClock,
{
    /// The ordinary stops of the last scan are forgotten when the scan can take stops, as no scan
    /// with debugging disabled takes one, so such a scan touches no debugger state (checklist P1-6).
    fn scan_begins(&self) -> bool {
        let enabled = self.debug_enabled();
        if enabled {
            self.debug.begin_cycle();
        }
        enabled
    }

    fn at_statement(&self, at: &StatementStart) -> Result<Boundary, RuntimeError> {
        #[cfg(test)]
        assert!(
            self.cells_free(),
            "a cell of the loop is borrowed when the statement hook is called"
        );
        self.debug.at_statement(at)
    }

    fn hold(&self, at: &StatementStart, view: &HeldView<'_>) -> Result<(), RuntimeError> {
        #[cfg(test)]
        assert!(
            self.cells_free(),
            "a cell of the loop is borrowed when a hold begins"
        );
        loop {
            let token = self.wake_token();
            #[cfg(test)]
            self.run_probe(view);
            // A scan that the debugger holds reaches no boundary, so the change that waits for one is
            // withdrawn here after five seconds, and the requests behind its reply are served again.
            self.expire_change();
            let more_frames = self.serve_held(view);
            self.idle();
            self.feed();
            if self.take_fault() {
                return Err(RuntimeError::InvalidExecutionState);
            }
            if !self.debug.holds(at)? {
                return Ok(());
            }
            if more_frames {
                continue;
            }
            let now = self.clock.now();
            let mut deadline = now.saturating_add(self.feed_interval());
            if self.link_sending() {
                // A reply in flight is driven to its end within a millisecond, as between scans
                // (specification 34, PORT-05).
                deadline = deadline.min(now.saturating_add(REPLY_SERVICE_INTERVAL));
            }
            if let Some(expiry) = self.change.expiry() {
                deadline = deadline.min(expiry);
            }
            if let Err(failure) = self.wait_until(deadline, token) {
                // The run ends with this failure at its next pass, as it ends when the wait fails
                // between scans; the scan fails now, because it cannot be held without a wait.
                self.set_port_failure(failure);
                return Err(RuntimeError::ControlError(SmolStr::new_static(
                    "the wait failed while the debugger held the scan",
                )));
            }
        }
    }

    #[cfg(test)]
    fn cells_free(&self) -> bool {
        Shell::cells_free(self)
    }
}

impl<L, W, P, C> Shell<L, W, P, C>
where
    L: ControlLink,
    W: RunnerWait,
    P: RunnerPlatform,
    C: MonotonicClock,
{
    /// Serves up to [`FRAMES_PER_PASS`] requests of a hold, in the order the link received them,
    /// and returns true when it stops with the link not yet dry. A hold has no boundary that a
    /// request could delay, so it starts every request it finds.
    fn serve_held(&self, view: &HeldView<'_>) -> bool {
        let program = HeldProgram {
            drivers: view.drivers,
        };
        for _ in 0..FRAMES_PER_PASS {
            let Some(received) = self.next_frame() else {
                return false;
            };
            let after_response = self.serve(received, &program);
            self.note_reply(after_response);
        }
        true
    }

    /// Calls the test's probe with the view of this pass, if there is one.
    #[cfg(test)]
    fn run_probe(&self, view: &HeldView<'_>) {
        if let Ok(probe) = self.probe.try_borrow()
            && let Some(probe) = probe.as_ref()
        {
            probe(view);
        }
    }
}
