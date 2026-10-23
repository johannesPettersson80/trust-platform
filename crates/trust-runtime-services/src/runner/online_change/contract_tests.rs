//! A chip's loop changes its program online (specification 11 §6.7.1; specification 34 §11.3,
//! STATE-08, UPDATE-01 to UPDATE-07 and PORT-05). The loop is the board fixture's: its link, wait,
//! clock and platform are the test doubles of `tests/runner_ports.rs`, and its programs run on the
//! shared engine behind a cycle that logs what the loop asks of it. The loader builds a candidate as a
//! chip's does, with the shared rules ([`prepare_candidate`], [`unwired_binding`], the engine's
//! replacement and safe-state check) and a fake line: the process I/O of the machine, which the
//! running program holds and the commit hands to the candidate. Every expected value is written out
//! here from the specification and the programs below (the board probe `led := button`, and the A4
//! program, whose retained `activations` is 1 at 30 ms, 2 at 50 ms and 3 at 80 ms, with the
//! non-retained `result` 3 at 80 ms, per the trace of its fixture), not taken from the code under test.
//! This file holds the harness and the tests of the queue, the reply and the memory check; what a change
//! keeps and what it refuses are in `outcomes`.

use alloc::boxed::Box;
use alloc::collections::VecDeque;
use alloc::format;
use alloc::rc::Rc;
use alloc::string::{String, ToString};
use alloc::vec;
use alloc::vec::Vec;
use core::cell::{Cell, RefCell};

use base64::engine::general_purpose::STANDARD;
use base64::Engine as _;
use serde_json::{json, Value};
use smol_str::SmolStr;
use trust_runtime_core::crc32;
use trust_runtime_core::deployment::{sha256, Configuration, GpioInput, GpioOutput, IoDriver};
use trust_runtime_core::error::RuntimeError;
use trust_runtime_core::io_address::IoAddress;
use trust_runtime_core::memory::Generation;
use trust_runtime_core::ports::{MonotonicInstant, TimeSpan};
use trust_runtime_core::retain::RestartMode;
use trust_runtime_core::scheduler::ResourceState;
use trust_runtime_core::value::{self, Value as Retained};
use trust_runtime_core::vm::{CycleHooks, PreparedModule, RetainApplyEvent};
use trust_runtime_core::watchdog::{FaultPolicy, RetainMode, WatchdogPolicy};

use super::{address_text, prepare_candidate, unwired_binding, Candidate};
use crate::chip::process_io::record_lines;
use crate::cycle_step::{StepOutcome, StepRefusal, StepRequest};
use crate::events::EVENT_STORE_CAPACITY;
use crate::loading::select::Booted;
use crate::runner::engine::{
    device_limits, CycleSettings, DeviceMemory, DeviceServices, EngineCycle,
};
use crate::runner::program::Program;
use crate::runner::{
    EngineLiveValues, HoldHandle, Loaded, ProgramLoader, ResourceCycle, RetainSource, Runner,
    TaskDescriptor,
};
use crate::status::IoDriverStatus;
use crate::tests::program_store::{CheckpointProbe, PROGRAM};
use crate::tests::runner_ports::{
    ms, Fixture, TestClock, TestIo, TestLink, TestPlatform, TestWait, CHIP_MEMORY, GPIO,
};
use crate::transfer::{TransferSession, PART_BYTES};

mod outcomes;

/// The bytes of memory the devices of these tests keep in reserve.
const RESERVE: usize = 20_000;
/// A heap with room for any change of these tests.
const ROOM: usize = 1 << 20;
/// The device's memory for the limits of a candidate: a 64 KiB region and the bring-up heap.
const MEMORY: DeviceMemory = DeviceMemory {
    region_bytes: 64 * 1024,
    heap_bytes: 72 * 1024,
    call_depth: 7,
};
/// The event capacity of a loaded program (the C6's default).
const CAPACITY: usize = 200;
/// The boot number of the transfer session.
const BOOT: u32 = 3;

/// What the loop and the loader did, in order.
type Log = Rc<RefCell<Vec<String>>>;

fn note(log: &Log, text: String) {
    log.borrow_mut().push(text);
}

fn entries(log: &Log) -> Vec<String> {
    log.borrow().clone()
}

fn texts(expected: &[&str]) -> Vec<String> {
    expected.iter().map(ToString::to_string).collect()
}

/// A logical time in whole milliseconds.
fn logical_ms(time: value::Duration) -> i64 {
    time.as_nanos() / 1_000_000
}

/// The cycle of a program under test: the shared engine, logging what the loop asks of it, with the
/// faults a test injects in front of it.
struct ChangeCycle {
    name: String,
    log: Log,
    faults: Rc<RefCell<VecDeque<RuntimeError>>>,
    engine: EngineCycle<'static, TestClock, TestIo>,
}

impl Drop for ChangeCycle {
    fn drop(&mut self) {
        note(&self.log, format!("drop {}", self.name));
    }
}

impl ResourceCycle for ChangeCycle {
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
        note(&self.log, format!("scan {} {}", self.name, logical_ms(now)));
        if let Some(fault) = self.faults.borrow_mut().pop_front() {
            return Err(fault);
        }
        self.engine.run_scan(now, deadline, observer)
    }

    fn latch_fault(&mut self, fault: RuntimeError) {
        self.engine.latch_fault(fault);
    }

    fn apply_safe_outputs(&mut self) -> Result<(), RuntimeError> {
        note(&self.log, format!("safe outputs {}", self.name));
        self.engine.apply_safe_outputs()
    }

    fn save_retain(&mut self) -> Result<(), RuntimeError> {
        self.engine.save_retain()
    }

    fn restart(
        &mut self,
        mode: RestartMode,
        source: RetainSource,
    ) -> Result<Vec<RetainApplyEvent>, RuntimeError> {
        note(&self.log, format!("restart {} {mode:?}", self.name));
        self.engine.restart(mode, source)
    }

    fn reset_task_timing(&mut self, now: value::Duration) {
        note(
            &self.log,
            format!("timing {} {}", self.name, logical_ms(now)),
        );
        self.engine.reset_task_timing(now);
    }

    fn step(
        &mut self,
        request: &StepRequest,
        restart_at: value::Duration,
        deadline: Option<MonotonicInstant>,
    ) -> Result<StepOutcome, StepRefusal> {
        self.engine.step(request, restart_at, deadline)
    }

    fn generation(&self) -> Generation {
        self.engine.generation()
    }

    fn io_drivers(&self) -> Vec<IoDriverStatus> {
        self.engine.io_drivers()
    }

    fn attach_hold(&mut self, hold: HoldHandle) {
        self.engine.attach_hold(hold);
    }
}

/// The settings of the program of `config`: its cycle interval (10 ms when the record sets none), the
/// halting fault policy and the safe value of each output.
fn settings(config: &Configuration) -> CycleSettings {
    let nanos = if config.cycle_interval_nanos == 0 {
        10_000_000
    } else {
        config.cycle_interval_nanos
    };
    CycleSettings {
        cycle_interval: value::Duration::from_nanos(
            i64::try_from(nanos).expect("a short interval"),
        ),
        watchdog: WatchdogPolicy::default(),
        fault_policy: FaultPolicy::Halt,
        safe_outputs: config
            .safe_state
            .iter()
            .map(|entry| (entry.address.clone(), entry.value.clone()))
            .collect(),
    }
}

/// The module of `artifact`, prepared as a chip prepares a program, for the rest of the test run.
fn prepared(artifact: &[u8]) -> &'static PreparedModule {
    Box::leak(Box::new(
        PreparedModule::from_bytes(artifact, device_limits(CHIP_MEMORY))
            .expect("the program is admitted"),
    ))
}

fn address(text: &str) -> IoAddress {
    IoAddress::parse(text).expect("an address")
}

/// A record with a `gpio` driver that has the input lines `inputs` and the output lines `outputs`, and
/// `retain_mode` as its retain mode. Its cycle interval is the 10 ms default of [`settings`].
fn record(retain_mode: RetainMode, inputs: &[&str], outputs: &[&str]) -> Configuration {
    Configuration {
        retain_mode,
        io_drivers: vec![IoDriver {
            name: SmolStr::new_static("gpio"),
            enabled: true,
            inputs: inputs
                .iter()
                .map(|text| GpioInput {
                    address: address(text),
                    line: SmolStr::new_static("GPIO9"),
                    invert: false,
                    debounce_ms: 0,
                })
                .collect(),
            outputs: outputs
                .iter()
                .map(|text| GpioOutput {
                    address: address(text),
                    line: SmolStr::new_static("GPIO18"),
                    invert: false,
                    initial: false,
                })
                .collect(),
        }],
        ..Configuration::default()
    }
}

/// The loader of the tests. It builds a candidate as the C6's loader does, over the shared rules, and
/// it builds the program of an installed container for a `restart`.
struct ChangeLoader {
    log: Log,
    clock: TestClock,
    services: &'static DeviceServices<TestClock>,
    /// The machine's lines: the process I/O that the running program holds.
    lines: TestIo,
    /// How many programs were given the lines.
    claims: Rc<Cell<u32>>,
    /// The bytes of heap that are free now, which a test sets.
    room: Rc<Cell<usize>>,
    faults: Rc<RefCell<VecDeque<RuntimeError>>>,
    /// The record of the program that runs.
    config: Configuration,
    /// The newest saved snapshot, through a handle that only reads.
    checkpoints: CheckpointProbe,
    /// The candidates built so far.
    built: u32,
}

impl ChangeLoader {
    /// The lines of the machine, for a program that is loaded in place.
    fn claim(&self) -> TestIo {
        self.claims.set(self.claims.get() + 1);
        self.lines.clone()
    }

    fn cycle(&self, name: String, engine: EngineCycle<'static, TestClock, TestIo>) -> ChangeCycle {
        ChangeCycle {
            name,
            log: Rc::clone(&self.log),
            faults: Rc::clone(&self.faults),
            engine,
        }
    }
}

impl ProgramLoader<TestPlatform, ChangeCycle> for ChangeLoader {
    fn room(&self) -> usize {
        self.room.get()
    }

    fn load(
        &mut self,
        platform: &mut TestPlatform,
        booted: Booted,
        mode: RestartMode,
    ) -> Result<Loaded<ChangeCycle>, String> {
        note(&self.log, format!("load {} {mode:?}", booted.sequence));
        let module = prepared(&booted.program);
        let live = EngineLiveValues::for_prepared(module, vec![None; module.io_bindings().len()])
            .map_err(|error| error.to_string())?;
        platform.live_values = Rc::new(live);
        self.config = booted.record.configuration;
        let engine =
            EngineCycle::start(module, self.services, self.claim(), settings(&self.config))
                .map_err(|error| error.to_string())?;
        Ok(Loaded {
            cycle: self.cycle(format!("installed {}", booted.sequence), engine),
            events: Vec::new(),
            event_capacity: CAPACITY,
        })
    }

    fn prepare_change(
        &mut self,
        _platform: &mut TestPlatform,
        running: &mut ChangeCycle,
        program: Vec<u8>,
    ) -> Result<Candidate<ChangeCycle>, String> {
        note(&self.log, String::from("prepare"));
        let (module, budget) = prepare_candidate(program, MEMORY, self.room.get(), RESERVE)?;
        let (inputs, outputs) = record_lines(&self.config, "the test chip")?;
        if let Some(unwired) = unwired_binding(module.io_bindings(), &inputs, &outputs) {
            return Err(format!(
                "the candidate binds {}, which the running program's record gives no line",
                address_text(&unwired)
            ));
        }
        let module: &'static PreparedModule = Box::leak(Box::new(module));
        let live = EngineLiveValues::for_prepared(module, vec![None; module.io_bindings().len()])
            .map_err(|error| budget.refusal(&error))?;
        // The candidate claims no line: its process I/O is a fresh one that drives nothing.
        let empty = TestIo::new(self.clock.clone());
        // The candidate is built from the running program's state, as a computer builds it; no
        // checkpoint is applied (decision OC-1).
        let settings = running.engine.settings().clone();
        let (engine, events) = running
            .engine
            .replacement(module, self.services, empty, settings)
            .map_err(|error| budget.refusal(&error))?;
        // From here on the candidate is a cycle, so a check that refuses it drops it with its log.
        let mut cycle = self.cycle(format!("change {}", self.built + 1), engine);
        cycle
            .engine
            .check_safe_outputs()
            .map_err(|error| error.to_string())?;
        live.set_generation(cycle.engine.generation());
        self.built += 1;
        // The device's checkpoint is left alone at a change (decision OC-1).
        note(
            &self.log,
            format!("checkpoint reads {}", self.checkpoints.reads()),
        );
        Ok(Candidate {
            cycle,
            live,
            events,
        })
    }

    fn commit_change(
        &mut self,
        platform: &mut TestPlatform,
        running: &mut ChangeCycle,
        candidate: &mut Candidate<ChangeCycle>,
    ) {
        note(&self.log, String::from("commit"));
        running.engine.hand_over_to(&mut candidate.cycle.engine);
        platform.live_values = Rc::new(candidate.live.clone());
    }
}

type ChangeRunner = Runner<TestLink, TestWait, ChangeCycle, TestPlatform, TestClock>;

/// One device: the board fixture's ports, the machine's lines, and what the loader of its loop was
/// told and did.
struct Rig {
    ports: Fixture,
    log: Log,
    room: Rc<Cell<usize>>,
    claims: Rc<Cell<u32>>,
    faults: Rc<RefCell<VecDeque<RuntimeError>>>,
}

impl Rig {
    /// A device whose link moves frames of up to 16 KiB, so that a change of the A4 program travels in
    /// one request.
    fn new() -> Self {
        Self {
            ports: Fixture::with_frames_of(16_384),
            log: Rc::default(),
            room: Rc::new(Cell::new(ROOM)),
            claims: Rc::new(Cell::new(0)),
            faults: Rc::default(),
        }
    }

    fn loader(&self, config: Configuration, checkpoints: CheckpointProbe) -> ChangeLoader {
        ChangeLoader {
            log: Rc::clone(&self.log),
            clock: self.ports.clock.clone(),
            services: self.ports.services(),
            lines: self.ports.io.clone(),
            claims: Rc::clone(&self.claims),
            room: Rc::clone(&self.room),
            faults: Rc::clone(&self.faults),
            config,
            checkpoints,
            built: 0,
        }
    }

    /// A loop that runs `artifact` with the record `config`, whose newest saved snapshot is in
    /// `checkpoints`. The program holds the machine's lines; the loop can change it online.
    fn runner(
        &self,
        artifact: &[u8],
        config: Configuration,
        checkpoints: CheckpointProbe,
    ) -> ChangeRunner {
        self.runner_from(artifact, config, checkpoints, None, TestPlatform::default())
    }

    fn runner_from(
        &self,
        artifact: &[u8],
        config: Configuration,
        checkpoints: CheckpointProbe,
        came_from: Option<u64>,
        platform: TestPlatform,
    ) -> ChangeRunner {
        self.runner_with(artifact, config, checkpoints, came_from, platform, false)
    }

    /// A loop whose program the debugger can hold at its statements, with `platform`.
    fn runner_holding(
        &self,
        artifact: &[u8],
        config: Configuration,
        platform: TestPlatform,
    ) -> ChangeRunner {
        self.runner_with(
            artifact,
            config,
            CheckpointProbe::erased(),
            None,
            platform,
            true,
        )
    }

    fn runner_with(
        &self,
        artifact: &[u8],
        config: Configuration,
        checkpoints: CheckpointProbe,
        came_from: Option<u64>,
        mut platform: TestPlatform,
        debugger: bool,
    ) -> ChangeRunner {
        let module = prepared(artifact);
        platform.live_values = Rc::new(
            EngineLiveValues::for_prepared(module, vec![None; module.io_bindings().len()])
                .expect("the program's retained names bind"),
        );
        platform.clock = Some(self.ports.clock.clone());
        let loader = self.loader(config, checkpoints);
        self.claims.set(self.claims.get() + 1);
        let engine = EngineCycle::start(
            module,
            self.ports.services(),
            self.ports.io.clone(),
            settings(&loader.config),
        )
        .expect("the program starts");
        let engine = if debugger {
            engine.with_debugger(None)
        } else {
            engine
        };
        let cycle = loader.cycle(String::from("running"), engine);
        Runner::new(
            self.ports.link.clone(),
            self.ports.wait.clone(),
            cycle,
            platform,
            self.ports.clock.clone(),
            EVENT_STORE_CAPACITY,
        )
        .with_program_loader(loader, came_from)
    }

    /// Moves the clock to `at` milliseconds and runs one pass.
    fn pass_at(&self, runner: &mut ChangeRunner, at: u64) {
        self.ports.clock.set(ms(at));
        runner.run_pass().expect("the test wait does not fail");
    }

    /// Sends `request` at `at` milliseconds, runs one pass and returns the replies it wrote.
    fn call_at(&self, runner: &mut ChangeRunner, at: u64, request: &Value) -> Vec<Value> {
        self.ports.link.send(request);
        self.pass_at(runner, at);
        self.ports.link.take_replies()
    }

    /// The replies written since the last call.
    fn replies(&self) -> Vec<Value> {
        self.ports.link.take_replies()
    }
}

fn reload(id: u64, program: &[u8]) -> Value {
    json!({
        "id": id,
        "type": "bytecode.reload",
        "params": {"bytes": STANDARD.encode(program)},
    })
}

fn request(id: u64, kind: &str) -> Value {
    json!({"id": id, "type": kind})
}

/// The reply of a change that was applied, with the audit record the platform numbered.
fn reloaded(id: u64, audit: u32) -> Value {
    json!({"id": id, "ok": true, "result": {"status": "reloaded"}, "audit_id": format!("audit-{audit}")})
}

/// The `Plant` program's variable `name` in the A4 program that the loop runs now.
fn plant(runner: &ChangeRunner, name: &str) -> Retained {
    let Program::Installed(cycle) = &runner.cycle else {
        panic!("a program is installed");
    };
    let storage = cycle.engine.state().storage();
    let Some(Retained::Instance(plant)) = storage.get_global("Plant") else {
        panic!("the A4 program has the program instance Plant");
    };
    storage
        .get_instance_var(*plant, name)
        .cloned()
        .unwrap_or_else(|| panic!("Plant has {name}"))
}

/// No reply was written.
fn no_replies() -> Vec<Value> {
    Vec::new()
}

#[test]
fn a_chip_applies_an_online_change_at_the_next_boundary_and_replies_reloaded() {
    // Specification 11 §6.7.1 and specification 34 §11.3: the change is applied at the program's next
    // boundary, and its reply is the one reply in flight (PORT-05). The board probe scans every 10 ms.
    // A change arrives at 15 ms: nothing is answered, and a `status` that arrives at 16 ms waits behind
    // it, because the loop reads no request while the reply is owed. The boundary at 20 ms prepares the
    // candidate, commits it, drops the running program and runs the candidate's first scan, and writes
    // `reloaded`; the `status` is answered in the next pass, by the program that was given. The loader
    // is called once for each step, in this order. Wrong behaviour caught: a change applied at the
    // request instead of the boundary; a reply written before the boundary; a request served while the
    // reply is owed; the old program dropped before the candidate is ready, or after its first scan.
    let rig = Rig::new();
    let mut runner = rig.runner(
        GPIO,
        record(RetainMode::None, &["%IX0.0"], &["%QX0.0"]),
        CheckpointProbe::erased(),
    );
    rig.pass_at(&mut runner, 0);
    rig.pass_at(&mut runner, 10);

    assert_eq!(rig.call_at(&mut runner, 15, &reload(7, GPIO)), no_replies());
    assert_eq!(
        rig.call_at(&mut runner, 16, &request(8, "status")),
        no_replies()
    );
    assert_eq!(
        entries(&rig.log),
        texts(&["timing running 0", "scan running 0", "scan running 10"]),
        "nothing is prepared before the boundary"
    );

    rig.pass_at(&mut runner, 20);
    assert_eq!(
        rig.replies(),
        [reloaded(7, 1)],
        "the reply to the change, and nothing behind it"
    );
    assert_eq!(
        entries(&rig.log),
        texts(&[
            "timing running 0",
            "scan running 0",
            "scan running 10",
            "prepare",
            "checkpoint reads 0",
            "commit",
            "drop running",
            "scan change 1 20",
        ]),
        "prepare, commit, the old program dropped last, then the candidate's first scan"
    );
    assert_eq!(runner.resource_state(), ResourceState::Running);

    rig.pass_at(&mut runner, 21);
    let status = rig.replies();
    assert_eq!(status.len(), 1, "the request that waited is answered now");
    assert_eq!(status[0]["id"], json!(8));
    assert_eq!(status[0]["result"]["state"], json!("running"));
}

#[test]
fn a_change_uploaded_in_parts_is_applied_and_its_upload_is_used_up() {
    // Specification 34 PORT-05 and §11.3: `bytecode.reload {transfer}` takes the complete upload when
    // the request is admitted; the upload is used up whether the change is applied or refused. The
    // program arrives in parts of 4,096 bytes (two for the 6,412 bytes of the A4 program), the request
    // is answered `reloaded` at the next boundary, and the id of the upload is unknown afterwards.
    // Wrong behaviour caught: an upload that stays in the session after the change took it, which
    // keeps the heap that a chip does not have; a change that applies an incomplete upload.
    let rig = Rig::new();
    let mut runner = rig
        .runner(
            PROGRAM,
            record(RetainMode::None, &[], &[]),
            CheckpointProbe::erased(),
        )
        .with_transfers(TransferSession::new(BOOT, 65_536, 8_192));
    rig.pass_at(&mut runner, 0);
    let begin = rig.call_at(
        &mut runner,
        1,
        &json!({
            "id": 1,
            "type": "transfer.begin",
            "params": {"bytes": PROGRAM.len(), "sha256": hex(&sha256(PROGRAM))},
        }),
    );
    let transfer = begin[0]["result"]["transfer"]
        .as_str()
        .expect("begin names the transfer")
        .to_string();
    for (index, part) in PROGRAM.chunks(PART_BYTES).enumerate() {
        let reply = rig.call_at(
            &mut runner,
            2,
            &json!({
                "id": 2 + index,
                "type": "transfer.part",
                "params": {
                    "transfer": transfer,
                    "offset": index * PART_BYTES,
                    "data": STANDARD.encode(part),
                    "crc32": format!("{:08x}", crc32::checksum(part)),
                },
            }),
        );
        assert_eq!(reply[0]["ok"], json!(true), "part {index}: {reply:?}");
    }
    let queued = rig.call_at(
        &mut runner,
        3,
        &json!({"id": 9, "type": "bytecode.reload", "params": {"transfer": transfer}}),
    );
    assert_eq!(queued, no_replies(), "the reply waits for the boundary");
    rig.pass_at(&mut runner, 10);
    assert_eq!(rig.replies().len(), 1);
    assert!(entries(&rig.log).contains(&String::from("commit")));
    let status = rig.call_at(
        &mut runner,
        11,
        &json!({"id": 10, "type": "transfer.status", "params": {"transfer": transfer}}),
    );
    assert_eq!(status[0]["ok"], json!(false));
    assert_eq!(status[0]["error"], json!("unknown transfer"));
}

fn hex(bytes: &[u8]) -> String {
    use core::fmt::Write as _;
    bytes.iter().fold(String::new(), |mut text, byte| {
        let _ = write!(text, "{byte:02x}");
        text
    })
}

#[test]
fn a_failed_preparation_leaves_the_running_program_unchanged() {
    // Specification 11 §6.7.1: if preparation fails the request returns an error and leaves the prior
    // module, variable storage, process image and bindings, logical time, cycle counter and fault and
    // status state unchanged. The A4 program runs seven scans, 0 to 60 ms, with `activations` 2. A
    // change of four bytes that are not bytecode arrives at 65 ms and fails at the boundary at 70 ms:
    // the reply is an error, no candidate is committed, the program keeps its values (`activations` 2
    // and `result` 2, not their initial 0), its lines (one claim, the same holders of the machine's
    // I/O), its logical time (70 ms, not 0) and its scan counter (8 scans, not 1), and it is Running
    // without a fault; at 80 ms its task runs as it would have, `activations` 3. Wrong behaviour
    // caught: a failed change that restarts the program, resets its time or its counter, drops its
    // lines or leaves the resource faulted.
    let rig = Rig::new();
    let mut runner = rig.runner(
        PROGRAM,
        record(RetainMode::None, &[], &[]),
        CheckpointProbe::erased(),
    );
    for at in [0, 10, 20, 30, 40, 50, 60] {
        rig.pass_at(&mut runner, at);
    }
    assert_eq!(plant(&runner, "activations"), Retained::DInt(2));
    assert_eq!(runner.cycle_count, 7);
    let holders = Rc::strong_count(&rig.ports.io.state);

    assert_eq!(
        rig.call_at(&mut runner, 65, &reload(5, &[0, 1, 2, 3])),
        no_replies()
    );
    rig.pass_at(&mut runner, 70);
    let replies = rig.replies();
    assert_eq!(replies.len(), 1);
    assert_eq!(replies[0]["id"], json!(5));
    assert_eq!(replies[0]["ok"], json!(false));
    assert!(
        replies[0]["error"]
            .as_str()
            .is_some_and(|error| error.contains("bytecode")),
        "the preparation's own error: {replies:?}"
    );

    let log = entries(&rig.log);
    assert!(log.contains(&String::from("prepare")));
    assert!(
        !log.contains(&String::from("commit")) && !log.iter().any(|entry| entry == "drop running"),
        "a failed change commits nothing and drops nothing: {log:?}"
    );
    assert_eq!(runner.cycle_count, 8, "seven scans, and the one at 70 ms");
    assert_eq!(runner.program_time, value::Duration::from_millis(70));
    assert_eq!(plant(&runner, "activations"), Retained::DInt(2));
    assert_eq!(plant(&runner, "result"), Retained::Int(2));
    assert_eq!(Rc::strong_count(&rig.ports.io.state), holders);
    assert_eq!(rig.claims.get(), 1, "the machine's lines were claimed once");
    assert_eq!(runner.resource_state(), ResourceState::Running);
    assert_eq!(runner.last_error(), None);

    rig.pass_at(&mut runner, 80);
    assert_eq!(plant(&runner, "activations"), Retained::DInt(3));
}

#[test]
fn an_online_change_that_needs_more_memory_than_the_heap_has_is_refused_and_the_program_runs_on() {
    // Specification 34 §11.3 (UPDATE-06): the candidate's limits are what the heap's room allows less the
    // reserve (20,000 bytes here). With 26,000 bytes free, 6,000 remain, fewer than the A4 program's
    // 6,412 bytes, and an artifact longer than the heap that remains cannot be prepared in it, so the
    // change is refused with `insufficient memory` and its numbers, and the program runs on. With the room back, the same
    // change is applied. Wrong behaviour caught: a candidate prepared with the limits of the whole heap,
    // which the running program and the upload already use; a refusal that breaks the program; a
    // refusal that sticks.
    let rig = Rig::new();
    let mut runner = rig.runner(
        PROGRAM,
        record(RetainMode::None, &[], &[]),
        CheckpointProbe::erased(),
    );
    for at in [0, 10, 20] {
        rig.pass_at(&mut runner, at);
    }
    rig.room.set(26_000);
    assert_eq!(
        rig.call_at(&mut runner, 25, &reload(1, PROGRAM)),
        no_replies()
    );
    rig.pass_at(&mut runner, 30);
    let refused = rig.replies();
    assert_eq!(refused.len(), 1);
    assert_eq!(refused[0]["ok"], json!(false));
    assert_eq!(
        refused[0]["error"],
        json!(
            "insufficient memory: a change of 6412 bytes needs more than the 6000 bytes of heap \
             that remain for it (26000 bytes are free, 20000 are kept in reserve)"
        )
    );
    assert!(!entries(&rig.log).contains(&String::from("commit")));
    rig.pass_at(&mut runner, 40);
    assert_eq!(plant(&runner, "activations"), Retained::DInt(1));
    assert_eq!(runner.resource_state(), ResourceState::Running);

    rig.room.set(ROOM);
    assert_eq!(
        rig.call_at(&mut runner, 45, &reload(2, PROGRAM)),
        no_replies()
    );
    rig.pass_at(&mut runner, 50);
    assert_eq!(rig.replies(), [reloaded(2, 2)]);
}
