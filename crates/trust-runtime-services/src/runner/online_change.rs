//! Online change on a device (specification 11 §6.7.1; specification 34 §11.3, STATE-08,
//! UPDATE-01 to UPDATE-07 and PORT-05).
//!
//! `bytecode.reload` replaces the running program at a boundary of the loop and writes no flash. The
//! request rule is the one every runtime shares ([`admit`](crate::loading::reload::admit)). The loop
//! queues the admitted change here and writes no reply until the change ends: the reply is the one
//! reply in flight, so the loop reads no further request meanwhile (PORT-05). At the program's next
//! boundary it does what a computer's scan thread does, in this order:
//!
//! 1. it takes the queued change, unless the five seconds that a request waits for a boundary
//!    ([`TAKE_WAIT`](crate::loading::reload::TAKE_WAIT)) have passed. The request is then withdrawn with
//!    `reload timeout` and never applied;
//! 2. the device prepares and checks the candidate while the running program stays installed
//!    ([`prepare_change`](crate::runner::ProgramLoader::prepare_change)), so the heap holds the running
//!    program, the upload and the candidate at once (UPDATE-06). A candidate that cannot be prepared,
//!    or that fails a check, ends the change with its error, and the running program is as it was:
//!    its cycle, its values, its lines, its logical time, its scan counter, its fault and its status;
//! 3. the commit hands the lines and the checkpoint store of the running program to the candidate
//!    ([`commit_change`](crate::runner::ProgramLoader::commit_change)), which becomes the loop's
//!    program. The writes and forces that were queued go with the running program's Live Values, the
//!    scan counter restarts and the logical time goes on;
//! 4. the running program is dropped, last.
//!
//! The rules that every device shares, the memory a candidate may use and the match of its bindings
//! with the lines of the running program's record, are in [`rules`].

mod rules;

use alloc::string::String;
use alloc::vec::Vec;
use core::cell::RefCell;

use trust_runtime_core::native::change::CarriedValueEvent;
use trust_runtime_core::ports::{
    ControlLink, MonotonicClock, MonotonicInstant, RunnerWait, TimeSpan,
};

pub use rules::{address_text, unwired_binding, ChangeMemory};

use super::program::Program;
use super::{DeviceLiveValues, ResourceCycle, Runner, RunnerPlatform};
use crate::loading::reload::{self, ReloadOutcome, TAKE_WAIT};
use crate::protocol::ControlResponse;

/// The program that replaces the running one in an online change, prepared and checked
/// (specification 34 §11.3).
pub struct Candidate<Y> {
    /// The candidate's cycle, built from the running program's state over its record. It holds no line
    /// and no checkpoint store: the commit hands those over from the running program.
    pub cycle: Y,
    /// The candidate's Live Values, which the commit gives the platform and the loop in place of the
    /// running program's: they hold no queued write and no force, and carry the candidate's
    /// installation generation.
    pub live: DeviceLiveValues,
    /// The variables of the candidate that did not keep the running program's values as they were,
    /// which the loop records at the moment of the commit (specification 34 §11.3).
    pub events: Vec<CarriedValueEvent>,
}

/// The online change that waits for the program's next boundary.
///
/// The change is queued by the request that was admitted ([`Self::begin`]), the loop holds the
/// request's reply ([`Self::hold_reply`]), and the change ends at a boundary, by the five seconds of
/// [`TAKE_WAIT`], or with a stop ([`Self::finish`]). The state is behind one cell that an operation
/// borrows for its own work only.
#[derive(Default)]
pub(super) struct ChangeState(RefCell<Parts>);

/// The parts of the state, behind its one cell.
#[derive(Default)]
struct Parts {
    /// Whether the loop was given a loader that can change the program online.
    enabled: bool,
    /// The change that waits.
    pending: Option<Pending>,
    /// The request being answered queued a change; the loop takes this once it has answered.
    began: bool,
}

/// A queued change.
struct Pending {
    /// The upload of the request, until a boundary takes it.
    program: Option<Vec<u8>>,
    /// The reply the request is owed, once the loop has answered it.
    held: Option<Held>,
}

/// The reply a queued change is owed.
struct Held {
    /// The request number, which the failure of the change is answered with.
    id: u64,
    /// The `reloaded` reply, written when the change is applied.
    reloaded: String,
    /// When the change was queued: the five seconds count from here.
    queued_at: MonotonicInstant,
}

impl ChangeState {
    /// Runs `operation` on the parts, or `None` when they are in use.
    fn with<R>(&self, operation: impl FnOnce(&mut Parts) -> R) -> Option<R> {
        let mut parts = self.0.try_borrow_mut().ok()?;
        Some(operation(&mut parts))
    }

    /// Lets the loop take `bytecode.reload`: it was given a loader that builds candidates.
    pub(super) fn enable(&self) {
        let _ = self.with(|parts| parts.enabled = true);
    }

    /// Whether the loop takes `bytecode.reload`. A loop without a loader answers
    /// `unsupported request`, as a platform without the operation does.
    pub(super) fn enabled(&self) -> bool {
        self.with(|parts| parts.enabled).unwrap_or(false)
    }

    /// Queues the change of the admitted upload `program`.
    ///
    /// # Errors
    ///
    /// The refusal as plain text: a change is queued already, or the state is in use. The loop reads
    /// no request while a change waits, so neither happens in practice.
    pub(super) fn begin(&self, program: Vec<u8>) -> Result<(), String> {
        self.with(|parts| {
            if parts.pending.is_some() {
                return Err(String::from("an online change is already queued"));
            }
            parts.pending = Some(Pending {
                program: Some(program),
                held: None,
            });
            parts.began = true;
            Ok(())
        })
        .unwrap_or_else(|| Err(String::from("the online change state is in use")))
    }

    /// Whether the request that was answered queued a change. It is true once.
    pub(super) fn take_began(&self) -> bool {
        self.with(|parts| core::mem::take(&mut parts.began))
            .unwrap_or(false)
    }

    /// Holds `reloaded`, the reply to request `id`, until the change ends, and starts the five
    /// seconds at `now`. Returns false when no change is queued.
    pub(super) fn hold_reply(&self, id: u64, reloaded: String, now: MonotonicInstant) -> bool {
        self.with(|parts| {
            parts.pending.as_mut().is_some_and(|pending| {
                pending.held = Some(Held {
                    id,
                    reloaded,
                    queued_at: now,
                });
                true
            })
        })
        .unwrap_or(false)
    }

    /// Whether a change waits, so that its reply is the one in flight and the loop reads no request.
    pub(super) fn reply_pending(&self) -> bool {
        self.with(|parts| parts.pending.is_some()).unwrap_or(false)
    }

    /// When the change that waits for a boundary is withdrawn, if there is one.
    pub(super) fn expiry(&self) -> Option<MonotonicInstant> {
        self.with(|parts| {
            let pending = parts.pending.as_ref()?;
            pending.program.as_ref()?;
            let held = pending.held.as_ref()?;
            Some(held.queued_at.saturating_add(TimeSpan::new(TAKE_WAIT)))
        })
        .flatten()
    }

    /// Takes the upload of the queued change out of the state, at a boundary. The reply is still owed.
    pub(super) fn take_program(&self) -> Option<Vec<u8>> {
        self.with(|parts| parts.pending.as_mut()?.program.take())
            .flatten()
    }

    /// Ends the change that no boundary took within [`TAKE_WAIT`] as withdrawn: it is never applied.
    /// Returns its reply, `reload timeout`.
    pub(super) fn withdraw_expired(&self, now: MonotonicInstant) -> Option<String> {
        if self.expiry().is_some_and(|expiry| expiry <= now) {
            self.finish(ReloadOutcome::Withdrawn)
        } else {
            None
        }
    }

    /// Ends the queued change with `outcome` and returns the reply it is owed: the held `reloaded`
    /// reply when it was applied, and the shared reply of the outcome otherwise. `None` when no change
    /// is queued.
    pub(super) fn finish(&self, outcome: ReloadOutcome<()>) -> Option<String> {
        let pending = self.with(|parts| parts.pending.take()).flatten()?;
        let id = pending.held.as_ref().map_or(0, |held| held.id);
        match (outcome, pending.held) {
            (ReloadOutcome::Applied(()), Some(held)) => Some(held.reloaded),
            (outcome, _) => Some(serialized(&reload::reply(id, outcome, |()| Ok(())))),
        }
    }
}

/// The reply as one JSON object; a reply that cannot be serialized becomes the stable failure.
fn serialized(response: &ControlResponse) -> String {
    serde_json::to_string(response).unwrap_or_else(|_| {
        String::from(r#"{"id":0,"ok":false,"error":"response serialization failed"}"#)
    })
}

impl<L, W, Y, P, C> Runner<L, W, Y, P, C>
where
    L: ControlLink,
    W: RunnerWait,
    Y: ResourceCycle,
    P: RunnerPlatform,
    C: MonotonicClock + Clone,
{
    /// Applies the online change that waits, at a boundary (specification 34 §11.3). Returns whether a
    /// change was applied.
    ///
    /// A change that no boundary took within five seconds is withdrawn instead. A change that fails
    /// ends with its error and leaves the running program as it was. The loop feeds the hardware
    /// watchdog between the steps (TIME-04).
    pub(super) fn apply_online_change(&mut self) -> bool {
        self.shell.expire_change();
        let Some(program) = self.shell.change.take_program() else {
            return false;
        };
        self.shell.feed();
        let candidate = self.prepare_change(program);
        self.shell.feed();
        match candidate.and_then(|candidate| self.commit_change(candidate)) {
            Ok(()) => {
                self.shell.finish_change(ReloadOutcome::Applied(()));
                true
            }
            Err(reason) => {
                self.shell.finish_change(ReloadOutcome::Failed(reason));
                false
            }
        }
    }

    /// Has the device prepare and check the candidate of `program`, while the running program stays
    /// as it is.
    fn prepare_change(&mut self, program: Vec<u8>) -> Result<Candidate<Y>, String> {
        let Program::Installed(running) = &mut self.cycle else {
            return Err(String::from("there is no installed program to change"));
        };
        let Some(loader) = self.loader.as_mut() else {
            return Err(String::from("the loop has no program loader"));
        };
        self.shell
            .with_platform(|platform| loader.prepare_change(platform, running, program))
            .unwrap_or_else(|| Err(String::from("the loop's platform is in use")))
    }

    /// The commit: the device hands the lines to the candidate, the candidate becomes the loop's
    /// program, the logical time goes on and the scan counter restarts, and the running program is
    /// dropped last. Nothing after the hand-over can fail.
    fn commit_change(&mut self, mut candidate: Candidate<Y>) -> Result<(), String> {
        let handed = match (&mut self.cycle, self.loader.as_mut()) {
            (Program::Installed(running), Some(loader)) => self
                .shell
                .with_platform(|platform| loader.commit_change(platform, running, &mut candidate)),
            _ => None,
        };
        if handed.is_none() {
            return Err(String::from("the loop's platform is in use"));
        }
        let Candidate {
            cycle,
            live,
            events,
        } = candidate;
        let old = core::mem::replace(&mut self.cycle, Program::Installed(cycle));
        self.cycle.attach_hold(self.hold.clone());
        self.record_program_memory();
        self.tasks = self.cycle.tasks();
        self.programs = self.cycle.programs();
        self.live = live;
        // The program's time goes on from the clock, and its tasks continue the running program's
        // schedule, which the candidate took from it, so the first scan after the change runs the
        // tasks that are due (specification 34 §11.3, decision OC-2). The scan counter restarts, and
        // the retained values that were dropped or migrated are recorded at this moment.
        let now = self.logical(self.clock.now());
        self.program_time = now;
        self.cycle_count = 0;
        self.record_carried_events(events);
        // The debugger keeps its breakpoints and its pause, and forgets what belonged to the old
        // statements.
        self.shell.debug.program_replaced();
        // The program is not the installed one any more, so a stop and a `restart` load the installed
        // container (specification 34 STATE-08).
        self.shell.install.set_loaded(None);
        drop(old);
        Ok(())
    }
}
