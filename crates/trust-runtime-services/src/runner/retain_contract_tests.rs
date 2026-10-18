//! The device runner's checkpoint store (specification 34 §10, specification 11 §6.7): the A4
//! program on the shared engine over a RAM checkpoint flash, the runner's link, wait and clock
//! from the board fixture, and the test platform's checkpoint store.
//!
//! The tests prove the whole path at the runner's boundary: a stop saves the retained values and a
//! new runner starts warm with them; a power cut during a save loses at most the save in progress;
//! the persister advances only between scans; a latched failure fails the next scan before
//! physical outputs; `mode = "none"` saves nothing; and a warm start writes an unchanged image
//! only under an identity it was not written for.

use alloc::boxed::Box;
use alloc::rc::Rc;
use alloc::vec;
use alloc::vec::Vec;
use core::cell::{Cell, RefCell};

use serde_json::json;
use trust_runtime_core::error::RuntimeError;
use trust_runtime_core::ports::CheckpointStorage;
use trust_runtime_core::retain::checkpoint::{
    CheckpointFlash, CheckpointIdentity, CheckpointLog, FlashError, FlashGeometry, RecordInfo, Slot,
};
use trust_runtime_core::retain::RestartMode;
use trust_runtime_core::scheduler::ResourceState;
use trust_runtime_core::value::{Duration, Value};
use trust_runtime_core::vm::PreparedModule;
use trust_runtime_core::watchdog::{FaultPolicy, RetainMode, WatchdogPolicy};

use super::engine::{device_limits, CycleSettings, DeviceServices, EngineCycle, ProcessIo};
use super::program::Program;
use super::{ResourceCycle, RetainConfig, RetainSource, Runner};
use crate::status::IoDriverStatus;
use crate::tests::runner_ports::{
    ms, Fixture, IoStep, TestClock, TestIo, TestLink, TestPlatform, TestWait, CHIP_MEMORY,
};

/// The A4 program: a 25 ms task over a counter and the retained `activations` (the same program
/// the device engine's contract tests run).
const A4_PROGRAM: &[u8] =
    include_bytes!("../../../trust-runtime/tests/fixtures/portability/stbc-2.0/program-v2.stbc");

/// The application identity every checkpoint of these tests is written for.
const APPLICATION: [u8; 16] = [0x11; 16];
/// The retain-schema identity every checkpoint of these tests is written for.
const SCHEMA: [u8; 16] = [0x22; 16];
/// The identity a checkpoint is written for unless a test chooses another schema.
const IDENTITY: CheckpointIdentity = CheckpointIdentity {
    application: APPLICATION,
    schema: SCHEMA,
};
/// The admitted retain image of these tests: larger than any staged image.
const IMAGE_BYTES: usize = 256;
/// The bytes one persister step may program; small enough that a save takes several steps.
const PROGRAM_BUDGET: usize = 64;

/// The STM32F407's checkpoint slots: one 16 KiB erase unit each (specification 34 §10.1).
fn f407() -> FlashGeometry {
    FlashGeometry::new(4, 16_384, 16_384).expect("the STM32F407's checkpoint geometry")
}

/// The RAM flash's bytes and the counts a test reads.
#[derive(Default)]
struct RamState {
    /// The two slots' bytes; a blank flash reads 0xFF.
    bytes: Vec<u8>,
    /// Every program and erase from now fails, as a power cut does.
    fail_writes: bool,
    /// The programs and erases that succeeded.
    writes: u32,
    /// The programs and erases that ran while a scan was running.
    writes_inside_scan: u32,
}

/// A RAM checkpoint flash with a chip's program and erase units: erasing fills a unit with 0xFF
/// and programming ANDs whole units, as the real flash does. A test can make every later program
/// and erase fail, as a power cut does.
#[derive(Clone)]
struct RamFlash {
    geometry: FlashGeometry,
    state: Rc<RefCell<RamState>>,
    /// True between a scan's input read and its output drive; the flash counts a program or erase
    /// there, which proves a step ran inside a scan.
    in_scan: Rc<Cell<bool>>,
}

impl RamFlash {
    /// A blank flash of `geometry`.
    fn new(geometry: FlashGeometry) -> Self {
        Self {
            geometry,
            state: Rc::new(RefCell::new(RamState {
                bytes: vec![0xFF; 2 * geometry.slot_len() as usize],
                ..RamState::default()
            })),
            in_scan: Rc::new(Cell::new(false)),
        }
    }

    /// The programs and erases so far.
    fn writes(&self) -> u32 {
        self.state.borrow().writes
    }

    /// How many programs and erases ran while a scan was running.
    fn writes_inside_scan(&self) -> u32 {
        self.state.borrow().writes_inside_scan
    }

    /// Makes every later program and erase fail, as a power cut does.
    fn fail_writes(&self) {
        self.state.borrow_mut().fail_writes = true;
    }

    /// Where `slot` starts in the two slots' bytes.
    fn slot_base(&self, slot: Slot) -> usize {
        let slot_len = self.geometry.slot_len() as usize;
        match slot {
            Slot::A => 0,
            Slot::B => slot_len,
        }
    }
}

impl CheckpointFlash for RamFlash {
    fn geometry(&self) -> FlashGeometry {
        self.geometry
    }

    fn read(&mut self, slot: Slot, offset: u32, bytes: &mut [u8]) -> Result<(), FlashError> {
        let state = self.state.borrow();
        let base = self.slot_base(slot) + offset as usize;
        let end = base + bytes.len();
        bytes.copy_from_slice(&state.bytes[base..end]);
        Ok(())
    }

    fn program(&mut self, slot: Slot, offset: u32, bytes: &[u8]) -> Result<(), FlashError> {
        let mut state = self.state.borrow_mut();
        if state.fail_writes {
            return Err(FlashError::Program);
        }
        state.writes += 1;
        if self.in_scan.get() {
            state.writes_inside_scan += 1;
        }
        let base = self.slot_base(slot) + offset as usize;
        for (index, byte) in bytes.iter().enumerate() {
            state.bytes[base + index] &= *byte;
        }
        Ok(())
    }

    fn erase(&mut self, slot: Slot, offset: u32) -> Result<(), FlashError> {
        let mut state = self.state.borrow_mut();
        if state.fail_writes {
            return Err(FlashError::Erase);
        }
        state.writes += 1;
        if self.in_scan.get() {
            state.writes_inside_scan += 1;
        }
        let base = self.slot_base(slot) + offset as usize;
        let end = base + self.geometry.erase_unit() as usize;
        state.bytes[base..end].fill(0xFF);
        Ok(())
    }
}

/// The board fixture's process I/O with a shared flag that is true only while a scan runs, so the
/// RAM flash can count a program or erase that happened inside one.
#[derive(Clone)]
struct ScanWindowIo {
    inner: TestIo,
    in_scan: Rc<Cell<bool>>,
}

impl ScanWindowIo {
    /// `inner` behind the scan window flag.
    fn new(inner: TestIo, in_scan: Rc<Cell<bool>>) -> Self {
        Self { inner, in_scan }
    }

    /// The process I/O's steps since the last call.
    fn take_steps(&self) -> Vec<IoStep> {
        self.inner.take_steps()
    }
}

impl ProcessIo for ScanWindowIo {
    fn read_inputs(&mut self, image: &mut [u8]) -> Result<(), RuntimeError> {
        self.in_scan.set(true);
        self.inner.read_inputs(image)
    }

    fn write_outputs(&mut self, image: &[u8]) -> Result<(), RuntimeError> {
        let result = self.inner.write_outputs(image);
        self.in_scan.set(false);
        result
    }

    fn write_safe_outputs(&mut self, image: &[u8]) -> Result<(), RuntimeError> {
        self.inner.write_safe_outputs(image)
    }

    fn driver_health(&self) -> Vec<IoDriverStatus> {
        self.inner.driver_health()
    }
}

/// The runner of one [`RetainFixture`]: the device loop over the installed program's engine cycle
/// and the RAM checkpoint flash.
type RetainRunner<'a> =
    Runner<TestLink, TestWait, EngineCycle<'a, TestClock, ScanWindowIo>, TestPlatform, TestClock>;

/// One runner's ports and program: the board fixture's clock, wait, link and process I/O, the A4
/// program admitted as a chip admits it, and the RAM checkpoint flash the test reads.
struct RetainFixture {
    clock: TestClock,
    wait: TestWait,
    link: TestLink,
    io: ScanWindowIo,
    prepared: PreparedModule,
    services: DeviceServices<TestClock>,
    flash: RamFlash,
    mode: RetainMode,
    save_interval: Duration,
    identity: CheckpointIdentity,
}

impl RetainFixture {
    /// A fixture over `flash` with the retain `mode` and `save_interval`.
    fn new(flash: RamFlash, mode: RetainMode, save_interval: Duration) -> Self {
        let ports = Fixture::with_program(A4_PROGRAM);
        let clock = ports.clock.clone();
        let in_scan = flash.in_scan.clone();
        Self {
            wait: ports.wait.clone(),
            link: ports.link.clone(),
            io: ScanWindowIo::new(TestIo::new(clock.clone()), in_scan),
            prepared: PreparedModule::from_bytes(A4_PROGRAM, device_limits(CHIP_MEMORY))
                .expect("the A4 program is admitted"),
            services: DeviceServices::new(clock.clone()),
            clock,
            flash,
            mode,
            save_interval,
            identity: IDENTITY,
        }
    }

    /// The fixture with `identity` as the installed application's checkpoint identity, so a test
    /// can prove another retain schema is applied by name.
    fn with_identity(mut self, identity: CheckpointIdentity) -> Self {
        self.identity = identity;
        self
    }

    /// A runner that starts the program cold or warm from the checkpoint log, as the composition
    /// starts it before the loop.
    fn build(&self, warm: bool) -> RetainRunner<'_> {
        let mut platform = TestPlatform::default();
        platform.set_checkpoint(Box::new(self.flash.clone()), PROGRAM_BUDGET);
        let store = platform
            .take_checkpoint_store()
            .expect("the test platform supplies its checkpoint store");
        let settings = CycleSettings {
            cycle_interval: Duration::from_millis(10),
            watchdog: WatchdogPolicy::default(),
            fault_policy: FaultPolicy::Halt,
            safe_outputs: Vec::new(),
        };
        let mut cycle = EngineCycle::with_retain(
            &self.prepared,
            &self.services,
            self.io.clone(),
            settings,
            RetainConfig {
                mode: self.mode,
                store,
                identity: self.identity,
                save_interval: self.save_interval,
                image_bytes: IMAGE_BYTES,
            },
        )
        .expect("the program starts");
        if warm {
            cycle
                .restart(RestartMode::Warm, RetainSource::Checkpoint)
                .expect("the warm start loads the checkpoint");
        }
        Runner::new(
            self.link.clone(),
            self.wait.clone(),
            cycle,
            platform,
            self.clock.clone(),
        )
    }

    /// Moves the clock to `at` milliseconds and runs one pass.
    fn pass_at(&self, runner: &mut RetainRunner<'_>, at: u64) {
        self.clock.set(ms(at));
        runner.run_pass().expect("the test wait does not fail");
    }

    /// Sends `request` at `at` milliseconds, runs one pass and returns the reply.
    fn call_at(
        &self,
        runner: &mut RetainRunner<'_>,
        at: u64,
        request: &serde_json::Value,
    ) -> serde_json::Value {
        self.clock.set(ms(at));
        self.link.send(request);
        runner.run_pass().expect("the test wait does not fail");
        let mut replies = self.link.take_replies();
        assert_eq!(replies.len(), 1, "one reply to {request}");
        replies.remove(0)
    }
}

/// The A4 program's `Plant` field `name` in the installed program.
fn plant_field(runner: &RetainRunner<'_>, name: &str) -> Value {
    let Program::Installed(cycle) = &runner.cycle else {
        panic!("the A4 program is installed");
    };
    let storage = cycle.state().storage();
    let Some(Value::Instance(plant)) = storage.get_global("Plant") else {
        panic!("the A4 program has the program instance Plant");
    };
    storage
        .get_instance_var(*plant, name)
        .cloned()
        .unwrap_or_else(|| panic!("Plant has {name}"))
}

/// The A4 program's retained `activations` in the installed program.
fn retained_activations(runner: &RetainRunner<'_>) -> Value {
    plant_field(runner, "activations")
}

/// The six scans of the A4 program's 25 ms task sampled every 10 ms, which reach `activations` 2
/// (the device engine's contract tests pin the same times).
const SCANS: [u64; 6] = [0, 10, 20, 30, 40, 50];

/// The newest committed checkpoint of `flash`, as a boot's recovery rule finds it
/// (specification 34 §10.1).
fn committed(flash: &RamFlash) -> Option<RecordInfo> {
    let mut log = CheckpointLog::open(flash.clone()).expect("the checkpoint log opens");
    let mut payload = vec![0; IMAGE_BYTES];
    log.recover(&mut payload).expect("the checkpoint recovers")
}

/// A cold runner over `flash` that scans the A4 program to `activations` 2 and then stops, so its
/// stop save commits the checkpoint the next warm start reads.
fn stop_after_two_activations(flash: &RamFlash) {
    let fixture = RetainFixture::new(
        flash.clone(),
        RetainMode::File,
        Duration::from_millis(60_000),
    );
    let mut runner = fixture.build(false);
    for at in SCANS {
        fixture.pass_at(&mut runner, at);
    }
    assert_eq!(retained_activations(&runner), Value::DInt(2));
    let _ = fixture.call_at(&mut runner, 52, &json!({"id": 1, "type": "shutdown"}));
    fixture.pass_at(&mut runner, 60);
    assert_eq!(runner.resource_state(), ResourceState::Stopped);
}

#[test]
fn a_retained_value_survives_a_restart_through_the_checkpoint_log() {
    // Spec 34 §10: a stop saves the newest retained image and a warm start from the checkpoint log
    // restores it; spec 11 §6.7: the values are matched by name.
    let flash = RamFlash::new(f407());
    let fixture = RetainFixture::new(
        flash.clone(),
        RetainMode::File,
        Duration::from_millis(60_000),
    );
    let mut runner = fixture.build(false);
    for at in SCANS {
        fixture.pass_at(&mut runner, at);
    }
    assert_eq!(retained_activations(&runner), Value::DInt(2));

    // The requested stop completes the save before the resource reports Stopped.
    let _ = fixture.call_at(&mut runner, 52, &json!({"id": 1, "type": "shutdown"}));
    fixture.pass_at(&mut runner, 60);
    assert_eq!(runner.resource_state(), ResourceState::Stopped);
    drop(runner);
    assert!(flash.writes() > 0, "the stop wrote a checkpoint");

    // A new runner on the same flash starts warm with the retained value and the other values with
    // their declared initial values.
    let fixture = RetainFixture::new(
        flash.clone(),
        RetainMode::File,
        Duration::from_millis(60_000),
    );
    let runner = fixture.build(true);
    assert_eq!(retained_activations(&runner), Value::DInt(2));
    assert_eq!(plant_field(&runner, "result"), Value::Int(0));

    // A checkpoint of another retain schema goes through the by-name rules, as a computer's retain
    // file does: the new runner restores the same value.
    let other = CheckpointIdentity {
        application: APPLICATION,
        schema: [0x33; 16],
    };
    let fixture = RetainFixture::new(
        flash.clone(),
        RetainMode::File,
        Duration::from_millis(60_000),
    )
    .with_identity(other);
    let runner = fixture.build(true);
    assert_eq!(retained_activations(&runner), Value::DInt(2));
}

#[test]
fn a_power_cut_during_a_save_restores_the_previous_checkpoint() {
    // Spec 34 §10, RETAIN-02: a save interrupted by a power cut loses at most the save in
    // progress; the newest committed checkpoint stays readable.
    let flash = RamFlash::new(f407());
    let fixture = RetainFixture::new(flash.clone(), RetainMode::File, Duration::from_millis(25));
    let mut runner = fixture.build(false);
    // The first save is due at 30 ms with `activations` 1 and commits six steps later, at 80 ms.
    for at in [0, 10, 20, 30, 40, 50, 60, 70, 80] {
        fixture.pass_at(&mut runner, at);
    }
    assert_eq!(retained_activations(&runner), Value::DInt(3));
    assert!(flash.writes() > 0, "the first checkpoint committed");

    // Power fails: every later program and erase is refused. A newer save starts and its first
    // step is refused, so it never commits.
    flash.fail_writes();
    fixture.pass_at(&mut runner, 90);
    drop(runner);

    // A new runner on the same flash restores the previous committed checkpoint, not a partial one.
    let fixture = RetainFixture::new(flash.clone(), RetainMode::File, Duration::from_millis(25));
    let runner = fixture.build(true);
    assert_eq!(retained_activations(&runner), Value::DInt(1));
}

#[test]
fn steps_run_only_between_scans() {
    // Spec 34 §10: the persister advances one log step at a time between scans, never inside one.
    let flash = RamFlash::new(f407());
    let fixture = RetainFixture::new(flash.clone(), RetainMode::File, Duration::ZERO);
    let mut runner = fixture.build(false);
    for at in SCANS {
        fixture.pass_at(&mut runner, at);
    }
    assert!(flash.writes() > 0, "a checkpoint was written");
    assert_eq!(
        flash.writes_inside_scan(),
        0,
        "a checkpoint step ran inside a scan"
    );
}

#[test]
fn stop_completes_the_save_before_stopped() {
    // Spec 11 §6.7, spec 34 §10: the stop save completes before the resource reports Stopped.
    let flash = RamFlash::new(f407());
    let fixture = RetainFixture::new(
        flash.clone(),
        RetainMode::File,
        Duration::from_millis(60_000),
    );
    let mut runner = fixture.build(false);
    for at in SCANS {
        fixture.pass_at(&mut runner, at);
    }
    assert_eq!(
        flash.writes(),
        0,
        "no periodic save is due within the 60 s interval"
    );

    let _ = fixture.call_at(&mut runner, 52, &json!({"id": 1, "type": "shutdown"}));
    fixture.pass_at(&mut runner, 60);
    assert_eq!(runner.resource_state(), ResourceState::Stopped);
    let after_stop = flash.writes();
    assert!(after_stop > 0, "the stop wrote the newest checkpoint");
    // While Stopped, a further pass writes nothing more.
    fixture.pass_at(&mut runner, 70);
    assert_eq!(flash.writes(), after_stop, "a stopped resource saved again");

    // The checkpoint the stop wrote restores the values the program held.
    drop(runner);
    let fixture = RetainFixture::new(
        flash.clone(),
        RetainMode::File,
        Duration::from_millis(60_000),
    );
    let runner = fixture.build(true);
    assert_eq!(retained_activations(&runner), Value::DInt(2));
}

#[test]
fn a_save_failure_fails_the_next_scan_before_outputs() {
    // Spec 11 §6.7, spec 34 §10: a latched retain failure fails the next scan before physical
    // outputs, through the runner's fault path and the project's fault policy.
    let flash = RamFlash::new(f407());
    let fixture = RetainFixture::new(flash.clone(), RetainMode::File, Duration::ZERO);
    let mut runner = fixture.build(false);
    fixture.pass_at(&mut runner, 0);
    let _ = fixture.io.take_steps();

    // The next step fails and latches without failing the running resource.
    flash.fail_writes();
    fixture.pass_at(&mut runner, 10);
    assert_eq!(runner.resource_state(), ResourceState::Running);
    let _ = fixture.io.take_steps();

    // The next scan fails with the latched RetainStore before its outputs are published.
    fixture.pass_at(&mut runner, 20);
    assert_eq!(runner.resource_state(), ResourceState::Faulted);
    assert_eq!(
        runner.last_error(),
        Some(RuntimeError::RetainStore(
            "checkpoint flash program failed".into()
        ))
    );
    let steps = fixture.io.take_steps();
    assert!(
        steps.contains(&IoStep::ReadInputs),
        "the scan ran to its output sample: {steps:?}"
    );
    assert!(
        !steps
            .iter()
            .any(|step| matches!(step, IoStep::WriteOutputs(_))),
        "no physical output was committed: {steps:?}"
    );
}

#[test]
fn mode_none_saves_nothing() {
    // Spec 34 §10, spec 11 §6.7: a `[runtime.retain] mode = "none"` project persists nothing.
    let flash = RamFlash::new(f407());
    let fixture = RetainFixture::new(flash.clone(), RetainMode::None, Duration::ZERO);
    let mut runner = fixture.build(false);
    for at in SCANS {
        fixture.pass_at(&mut runner, at);
    }
    assert_eq!(retained_activations(&runner), Value::DInt(2));
    assert_eq!(flash.writes(), 0, "mode none wrote a checkpoint");

    // A warm start has no checkpoint to load: the retained values start with their initial values.
    drop(runner);
    let fixture = RetainFixture::new(flash.clone(), RetainMode::None, Duration::ZERO);
    let runner = fixture.build(true);
    assert_eq!(retained_activations(&runner), Value::DInt(0));
}

#[test]
fn a_warm_start_with_unchanged_values_writes_nothing() {
    // Spec 11 §6.7: a retain store writes only when the retained image changed, so a chip that
    // starts warm does not write the checkpoint it has just restored, and its flash does not wear
    // with a rewritten checkpoint on every start.
    let flash = RamFlash::new(f407());
    stop_after_two_activations(&flash);
    let saved = committed(&flash).expect("the stop save committed a checkpoint");
    assert_eq!((saved.sequence, saved.identity), (1, IDENTITY));
    let after_stop = flash.writes();
    assert!(after_stop > 0, "the stop wrote a checkpoint");

    // The new runner starts warm from that checkpoint. The A4 program's 25 ms task first activates
    // at 25 ms, so its scans at 10 ms and 20 ms pass the 10 ms save interval with the restored image
    // unchanged, and the stop at 30 ms saves the same image again.
    let fixture = RetainFixture::new(flash.clone(), RetainMode::File, Duration::from_millis(10));
    let mut runner = fixture.build(true);
    assert_eq!(retained_activations(&runner), Value::DInt(2));
    for at in [0, 10, 20] {
        fixture.pass_at(&mut runner, at);
    }
    assert_eq!(
        retained_activations(&runner),
        Value::DInt(2),
        "the program changed the retained value, so the image was not unchanged"
    );
    let _ = fixture.call_at(&mut runner, 21, &json!({"id": 1, "type": "shutdown"}));
    fixture.pass_at(&mut runner, 30);
    assert_eq!(runner.resource_state(), ResourceState::Stopped);
    drop(runner);

    assert_eq!(
        flash.writes(),
        after_stop,
        "a warm start with an unchanged image programmed or erased the flash"
    );
    let newest = committed(&flash).expect("the checkpoint recovers");
    assert_eq!((newest.sequence, newest.identity), (1, IDENTITY));
}

#[test]
fn a_warm_start_under_a_new_identity_writes_once() {
    // Spec 34 §10.1, spec 11 §6.7: a checkpoint written for another retain schema is not the image
    // this program last wrote, so the first due save writes the restored image once under the
    // installed identity, and a later restart under that identity writes nothing.
    let new = CheckpointIdentity {
        application: APPLICATION,
        schema: [0x33; 16],
    };
    let flash = RamFlash::new(f407());
    stop_after_two_activations(&flash);
    let saved = committed(&flash).expect("the stop save committed a checkpoint");
    assert_eq!((saved.sequence, saved.identity), (1, IDENTITY));

    let fixture = RetainFixture::new(flash.clone(), RetainMode::File, Duration::from_millis(10))
        .with_identity(new);
    let mut runner = fixture.build(true);
    assert_eq!(retained_activations(&runner), Value::DInt(2));
    for at in [0, 10, 20] {
        fixture.pass_at(&mut runner, at);
    }
    let _ = fixture.call_at(&mut runner, 21, &json!({"id": 1, "type": "shutdown"}));
    fixture.pass_at(&mut runner, 30);
    assert_eq!(runner.resource_state(), ResourceState::Stopped);
    drop(runner);
    // One checkpoint, under the installed identity: the first due save wrote the restored image,
    // and the later due saves and the stop wrote nothing.
    let rewritten = committed(&flash).expect("the new checkpoint recovers");
    assert_eq!((rewritten.sequence, rewritten.identity), (2, new));

    // A runner under that identity starts warm from the checkpoint written for it and writes
    // nothing: the checkpoint it restored carries its identity.
    let fixture = RetainFixture::new(flash.clone(), RetainMode::File, Duration::from_millis(10))
        .with_identity(new);
    let mut runner = fixture.build(true);
    assert_eq!(retained_activations(&runner), Value::DInt(2));
    let after_restart = flash.writes();
    for at in [0, 10, 20] {
        fixture.pass_at(&mut runner, at);
    }
    let _ = fixture.call_at(&mut runner, 21, &json!({"id": 1, "type": "shutdown"}));
    fixture.pass_at(&mut runner, 30);
    assert_eq!(runner.resource_state(), ResourceState::Stopped);
    drop(runner);
    assert_eq!(
        flash.writes(),
        after_restart,
        "a checkpoint written for the installed identity was written again"
    );
    let newest = committed(&flash).expect("the checkpoint recovers");
    assert_eq!((newest.sequence, newest.identity), (2, new));
}
