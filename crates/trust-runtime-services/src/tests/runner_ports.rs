//! Platform ports for the resource loop's contract tests: a control link, a wait and a clock that
//! the test drives, and the board probe program (`portability/f401/gpio.stbc`) on the real engine
//! behind the cycle port with the failures a test injects; the platform is in
//! [`super::runner_platform`].

use alloc::collections::VecDeque;
use alloc::rc::Rc;
use alloc::string::{String, ToString};
use alloc::vec::Vec;
use core::cell::{Cell, RefCell};
use core::time::Duration;

use smol_str::SmolStr;
use trust_runtime_core::error::RuntimeError;
use trust_runtime_core::ports::{
    ControlLink, LinkFrame, MonotonicClock, MonotonicInstant, PeerClass, PortError, RunnerWait,
    WakeReason, WakeToken,
};
use trust_runtime_core::retain::{RestartMode, RetainSnapshot};
use trust_runtime_core::value::{self, DateTimeValue};
use trust_runtime_core::vm::{
    CycleHooks, ExecutionServices, PreparationLimits, PreparedModule, RetainApplyEvent,
    RuntimeState,
};
use trust_runtime_core::watchdog::{FaultPolicy, WatchdogPolicy};

pub(crate) use super::runner_platform::TestPlatform;
use crate::runner::{resource_tasks, ResourceCycle, RetainSource, Runner, TaskDescriptor};

/// The board probe: `led := button` in task `Scan` every 10 ms on resource `Controller`.
const GPIO: &[u8] =
    include_bytes!("../../../trust-runtime/tests/fixtures/portability/f401/gpio.stbc");

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

#[derive(Default)]
struct WaitState {
    wakes: u32,
    calls: Vec<WaitCall>,
    failure: Option<PortError>,
}

/// The loop's wait. It never blocks: it records the wait and whether a real one would sleep.
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
        Ok(if woken {
            WakeReason::Event
        } else {
            WakeReason::Deadline
        })
    }
}

#[derive(Default)]
struct LinkState {
    incoming: VecDeque<(Vec<u8>, PeerClass)>,
    replies: Vec<String>,
}

/// The control link: the test queues request frames and takes the replies.
#[derive(Clone)]
pub(crate) struct TestLink {
    state: Rc<RefCell<LinkState>>,
    max_frame_len: usize,
}

impl TestLink {
    fn new(max_frame_len: usize) -> Self {
        Self {
            state: Rc::new(RefCell::new(LinkState::default())),
            max_frame_len,
        }
    }

    /// Queues `request` from a local client, such as one on USB.
    pub(crate) fn send(&self, request: &serde_json::Value) {
        self.send_bytes(request.to_string().as_bytes(), PeerClass::Local);
    }

    /// Queues raw frame bytes from `peer`.
    pub(crate) fn send_bytes(&self, frame: &[u8], peer: PeerClass) {
        self.state
            .borrow_mut()
            .incoming
            .push_back((frame.to_vec(), peer));
    }

    /// The replies since the last call, as JSON.
    pub(crate) fn take_replies(&self) -> Vec<serde_json::Value> {
        core::mem::take(&mut self.state.borrow_mut().replies)
            .iter()
            .map(|reply| serde_json::from_str(reply).expect("every reply is one JSON object"))
            .collect()
    }
}

impl ControlLink for TestLink {
    fn max_frame_len(&self) -> usize {
        self.max_frame_len
    }

    fn poll_frame(&mut self, buffer: &mut [u8]) -> Option<LinkFrame> {
        let (frame, peer) = self.state.borrow_mut().incoming.pop_front()?;
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

/// The failures and timing a test gives the program, and what the loop asked of it.
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
    /// The durable retain checkpoint of the last save.
    pub(crate) checkpoint: Option<RetainSnapshot>,
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
            checkpoint: None,
        }
    }
}

/// Execution services with a physical deadline on the test clock.
pub(crate) struct TestServices {
    clock: TestClock,
    deadline: Cell<Option<MonotonicInstant>>,
}

impl ExecutionServices for TestServices {
    fn deadline_exceeded(&self) -> bool {
        self.deadline
            .get()
            .is_some_and(|deadline| self.clock.now() >= deadline)
    }

    fn has_wall_clock(&self) -> bool {
        false
    }

    fn current_dt(&self) -> Result<DateTimeValue, RuntimeError> {
        Err(RuntimeError::ProfileUnsupported(SmolStr::new(
            "no UTC clock in the loop tests",
        )))
    }
}

/// The board probe on the real engine, behind the cycle port.
pub(crate) struct EngineCycle<'a> {
    state: RuntimeState<'a>,
    prepared: &'a PreparedModule,
    services: &'a TestServices,
    clock: TestClock,
    script: Rc<RefCell<CycleScript>>,
}

/// The span of a logical time, for the calls the test compares.
fn since_start(time: value::Duration) -> Duration {
    Duration::from_nanos(u64::try_from(time.as_nanos()).expect("test times are not negative"))
}

impl ResourceCycle for EngineCycle<'_> {
    fn tasks(&self) -> Vec<TaskDescriptor> {
        resource_tasks(self.prepared, 0)
    }

    fn cycle_interval(&self) -> value::Duration {
        self.script.borrow().cycle_interval
    }

    fn watchdog(&self) -> WatchdogPolicy {
        self.script.borrow().watchdog
    }

    fn fault_policy(&self) -> FaultPolicy {
        self.script.borrow().fault_policy
    }

    fn run_scan(
        &mut self,
        now: value::Duration,
        deadline: Option<MonotonicInstant>,
        observer: &mut dyn CycleHooks,
    ) -> Result<(), RuntimeError> {
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
        self.services.deadline.set(deadline);
        let result = self.state.execute_cycle_with_hooks(now, observer);
        self.services.deadline.set(None);
        self.clock.advance(scan_time);
        result
    }

    fn latch_fault(&mut self, fault: RuntimeError) {
        self.script
            .borrow_mut()
            .calls
            .push(CycleCall::Latch(fault.to_string()));
        self.state.latch_fault(fault);
    }

    fn apply_safe_outputs(&mut self) -> Result<(), RuntimeError> {
        let mut script = self.script.borrow_mut();
        script.calls.push(CycleCall::SafeOutputs);
        script.safe_outputs_error.clone().map_or(Ok(()), Err)
    }

    fn save_retain(&mut self) -> Result<(), RuntimeError> {
        let mut script = self.script.borrow_mut();
        script.calls.push(CycleCall::SaveRetain);
        if let Some(error) = script.retain_error.clone() {
            return Err(error);
        }
        script.checkpoint = Some(self.state.retain_snapshot()?);
        Ok(())
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
        let mut events = match (source, mode) {
            (RetainSource::Checkpoint, RestartMode::Warm) => {
                let checkpoint = script.checkpoint.clone().unwrap_or_default();
                self.state
                    .restart_with_retain_snapshot(RestartMode::Warm, &checkpoint)?
            }
            (RetainSource::Memory | RetainSource::Checkpoint, mode) => {
                self.state.restart(mode)?;
                Vec::new()
            }
        };
        events.append(&mut script.restart_events);
        Ok(events)
    }

    fn reset_task_timing(&mut self, now: value::Duration) {
        self.script
            .borrow_mut()
            .calls
            .push(CycleCall::TaskTiming(since_start(now)));
        self.state.reset_task_timing(now);
    }
}

/// The loop under test.
pub(crate) type TestRunner<'a> =
    Runner<TestLink, TestWait, EngineCycle<'a>, TestPlatform, TestClock>;

/// One loop's ports: the test drives the clock, the link and the wait, and scripts the program.
pub(crate) struct Fixture {
    pub(crate) clock: TestClock,
    pub(crate) wait: TestWait,
    pub(crate) link: TestLink,
    pub(crate) script: Rc<RefCell<CycleScript>>,
    prepared: PreparedModule,
    services: TestServices,
}

impl Fixture {
    /// The board probe, admitted and ready, at time zero; the link moves frames of up to 4,096
    /// bytes.
    pub(crate) fn board_probe() -> Self {
        Self::with_frames_of(4_096)
    }

    /// The board probe on a link that moves frames of up to `max_frame_len` bytes.
    pub(crate) fn with_frames_of(max_frame_len: usize) -> Self {
        let clock = TestClock::default();
        let prepared = PreparedModule::from_bytes(GPIO, PreparationLimits::default())
            .expect("the board probe is admitted");
        Self {
            wait: TestWait::new(clock.clone()),
            link: TestLink::new(max_frame_len),
            script: Rc::new(RefCell::new(CycleScript::default())),
            services: TestServices {
                clock: clock.clone(),
                deadline: Cell::new(None),
            },
            prepared,
            clock,
        }
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

    /// The program started, instantiated as the composition starts it at power-on, and the loop
    /// built at the clock's current time.
    pub(crate) fn power_on(&self, platform: TestPlatform) -> TestRunner<'_> {
        let state = self
            .prepared
            .instantiate_with_services(0, &self.services)
            .expect("the board probe starts");
        let cycle = EngineCycle {
            state,
            prepared: &self.prepared,
            services: &self.services,
            clock: self.clock.clone(),
            script: Rc::clone(&self.script),
        };
        Runner::new(
            self.link.clone(),
            self.wait.clone(),
            cycle,
            platform,
            self.clock.clone(),
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
