//! A restart in place on the C6 (specification 34 STATE-08, UPDATE-04 and §4.3.1), on the C6's own
//! loader and program memory and the real engine, under the shared resource loop, as the firmware
//! boots: the loader builds the program and the loop takes it. The program is the corpus fixture
//! `retained_blocks` (`crates/trust-runtime/tests/fixtures/bounded-c/retained_blocks`): a 10 ms task
//! calls the function-block instance `kept` with `delta := DINT#1`, so its output `count` rises by one
//! in each activation. The loop's ports are fakes: a link that carries the JSON requests, a wait that
//! never sleeps and a clock the test moves. The test reads the program's own variables through a
//! shared handle on its cycle. Every expected value is written out here from the specification.

#[path = "online_change/support.rs"]
mod support;

use std::cell::{Cell, RefCell};
use std::collections::VecDeque;
use std::rc::Rc;
use std::time::Duration as StdDuration;

use serde_json::{json, Value};
use smol_str::SmolStr;
use support::{booted, configuration, Dog, Hardware, Lines, TestClock, PROGRAM};
use trust_platform_esp32::loader::Esp32Loader;
use trust_platform_esp32::profile::{runtime_id, MAIN_STACK};
use trust_platform_esp32::program_memory::ProgramCycle;
use trust_platform_esp32::Platform;
use trust_runtime_core::deployment::{Configuration, ControlMode};
use trust_runtime_core::error::RuntimeError;
use trust_runtime_core::memory::Generation;
use trust_runtime_core::ports::{
    ControlLink, LinkFrame, MonotonicClock, MonotonicInstant, PeerClass, PortError, RunnerWait,
    TimeSpan, WakeReason, WakeToken,
};
use trust_runtime_core::retain::RestartMode;
use trust_runtime_core::value::{Duration, Value as Retained};
use trust_runtime_core::vm::{CycleHooks, RetainApplyEvent};
use trust_runtime_core::watchdog::{FaultPolicy, RetainMode, WatchdogPolicy};
use trust_runtime_services::capabilities::{LastReset, ResetCauseName};
use trust_runtime_services::chip::platform::status_settings;
use trust_runtime_services::cycle_step::{StepOutcome, StepRefusal, StepRequest};
use trust_runtime_services::live_values::var_requests::VariableQueuePort;
use trust_runtime_services::runner::engine::DeviceMemory;
use trust_runtime_services::runner::{
    EngineLiveValues, HoldHandle, Loaded, ProgramLoader, ResourceCycle, RetainSource, Runner,
    RunnerPlatform, TaskDescriptor,
};
use trust_runtime_services::status::IoDriverStatus;

const BLOCKS: &[u8] =
    include_bytes!("../../trust-runtime/tests/fixtures/bounded-c/retained_blocks/program.stbc");

const MEMORY: DeviceMemory = DeviceMemory {
    region_bytes: 64 * 1024,
    heap_bytes: 136 * 1024,
    call_depth: MAIN_STACK.call_depth(),
};

/// A heap with room for the program of this test.
fn room() -> usize {
    1 << 20
}

thread_local! {
    static NOW_MS: Cell<u64> = const { Cell::new(0) };
}

/// The loop's monotonic clock, which the test moves.
#[derive(Clone, Copy)]
struct LoopClock;

impl MonotonicClock for LoopClock {
    fn now(&self) -> MonotonicInstant {
        MonotonicInstant::from_origin(StdDuration::from_millis(NOW_MS.with(Cell::get)))
    }
}

/// A wait that never sleeps: the test runs one pass at a time.
#[derive(Clone, Copy)]
struct LoopWait;

impl RunnerWait for LoopWait {
    fn wake_token(&self) -> WakeToken {
        WakeToken::new(0)
    }

    fn wait_until(
        &mut self,
        _deadline: MonotonicInstant,
        _observed: WakeToken,
    ) -> Result<WakeReason, PortError> {
        Ok(WakeReason::Deadline)
    }
}

#[derive(Default)]
struct LinkState {
    requests: VecDeque<Vec<u8>>,
    replies: Vec<String>,
}

/// The control link: the test queues request frames and takes the replies.
#[derive(Clone, Default)]
struct LoopLink(Rc<RefCell<LinkState>>);

impl LoopLink {
    fn send(&self, request: &Value) {
        self.0
            .borrow_mut()
            .requests
            .push_back(request.to_string().into_bytes());
    }

    fn take_replies(&self) -> Vec<Value> {
        std::mem::take(&mut self.0.borrow_mut().replies)
            .iter()
            .map(|reply| serde_json::from_str(reply).expect("every reply is one JSON object"))
            .collect()
    }
}

impl ControlLink for LoopLink {
    fn max_frame_len(&self) -> usize {
        8_192
    }

    fn poll_frame(&mut self, buffer: &mut [u8]) -> Option<LinkFrame> {
        let frame = self.0.borrow_mut().requests.pop_front()?;
        buffer[..frame.len()].copy_from_slice(&frame);
        Some(LinkFrame::Frame {
            len: frame.len(),
            peer: PeerClass::Local,
        })
    }

    fn send_frame(&mut self, frame: &[u8]) -> Result<(), PortError> {
        let reply = String::from_utf8(frame.to_vec()).expect("every reply is UTF-8");
        self.0.borrow_mut().replies.push(reply);
        Ok(())
    }
}

/// The C6's cycle of the program, behind a handle that the test keeps: the loop drives the real
/// cycle through it, and the test reads the program's variables between the loop's passes.
#[derive(Clone)]
struct Shared(Rc<RefCell<ProgramCycle<TestClock, Lines>>>);

impl ResourceCycle for Shared {
    fn tasks(&self) -> Vec<TaskDescriptor> {
        self.0.borrow().tasks()
    }

    fn cycle_interval(&self) -> Duration {
        self.0.borrow().cycle_interval()
    }

    fn watchdog(&self) -> WatchdogPolicy {
        self.0.borrow().watchdog()
    }

    fn fault_policy(&self) -> FaultPolicy {
        self.0.borrow().fault_policy()
    }

    fn held_time(&self) -> TimeSpan {
        self.0.borrow().held_time()
    }

    fn run_scan(
        &mut self,
        now: Duration,
        deadline: Option<MonotonicInstant>,
        observer: &mut dyn CycleHooks,
    ) -> Result<(), RuntimeError> {
        self.0.borrow_mut().run_scan(now, deadline, observer)
    }

    fn latch_fault(&mut self, fault: RuntimeError) {
        self.0.borrow_mut().latch_fault(fault);
    }

    fn apply_safe_outputs(&mut self) -> Result<(), RuntimeError> {
        self.0.borrow_mut().apply_safe_outputs()
    }

    fn save_retain(&mut self) -> Result<(), RuntimeError> {
        self.0.borrow_mut().save_retain()
    }

    fn step_retain(&mut self) {
        self.0.borrow_mut().step_retain();
    }

    fn restart(
        &mut self,
        mode: RestartMode,
        source: RetainSource,
    ) -> Result<Vec<RetainApplyEvent>, RuntimeError> {
        self.0.borrow_mut().restart(mode, source)
    }

    fn reset_task_timing(&mut self, now: Duration) {
        self.0.borrow_mut().reset_task_timing(now);
    }

    fn step(
        &mut self,
        request: &StepRequest,
        restart_at: Duration,
        deadline: Option<MonotonicInstant>,
    ) -> Result<StepOutcome, StepRefusal> {
        self.0.borrow_mut().step(request, restart_at, deadline)
    }

    fn generation(&self) -> Generation {
        ResourceCycle::generation(&*self.0.borrow())
    }

    fn io_drivers(&self) -> Vec<IoDriverStatus> {
        self.0.borrow().io_drivers()
    }

    fn attach_hold(&mut self, hold: HoldHandle) {
        self.0.borrow_mut().attach_hold(hold);
    }
}

impl Shared {
    /// The identity of the function-block instance `kept` and the `count` it holds, now.
    fn kept(&self) -> (u32, Retained) {
        let cycle = self.0.borrow();
        let storage = cycle.storage();
        let Some(Retained::Instance(kept)) = storage.get_global("kept") else {
            panic!("kept is a function-block instance");
        };
        let count = storage
            .get_instance_var(*kept, "count")
            .cloned()
            .expect("the instance has a count");
        (kept.0, count)
    }
}

fn platform() -> Platform<Dog> {
    Platform::new(
        Dog::default(),
        SmolStr::new_static("Controller"),
        status_settings(),
        false,
        ControlMode::Production,
        EngineLiveValues::empty(),
        LastReset {
            cause: ResetCauseName::PowerOn,
            text: SmolStr::default(),
            boot_count: 1,
        },
        runtime_id([0x20, 0x6E, 0xF1, 0x17, 0x04, 0xC0]),
    )
}

/// The loop under test: over the C6's platform and the real cycle of the program.
type TestRunner = Runner<LoopLink, LoopWait, Shared, Platform<Dog>, LoopClock>;

/// Moves the clock to `at` milliseconds and runs one pass.
fn pass_at(runner: &mut TestRunner, at: u64) {
    NOW_MS.with(|now| now.set(at));
    runner.run_pass().expect("the test wait does not fail");
}

/// Sends `request`, runs one pass and returns the one reply.
fn call(runner: &mut TestRunner, link: &LoopLink, request: &Value) -> Value {
    link.send(request);
    runner.run_pass().expect("the test wait does not fail");
    let mut replies = link.take_replies();
    assert_eq!(replies.len(), 1, "one reply to {request}: {replies:?}");
    replies.remove(0)
}

/// `restart` in `mode`.
fn restart(id: u64, mode: &str) -> Value {
    json!({"id": id, "type": "restart", "params": {"mode": mode}})
}

/// `var.force` of `value` on `target`.
fn var_force(id: u64, target: &str, value: &str) -> Value {
    json!({"id": id, "type": "var.force", "params": {"target": target, "value": value}})
}

/// Moves the clock to `at` milliseconds, sends `request`, runs one pass and returns the one reply.
fn call_at(runner: &mut TestRunner, link: &LoopLink, at: u64, request: &Value) -> Value {
    NOW_MS.with(|now| now.set(at));
    call(runner, link, request)
}

#[test]
fn a_write_by_identity_after_a_warm_restart_reaches_the_variable() {
    // Specification 34 STATE-08, UPDATE-04 and §4.3.1: a `restart` installs the next generation of the
    // program's state, and the loop gives its Live Values that generation before the next cycle, so a
    // request that names an instance by a bare identity and arrives after the restart belongs to the
    // generation installed and reaches the variable. The loader builds the program as the boot does and
    // the loop takes it. Scans at 10 and 20 ms run the task twice (`count` is 2); a warm `restart`
    // queued at 25 ms commits at the boundary at 30 ms, in generation 1, where the program was in
    // generation 0. `var.force` of `instance:<id>:count` to 100 arrives after it and is accepted; the
    // scan at 40 ms applies the force before the program runs and again after the outputs are sampled,
    // so `count` reads 100, not what the program alone would count to, and the force is still active.
    // Wrong behaviour caught: Live Values that keep the generation of the program's first start, which
    // tag the force with generation 0, so the scan refuses it as stale, withdraws it and leaves `count`
    // at its own value; a loop that gives the Live Values a generation that is not the program's.
    let hardware = Hardware::erased();
    let mut loader = Esp32Loader::new(hardware, TestClock, MEMORY, room);
    let mut platform = platform();
    let record = Configuration {
        cycle_interval_nanos: 10_000_000,
        control_debug_enabled: true,
        ..configuration(RetainMode::None, Vec::new())
    };
    let Loaded { cycle, .. } = loader
        .load(&mut platform, booted(BLOCKS, record), RestartMode::Cold)
        .expect("the program loads");
    let live = platform.live_values().clone();
    let program = Shared(Rc::new(RefCell::new(cycle)));
    let link = LoopLink::default();
    NOW_MS.with(|now| now.set(0));
    let mut runner = Runner::new(
        link.clone(),
        LoopWait,
        program.clone(),
        platform,
        LoopClock,
        200,
    );

    assert_eq!(program.generation(), Generation::FIRST);
    assert_eq!(live.generation(), Generation::FIRST);
    pass_at(&mut runner, 0);
    pass_at(&mut runner, 10);
    pass_at(&mut runner, 20);
    assert_eq!(program.kept().1, Retained::DInt(2), "two activations");

    let queued = call_at(&mut runner, &link, 25, &restart(1, "warm"));
    assert_eq!(queued["result"], json!({"status": "restart queued"}));
    pass_at(&mut runner, 30);
    let second = Generation::FIRST.next().expect("a second generation");
    assert_eq!(program.generation(), second, "the program");
    assert_eq!(live.generation(), second, "the Live Values");

    let (kept, _) = program.kept();
    let target = format!("instance:{kept}:count");
    let forced = call(&mut runner, &link, &var_force(2, &target, "100"));
    assert_eq!(forced["result"], json!({"status": "forced"}), "{forced}");
    pass_at(&mut runner, 40);
    assert_eq!(
        program.kept().1,
        Retained::DInt(100),
        "the force by identity reached the variable"
    );
    let listed = call(&mut runner, &link, &json!({"id": 3, "type": "var.forced"}));
    let active = listed["result"]["vars"]
        .as_array()
        .expect("var.forced lists the active forces");
    assert_eq!(
        active.iter().map(|var| &var["target"]).collect::<Vec<_>>(),
        [&json!(target)],
        "the force is still active: a refused force is withdrawn"
    );
}

#[test]
fn a_chip_that_boots_warm_gives_its_live_values_the_generation_of_its_program() {
    // Specification 34 STATE-08 and UPDATE-04: power-on starts a program that keeps retained values as
    // a warm restart (`main.rs` of the firmware asks the loader for `RestartMode::Warm`), which the
    // loader performs over the program it built, in the generation that restart installs (1); the
    // Live Values it built beside the program are in the first generation (0). The loop that takes the
    // program gives its Live Values the program's generation. Wrong behaviour caught: Live Values that
    // stay in generation 0 while the program is in generation 1, which refuse every force by instance
    // identity on a chip that booted warm, until the next restart in place.
    let hardware = Hardware::erased();
    let mut loader = Esp32Loader::new(hardware, TestClock, MEMORY, room);
    let mut platform = platform();
    let record = configuration(RetainMode::File, Vec::new());
    let Loaded { cycle, .. } = loader
        .load(&mut platform, booted(PROGRAM, record), RestartMode::Warm)
        .expect("the program loads");
    let live = platform.live_values().clone();
    let program = Shared(Rc::new(RefCell::new(cycle)));
    let first = Generation::FIRST;
    let second = first.next().expect("a second generation");
    assert_eq!(program.generation(), second, "the warm start restarted it");
    assert_eq!(live.generation(), first, "the loader's Live Values");

    NOW_MS.with(|now| now.set(0));
    let _runner = Runner::new(
        LoopLink::default(),
        LoopWait,
        program.clone(),
        platform,
        LoopClock,
        200,
    );
    assert_eq!(live.generation(), second, "the loop's Live Values");
}
