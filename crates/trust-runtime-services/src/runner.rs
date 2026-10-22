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
//! The debugger can hold a scan inside the engine, at the first instruction of a statement
//! (specification 13, "Safe Points"). The loop is then not running, so the hold runs a pass of its
//! own that does what the loop does while it waits: it serves the link, does the platform's idle
//! work, feeds the hardware watchdog and waits (specification 34, STATE-07 and TIME-04). The link,
//! wait, platform, resource state and records that both passes use are shared through one shell;
//! `shell.rs` states how it is borrowed, and `hold.rs` the hold pass.
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
//!
//! A device that stores its program in flash gives the loop a transfer session and a program store
//! ([`Runner::with_transfers`], [`Runner::with_program_store`]). A `program.install` then commits in
//! steps, one in each pass while the resource is Stopped, and its `installed` reply is the one reply
//! in flight until the last step is done: the loop reads no request from its link meanwhile
//! (specification 34 §11.2 and PORT-05; the install state is in `install.rs`).
//!
//! A device that can build a program from an admitted container also gives the loop a
//! [`ProgramLoader`] ([`Runner::with_program_loader`]). A `restart` of the Stopped resource then loads
//! the newest committed container in place when it is newer than the one the program came from: the
//! loop drops the program, reads and admits the container, and has the loader build and start it,
//! without a reset (specification 34 STATE-08; the order is in `reload.rs`). With a program store the
//! loop also answers `program.active` from the store, in bounded reads.
//!
//! The same loader lets the loop change its program online (`bytecode.reload`, specification 34
//! §11.3): the admitted change waits for the program's next boundary, where the loop prepares the
//! candidate while the running program stays installed, hands it the running program's lines and
//! drops the running program. The reply is the one reply in flight until the change ends (the order is
//! in `online_change.rs`).

mod cycle;
mod debugger;
pub mod engine;
mod hold;
mod host;
mod install;
mod observer;
mod online_change;
mod program;
mod reload;
mod retain;
mod shell;
mod state;
mod supervision;

use alloc::boxed::Box;
use alloc::rc::Rc;
use alloc::vec::Vec;
use core::alloc::Layout;
use core::ptr::NonNull;

use smol_str::SmolStr;
use trust_runtime_core::error::RuntimeError;
use trust_runtime_core::ports::{
    ControlLink, MonotonicClock, MonotonicInstant, PortError, ProgramStore, RunnerWait, TimeSpan,
    WakeReason,
};
use trust_runtime_core::scheduler::ResourceState;
use trust_runtime_core::value;
use trust_runtime_core::vm::RetainApplyEvent;

pub use cycle::{resource_tasks, ResourceCycle, RetainSource, TaskDescriptor};
pub use engine::EngineLiveValues;
pub use hold::HoldHandle;
pub use host::RunnerPlatform;
pub use install::{
    check_install, check_retained_values, newest_checkpoint, newest_checkpoint_payload_len,
};
pub use online_change::{
    address_text, is_call_depth_limit, is_memory_limit, prepare_candidate, stack_shortage,
    too_deep, unwired_binding, Candidate, ChangeMemory,
};
pub use reload::{Loaded, ProgramLoader};
pub use retain::RetainConfig;
pub use state::event_store_bytes;

use crate::installation::Admitted;
use crate::protocol::ControlAfterResponse;
use crate::resource::supervisor::AutomaticRestartLimiter;
use crate::transfer::TransferSession;
use program::Program;
use shell::Shell;
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

/// The leaf of a standard B-tree of thread numbers and call depths, which the debugger's table of
/// threads takes when the loop is built ([`Runner::heap_bytes`]). The standard library lays a leaf
/// out in C order: the link to its parent, its index in the parent and its length, then room for
/// eleven keys and eleven values. Only its size is read.
#[allow(dead_code, reason = "only the size of the layout is read")]
#[repr(C)]
struct ThreadTableLeaf {
    parent: Option<NonNull<u8>>,
    parent_index: u16,
    len: u16,
    keys: [u32; 11],
    values: [u32; 11],
}

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
/// (TIME-04), which the loop then no longer feeds. A scan that a debugger holds is not such a
/// scan: its hold feeds the watchdog, and a stop ends the hold.
pub struct Runner<L, W, Y, P, C> {
    /// The link, wait, platform, resource state, records and debugger, which the loop shares with
    /// the pass that serves them while the debugger holds a scan.
    shell: Rc<Shell<L, W, P, C>>,
    cycle: Program<Y>,
    clock: C,
    /// The Live Values the platform answers the control requests from, which the scan applies. The
    /// loop holds its own handle so that a scan does not borrow the platform.
    live: EngineLiveValues,
    /// The loop's pass for a scan that a debugger holds, which every program of the loop gets when it
    /// is loaded in place ([`Runner::with_program_loader`]).
    hold: HoldHandle,
    /// What loads the installed program in place, when the device gave the loop one.
    loader: Option<Box<dyn ProgramLoader<P, Y>>>,
    tasks: Vec<TaskDescriptor>,
    /// Where the program's logical time and uptime start: its start, as a new trust-runtime
    /// process starts its scheduler clock and its metrics.
    program_start: MonotonicInstant,
    /// The program's next boundary, where the loop takes the requests that wait for it: when the
    /// next scan is due while running, the next cycle interval while paused, the next poll of
    /// the faulted wait while faulted; none once the program has ended.
    next_boundary: Option<MonotonicInstant>,
    /// Scans since the program last started, as the cycle events count them.
    cycle_count: u64,
    /// The program's current time: the logical time of its last scan, or of its last restart.
    program_time: value::Duration,
    restarts: AutomaticRestartLimiter,
    /// Whether one request may be answered although the next boundary is due. The boundary is
    /// due again as soon as it is taken after a scan that overran its interval, and without this
    /// grant such a program would never let the link be answered (specification 34, STATE-08).
    request_grant: bool,
}

impl<L, W, Y, P, C> Runner<L, W, Y, P, C>
where
    L: ControlLink + 'static,
    W: RunnerWait + 'static,
    Y: ResourceCycle,
    P: RunnerPlatform + 'static,
    C: MonotonicClock + Clone + 'static,
{
    /// A loop for the started program of `cycle`: the resource is Running, its logical time
    /// starts now, its task timing starts again at that time, and its first scan is due at
    /// once, as trust-runtime's scheduler starts its clock and its first pass.
    ///
    /// The composition starts the installed program before, as trust-runtime does before it
    /// starts its scheduler: cold, or warm with the retain checkpoint ([`ResourceCycle::restart`]).
    /// The loop gives the Live Values the generation the program's state is in
    /// ([`ResourceCycle::generation`]), which a warm start has advanced.
    /// The loop keeps a receive buffer of [`ControlLink::max_frame_len`] bytes, and its records,
    /// among them the 512 durations of the latest scans, in place: a firmware keeps it in static
    /// memory or on the main stack. Its event store keeps the newest `event_capacity` events
    /// (specification 11, the event store): a device without an operating system passes its
    /// descriptor's default, or the capacity its deployment record requests.
    ///
    /// The loop hands the cycle its pass for a scan that a debugger holds
    /// ([`ResourceCycle::attach_hold`]); the link, wait, platform and clock live as long as the
    /// pass does, which a firmware keeps for the rest of the boot (`'static`). The loop takes its
    /// own handle on the platform's Live Values ([`RunnerPlatform::live_values`]).
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
        let start = clock.now();
        cycle.reset_task_timing(value::Duration::ZERO);
        let live = platform.live_values().clone();
        // The composition may have restarted the program warm, which installs the next generation,
        // before it built the loop: the Live Values carry the generation the program is in.
        live.set_generation(cycle.generation());
        let records = Records::new(clock.clone(), start, event_capacity);
        // `status` reports the memory figure of the program the loop runs (specification 34 MEM-12).
        let memory = cycle.program_memory();
        records.record(|metrics| metrics.set_program_memory(memory));
        let shell = Rc::new(Shell::new(
            link,
            wait,
            platform,
            clock.clone(),
            ResourceCell::running(),
            records,
        ));
        let mut cycle = Program::Installed(cycle);
        let hold = HoldHandle::new(&shell);
        cycle.attach_hold(hold.clone());
        Self {
            shell,
            cycle,
            clock,
            live,
            hold,
            loader: None,
            tasks,
            program_start: start,
            next_boundary: Some(start),
            cycle_count: 0,
            program_time: value::Duration::ZERO,
            restarts: AutomaticRestartLimiter::default(),
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
        let start = clock.now();
        let live = platform.live_values().clone();
        let records = Records::new(clock.clone(), start, event_capacity);
        let shell = Rc::new(Shell::new(
            link,
            wait,
            platform,
            clock.clone(),
            ResourceCell::stopped(),
            records,
        ));
        let hold = HoldHandle::new(&shell);
        Self {
            shell,
            cycle: Program::Missing(reason),
            clock,
            live,
            hold,
            loader: None,
            tasks: Vec::new(),
            program_start: start,
            next_boundary: None,
            cycle_count: 0,
            program_time: value::Duration::ZERO,
            restarts: AutomaticRestartLimiter::default(),
            request_grant: false,
        }
    }

    /// Runs `operation` on the loop's platform, for the steps of a device's boot that follow the
    /// building of its loop (specification 34 STATE-08): the firmware sets the memory the device
    /// has for programs, which it measures once the loop holds its own, and starts the hardware
    /// watchdog after the boot's load ([`Self::boot`]). `None` when the platform is in use, which
    /// it never is between the loop's passes.
    pub fn with_platform<R>(&self, operation: impl FnOnce(&mut P) -> R) -> Option<R> {
        self.shell.with_platform(operation)
    }
}

impl<L, W, Y, P, C> Runner<L, W, Y, P, C> {
    /// The bytes of heap a loop of this type takes when it is built, beside any program
    /// (specification 34 MEM-12, Appendix D): its shell behind a shared pointer, the shell's receive
    /// buffer of `frame_len` bytes, the event store of `event_capacity` events, the debugger's table
    /// of threads, and the loop itself, which a firmware keeps in a box. A chip keeps these bytes
    /// beside the memory it has for programs; what the firmware gives the loop besides, such as a
    /// program loader or a retained-value check, it counts itself.
    #[must_use]
    pub fn heap_bytes(frame_len: usize, event_capacity: usize) -> usize {
        // A shared pointer keeps its two counts before the value, as `Rc` lays it out.
        let shell = Layout::new::<[usize; 2]>()
            .extend(Layout::new::<Shell<L, W, P, C>>())
            .map_or(usize::MAX, |(layout, _)| layout.pad_to_align().size());
        [
            shell,
            frame_len,
            event_store_bytes(event_capacity),
            core::mem::size_of::<ThreadTableLeaf>(),
            core::mem::size_of::<Self>(),
        ]
        .iter()
        .fold(0usize, |sum, bytes| sum.saturating_add(*bytes))
    }

    /// Gives the loop the transfer session of this boot, which the transfer operations use (specification
    /// 34, PORT-05). A device builds it with the boot number it keeps across resets, so that a transfer
    /// id of an earlier boot never names a transfer of this one.
    ///
    /// Without a session of its own the loop uses the platform's ([`RunnerPlatform::with_transfers`]).
    #[must_use]
    pub fn with_transfers(self, session: TransferSession) -> Self {
        self.shell.install.set_transfers(session);
        self
    }

    /// Gives the loop the device's program store, which `program.install` commits to in steps
    /// (specification 34 §5.1 and §11.2), and the device's retained-value check of an install (decision
    /// G13): the warm start of the admitted program with the newest checkpoint, which refuses the install
    /// before any byte is written. A device whose checkpoints are in a checkpoint flash builds the check
    /// with [`check_retained_values`], over a handle on the slots that it only reads. The loop answers
    /// `program.install` with `unsupported request` until it is given a store.
    #[must_use]
    pub fn with_program_store(
        self,
        store: ProgramStore,
        retained_check: impl FnMut(&Admitted<'_>) -> Result<(), RuntimeError> + 'static,
    ) -> Self {
        self.shell
            .install
            .set_store(store, Box::new(retained_check));
        self
    }
}

impl<L, W, Y, P, C> Runner<L, W, Y, P, C>
where
    L: ControlLink,
    W: RunnerWait,
    Y: ResourceCycle,
    P: RunnerPlatform,
    C: MonotonicClock + Clone,
{
    /// The resource state, for the composition's own indicators.
    #[must_use]
    pub fn resource_state(&self) -> ResourceState {
        self.shell.resource.state()
    }

    /// The fault that stopped the resource, if any.
    #[must_use]
    pub fn last_error(&self) -> Option<RuntimeError> {
        self.shell.resource.last_error()
    }

    /// The shell the loop shares with a hold, for the tests that drive the debugger.
    #[cfg(test)]
    fn shell(&self) -> &Rc<Shell<L, W, P, C>> {
        &self.shell
    }

    /// Records the retained values that the boot's warm start dropped or migrated, at the program's
    /// time zero, as a restart in place records them (specification 34 STATE-08; specification 11
    /// §6.7). The composition restarts the program before it builds the loop and passes that
    /// restart's events here, so a boot's events reach the event store as a restart's do.
    pub fn record_boot_retain_events(&self, events: Vec<RetainApplyEvent>) {
        self.record_retain_events(events);
    }

    /// Runs passes until a port fails, and returns that failure.
    pub fn run(&mut self) -> PortError {
        loop {
            if let Err(error) = self.run_pass() {
                return error;
            }
        }
    }

    /// One pass of the loop: it reads the wake token, advances a pending install by one step, serves
    /// the requests that have arrived up to a due boundary, takes the program's boundary when it is
    /// due, advances a pending retained checkpoint by one step, does the platform's idle work, feeds
    /// the hardware watchdog and waits until the next boundary or a wake (PORT-03).
    ///
    /// The wait never outlasts the watchdog's feed interval, so the loop feeds it in every state,
    /// also while the program is stopped, paused or faulted (TIME-04). A pass that served
    /// [`FRAMES_PER_PASS`] requests, or that held a request back for a due boundary, returns
    /// without waiting.
    ///
    /// While an install commits (specification 34 §11.2), its reply is the one reply in flight
    /// (PORT-05): the pass serves no request, takes its one step before it serves the link, and
    /// returns without waiting, so the next pass takes the next step. The pass that reads the
    /// install's request writes no byte of it; the passes after it take one step each.
    ///
    /// While an online change waits for the program's boundary (specification 34 §11.3), its reply
    /// is the one in flight too: the pass serves no request, but it takes the boundaries and runs the
    /// scans, and it waits no longer than the five seconds after which the change is withdrawn.
    ///
    /// # Errors
    ///
    /// The [`PortError`] of a wait that failed.
    pub fn run_pass(&mut self) -> Result<WakeReason, PortError> {
        // A wait that failed inside a hold ends the run, as one that fails here does.
        if let Some(failure) = self.shell.take_port_failure() {
            return Err(failure);
        }
        let token = self.shell.wake_token();
        self.shell.step_install();
        self.shell.expire_change();
        let more_frames = self.serve_link();
        self.supervise();
        self.step_retain();
        self.shell.idle();
        self.shell.feed();
        self.fault_on_shell_defect();
        if more_frames || self.shell.install_pending() {
            return Ok(WakeReason::Event);
        }
        let liveness = self.clock.now().saturating_add(self.shell.feed_interval());
        let mut deadline = self
            .next_boundary
            .map_or(liveness, |boundary| boundary.min(liveness));
        if self.shell.link_sending() {
            // The link still holds reply bytes: service it soon again, so the reply reaches its
            // end without waiting for the next request to wake the loop (specification 34,
            // PORT-05). An idle link keeps the loop's own waits.
            deadline = deadline.min(self.clock.now().saturating_add(REPLY_SERVICE_INTERVAL));
        }
        if let Some(expiry) = self.shell.change.expiry() {
            // An online change that waits for a boundary is withdrawn after five seconds, whether or
            // not a boundary comes (specification 34 §11.3).
            deadline = deadline.min(expiry);
        }
        self.shell.wait_until(deadline, token)
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
            let Some(received) = self.shell.next_frame() else {
                return false;
            };
            if self.boundary_due() && !core::mem::take(&mut self.request_grant) {
                self.shell.hold_back(received);
                return true;
            }
            let after_response = self.shell.serve(received, &self.cycle);
            self.after_reply(after_response);
        }
        true
    }

    /// What a served request leaves for the loop, in the order of the requests: a `shutdown`
    /// waits for the program's next boundary, and a restart of a program that has ended starts
    /// it at once.
    fn after_reply(&mut self, after_response: Option<ControlAfterResponse>) {
        self.shell.note_reply(after_response);
        if self.shell.resource.ended()
            && let Some(mode) = self.shell.resource.take_pending_restart()
        {
            self.start(mode);
        }
    }

    /// The logical time of monotonic reading `instant`: the time since the program started
    /// (spec 34 §7.1), taken in whole nanoseconds, without a `Duration` between, whose seconds a
    /// 32-bit chip divides out in software (checklist P1-6).
    fn logical(&self, instant: MonotonicInstant) -> value::Duration {
        let since_start = instant.saturating_since(self.program_start).as_nanos();
        value::Duration::from_nanos(i64::try_from(since_start).unwrap_or(i64::MAX))
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

/// A configured span on the monotonic clock. A span that is not positive is none: what waits for
/// it is due at once, as trust-runtime's scheduler then does not sleep.
fn span(configured: value::Duration) -> TimeSpan {
    TimeSpan::from_time(configured).unwrap_or(TimeSpan::ZERO)
}

#[cfg(test)]
mod contract_tests;

#[cfg(test)]
mod disconnect_contract_tests;

#[cfg(test)]
mod hold_contract_tests;

#[cfg(test)]
mod hold_link_contract_tests;

#[cfg(test)]
mod install_contract_tests;

#[cfg(test)]
mod link_contract_tests;

#[cfg(test)]
mod live_values_contract_tests;

#[cfg(test)]
mod retain_contract_tests;

#[cfg(test)]
mod reload_contract_tests;

#[cfg(test)]
mod reply_contract_tests;

#[cfg(test)]
mod scan_contract_tests;

#[cfg(test)]
mod stopped_contract_tests;

#[cfg(test)]
mod timing_contract_tests;

#[cfg(test)]
mod scan_cost_probe_tests;
