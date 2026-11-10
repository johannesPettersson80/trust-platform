//! Hosted hang supervision (specification 11 section 6.6, "Hosted hang supervision";
//! specification 34 TIME-03).
//!
//! The scan thread publishes the active-execution deadline of the cycle it runs and withdraws it
//! when the cycle ends. A supervisor thread checks that deadline, so a driver or native callback
//! that never returns is still faulted, which the scan thread's own watchdog check cannot do.
//!
//! Active time is wall time minus debugger holds. The published deadline is therefore stored on
//! the active clock, and a hold that is in progress suspends the check, so operator dwell time
//! never counts. The pause total is the watchdog's existing accounting, read from the debug
//! control; nothing here keeps a second pause count. A check reads the clock before it asks
//! whether a hold is in progress, so a hold that begins between the two readings can only shorten
//! the active time it computes.

use std::sync::atomic::{AtomicBool, AtomicU64, Ordering};
use std::sync::mpsc::{self, RecvTimeoutError};
use std::sync::{Arc, Mutex};
use std::thread;
use std::time::{Duration as StdDuration, Instant};

use super::{recover_mutex_lock, set_last_error, set_resource_state, ResourceState};
use crate::debug::DebugControl;
use crate::error::RuntimeError;
use crate::value::Duration;
use crate::watchdog::HangAction;

/// Published deadline value meaning that no cycle is running.
const NO_CYCLE: u64 = u64::MAX;
/// Shortest and longest interval between two supervisor checks.
const MIN_CHECK_INTERVAL: StdDuration = StdDuration::from_millis(1);
const MAX_CHECK_INTERVAL: StdDuration = StdDuration::from_millis(100);
/// Process exit status of `hang_action = "abort"` (specification 11 section 6.6).
pub(super) const HANG_ABORT_EXIT_CODE: i32 = 70;

/// The supervision check interval, `clamp(timeout / 4, 1 ms, 100 ms)`.
fn check_interval_for(timeout_ns: u64) -> StdDuration {
    StdDuration::from_nanos(timeout_ns / 4).clamp(MIN_CHECK_INTERVAL, MAX_CHECK_INTERVAL)
}

fn saturating_nanos(duration: StdDuration) -> u64 {
    u64::try_from(duration.as_nanos()).unwrap_or(u64::MAX)
}

/// State shared by a resource's scan thread, its hang supervisor and its control handles.
#[derive(Debug)]
pub(super) struct ScanSupervision {
    /// Origin of every published time, so that a deadline fits in one atomic.
    epoch: Instant,
    /// Orders publish, withdraw and the supervisor's decision. The supervisor never declares a
    /// hang for a deadline that the scan thread has already withdrawn.
    gate: Mutex<()>,
    /// Active-execution deadline of the running cycle, in nanoseconds since `epoch`.
    deadline_ns: AtomicU64,
    /// Watchdog timeout that sets the check interval, in nanoseconds.
    timeout_ns: AtomicU64,
    /// Set when the supervisor declared the scan thread hung. Cleared when the blocked call
    /// returns and the scan thread withdraws its cycle, and when a recovery succeeds.
    hung: AtomicBool,
    /// Set by the control `shutdown`: the scan thread stops the program and keeps waiting in the
    /// Stopped state (specification 34, STATE-08). The scan thread takes it once per request.
    program_stop: AtomicBool,
    state: Arc<Mutex<ResourceState>>,
    last_error: Arc<Mutex<Option<RuntimeError>>>,
}

impl ScanSupervision {
    pub(super) fn new(
        state: Arc<Mutex<ResourceState>>,
        last_error: Arc<Mutex<Option<RuntimeError>>>,
    ) -> Self {
        Self {
            epoch: Instant::now(),
            gate: Mutex::new(()),
            deadline_ns: AtomicU64::new(NO_CYCLE),
            timeout_ns: AtomicU64::new(0),
            hung: AtomicBool::new(false),
            program_stop: AtomicBool::new(false),
            state,
            last_error,
        }
    }

    /// Asks the scan thread to stop the program and wait in the Stopped state.
    pub(super) fn request_program_stop(&self) {
        self.program_stop.store(true, Ordering::SeqCst);
    }

    /// Takes a pending program stop request: true once for each request.
    pub(super) fn take_program_stop(&self) -> bool {
        self.program_stop.swap(false, Ordering::SeqCst)
    }

    /// Whether a program stop request waits for the scan thread.
    #[cfg(test)]
    pub(super) fn program_stop_requested(&self) -> bool {
        self.program_stop.load(Ordering::SeqCst)
    }

    /// Whether the scan thread is inside a call that the supervisor declared hung.
    pub(super) fn is_hung(&self) -> bool {
        self.hung.load(Ordering::SeqCst)
    }

    /// Clears the hung flag. Called when a restart or an online change has succeeded.
    pub(super) fn clear_hung(&self) {
        self.hung.store(false, Ordering::SeqCst);
    }

    /// Sets the hung flag directly, for control-plane tests that have no scan thread.
    #[cfg(test)]
    pub(super) fn set_hung_for_test(&self, hung: bool) {
        self.hung.store(hung, Ordering::SeqCst);
    }

    /// Publishes the active-execution deadline of a cycle that starts at `cycle_start`.
    /// `paused_at_start` is the debugger pause total read at the same moment. A later hold moves
    /// the deadline by exactly the time it pauses the cycle, as the watchdog accounting does.
    pub(super) fn publish(
        &self,
        cycle_start: Instant,
        timeout: Duration,
        paused_at_start: StdDuration,
    ) {
        let _gate = recover_mutex_lock(self.gate.lock());
        let timeout_ns = u64::try_from(timeout.as_nanos()).unwrap_or(0);
        let start_ns = self.active_nanos(cycle_start, paused_at_start);
        self.timeout_ns.store(timeout_ns, Ordering::SeqCst);
        self.deadline_ns
            .store(start_ns.saturating_add(timeout_ns), Ordering::SeqCst);
    }

    /// Withdraws the deadline when the cycle ends. A blocked call that has returned is no longer
    /// hung; the fault it leaves behind is handled by the scan thread's watchdog action.
    pub(super) fn withdraw(&self) {
        let _gate = recover_mutex_lock(self.gate.lock());
        self.deadline_ns.store(NO_CYCLE, Ordering::SeqCst);
        self.hung.store(false, Ordering::SeqCst);
    }

    /// Active clock reading for `at`: monotonic time since `epoch` minus the debugger pause total.
    fn active_nanos(&self, at: Instant, paused: StdDuration) -> u64 {
        saturating_nanos(
            at.saturating_duration_since(self.epoch)
                .saturating_sub(paused),
        )
    }

    /// Declares the running cycle hung when it still runs one check interval after its deadline.
    /// A debugger hold in progress suspends the check. `hold_view` reads whether the scan waits
    /// in a hold and the time of the holds that ended, as [`DebugControl::watchdog_hold_view`]
    /// does. It is read after the clock: read before, a hold that began while the supervisor
    /// waited between the two readings, as a thread that is preempted does, would count as
    /// active time. `declared` holds the deadline that was already declared, so one stalled
    /// cycle is recorded once. Returns true when this call declared the hang.
    pub(super) fn declare_hang_if_overdue(
        &self,
        hold_view: impl FnOnce() -> (bool, StdDuration),
        declared: &mut Option<u64>,
    ) -> bool {
        let _gate = recover_mutex_lock(self.gate.lock());
        let deadline = self.deadline_ns.load(Ordering::SeqCst);
        if deadline == NO_CYCLE || *declared == Some(deadline) {
            return false;
        }
        let at = Instant::now();
        let (holding, paused) = hold_view();
        if holding {
            return false;
        }
        let interval = check_interval_for(self.timeout_ns.load(Ordering::SeqCst));
        let now = self.active_nanos(at, paused);
        if now <= deadline.saturating_add(saturating_nanos(interval)) {
            return false;
        }
        *declared = Some(deadline);
        self.hung.store(true, Ordering::SeqCst);
        set_last_error(&self.last_error, RuntimeError::WatchdogTimeout);
        set_resource_state(&self.state, ResourceState::Faulted);
        // The detail that the scan thread did not return is not part of the stable error
        // variant, so it is reported here rather than in `last_error`.
        eprintln!(
            "trust-runtime: scan thread did not return within its watchdog interval; resource faulted (watchdog timeout)"
        );
        true
    }

    fn check_interval(&self) -> StdDuration {
        check_interval_for(self.timeout_ns.load(Ordering::SeqCst))
    }
}

/// The supervisor thread of one resource. Dropping it stops the thread and waits for it.
#[derive(Debug)]
pub(super) struct HangSupervisor {
    stop: Option<mpsc::Sender<()>>,
    join: Option<thread::JoinHandle<()>>,
}

impl HangSupervisor {
    /// Starts supervision for a resource whose watchdog is enabled with `timeout`.
    pub(super) fn start(
        supervision: &Arc<ScanSupervision>,
        debug: Option<DebugControl>,
        hang_action: HangAction,
        timeout: Duration,
    ) -> Result<Self, RuntimeError> {
        // The first check interval follows the policy; each cycle republishes the timeout.
        supervision.timeout_ns.store(
            u64::try_from(timeout.as_nanos()).unwrap_or(0),
            Ordering::SeqCst,
        );
        let (stop_tx, stop_rx) = mpsc::channel::<()>();
        let supervision = Arc::clone(supervision);
        let join = thread::Builder::new()
            .name("resource-hang-supervisor".to_string())
            .spawn(move || {
                let mut declared = None;
                loop {
                    match stop_rx.recv_timeout(supervision.check_interval()) {
                        Err(RecvTimeoutError::Timeout) => {}
                        Ok(()) | Err(RecvTimeoutError::Disconnected) => return,
                    }
                    let hold_view = || {
                        debug
                            .as_ref()
                            .map_or((false, StdDuration::ZERO), DebugControl::watchdog_hold_view)
                    };
                    if supervision.declare_hang_if_overdue(hold_view, &mut declared)
                        && hang_action == HangAction::Abort
                    {
                        // Deliberate: the scan thread may still hold driver state, and no
                        // destructors run. A service manager restarts the process.
                        std::process::exit(HANG_ABORT_EXIT_CODE);
                    }
                }
            })
            .map_err(|err| RuntimeError::ThreadSpawn(err.to_string().into()))?;
        Ok(Self {
            stop: Some(stop_tx),
            join: Some(join),
        })
    }
}

impl Drop for HangSupervisor {
    fn drop(&mut self) {
        // Dropping the sender disconnects the channel, which ends the supervisor thread.
        self.stop = None;
        if let Some(join) = self.join.take() {
            let _ = join.join();
        }
    }
}

/// Keeps the hang supervisor running exactly while the watchdog is enabled.
pub(super) fn sync_hang_supervisor<C: super::Clock + Clone>(
    slot: &mut Option<HangSupervisor>,
    runner: &super::ResourceRunner<C>,
    supervision: &Arc<ScanSupervision>,
) -> Result<(), RuntimeError> {
    let policy = runner.program().watchdog_policy();
    match (policy.enabled, slot.is_some()) {
        (true, false) => {
            let started = HangSupervisor::start(
                supervision,
                runner.program().debug_control(),
                runner.hang_action,
                policy.timeout,
            )?;
            *slot = Some(started);
        }
        (false, true) => *slot = None,
        _ => {}
    }
    Ok(())
}

#[cfg(test)]
#[path = "supervisor_tests.rs"]
mod tests;
