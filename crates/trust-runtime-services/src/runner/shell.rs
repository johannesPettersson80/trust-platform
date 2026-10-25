//! What the resource loop shares with a scan that a debugger holds (specification 34, STATE-07,
//! STATE-08, PORT-05 and TIME-04).
//!
//! While the debugger holds a scan, the scan's native frames stay on the stack, inside the engine,
//! and the loop's [`run_pass`](crate::runner::Runner::run_pass) cannot run. The hold runs a shorter
//! pass of its own (`hold.rs`) that serves the control link, does the platform's idle work, feeds the
//! hardware watchdog and waits. Both passes work on the same link, wait, platform, receive buffer,
//! resource state and records, so those live here, in one [`Shell`] that the loop and the device
//! services' statement hook share through an `Rc`.
//!
//! # Borrows
//!
//! The loop holds its program (`Program`) by `&mut` across `run_scan`, as a plain Rust borrow that
//! the compiler checks, and nothing else: the hold never reaches the program. The one thing it needs
//! from it, the health of the process I/O, is published by the cycle into the device services
//! before the program runs.
//!
//! Everything that is shared sits in the [`Shell`]. Each `RefCell` of it is borrowed by one leaf
//! operation at a time, for the length of one call to a port, and no operation calls another
//! operation while it holds a borrow. So no borrow is held across `run_scan`, across the engine's
//! statement hook or across the hold: the loop holds none when it calls the cycle, and the hold holds
//! none when it returns. A request that a hold serves runs on a `RunnerHost`, which holds a shared
//! borrow of the platform and of the receive buffer only while the dispatcher answers it, and the
//! dispatcher never starts a scan.
//!
//! A cell that is nevertheless in use is a defect, not a state to wait out. The operation that finds
//! it does nothing, raises the shell's fault ([`Shell::take_fault`]), and the loop faults the
//! resource with `InvalidExecutionState` at its next check. A `RefCell` panic would be a firmware
//! fault on a device (specification 34, STATE-05).
//!
//! # An install in progress
//!
//! The request that begins a stepped install (`program.install`, specification 34 §11.2) is answered
//! when its commit ends, not when the request is served. [`Shell::serve`] holds the reply the
//! dispatcher built, [`Shell::step_install`] advances the commit by one step in each pass and writes
//! the reply when it ends, and [`Shell::next_frame`] hands the loop no request in between: the reply
//! is the one reply in flight (specification 34, PORT-05).
//!
//! # An online change in progress
//!
//! The request that queues an online change (`bytecode.reload`, specification 34 §11.3) is answered
//! the same way: [`Shell::serve`] holds the reply the dispatcher built, the loop writes it when the
//! change is applied (or the failure that ended it), and [`Shell::next_frame`] hands the loop no
//! request in between. The five seconds that a change waits for a boundary are checked by
//! [`Shell::expire_change`], which the loop and a debugger's hold both call, so that a scan that the
//! debugger holds does not keep the reply of a withdrawn change from being written.

use alloc::string::{String, ToString};
use alloc::vec;
use alloc::vec::Vec;
use core::cell::{Cell, RefCell};

use serde::Deserialize;
use trust_runtime_core::deployment::DeviceDescriptor;
use trust_runtime_core::ports::{
    ControlLink, HardwareWatchdog, LinkFrame, MonotonicClock, MonotonicInstant, PeerClass,
    PortError, RunnerWait, TimeSpan, WakeReason, WakeToken,
};
use trust_runtime_core::scheduler::ResourceState;

use super::config::ConfigState;
use super::debugger::ChipDebug;
use super::host::{ProgramView, RunnerHost};
use super::install::{InstallState, Step};
use super::online_change::ChangeState;
use super::state::{Records, ResourceCell};
use super::stepping::StepState;
use super::{RunnerPlatform, REPLY_SERVICE_INTERVAL};
use crate::dispatch::{invalid_line, prepare_line_within, reply_within, PreparedLine};
use crate::loading::reload::ReloadOutcome;
use crate::ports::{AccessPort, StatusPort};
use crate::protocol::{ControlAfterResponse, ControlResponse};
use crate::transfer::TransferSession;

/// How long after the reply of a `firmware.install` the chip resets, so that the host reads the
/// reply's last bytes from the link first (specification 34 TARGET-04).
const RESET_GRACE: TimeSpan = TimeSpan::new(core::time::Duration::from_millis(100));

/// The link, wait, platform and receive buffer of the loop, with the resource state, the records
/// and the debugger, which the loop and a hold on a scan share.
pub(super) struct Shell<L, W, P, C> {
    link: RefCell<L>,
    wait: RefCell<W>,
    platform: RefCell<P>,
    /// The receive buffer of [`ControlLink::max_frame_len`] bytes.
    frame: RefCell<Vec<u8>>,
    /// The size of the buffer, and of the longest reply the link moves.
    max_frame_len: usize,
    /// A request taken from the link and held back because the next boundary was due; the next
    /// pass answers it first.
    held: Cell<Option<LinkFrame>>,
    /// A `shutdown` that waits for the program's next boundary.
    stop_requested: Cell<bool>,
    /// A cell was found in use (see the module documentation).
    fault: Cell<bool>,
    /// The time from which the chip resets into the firmware a `firmware.install` selected, once the
    /// link has sent the reply (specification 34 TARGET-04). The loop takes no request meanwhile.
    reset_after: Cell<Option<MonotonicInstant>>,
    /// The wait failed inside a hold: the run ends with this failure at its next pass.
    port_failure: Cell<Option<PortError>>,
    pub(super) resource: ResourceCell,
    pub(super) records: Records<C>,
    pub(super) debug: ChipDebug,
    /// The transfers, the program store and the install that runs.
    pub(super) install: InstallState,
    /// The online change that waits for the program's next boundary.
    pub(super) change: ChangeState,
    /// The stepped scan whose cold restart loads a newer program, deferred to the loop.
    pub(super) step: StepState,
    /// The change of a `config.set` and the scheduler's copies it left for the loop.
    pub(super) config: ConfigState,
    pub(super) clock: C,
}

impl<L, W, P, C> Shell<L, W, P, C>
where
    L: ControlLink,
    W: RunnerWait,
    P: RunnerPlatform,
    C: MonotonicClock,
{
    /// The shell of a loop over `link`, `wait` and `platform`, with a receive buffer of the link's
    /// frame length and a debugger whose buffer of stops takes `stop_capacity` stops.
    pub(super) fn new(
        link: L,
        wait: W,
        platform: P,
        clock: C,
        resource: ResourceCell,
        records: Records<C>,
        stop_capacity: usize,
    ) -> Self {
        let max_frame_len = link.max_frame_len();
        Self {
            link: RefCell::new(link),
            wait: RefCell::new(wait),
            platform: RefCell::new(platform),
            frame: RefCell::new(vec![0; max_frame_len]),
            max_frame_len,
            held: Cell::new(None),
            stop_requested: Cell::new(false),
            fault: Cell::new(false),
            reset_after: Cell::new(None),
            port_failure: Cell::new(None),
            resource,
            records,
            debug: ChipDebug::new(stop_capacity),
            install: InstallState::default(),
            change: ChangeState::default(),
            step: StepState::default(),
            config: ConfigState::default(),
            clock,
        }
    }

    /// Runs `operation` on the value in `cell`, or raises the fault and runs nothing.
    fn read<T, R>(&self, cell: &RefCell<T>, operation: impl FnOnce(&T) -> R) -> Option<R> {
        if let Ok(value) = cell.try_borrow() {
            Some(operation(&value))
        } else {
            self.fault.set(true);
            None
        }
    }

    /// Runs `operation` on the value in `cell`, or raises the fault and runs nothing.
    fn update<T, R>(&self, cell: &RefCell<T>, operation: impl FnOnce(&mut T) -> R) -> Option<R> {
        if let Ok(mut value) = cell.try_borrow_mut() {
            Some(operation(&mut value))
        } else {
            self.fault.set(true);
            None
        }
    }

    /// Whether a cell was found in use, or the debugger's state was, since the last call. The loop
    /// faults the resource when it was; a hold fails its scan.
    pub(super) fn take_fault(&self) -> bool {
        let debug_busy = self.debug.take_busy();
        self.fault.take() || debug_busy
    }

    /// The failure of a wait that failed inside a hold, if one did since the last call.
    pub(super) fn take_port_failure(&self) -> Option<PortError> {
        self.port_failure.take()
    }

    /// Records the failure of a wait that failed inside a hold.
    pub(super) fn set_port_failure(&self, failure: PortError) {
        self.port_failure.set(Some(failure));
    }

    /// The wake token to read before a pass checks for work (PORT-03).
    pub(super) fn wake_token(&self) -> WakeToken {
        self.read(&self.wait, RunnerWait::wake_token)
            .unwrap_or(WakeToken::new(0))
    }

    /// Waits until `deadline` on the monotonic clock or a wake after `token` was read (PORT-03).
    ///
    /// # Errors
    ///
    /// The [`PortError`] of a wait that failed.
    pub(super) fn wait_until(
        &self,
        deadline: MonotonicInstant,
        token: WakeToken,
    ) -> Result<WakeReason, PortError> {
        self.update(&self.wait, |wait| wait.wait_until(deadline, token))
            // A wait that cannot start ends at once, and the next check of the loop faults.
            .unwrap_or(Ok(WakeReason::Event))
    }

    /// The platform's idle work, which must not wait (specification 34, O1: it also runs while a
    /// hold waits, as trust-runtime's communication drivers keep running).
    pub(super) fn idle(&self) {
        self.update(&self.platform, RunnerPlatform::idle);
    }

    /// When the platform's idle work is next due, at the earliest a millisecond after `now`
    /// (specification 34 ARCH-12).
    pub(super) fn idle_due(&self, now: MonotonicInstant) -> Option<MonotonicInstant> {
        self.read(&self.platform, RunnerPlatform::idle_due)
            .flatten()
            .map(|due| due.max(now.saturating_add(REPLY_SERVICE_INTERVAL)))
    }

    /// Feeds the platform's hardware watchdog (TIME-04).
    pub(super) fn feed(&self) {
        self.update(&self.platform, HardwareWatchdog::feed);
    }

    /// Runs `operation` on the platform, for the load of a program in place (specification 34
    /// STATE-08): the device's loader sets the values of the new record there. `None`, with the
    /// shell's fault raised, when the platform is in use, which the call structure rules out.
    pub(super) fn with_platform<R>(&self, operation: impl FnOnce(&mut P) -> R) -> Option<R> {
        self.update(&self.platform, operation)
    }

    /// The device descriptor admission reads, from the platform.
    pub(super) fn device_descriptor(&self) -> Option<DeviceDescriptor> {
        self.read(&self.platform, StatusPort::device_descriptor)
    }

    /// The longest wait between two feeds of the hardware watchdog.
    pub(super) fn feed_interval(&self) -> TimeSpan {
        self.read(&self.platform, HardwareWatchdog::feed_interval)
            .unwrap_or_default()
    }

    /// Whether the link still holds reply bytes it has not handed to its hardware (PORT-05).
    pub(super) fn link_sending(&self) -> bool {
        self.read(&self.link, ControlLink::sending).unwrap_or(false)
    }

    /// Whether the platform enables the debug surface (specification 11 §6.9).
    pub(super) fn debug_enabled(&self) -> bool {
        self.read(&self.platform, AccessPort::debug_enabled)
            .unwrap_or(false)
    }

    /// Whether a `shutdown` waits for the program's next boundary.
    pub(super) fn stop_requested(&self) -> bool {
        self.stop_requested.get()
    }

    /// Sets whether a `shutdown` waits for the program's next boundary.
    pub(super) fn set_stop_requested(&self, requested: bool) {
        self.stop_requested.set(requested);
    }

    /// The request to answer next: the one held back for a due boundary, which goes first, else the
    /// next frame the link has received. None while an install's or an online change's reply is
    /// pending: that reply is the one reply in flight, so the loop reads no request until it is
    /// written (specification 34, PORT-05). The requests that arrive meanwhile wait in the link.
    pub(super) fn next_frame(&self) -> Option<LinkFrame> {
        if self.install.reply_pending() || self.change.reply_pending() || self.resetting() {
            return None;
        }
        if let Some(held) = self.held.take() {
            return Some(held);
        }
        let (Ok(mut link), Ok(mut frame)) =
            (self.link.try_borrow_mut(), self.frame.try_borrow_mut())
        else {
            self.fault.set(true);
            return None;
        };
        link.poll_frame(&mut frame)
    }

    /// Holds `frame` back: the next pass answers it first.
    pub(super) fn hold_back(&self, frame: LinkFrame) {
        self.held.set(Some(frame));
    }

    /// Answers `received` and sends the reply when it fits a frame, and returns the work the reply
    /// leaves. A reply the link cannot deliver is lost, as trust-runtime loses a reply to a
    /// connection that has closed; so is one that does not fit a frame even as the error that
    /// replaces a longer reply.
    ///
    /// A request that began a stepped install is not answered yet: the reply the dispatcher built is
    /// held, and [`Self::step_install`] writes it when the commit ends (specification 34 §11.2).
    pub(super) fn serve(
        &self,
        received: LinkFrame,
        program: &dyn ProgramView,
    ) -> Option<ControlAfterResponse> {
        let prepared = self.answer(received, program);
        // A `config.set` that every check passed changes the platform before its reply is written.
        if let Some(commit) = self.config.take_commit() {
            self.update(&self.platform, |platform| commit.apply(platform));
        }
        let prepared = prepared?;
        if self.install.take_began() {
            if !self
                .install
                .hold_reply(self.request_id(received), prepared.response)
            {
                self.fault.set(true);
            }
            return prepared.after_response;
        }
        if self.change.take_began() {
            if !self.change.hold_reply(
                self.request_id(received),
                prepared.response,
                self.clock.now(),
            ) {
                self.fault.set(true);
            }
            return prepared.after_response;
        }
        if let Some(request) = self.step.take_deferred() {
            // The loop loads the program and writes the step's reply after this pass (stepping.rs).
            self.step.hold(self.request_id(received), request);
            return prepared.after_response;
        }
        self.send(&prepared.response);
        // A `device.reset` resets the device once its reply is sent (specification 34 TARGET-02).
        if self.install.take_reset_request() {
            self.reset_after
                .set(Some(self.clock.now().saturating_add(RESET_GRACE)));
        }
        prepared.after_response
    }

    /// Writes `response`, the reply to a request that the loop finished after the request's pass,
    /// as the dispatcher writes a reply: one frame, or the error that replaces a reply too long for
    /// it.
    pub(super) fn deliver(&self, response: ControlResponse, program: &dyn ProgramView) {
        let Ok(platform) = self.platform.try_borrow() else {
            self.fault.set(true);
            return;
        };
        let host = RunnerHost {
            platform: &*platform,
            program,
            resource: &self.resource,
            records: &self.records,
            debug: &self.debug,
            install: &self.install,
            change: &self.change,
            step: &self.step,
            config: &self.config,
        };
        let prepared = reply_within(response, self.max_frame_len, &host);
        self.send(&prepared.response);
    }

    /// Sends `response`, one reply, when it fits a frame.
    fn send(&self, response: &str) {
        if response.len() <= self.max_frame_len {
            self.update(&self.link, |link| {
                let _ = link.send_frame(response.as_bytes());
            });
        }
    }

    /// The request number of the request line `received`, which the dispatcher has read as a request:
    /// the number its reply carries. A line that cannot be read again has number 0, as a request that
    /// cannot be read does (specification 11 §6.8.1).
    fn request_id(&self, received: LinkFrame) -> u64 {
        let LinkFrame::Frame { len, .. } = received else {
            return 0;
        };
        let Ok(frame) = self.frame.try_borrow() else {
            self.fault.set(true);
            return 0;
        };
        frame
            .get(..len)
            .and_then(|bytes| core::str::from_utf8(bytes).ok())
            .and_then(|line| serde_json::from_str::<RequestNumber>(line).ok())
            .map_or(0, |request| request.id)
    }

    /// Advances the install that runs by one step, and writes its reply when the commit ends: the
    /// `installed` reply, or the failure of the step that failed (specification 34 §11.2).
    pub(super) fn step_install(&self) {
        match self.install.step() {
            Step::Idle | Step::Running => {}
            Step::Finished(Some(reply)) => self.send(&reply),
            Step::FinishedReset(Some(reply)) => {
                self.send(&reply);
                self.reset_after
                    .set(Some(self.clock.now().saturating_add(RESET_GRACE)));
            }
            Step::Finished(None) | Step::FinishedReset(None) | Step::InUse => {
                self.fault.set(true);
            }
        }
    }

    /// Whether the chip resets into a new firmware once its reply is sent.
    pub(super) fn resetting(&self) -> bool {
        self.reset_after.get().is_some()
    }

    /// Resets the chip into the firmware a `firmware.install` selected, once the link has handed its
    /// reply to the hardware and the grace for the host to read it has passed (specification 34
    /// TARGET-04).
    pub(super) fn reset_when_sent(&self) {
        let Some(after) = self.reset_after.get() else {
            return;
        };
        if !self.link_sending() && self.clock.now() >= after {
            self.update(&self.platform, RunnerPlatform::reset_device);
        }
    }

    /// Whether an install's reply is pending: its commit runs, and the loop reads no request.
    pub(super) fn install_pending(&self) -> bool {
        self.install.reply_pending()
    }

    /// Whether the loop's transfer session holds a complete image upload whose hash is still owed
    /// (specification 44 §5.11): the loop holds the requests back until it has hashed it.
    pub(super) fn hash_owed(&self) -> bool {
        self.install
            .with_transfers(|session| session.hash_owed())
            .unwrap_or(false)
    }

    /// Whether the loop's image upload has received bytes to hash (specification 44 §5.11).
    pub(super) fn hash_work(&self) -> bool {
        self.install
            .with_transfers(|session| session.hash_work())
            .unwrap_or(false)
    }

    /// Hashes the next block of the loop's image upload between scans, and returns whether there
    /// was one (specification 44 §5.11).
    pub(super) fn hash_step(&self) -> bool {
        self.install
            .with_transfers(TransferSession::hash_step)
            .unwrap_or(false)
    }

    /// Withdraws the online change that no boundary took within five seconds, and writes its reply,
    /// `reload timeout` (specification 11 §6.7.1; specification 34 §11.3). The change is never
    /// applied.
    pub(super) fn expire_change(&self) {
        // The clock is read only while a change waits for a boundary (checklist P1-6).
        if self.change.expiry().is_none() {
            return;
        }
        if let Some(reply) = self.change.withdraw_expired(self.clock.now()) {
            self.send(&reply);
        }
    }

    /// Ends the online change that waits with `outcome` and writes its reply: `reloaded` when it was
    /// applied, and the failure otherwise. Does nothing when no change waits.
    pub(super) fn finish_change(&self, outcome: ReloadOutcome<()>) {
        if let Some(reply) = self.change.finish(outcome) {
            self.send(&reply);
        }
    }

    /// The reply to one received frame, at most one frame long. A frame longer than the link
    /// accepts, or one that is not UTF-8 text, cannot be a request line and gets
    /// `invalid request` with id 0.
    fn answer(&self, received: LinkFrame, program: &dyn ProgramView) -> Option<PreparedLine> {
        let (Ok(platform), Ok(frame)) = (self.platform.try_borrow(), self.frame.try_borrow())
        else {
            self.fault.set(true);
            return None;
        };
        let max_len = self.max_frame_len;
        let LinkFrame::Frame { len, peer } = received else {
            return Some(too_long(max_len));
        };
        let Some(bytes) = frame.get(..len) else {
            return Some(too_long(max_len));
        };
        let line = match core::str::from_utf8(bytes) {
            Ok(line) => line,
            Err(error) => return Some(invalid_line(&error.to_string())),
        };
        let client = client_label(peer);
        let host = RunnerHost {
            platform: &*platform,
            program,
            resource: &self.resource,
            records: &self.records,
            debug: &self.debug,
            install: &self.install,
            change: &self.change,
            step: &self.step,
            config: &self.config,
        };
        Some(prepare_line_within(line, &host, client.as_deref(), max_len))
    }

    /// What a served request leaves for the loop: a `shutdown` waits for the program's next
    /// boundary, and ends the debugger's holds at once, so a hold needs no `resume` before the stop
    /// (specification 11 §6.9, "Operations while the debugger holds the scan"). The explicit exit
    /// ends a computer's process; a device's firmware never ends, so it answers the exit as
    /// `shutdown` (specification 34, STATE-08).
    pub(super) fn note_reply(&self, after_response: Option<ControlAfterResponse>) {
        let stops = matches!(
            after_response,
            Some(ControlAfterResponse::StopResource | ControlAfterResponse::ExitRuntime)
        );
        if !stops {
            return;
        }
        if !self.resource.ended() {
            self.stop_requested.set(true);
            self.debug.release_for_stop();
        } else if self.resource.state() == ResourceState::Faulted {
            // A start that failed left the resource Faulted with no program running, and the
            // outputs as the stop before it left them: the stop leaves it Stopped and keeps the
            // error until the next start (specification 34 STATE-08).
            self.resource.set_state(ResourceState::Stopped);
        }
    }

    /// Whether no cell of the shell is borrowed, for the tests that prove no borrow outlives an
    /// operation.
    #[cfg(test)]
    pub(super) fn cells_free(&self) -> bool {
        self.link.try_borrow_mut().is_ok()
            && self.wait.try_borrow_mut().is_ok()
            && self.platform.try_borrow_mut().is_ok()
            && self.frame.try_borrow_mut().is_ok()
            && self.debug.is_free()
    }
}

/// The request number of a request line: the one member of the request envelope that the reply of an
/// install needs again, because the reply is written after the request is served.
#[derive(Deserialize)]
struct RequestNumber {
    id: u64,
}

fn too_long(max_len: usize) -> PreparedLine {
    invalid_line(&alloc::format!("request longer than {max_len} bytes"))
}

/// The transport label the access rules read: none for a local client (USB counts as local,
/// like trust-runtime's local socket), the socket address for a network client, as trust-runtime
/// labels a TCP client.
fn client_label(peer: PeerClass) -> Option<String> {
    match peer {
        PeerClass::Local => None,
        PeerClass::Network(address) => Some(address.to_string()),
    }
}
