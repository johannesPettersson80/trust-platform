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

use alloc::string::{String, ToString};
use alloc::vec;
use alloc::vec::Vec;
use core::cell::{Cell, RefCell};

use trust_runtime_core::ports::{
    ControlLink, HardwareWatchdog, LinkFrame, MonotonicClock, MonotonicInstant, PeerClass,
    PortError, RunnerWait, TimeSpan, WakeReason, WakeToken,
};

use super::debugger::ChipDebug;
use super::host::{ProgramView, RunnerHost};
use super::state::{Records, ResourceCell};
use super::RunnerPlatform;
use crate::dispatch::{invalid_line, prepare_line_within, PreparedLine};
use crate::ports::AccessPort;
use crate::protocol::ControlAfterResponse;

/// What a test does at each pass of a hold with the halted state that the statement hook lent: it
/// reads the state as a debug request will.
#[cfg(test)]
pub(super) type HoldProbe = dyn for<'v> Fn(&super::hold::HeldView<'v>);

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
    /// The wait failed inside a hold: the run ends with this failure at its next pass.
    port_failure: Cell<Option<PortError>>,
    pub(super) resource: ResourceCell,
    pub(super) records: Records<C>,
    pub(super) debug: ChipDebug,
    pub(super) clock: C,
    #[cfg(test)]
    pub(super) probe: RefCell<Option<alloc::boxed::Box<HoldProbe>>>,
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
            port_failure: Cell::new(None),
            resource,
            records,
            debug: ChipDebug::new(stop_capacity),
            clock,
            #[cfg(test)]
            probe: RefCell::new(None),
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

    /// Feeds the platform's hardware watchdog (TIME-04).
    pub(super) fn feed(&self) {
        self.update(&self.platform, HardwareWatchdog::feed);
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
    /// next frame the link has received.
    pub(super) fn next_frame(&self) -> Option<LinkFrame> {
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
    pub(super) fn serve(
        &self,
        received: LinkFrame,
        program: &dyn ProgramView,
    ) -> Option<ControlAfterResponse> {
        let prepared = self.answer(received, program)?;
        if prepared.response.len() <= self.max_frame_len {
            self.update(&self.link, |link| {
                let _ = link.send_frame(prepared.response.as_bytes());
            });
        }
        prepared.after_response
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
        if stops && !self.resource.ended() {
            self.stop_requested.set(true);
            self.debug.release_for_stop();
        }
    }

    /// Calls `probe` at each pass of a hold with the halted state that the hook lent to it.
    #[cfg(test)]
    pub(super) fn set_probe(&self, probe: impl for<'v> Fn(&super::hold::HeldView<'v>) + 'static) {
        *self.probe.borrow_mut() = Some(alloc::boxed::Box::new(probe));
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
