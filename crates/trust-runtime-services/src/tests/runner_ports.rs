//! Platform ports for the resource loop's contract tests: a control link, a wait and a clock that
//! the test drives, a process I/O that the test reads and fails, and the board probe program
//! (`portability/f401/gpio.stbc`) on the shared engine cycle ([`crate::runner::engine`]) behind
//! the cycle port with the failures a test injects; the platform is in
//! [`super::runner_platform`].

use alloc::boxed::Box;
use alloc::collections::VecDeque;
use alloc::rc::Rc;
use alloc::string::{String, ToString};
use alloc::vec::Vec;
use core::cell::{Cell, RefCell};
use core::time::Duration;

use smol_str::SmolStr;
use trust_runtime_core::error::RuntimeError;
use trust_runtime_core::io_address::IoAddress;
use trust_runtime_core::memory::Generation;
use trust_runtime_core::ports::{
    ControlLink, LinkFrame, MonotonicClock, MonotonicInstant, PeerClass, PortError, RunnerWait,
    TimeSpan, WakeReason, WakeToken,
};
use trust_runtime_core::retain::RestartMode;
use trust_runtime_core::value::{self, Value};
use trust_runtime_core::vm::{
    CycleAccess, CycleHooks, CyclePhase, PreparedModule, RetainApplyEvent,
};
use trust_runtime_core::watchdog::{FaultPolicy, WatchdogPolicy};

pub(crate) use super::runner_platform::TestPlatform;
use crate::events::EVENT_STORE_CAPACITY;
use crate::runner::engine::{
    device_limits, CycleSettings, DeviceMemory, DeviceServices, EngineCycle, ProcessIo,
};
use crate::runner::{
    EngineLiveValues, HoldHandle, ResourceCycle, RetainSource, Runner, TaskDescriptor,
};
use crate::status::IoDriverStatus;

/// The board probe: `led := button` in task `Scan` every 10 ms on resource `Controller`.
pub(crate) const GPIO: &[u8] =
    include_bytes!("../../../trust-runtime/tests/fixtures/portability/f401/gpio.stbc");

/// The application region and heap of today's chips (16 KiB and 72 KiB), whose admission limits
/// the board fixture's runs proved on both.
pub(crate) const CHIP_MEMORY: DeviceMemory = DeviceMemory {
    region_bytes: 16 * 1024,
    heap_bytes: 72 * 1024,
};

/// A span of `value` milliseconds.
pub(crate) fn ms(value: u64) -> Duration {
    Duration::from_millis(value)
}

/// The monotonic clock. Only the test moves it, and a scan that the test makes take time.
#[derive(Clone, Default)]
pub(crate) struct TestClock(Rc<Cell<Duration>>);

impl TestClock {
    pub(crate) fn set(&self, now: Duration) {
        self.0.set(now);
    }

    pub(crate) fn advance(&self, by: Duration) {
        self.0.set(self.0.get().saturating_add(by));
    }
}

impl MonotonicClock for TestClock {
    fn now(&self) -> MonotonicInstant {
        MonotonicInstant::from_origin(self.0.get())
    }
}

/// One wait the loop asked for.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) struct WaitCall {
    /// The deadline, as the time since the clock's origin.
    pub(crate) deadline: Duration,
    /// Whether a real wait would sleep: no wake since the token was read and the deadline still
    /// ahead.
    pub(crate) sleeps: bool,
}

/// The sleeps after which an advancing wait stops a loop that never ends a hold: no test sleeps so
/// often, so a loop that does waits for a release that never comes.
const MAX_SLEEPS: usize = 10_000;

#[derive(Default)]
struct WaitState {
    wakes: u32,
    calls: Vec<WaitCall>,
    failure: Option<PortError>,
    /// Whether a sleep lets time pass on the test clock, as a real wait does.
    advancing: bool,
    /// The sleeps of an advancing wait.
    sleeps: usize,
    /// What happens at a time on the clock, in the order given.
    scheduled: Vec<(Duration, Box<dyn FnOnce()>)>,
}

/// The loop's wait. By default it never blocks: it records the wait and whether a real one would
/// sleep. An advancing wait ([`TestWait::advance_time`]) lets the time of its sleep pass on the
/// clock and runs what the test scheduled for that time ([`TestWait::at`]), as a real wait would
/// sleep through it; a loop that holds a scan runs inside one call of `run_pass`, so the test
/// cannot move the clock between its passes.
#[derive(Clone)]
pub(crate) struct TestWait {
    state: Rc<RefCell<WaitState>>,
    clock: TestClock,
}

impl TestWait {
    fn new(clock: TestClock) -> Self {
        Self {
            state: Rc::new(RefCell::new(WaitState::default())),
            clock,
        }
    }

    /// A wake, as a link interrupt sends it.
    pub(crate) fn wake(&self) {
        let mut state = self.state.borrow_mut();
        state.wakes = state.wakes.wrapping_add(1);
    }

    /// The waits since the last call.
    pub(crate) fn take_calls(&self) -> Vec<WaitCall> {
        core::mem::take(&mut self.state.borrow_mut().calls)
    }

    /// The deadlines of the waits since the last call.
    pub(crate) fn take_deadlines(&self) -> Vec<Duration> {
        self.take_calls().iter().map(|call| call.deadline).collect()
    }

    /// The next wait fails with `error`.
    pub(crate) fn fail_next(&self, error: PortError) {
        self.state.borrow_mut().failure = Some(error);
    }

    /// From now on a sleep lets its time pass on the clock.
    pub(crate) fn advance_time(&self) {
        self.state.borrow_mut().advancing = true;
    }

    /// Runs `action` and wakes the wait when an advancing wait sleeps through `when` on the clock,
    /// as an interrupt that a host's request or a button raises would.
    pub(crate) fn at(&self, when: Duration, action: impl FnOnce() + 'static) {
        self.state
            .borrow_mut()
            .scheduled
            .push((when, Box::new(action)));
    }

    /// Lets time pass on the clock until the earlier of `deadline` and the next scheduled time that
    /// is not after it, runs what was scheduled up to then, and wakes the wait if anything ran.
    fn sleep_until(&self, deadline: Duration) {
        let (until, due) = {
            let mut state = self.state.borrow_mut();
            state.sleeps += 1;
            assert!(
                state.sleeps <= MAX_SLEEPS,
                "the loop slept {MAX_SLEEPS} times: it waits for a wake that never comes"
            );
            let next = state
                .scheduled
                .iter()
                .map(|(when, _)| *when)
                .filter(|when| *when <= deadline)
                .min();
            // Time never goes back: what was scheduled for a time that has passed runs now.
            let until = next
                .unwrap_or(deadline)
                .max(self.clock.now().since_origin());
            let (due, later): (Vec<_>, Vec<_>) = core::mem::take(&mut state.scheduled)
                .into_iter()
                .partition(|(when, _)| *when <= until);
            state.scheduled = later;
            (until, due)
        };
        self.clock.set(until);
        let ran = !due.is_empty();
        for (_, action) in due {
            action();
        }
        if ran {
            self.wake();
        }
    }
}

impl RunnerWait for TestWait {
    fn wake_token(&self) -> WakeToken {
        WakeToken::new(self.state.borrow().wakes)
    }

    fn wait_until(
        &mut self,
        deadline: MonotonicInstant,
        observed: WakeToken,
    ) -> Result<WakeReason, PortError> {
        let advancing = {
            let mut state = self.state.borrow_mut();
            if let Some(error) = state.failure.take() {
                return Err(error);
            }
            let woken = observed != WakeToken::new(state.wakes);
            let due = deadline <= self.clock.now();
            state.calls.push(WaitCall {
                deadline: deadline.since_origin(),
                sleeps: !woken && !due,
            });
            state.advancing && !woken && !due
        };
        if advancing {
            self.sleep_until(deadline.since_origin());
        }
        let woken = observed != WakeToken::new(self.state.borrow().wakes);
        Ok(if woken {
            WakeReason::Event
        } else {
            WakeReason::Deadline
        })
    }
}

#[derive(Default)]
struct LinkState {
    /// The frames the link has received or will receive, each with the time on the clock at which
    /// it reaches the loop, in the order they arrive.
    incoming: VecDeque<(Vec<u8>, PeerClass, Duration)>,
    replies: Vec<String>,
    /// How many times the loop has polled the link for a request.
    polls: u32,
}

/// The control link: the test queues request frames and takes the replies.
#[derive(Clone)]
pub(crate) struct TestLink {
    state: Rc<RefCell<LinkState>>,
    max_frame_len: usize,
    clock: TestClock,
}

impl TestLink {
    fn new(max_frame_len: usize, clock: TestClock) -> Self {
        Self {
            state: Rc::new(RefCell::new(LinkState::default())),
            max_frame_len,
            clock,
        }
    }

    /// Queues `request` from a local client, such as one on USB.
    pub(crate) fn send(&self, request: &serde_json::Value) {
        self.send_bytes(request.to_string().as_bytes(), PeerClass::Local);
    }

    /// Queues `request` from a local client, which reaches the loop when the clock reaches `at`.
    pub(crate) fn send_at(&self, request: &serde_json::Value, at: Duration) {
        self.state.borrow_mut().incoming.push_back((
            request.to_string().into_bytes(),
            PeerClass::Local,
            at,
        ));
    }

    /// Queues raw frame bytes from `peer`.
    pub(crate) fn send_bytes(&self, frame: &[u8], peer: PeerClass) {
        self.state
            .borrow_mut()
            .incoming
            .push_back((frame.to_vec(), peer, Duration::ZERO));
    }

    /// The replies since the last call, as JSON.
    pub(crate) fn take_replies(&self) -> Vec<serde_json::Value> {
        core::mem::take(&mut self.state.borrow_mut().replies)
            .iter()
            .map(|reply| serde_json::from_str(reply).expect("every reply is one JSON object"))
            .collect()
    }

    /// How many times the loop has polled the link for a request since the link was built.
    pub(crate) fn polls(&self) -> u32 {
        self.state.borrow().polls
    }
}

impl ControlLink for TestLink {
    fn max_frame_len(&self) -> usize {
        self.max_frame_len
    }

    fn poll_frame(&mut self, buffer: &mut [u8]) -> Option<LinkFrame> {
        let mut state = self.state.borrow_mut();
        state.polls += 1;
        let now = self.clock.now().since_origin();
        if state.incoming.front().is_none_or(|(_, _, at)| *at > now) {
            return None;
        }
        let (frame, peer, _) = state.incoming.pop_front()?;
        let Some(target) = buffer.get_mut(..frame.len()) else {
            return Some(LinkFrame::Oversized { peer });
        };
        target.copy_from_slice(&frame);
        Some(LinkFrame::Frame {
            len: frame.len(),
            peer,
        })
    }

    fn send_frame(&mut self, frame: &[u8]) -> Result<(), PortError> {
        assert!(
            frame.len() <= self.max_frame_len,
            "the loop never sends a frame longer than the link moves"
        );
        let reply = String::from_utf8(frame.to_vec()).expect("every reply is UTF-8");
        self.state.borrow_mut().replies.push(reply);
        Ok(())
    }
}

/// What the engine cycle asked of the process I/O, in order, among the engine phases that a
/// [`PhaseRecorder`] saw.
#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) enum IoStep {
    /// An engine phase.
    Phase(CyclePhase),
    /// The inputs were read.
    ReadInputs,
    /// The publication drove this output image.
    WriteOutputs(Vec<u8>),
    /// A stop or a fault drove this output image.
    WriteSafeOutputs(Vec<u8>),
}

/// What the process I/O delivers and reports; the test sets it.
#[derive(Default)]
pub(crate) struct IoState {
    steps: Vec<IoStep>,
    /// The input image the reads deliver, from its first byte; empty leaves the image as it is.
    pub(crate) inputs: Vec<u8>,
    /// The next read fails with this error.
    pub(crate) read_error: Option<RuntimeError>,
    /// The next output write fails with this error.
    pub(crate) write_error: Option<RuntimeError>,
    /// The next safe-output write fails with this error.
    pub(crate) safe_error: Option<RuntimeError>,
    /// Time that every read takes on the test clock.
    pub(crate) read_time: Duration,
    /// The health the drivers report.
    pub(crate) health: Vec<IoDriverStatus>,
}

/// The process I/O of the board probe's input and output.
#[derive(Clone)]
pub(crate) struct TestIo {
    pub(crate) state: Rc<RefCell<IoState>>,
    clock: TestClock,
}

impl TestIo {
    pub(crate) fn new(clock: TestClock) -> Self {
        Self {
            state: Rc::new(RefCell::new(IoState::default())),
            clock,
        }
    }

    /// The steps since the last call.
    pub(crate) fn take_steps(&self) -> Vec<IoStep> {
        core::mem::take(&mut self.state.borrow_mut().steps)
    }

    /// A scan observer that records each engine phase among the process I/O's steps.
    pub(crate) fn phases(&self) -> PhaseRecorder {
        PhaseRecorder(Rc::clone(&self.state))
    }
}

impl ProcessIo for TestIo {
    fn read_inputs(&mut self, image: &mut [u8]) -> Result<(), RuntimeError> {
        let mut state = self.state.borrow_mut();
        state.steps.push(IoStep::ReadInputs);
        self.clock.advance(state.read_time);
        if let Some(error) = state.read_error.take() {
            return Err(error);
        }
        for (byte, value) in image.iter_mut().zip(&state.inputs) {
            *byte = *value;
        }
        Ok(())
    }

    fn write_outputs(&mut self, image: &[u8]) -> Result<(), RuntimeError> {
        let mut state = self.state.borrow_mut();
        state.steps.push(IoStep::WriteOutputs(image.to_vec()));
        state.write_error.take().map_or(Ok(()), Err)
    }

    fn write_safe_outputs(&mut self, image: &[u8]) -> Result<(), RuntimeError> {
        let mut state = self.state.borrow_mut();
        state.steps.push(IoStep::WriteSafeOutputs(image.to_vec()));
        state.safe_error.take().map_or(Ok(()), Err)
    }

    fn driver_health(&self) -> Vec<IoDriverStatus> {
        self.state.borrow().health.clone()
    }
}

/// A scan observer that records the engine phases among the process I/O's steps.
pub(crate) struct PhaseRecorder(Rc<RefCell<IoState>>);

impl CycleHooks for PhaseRecorder {
    fn on_phase(
        &mut self,
        phase: CyclePhase,
        _access: &mut CycleAccess<'_, '_>,
    ) -> Result<(), RuntimeError> {
        self.0.borrow_mut().steps.push(IoStep::Phase(phase));
        Ok(())
    }
}

/// What the loop asked of the installed program, in order. Times are logical times.
#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) enum CycleCall {
    Scan {
        now: Duration,
        /// The deadline, as the time since the monotonic clock's origin.
        deadline: Option<Duration>,
    },
    Latch(String),
    SafeOutputs,
    SaveRetain,
    Restart {
        mode: RestartMode,
        source: RetainSource,
    },
    TaskTiming(Duration),
}

/// The settings and failures a test gives the program, and what the loop asked of it. The
/// settings are those of the program's start.
pub(crate) struct CycleScript {
    pub(crate) calls: Vec<CycleCall>,
    pub(crate) cycle_interval: value::Duration,
    /// The next scans fail with these errors without running.
    pub(crate) failing_scans: VecDeque<RuntimeError>,
    /// Time that passes before a scan's work starts, so the engine can miss its deadline.
    pub(crate) stall: Duration,
    /// Time that passes after a scan's work, inside the scan.
    pub(crate) scan_time: Duration,
    /// The next restarts fail with these errors and change nothing.
    pub(crate) failing_restarts: VecDeque<RuntimeError>,
    pub(crate) safe_outputs_error: Option<RuntimeError>,
    pub(crate) retain_error: Option<RuntimeError>,
    /// Retained-value events the next successful restart reports.
    pub(crate) restart_events: Vec<RetainApplyEvent>,
    pub(crate) watchdog: WatchdogPolicy,
    pub(crate) fault_policy: FaultPolicy,
    /// The safe value of each output, as `[[io.safe_state]]` gives it.
    pub(crate) safe_outputs: Vec<(IoAddress, Value)>,
}

impl Default for CycleScript {
    fn default() -> Self {
        Self {
            calls: Vec::new(),
            cycle_interval: value::Duration::from_millis(10),
            failing_scans: VecDeque::new(),
            stall: Duration::ZERO,
            scan_time: Duration::ZERO,
            failing_restarts: VecDeque::new(),
            safe_outputs_error: None,
            retain_error: None,
            restart_events: Vec::new(),
            watchdog: WatchdogPolicy::default(),
            fault_policy: FaultPolicy::Halt,
            safe_outputs: Vec::new(),
        }
    }
}

/// The board probe on the shared engine cycle, behind the cycle port: it records what the loop
/// asks and injects the script's failures in front of the engine cycle.
pub(crate) struct ScriptedCycle<'a> {
    engine: EngineCycle<'a, TestClock, TestIo>,
    clock: TestClock,
    script: Rc<RefCell<CycleScript>>,
    /// The loop's pass for a held scan, which the loop gave the cycle: kept to prove, at every
    /// scan, that the loop calls the cycle with no cell of its own borrowed.
    hold: Option<HoldHandle>,
}

impl<'a> ScriptedCycle<'a> {
    /// The engine cycle `engine` behind the script, which records what the loop asks of it: the
    /// program a loader built (specification 34 STATE-08).
    pub(crate) fn new(
        engine: EngineCycle<'a, TestClock, TestIo>,
        clock: TestClock,
        script: Rc<RefCell<CycleScript>>,
    ) -> Self {
        Self {
            engine,
            clock,
            script,
            hold: None,
        }
    }

    /// The engine cycle behind the script, for a test that reads the program's values.
    pub(crate) fn engine(&self) -> &EngineCycle<'a, TestClock, TestIo> {
        &self.engine
    }
}

/// The span of a logical time, for the calls the test compares.
fn since_start(time: value::Duration) -> Duration {
    Duration::from_nanos(u64::try_from(time.as_nanos()).expect("test times are not negative"))
}

impl ResourceCycle for ScriptedCycle<'_> {
    fn tasks(&self) -> Vec<TaskDescriptor> {
        self.engine.tasks()
    }

    fn cycle_interval(&self) -> value::Duration {
        self.engine.cycle_interval()
    }

    fn watchdog(&self) -> WatchdogPolicy {
        self.engine.watchdog()
    }

    fn fault_policy(&self) -> FaultPolicy {
        self.engine.fault_policy()
    }

    fn held_time(&self) -> TimeSpan {
        self.engine.held_time()
    }

    fn run_scan(
        &mut self,
        now: value::Duration,
        deadline: Option<MonotonicInstant>,
        observer: &mut dyn CycleHooks,
    ) -> Result<(), RuntimeError> {
        if let Some(hold) = &self.hold {
            assert!(
                hold.cells_free(),
                "a cell of the loop is borrowed when the loop runs a scan"
            );
        }
        let (stall, scan_time) = {
            let mut script = self.script.borrow_mut();
            script.calls.push(CycleCall::Scan {
                now: since_start(now),
                deadline: deadline.map(MonotonicInstant::since_origin),
            });
            if let Some(error) = script.failing_scans.pop_front() {
                return Err(error);
            }
            (script.stall, script.scan_time)
        };
        self.clock.advance(stall);
        let result = self.engine.run_scan(now, deadline, observer);
        self.clock.advance(scan_time);
        if let Some(hold) = &self.hold {
            assert!(
                hold.cells_free(),
                "a cell of the loop is borrowed when a scan returns to the loop"
            );
        }
        result
    }

    fn latch_fault(&mut self, fault: RuntimeError) {
        self.script
            .borrow_mut()
            .calls
            .push(CycleCall::Latch(fault.to_string()));
        self.engine.latch_fault(fault);
    }

    fn apply_safe_outputs(&mut self) -> Result<(), RuntimeError> {
        let injected = {
            let mut script = self.script.borrow_mut();
            script.calls.push(CycleCall::SafeOutputs);
            script.safe_outputs_error.clone()
        };
        match injected {
            Some(error) => Err(error),
            None => self.engine.apply_safe_outputs(),
        }
    }

    fn save_retain(&mut self) -> Result<(), RuntimeError> {
        let injected = {
            let mut script = self.script.borrow_mut();
            script.calls.push(CycleCall::SaveRetain);
            script.retain_error.clone()
        };
        match injected {
            Some(error) => Err(error),
            None => self.engine.save_retain(),
        }
    }

    fn restart(
        &mut self,
        mode: RestartMode,
        source: RetainSource,
    ) -> Result<Vec<RetainApplyEvent>, RuntimeError> {
        let mut script = self.script.borrow_mut();
        script.calls.push(CycleCall::Restart { mode, source });
        if let Some(error) = script.failing_restarts.pop_front() {
            return Err(error);
        }
        let mut events = self.engine.restart(mode, source)?;
        events.append(&mut script.restart_events);
        Ok(events)
    }

    fn reset_task_timing(&mut self, now: value::Duration) {
        self.script
            .borrow_mut()
            .calls
            .push(CycleCall::TaskTiming(since_start(now)));
        self.engine.reset_task_timing(now);
    }

    fn generation(&self) -> Generation {
        self.engine.generation()
    }

    fn io_drivers(&self) -> Vec<IoDriverStatus> {
        self.engine.io_drivers()
    }

    fn attach_hold(&mut self, hold: HoldHandle) {
        self.hold = Some(hold.clone());
        self.engine.attach_hold(hold);
    }
}

/// The loop under test.
pub(crate) type TestRunner<'a> =
    Runner<TestLink, TestWait, ScriptedCycle<'a>, TestPlatform, TestClock>;

/// One loop's ports: the test drives the clock, the link, the wait and the process I/O, and
/// scripts the program.
pub(crate) struct Fixture {
    pub(crate) clock: TestClock,
    pub(crate) wait: TestWait,
    pub(crate) link: TestLink,
    pub(crate) io: TestIo,
    pub(crate) script: Rc<RefCell<CycleScript>>,
    /// The Live Values the runner applies and the platform answers from.
    pub(crate) live: Rc<EngineLiveValues>,
    /// The program and the execution services live for the rest of the test run, as a firmware
    /// keeps them, so that the debugger can find the program's statement starts.
    prepared: &'static PreparedModule,
    services: &'static DeviceServices<TestClock>,
    /// Whether a loop started from this fixture lets the debugger hold its scans.
    debugger: bool,
}

impl Fixture {
    /// The board probe, admitted as a chip admits it, at time zero; the link moves frames of up
    /// to 4,096 bytes.
    pub(crate) fn board_probe() -> Self {
        Self::with_frames_of(4_096)
    }

    /// The board probe on a link that moves frames of up to `max_frame_len` bytes.
    pub(crate) fn with_frames_of(max_frame_len: usize) -> Self {
        Self::installing(GPIO, max_frame_len)
    }

    /// The program `artifact`, admitted as a chip admits it, at time zero; the link moves
    /// frames of up to 4,096 bytes.
    pub(crate) fn with_program(artifact: &[u8]) -> Self {
        Self::installing(artifact, 4_096)
    }

    fn installing(artifact: &[u8], max_frame_len: usize) -> Self {
        let clock = TestClock::default();
        let prepared: &'static PreparedModule = Box::leak(Box::new(
            PreparedModule::from_bytes(artifact, device_limits(CHIP_MEMORY))
                .expect("the program is admitted"),
        ));
        let live = Rc::new(
            EngineLiveValues::for_prepared(
                prepared,
                alloc::vec![None; prepared.io_bindings().len()],
            )
            .expect("the program's retained names bind"),
        );
        Self {
            wait: TestWait::new(clock.clone()),
            link: TestLink::new(max_frame_len, clock.clone()),
            io: TestIo::new(clock.clone()),
            script: Rc::new(RefCell::new(CycleScript::default())),
            live,
            services: Box::leak(Box::new(DeviceServices::new(clock.clone()))),
            prepared,
            clock,
            debugger: false,
        }
    }

    /// The execution services of every loop this fixture starts.
    pub(crate) fn services(&self) -> &'static DeviceServices<TestClock> {
        self.services
    }

    /// The fixture with the debugger attached to the program of every loop it starts: the program
    /// is held at the statement starts of its `LINE_TABLE` when the debugger says so.
    pub(crate) fn with_debugger(mut self) -> Self {
        self.debugger = true;
        self
    }

    /// Queues `request` to reach the link when the clock reaches `at`, as a host sends it, and
    /// wakes the wait then, as the link's interrupt does. An advancing wait ([`TestWait::advance_time`])
    /// sleeps through the time.
    pub(crate) fn arrive_at(&self, at: Duration, request: &serde_json::Value) {
        self.link.send_at(request, at);
        self.wait.at(at, || {});
    }

    /// A loop that has just started the program, with the test platform; what the loop asked of
    /// the program at power-on is taken already.
    pub(crate) fn runner(&self) -> TestRunner<'_> {
        self.runner_with(TestPlatform::default())
    }

    /// A loop that has just started the program, with `platform`; what the loop asked of the
    /// program at power-on is taken already.
    pub(crate) fn runner_with(&self, platform: TestPlatform) -> TestRunner<'_> {
        let runner = self.power_on(platform);
        let _ = self.take_calls();
        runner
    }

    /// A loop that has just started the program with `link` in place of the test link, with the
    /// test platform; what the loop asked of the program at power-on is taken already.
    ///
    /// A test that drives the link itself, for example one whose reply the loop must finish,
    /// builds its loop with this.
    pub(crate) fn runner_with_link<L>(
        &self,
        link: L,
    ) -> Runner<L, TestWait, ScriptedCycle<'_>, TestPlatform, TestClock>
    where
        L: ControlLink + 'static,
    {
        let runner =
            self.start_with_link(link, TestPlatform::default(), EVENT_STORE_CAPACITY, false);
        let _ = self.take_calls();
        runner
    }

    /// A loop that has just started the program with an event store of `event_capacity`, with the
    /// test platform; what the loop asked of the program at power-on is taken already.
    pub(crate) fn runner_with_capacity(&self, event_capacity: usize) -> TestRunner<'_> {
        let runner = self.power_on_with_capacity(TestPlatform::default(), event_capacity);
        let _ = self.take_calls();
        runner
    }

    /// The program started on the shared engine cycle with the script's settings, as the
    /// composition starts it at power-on, and the loop built at the clock's current time, with
    /// the computer's default event store.
    pub(crate) fn power_on(&self, platform: TestPlatform) -> TestRunner<'_> {
        self.power_on_with_capacity(platform, EVENT_STORE_CAPACITY)
    }

    /// The program started on the shared engine cycle with the script's settings, as the
    /// composition starts it at power-on, and the loop built at the clock's current time with an
    /// event store of `event_capacity`.
    pub(crate) fn power_on_with_capacity(
        &self,
        platform: TestPlatform,
        event_capacity: usize,
    ) -> TestRunner<'_> {
        self.start_with_link(self.link.clone(), platform, event_capacity, false)
    }

    /// The program started on the shared engine cycle and then restarted warm, as the composition
    /// starts a program that keeps retained values, and the loop built at the clock's current time
    /// with the test platform. The restart installed the next generation of the program's state
    /// before the loop was built; what the loop asked of the program at power-on is taken already.
    pub(crate) fn power_on_restarted_warm(&self) -> TestRunner<'_> {
        let runner = self.start_with_link(
            self.link.clone(),
            TestPlatform::default(),
            EVENT_STORE_CAPACITY,
            true,
        );
        let _ = self.take_calls();
        runner
    }

    /// The program started on the shared engine cycle with the script's settings, as the
    /// composition starts it at power-on, and the loop built over `link` at the clock's current
    /// time with an event store of `event_capacity`. `restarted_warm` restarts the program warm
    /// before the loop is built, as the composition does for a program that keeps retained values.
    fn start_with_link<L>(
        &self,
        link: L,
        mut platform: TestPlatform,
        event_capacity: usize,
        restarted_warm: bool,
    ) -> Runner<L, TestWait, ScriptedCycle<'_>, TestPlatform, TestClock>
    where
        L: ControlLink + 'static,
    {
        // The runner applies the Live Values of the prepared program, so the platform answers the
        // control requests from the same queues and snapshot.
        platform.live_values = Rc::clone(&self.live);
        platform.clock = Some(self.clock.clone());
        let settings = {
            let script = self.script.borrow();
            CycleSettings {
                cycle_interval: script.cycle_interval,
                watchdog: script.watchdog,
                fault_policy: script.fault_policy,
                safe_outputs: script.safe_outputs.clone(),
            }
        };
        let mut engine =
            EngineCycle::start(self.prepared, self.services, self.io.clone(), settings)
                .expect("the board probe starts");
        if restarted_warm {
            engine
                .restart(RestartMode::Warm, RetainSource::Checkpoint)
                .expect("the board probe restarts warm");
        }
        let engine = if self.debugger {
            engine.with_debugger()
        } else {
            engine
        };
        let cycle = ScriptedCycle {
            engine,
            clock: self.clock.clone(),
            script: Rc::clone(&self.script),
            hold: None,
        };
        Runner::new(
            link,
            self.wait.clone(),
            cycle,
            platform,
            self.clock.clone(),
            event_capacity,
        )
    }

    /// A loop of a device without an installed program, for `reason`, with the test platform and
    /// the computer's default event store.
    pub(crate) fn without_program(&self, reason: &'static str) -> TestRunner<'_> {
        Runner::stopped(
            self.link.clone(),
            self.wait.clone(),
            TestPlatform::default(),
            self.clock.clone(),
            SmolStr::new_static(reason),
            EVENT_STORE_CAPACITY,
        )
    }

    /// Moves the clock to `at` milliseconds and runs one pass.
    pub(crate) fn pass_at(&self, runner: &mut TestRunner<'_>, at: u64) {
        self.clock.set(ms(at));
        runner.run_pass().expect("the test wait does not fail");
    }

    /// Sends `request` at the current time, runs one pass and returns the reply.
    pub(crate) fn call(
        &self,
        runner: &mut TestRunner<'_>,
        request: &serde_json::Value,
    ) -> serde_json::Value {
        self.link.send(request);
        runner.run_pass().expect("the test wait does not fail");
        let mut replies = self.link.take_replies();
        assert_eq!(replies.len(), 1, "one reply to {request}: {replies:?}");
        replies.remove(0)
    }

    /// Sends `request` at `at` milliseconds, runs one pass and returns the reply.
    pub(crate) fn call_at(
        &self,
        runner: &mut TestRunner<'_>,
        at: u64,
        request: &serde_json::Value,
    ) -> serde_json::Value {
        self.clock.set(ms(at));
        self.call(runner, request)
    }

    /// What the loop asked of the program since the last call.
    pub(crate) fn take_calls(&self) -> Vec<CycleCall> {
        core::mem::take(&mut self.script.borrow_mut().calls)
    }

    /// The logical times of the scans the loop ran since the last call, in milliseconds.
    pub(crate) fn take_scan_times(&self) -> Vec<u128> {
        self.take_calls()
            .into_iter()
            .filter_map(|call| match call {
                CycleCall::Scan { now, .. } => Some(now.as_millis()),
                _ => None,
            })
            .collect()
    }
}
