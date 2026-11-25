//! The clock of a simulator for machine tests (specification 11 §6.8.1, "Simulator for machine
//! tests"; specification 48 RU-58): a logical time that only the test moves. The scan loop holds
//! before each cycle until the test allows the cycle's time, and a queued request wakes it once, so
//! that it answers between the cycles while the time is held.

use std::sync::{Arc, Condvar, Mutex, MutexGuard, PoisonError};

use trust_runtime_services::simulator_advance::{Advance, AdvanceRefusal, Advanced};

use crate::value::Duration;

#[cfg(test)]
mod tests;

/// How long an advance waits for one cycle of the scan: a scan that takes longer hangs.
const CYCLE_PATIENCE: std::time::Duration = std::time::Duration::from_secs(60);

/// How often a waiting advance looks whether the resource still runs.
const LOOK: std::time::Duration = std::time::Duration::from_millis(50);

#[derive(Debug)]
struct State {
    /// The logical time.
    now: Duration,
    /// The latest time at which the scan loop may run a cycle, which the test moves; none before
    /// the first cycle.
    allowed: Option<Duration>,
    /// The time the scan loop sleeps until, while it sleeps.
    sleeping_until: Option<Duration>,
    /// A wake that ends the loop's current or next sleep once, so that it answers a request.
    wake: bool,
    /// The sleeps the scan loop began: one before each cycle, and one after each wake.
    sleeps: u64,
    /// The sleeps the loop had begun when the test allowed its last cycle.
    sleeps_at_allow: u64,
}

/// A simulator's clock: a logical time that starts at 0 and that only [`SteppedClock::advance`]
/// moves.
#[derive(Debug, Clone)]
pub struct SteppedClock {
    inner: Arc<(Mutex<State>, Condvar)>,
}

impl Default for SteppedClock {
    fn default() -> Self {
        Self::new()
    }
}

impl SteppedClock {
    /// A clock at the logical time 0 that allows no cycle yet.
    #[must_use]
    pub fn new() -> Self {
        Self {
            inner: Arc::new((
                Mutex::new(State {
                    now: Duration::ZERO,
                    allowed: None,
                    sleeping_until: None,
                    wake: false,
                    sleeps: 0,
                    sleeps_at_allow: 0,
                }),
                Condvar::new(),
            )),
        }
    }

    fn state(&self) -> MutexGuard<'_, State> {
        self.inner.0.lock().unwrap_or_else(PoisonError::into_inner)
    }

    /// The logical time.
    #[must_use]
    pub fn now(&self) -> Duration {
        self.state().now
    }

    /// Whether the test allowed a cycle at `time`.
    pub(super) fn may_run_at(&self, time: Duration) -> bool {
        self.state()
            .allowed
            .is_some_and(|allowed| time.as_nanos() <= allowed.as_nanos())
    }

    /// Sleeps until the test allows a cycle at `deadline`, which moves the time to it, or until a
    /// wake.
    pub(super) fn sleep_until(&self, deadline: Duration) {
        let (_, condvar) = &*self.inner;
        let mut state = self.state();
        state.sleeping_until = Some(deadline);
        state.sleeps = state.sleeps.wrapping_add(1);
        condvar.notify_all();
        loop {
            if state.wake {
                state.wake = false;
                break;
            }
            if state
                .allowed
                .is_some_and(|allowed| deadline.as_nanos() <= allowed.as_nanos())
            {
                if state.now.as_nanos() < deadline.as_nanos() {
                    state.now = deadline;
                }
                break;
            }
            state = condvar.wait(state).unwrap_or_else(PoisonError::into_inner);
        }
        state.sleeping_until = None;
        condvar.notify_all();
    }

    /// Ends the scan loop's current or next sleep once, so that it answers a queued request.
    pub(super) fn wake(&self) {
        self.state().wake = true;
        self.inner.1.notify_all();
    }

    /// Runs the cycles of `advance` (`simulator.advance`): lets the scan loop run at each next
    /// boundary in turn while `runs` holds, and moves the time to each, then, for a duration, to its
    /// end. A cycle is over when the loop sleeps again past it or leaves running. When `runs` stops
    /// holding during the advance, as at a fault, the advance ends at its last cycle and the rest of
    /// a duration does not pass; a resource that runs no cycles from the start, as one a halting
    /// fault left faulted, runs none while a duration's time passes (specification 11 §6.8.1,
    /// "Simulator for machine tests").
    ///
    /// # Errors
    ///
    /// [`AdvanceRefusal::Unavailable`] when the loop neither reaches its next boundary nor finishes
    /// a cycle within a minute: a scan that hangs.
    pub fn advance(
        &self,
        advance: Advance,
        runs: &dyn Fn() -> bool,
    ) -> Result<Advanced, AdvanceRefusal> {
        let end = match advance {
            Advance::Duration(duration) => Some(Duration::from_nanos(
                self.now().as_nanos().saturating_add(duration.as_nanos()),
            )),
            Advance::Cycles(_) => None,
        };
        let running = runs();
        let mut cycles = 0_u64;
        loop {
            if let Advance::Cycles(count) = advance
                && cycles >= count
            {
                break;
            }
            if !runs() {
                break;
            }
            let Some(next) = self.wait(runs, |state| state.sleeping_until)? else {
                break;
            };
            if end.is_some_and(|end| next.as_nanos() > end.as_nanos()) {
                break;
            }
            self.allow(next);
            self.wait(runs, |state| {
                state
                    .sleeping_until
                    .filter(|until| until.as_nanos() > next.as_nanos())
            })?;
            cycles += 1;
        }
        let ended_early = running && !runs();
        if let Some(end) = end
            && !ended_early
        {
            let mut state = self.state();
            if state.now.as_nanos() < end.as_nanos() {
                state.now = end;
            }
        }
        Ok(Advanced {
            time: self.now(),
            cycles,
        })
    }

    /// Allows the cycle at `time` and moves the time to it.
    fn allow(&self, time: Duration) {
        let mut state = self.state();
        state.allowed = Some(time);
        state.sleeps_at_allow = state.sleeps;
        if state.now.as_nanos() < time.as_nanos() {
            state.now = time;
        }
        drop(state);
        self.inner.1.notify_all();
    }

    /// Waits until the scan loop has done what follows the last cycle the test allowed: it sleeps
    /// again, as after a fault whose policy restarts the program, or `active` no longer holds, as
    /// when a halt leaves the resource faulted. An advance that a fault ended answers so with the
    /// state the fault policy left (specification 11 §6.8.1, "Simulator for machine tests").
    ///
    /// # Errors
    ///
    /// [`AdvanceRefusal::Unavailable`] when the loop does neither within a minute.
    pub fn settle(&self, active: &dyn Fn() -> bool) -> Result<(), AdvanceRefusal> {
        self.wait(active, |state| {
            (state.sleeps != state.sleeps_at_allow).then_some(state.now)
        })
        .map(drop)
    }

    /// Waits until `done` holds, at most a minute: a fault that reaches the events through their
    /// own thread, after the scan loop has applied the fault policy.
    ///
    /// # Errors
    ///
    /// [`AdvanceRefusal::Unavailable`] when `done` does not hold within a minute.
    pub fn wait_until(&self, done: &dyn Fn() -> bool) -> Result<(), AdvanceRefusal> {
        self.wait(&|| !done(), |_| None).map(drop)
    }

    /// Waits until `ready` finds a time in the clock's state: none once `runs` no longer holds.
    fn wait(
        &self,
        runs: &dyn Fn() -> bool,
        ready: impl Fn(&State) -> Option<Duration>,
    ) -> Result<Option<Duration>, AdvanceRefusal> {
        let started = std::time::Instant::now();
        loop {
            {
                let state = self.state();
                if let Some(time) = ready(&state) {
                    return Ok(Some(time));
                }
                let (state, _) = self
                    .inner
                    .1
                    .wait_timeout(state, LOOK)
                    .unwrap_or_else(PoisonError::into_inner);
                if let Some(time) = ready(&state) {
                    return Ok(Some(time));
                }
            }
            if !runs() {
                return Ok(None);
            }
            if started.elapsed() >= CYCLE_PATIENCE {
                return Err(AdvanceRefusal::Unavailable(format!(
                    "the scan did not finish a cycle within {} s",
                    CYCLE_PATIENCE.as_secs()
                )));
            }
        }
    }
}
