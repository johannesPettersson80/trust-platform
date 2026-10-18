//! The resource loop of a device without an operating system (specification 34, STATE-08 and
//! ARCH-11).
//!
//! One loop serves the control link between scans and while stopped, runs the installed
//! program's scan when it is due, does the platform's idle work, feeds the hardware watchdog and
//! then waits until the program's next boundary or a wake. It owns the resource state machine of
//! trust-runtime's scheduler — Running, Paused, Stopped and Faulted — and follows the shared rules
//! at the same points: the operations of [`crate::resource::operations`], the supervisor's
//! decisions of [`crate::resource::supervisor`] and the command rules of
//! [`crate::resource::commands`]. The platform adapter supplies the link, the wait and wake, the
//! clock, the installed program ([`ResourceCycle`]; [`engine`] runs it on the shared engine over
//! the adapter's process I/O) and its own values and watchdog ([`RunnerPlatform`]).
//!
//! A device without an installed program runs the same loop without one ([`Runner::stopped`]):
//! the resource is Stopped, the loop serves the link and feeds the hardware watchdog, and
//! `restart` is refused with the reason until a program is installed.
//!
//! Requests take effect where trust-runtime's scan thread takes them. The loop answers each
//! request at once, unless the program's next boundary is due: then it takes that boundary first,
//! so a due cycle waits at most for the request already being answered (STATE-08). A `pause`,
//! `resume`, `restart` or `shutdown` then waits for the program's next boundary, as
//! trust-runtime's scan thread takes it at its next pass: while running, when the next scan is
//! due; while paused, one cycle interval after the last boundary; while faulted, at
//! the end of each poll period of the faulted wait. At a boundary the stop comes first, then the
//! queued pause and resume, then a pending restart, and then the scan; at a poll of the faulted
//! wait a restart comes before the stop. `shutdown` stops the program as trust-runtime stops it,
//! and the device stays reachable and keeps serving the link; a `restart` of a program that has
//! ended takes effect at once and starts it as a new trust-runtime process would.

mod cycle;
pub mod engine;
mod host;
mod observer;
mod program;
mod retain;
mod state;
mod supervision;

use alloc::format;
use alloc::string::{String, ToString};
use alloc::vec;
use alloc::vec::Vec;

use smol_str::SmolStr;
use trust_runtime_core::error::RuntimeError;
use trust_runtime_core::ports::{
    ControlLink, LinkFrame, MonotonicClock, MonotonicInstant, PeerClass, PortError, RunnerWait,
    TimeSpan, WakeReason,
};
use trust_runtime_core::scheduler::ResourceState;
use trust_runtime_core::value;

pub use cycle::{resource_tasks, ResourceCycle, RetainSource, TaskDescriptor};
pub use engine::EngineLiveValues;
pub use host::RunnerPlatform;
pub use retain::RetainConfig;

use crate::dispatch::{invalid_line, prepare_line_within, PreparedLine};
use crate::protocol::ControlAfterResponse;
use crate::resource::supervisor::AutomaticRestartLimiter;
use host::RunnerHost;
use program::Program;
use state::{Records, ResourceCell};

/// Requests the loop answers in one pass before it does the pass's other work (the retained
/// checkpoint step, the platform's idle work and the watchdog feed), so that a busy link cannot
/// hold that work back. A pass that reaches the bound does not wait, and the next pass serves the
/// rest. A due boundary ends the serving sooner, so a busy link cannot hold back a due scan either.
pub const FRAMES_PER_PASS: usize = 8;

/// The longest the loop waits while its control link still holds reply bytes it has not handed to
/// its hardware, so a reply in flight is driven to its end within a millisecond per packet without
/// depending on the link's interrupts (specification 34, PORT-05).
pub const REPLY_SERVICE_INTERVAL: TimeSpan = TimeSpan::new(core::time::Duration::from_millis(1));

/// The single-thread resource loop of one installed program, or of a device without one.
///
/// `L` is the control link, `W` the wait, `Y` the installed program, `P` the platform's values
/// and watchdog, and `C` the monotonic clock, the one `W` waits on.
///
/// Stop response (PORT-03): a `shutdown` takes effect at the program's next boundary — within one
/// cycle interval while running or paused, within the automatic restart's backoff of at most one
/// second after an automatic restart, and within the faulted wait's poll period
/// ([`crate::resource::supervisor::FAULTED_WAIT_POLL`]) while faulted — and then within the pass
/// that takes it, after at most the request being answered, with the stop's outputs and retain
/// save. A scan or callback that never returns is not ended by a stop but by the hardware watchdog
/// (TIME-04), which the loop then no longer feeds.
pub struct Runner<L, W, Y, P, C> {
    link: L,
    wait: W,
    cycle: Program<Y>,
    platform: P,
    clock: C,
    resource: ResourceCell,
    records: Records<C>,
    tasks: Vec<TaskDescriptor>,
    frame: Vec<u8>,
    /// Where the program's logical time and uptime start: its start, as a new trust-runtime
    /// process starts its scheduler clock and its metrics.
    program_start: MonotonicInstant,
    /// The program's next boundary, where the loop takes the requests that wait for it: when the
    /// next scan is due while running, the next cycle interval while paused, the next poll of
    /// the faulted wait while faulted; none once the program has ended.
    next_boundary: Option<MonotonicInstant>,
    /// A `shutdown` that waits for the program's next boundary.
    stop_requested: bool,
    /// Scans since the program last started, as the cycle events count them.
    cycle_count: u64,
    /// The program's current time: the logical time of its last scan, or of its last restart.
    program_time: value::Duration,
    restarts: AutomaticRestartLimiter,
    /// A request taken from the link and held back because the next boundary was due. The next
    /// pass answers it once that boundary has been taken.
    held: Option<LinkFrame>,
    /// Whether one request may be answered although the next boundary is due. The boundary is
    /// due again as soon as it is taken after a scan that overran its interval, and without this
    /// grant such a program would never let the link be answered (specification 34, STATE-08).
    request_grant: bool,
}

impl<L, W, Y, P, C> Runner<L, W, Y, P, C>
where
    L: ControlLink,
    W: RunnerWait,
    Y: ResourceCycle,
    P: RunnerPlatform,
    C: MonotonicClock + Clone,
{
    /// A loop for the started program of `cycle`: the resource is Running, its logical time
    /// starts now, its task timing starts again at that time, and its first scan is due at once,
    /// as trust-runtime's scheduler starts its clock and its first pass.
    ///
    /// The composition starts the installed program before, as trust-runtime does before it
    /// starts its scheduler: cold, or warm with the retain checkpoint ([`ResourceCycle::restart`]).
    /// The loop keeps a receive buffer of [`ControlLink::max_frame_len`] bytes, and its records,
    /// among them the 512 durations of the latest scans, in place: a firmware keeps it in static
    /// memory or on the main stack. Its event store keeps the newest `event_capacity` events
    /// (specification 11, the event store): a device without an operating system passes its
    /// descriptor's default, or the capacity its deployment record requests.
    #[must_use]
    pub fn new(
        link: L,
        wait: W,
        mut cycle: Y,
        platform: P,
        clock: C,
        event_capacity: usize,
    ) -> Self {
        let tasks = cycle.tasks();
        let frame = vec![0; link.max_frame_len()];
        let start = clock.now();
        cycle.reset_task_timing(value::Duration::ZERO);
        let records = Records::new(clock.clone(), start, event_capacity);
        Self {
            link,
            wait,
            cycle: Program::Installed(cycle),
            platform,
            clock,
            resource: ResourceCell::running(),
            records,
            tasks,
            frame,
            program_start: start,
            next_boundary: Some(start),
            stop_requested: false,
            cycle_count: 0,
            program_time: value::Duration::ZERO,
            restarts: AutomaticRestartLimiter::default(),
            held: None,
            request_grant: false,
        }
    }

    /// A loop for a device without an installed program (specification 34, STATE-08): its
    /// application region holds none, or one that the device refused, for `reason`. The
    /// resource is Stopped without a fault and has no boundary; the loop serves the link and
    /// feeds the hardware watchdog, every request answers as while Stopped, and `restart` is
    /// refused with `no installed program: <reason>` until a program is installed.
    ///
    /// The loop keeps its receive buffer and its records as [`Self::new`] does, its event store
    /// the newest `event_capacity` events.
    #[must_use]
    pub fn stopped(
        link: L,
        wait: W,
        platform: P,
        clock: C,
        reason: SmolStr,
        event_capacity: usize,
    ) -> Self {
        let frame = vec![0; link.max_frame_len()];
        let start = clock.now();
        let records = Records::new(clock.clone(), start, event_capacity);
        Self {
            link,
            wait,
            cycle: Program::Missing(reason),
            platform,
            clock,
            resource: ResourceCell::stopped(),
            records,
            tasks: Vec::new(),
            frame,
            program_start: start,
            next_boundary: None,
            stop_requested: false,
            cycle_count: 0,
            program_time: value::Duration::ZERO,
            restarts: AutomaticRestartLimiter::default(),
            held: None,
            request_grant: false,
        }
    }

    /// The resource state, for the composition's own indicators.
    #[must_use]
    pub fn resource_state(&self) -> ResourceState {
        self.resource.state()
    }

    /// The fault that stopped the resource, if any.
    #[must_use]
    pub fn last_error(&self) -> Option<RuntimeError> {
        self.resource.last_error()
    }

    /// Runs passes until a port fails, and returns that failure.
    pub fn run(&mut self) -> PortError {
        loop {
            if let Err(error) = self.run_pass() {
                return error;
            }
        }
    }

    /// One pass of the loop: it reads the wake token, serves the requests that have arrived up to a
    /// due boundary, takes the program's boundary when it is due, advances a pending retained
    /// checkpoint by one step, does the platform's idle work, feeds the hardware watchdog and waits
    /// until the next boundary or a wake (PORT-03).
    ///
    /// The wait never outlasts the watchdog's feed interval, so the loop feeds it in every state,
    /// also while the program is stopped, paused or faulted (TIME-04). A pass that served
    /// [`FRAMES_PER_PASS`] requests, or that held a request back for a due boundary, returns
    /// without waiting.
    ///
    /// # Errors
    ///
    /// The [`PortError`] of a wait that failed.
    pub fn run_pass(&mut self) -> Result<WakeReason, PortError> {
        let token = self.wait.wake_token();
        let more_frames = self.serve_link();
        self.supervise();
        self.step_retain();
        self.platform.idle();
        self.platform.feed();
        if more_frames {
            return Ok(WakeReason::Event);
        }
        let liveness = self
            .clock
            .now()
            .saturating_add(self.platform.feed_interval());
        let mut deadline = self
            .next_boundary
            .map_or(liveness, |boundary| boundary.min(liveness));
        if self.link.sending() {
            // The link still holds reply bytes: service it soon again, so the reply reaches its
            // end without waiting for the next request to wake the loop (specification 34,
            // PORT-05). An idle link keeps the loop's own waits.
            deadline = deadline.min(self.clock.now().saturating_add(REPLY_SERVICE_INTERVAL));
        }
        self.wait.wait_until(deadline, token)
    }

    /// Advances a pending retained checkpoint by one persister step, between scans and while the
    /// loop otherwise waits for the next release (specification 34 §10). The step never runs
    /// inside a scan, so an erase that outlasts the time before the next release only makes that
    /// scan late; a failed step latches a retain-store failure that the next scan fails with
    /// before physical outputs.
    fn step_retain(&mut self) {
        // The installed program steps its own checkpoint store. The wrapping `Program` forwards
        // the operations the loop asks of the cycle; the step is taken from the installed cycle
        // directly so no dispatch of the wrapper is needed for it.
        if let Program::Installed(cycle) = &mut self.cycle {
            cycle.step_retain();
        }
    }

    /// Serves up to [`FRAMES_PER_PASS`] requests, and returns true when it stops with the link not
    /// yet dry: at the bound, or at a request it holds back. It starts no request while the next
    /// boundary is due, so the boundary waits for at most the request being answered; the one
    /// exception is a boundary that is due again as soon as it is taken, which lets one request
    /// through (see `request_grant`).
    fn serve_link(&mut self) -> bool {
        for _ in 0..FRAMES_PER_PASS {
            let Some(received) = self.next_frame() else {
                return false;
            };
            if self.boundary_due() && !core::mem::take(&mut self.request_grant) {
                self.held = Some(received);
                return true;
            }
            let prepared = self.answer(received);
            // A reply the link cannot deliver is lost, as trust-runtime loses a reply to a
            // connection that has closed; so is one that does not fit a frame even as the error
            // that replaces a longer reply.
            if prepared.response.len() <= self.frame.len() {
                let _ = self.link.send_frame(prepared.response.as_bytes());
            }
            self.after_reply(prepared.after_response);
        }
        true
    }

    /// The request to answer next: the one held back for a due boundary, which goes first, else the
    /// next frame the link has received.
    fn next_frame(&mut self) -> Option<LinkFrame> {
        self.held
            .take()
            .or_else(|| self.link.poll_frame(&mut self.frame))
    }

    /// The reply to one received frame, at most one frame long. A frame longer than the link
    /// accepts, or one that is not UTF-8 text, cannot be a request line and gets
    /// `invalid request` with id 0.
    fn answer(&self, received: LinkFrame) -> PreparedLine {
        let max_len = self.frame.len();
        let LinkFrame::Frame { len, peer } = received else {
            return too_long(max_len);
        };
        let Some(bytes) = self.frame.get(..len) else {
            return too_long(max_len);
        };
        let line = match core::str::from_utf8(bytes) {
            Ok(line) => line,
            Err(error) => return invalid_line(&error.to_string()),
        };
        let client = client_label(peer);
        let host = RunnerHost {
            platform: &self.platform,
            program: &self.cycle,
            resource: &self.resource,
            records: &self.records,
        };
        prepare_line_within(line, &host, client.as_deref(), max_len)
    }

    /// What a served request leaves for the loop, in the order of the requests: a `shutdown`
    /// waits for the program's next boundary, and a restart of a program that has ended starts
    /// it at once.
    fn after_reply(&mut self, after_response: Option<ControlAfterResponse>) {
        if after_response == Some(ControlAfterResponse::StopResource) && !self.resource.ended() {
            self.stop_requested = true;
        }
        if self.resource.ended()
            && let Some(mode) = self.resource.take_pending_restart()
        {
            self.start(mode);
        }
    }

    /// The logical time of monotonic reading `instant`: the time since the program started
    /// (spec 34 §7.1).
    fn logical(&self, instant: MonotonicInstant) -> value::Duration {
        let since_start = instant.saturating_since(self.program_start).as_duration();
        value::Duration::from_nanos(i64::try_from(since_start.as_nanos()).unwrap_or(i64::MAX))
    }

    /// The program's cycle interval as a span.
    fn interval(&self) -> TimeSpan {
        span(self.cycle.cycle_interval())
    }

    /// Whether the program's next boundary is due now; a program that has ended has none.
    fn boundary_due(&self) -> bool {
        self.next_boundary
            .is_some_and(|boundary| boundary <= self.clock.now())
    }
}

fn too_long(max_len: usize) -> PreparedLine {
    invalid_line(&format!("request longer than {max_len} bytes"))
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

/// A configured span on the monotonic clock. A span that is not positive is none: what waits for
/// it is due at once, as trust-runtime's scheduler then does not sleep.
fn span(configured: value::Duration) -> TimeSpan {
    TimeSpan::from_time(configured).unwrap_or(TimeSpan::ZERO)
}

#[cfg(test)]
mod contract_tests;

#[cfg(test)]
mod link_contract_tests;

#[cfg(test)]
mod live_values_contract_tests;

#[cfg(test)]
mod retain_contract_tests;

#[cfg(test)]
mod reply_contract_tests;

#[cfg(test)]
mod scan_contract_tests;

#[cfg(test)]
mod stopped_contract_tests;

#[cfg(test)]
mod timing_contract_tests;
