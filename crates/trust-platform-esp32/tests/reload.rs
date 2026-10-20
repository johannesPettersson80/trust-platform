//! A chip loads a newer installed program in place (specification 34 STATE-08 and §11.2), on the C6's own
//! loader and program memory, over its real flash layers (the program slots, and the checkpoint writer
//! that is handed out one at a time) on a RAM flash, with the shared resource loop above them. The loop's ports are fakes: a link that carries the JSON requests, a wait that never
//! sleeps, a clock the test moves, a watchdog that counts, and GPIO lines that hold pin claims.
//!
//! A counting global allocator records the bytes live on the test's thread, so the heap after a round of
//! install and restart can be compared with the heap after the first. Every expected value is written
//! out here from the specification, not taken from the code under test.

use std::alloc::{GlobalAlloc, Layout, System};
use std::cell::{Cell, RefCell};
use std::collections::VecDeque;
use std::rc::Rc;
use std::time::Duration as StdDuration;

use serde_json::{json, Value};
use smol_str::SmolStr;
use trust_platform_esp32::checkpoint::RawFlash;
use trust_platform_esp32::loader::{
    install_check, CycleWatchdog, Esp32Loader, ProgramHardware, ONLINE_CHANGE_WORKING_MEMORY_BYTES,
};
use trust_platform_esp32::process_io::{
    check_pin, GpioPin, PinClaim, PinClaims, CHIP_GPIO_PINS, CHIP_GPIO_RESERVED,
};
use trust_platform_esp32::profile::{runtime_id, transfer_session, PROGRAM_STEP_BYTES};
use trust_platform_esp32::program_flash::FlashOwner;
use trust_platform_esp32::program_memory::ProgramCycle;
use trust_platform_esp32::watchdog_rule::watchdog_timeout_ms;
use trust_platform_esp32::Platform;
use trust_runtime_core::bytecode::BytecodeVersion;
use trust_runtime_core::crc32;
use trust_runtime_core::deployment::{
    sha256, ApplicationIdentity, Capabilities, Configuration, Contracts, DeploymentRecord,
    GpioInput, GpioOutput, IoDriver, NumericContract, ProcessImageSummary, RequestedLimits,
    SchedulingProfile, TimingEvidence,
};
use trust_runtime_core::error::RuntimeError;
use trust_runtime_core::io_address::IoAddress;
use trust_runtime_core::ports::{
    ControlLink, HardwareWatchdog, LinkFrame, MonotonicClock, MonotonicInstant, PeerClass,
    PortError, ProgramStore, RunnerWait, TimeSpan, WakeReason, WakeToken,
};
use trust_runtime_core::retain::checkpoint::{CheckpointFlash, FlashError};
use trust_runtime_core::scheduler::ResourceState;
use trust_runtime_core::value::Duration;
use trust_runtime_core::vm::{PreparationLimits, PreparedModule};
use trust_runtime_core::watchdog::{RetainMode, WatchdogPolicy};
use trust_runtime_services::capabilities::{LastReset, ResetCauseName};
use trust_runtime_services::chip::platform::status_settings;
use trust_runtime_services::chip::process_io::record_lines;
use trust_runtime_services::installation::encode_sequenced;
use trust_runtime_services::runner::engine::{DeviceMemory, ProcessIo};
use trust_runtime_services::runner::{EngineLiveValues, Runner};
use trust_runtime_services::status::IoDriverStatus;

/// The installed program of these tests: the fixture `program-v2.stbc`, which has the retained
/// `activations` of its program `Plant` (6,412 bytes).
const PROGRAM: &[u8] =
    include_bytes!("../../trust-runtime/tests/fixtures/portability/stbc-2.0/program-v2.stbc");

// ---- the counting allocator ---------------------------------------------------------------------

thread_local! {
    static ARMED: Cell<bool> = const { Cell::new(false) };
    static LIVE: Cell<i64> = const { Cell::new(0) };
}

/// Adds `change` to the bytes live on this thread while it is armed. Only const-initialized `Cell`s are
/// touched, so counting never allocates, blocks or unwinds.
fn record(change: i64) {
    let _ = ARMED.try_with(|armed| {
        if armed.get() {
            let _ = LIVE.try_with(|live| live.set(live.get().saturating_add(change)));
        }
    });
}

fn bytes(size: usize) -> i64 {
    i64::try_from(size).unwrap_or(i64::MAX)
}

struct CountingAllocator;

// SAFETY: every operation forwards the caller's unchanged layout, pointer and new size to `System`;
// the counting touches only const-initialized thread-local `Cell`s.
unsafe impl GlobalAlloc for CountingAllocator {
    unsafe fn alloc(&self, layout: Layout) -> *mut u8 {
        record(bytes(layout.size()));
        // SAFETY: the caller supplies a valid allocation layout.
        unsafe { System.alloc(layout) }
    }

    unsafe fn dealloc(&self, pointer: *mut u8, layout: Layout) {
        record(-bytes(layout.size()));
        // SAFETY: `pointer` and `layout` are the pair `System` returned.
        unsafe { System.dealloc(pointer, layout) }
    }

    unsafe fn realloc(&self, pointer: *mut u8, layout: Layout, new_size: usize) -> *mut u8 {
        // SAFETY: `System` returned `pointer` for `layout`; `new_size` is the caller's new size.
        let moved = unsafe { System.realloc(pointer, layout, new_size) };
        if !moved.is_null() {
            record(bytes(new_size) - bytes(layout.size()));
        }
        moved
    }
}

#[global_allocator]
static ALLOCATOR: CountingAllocator = CountingAllocator;

/// Starts counting the bytes live on this thread from zero.
fn arm() {
    LIVE.with(|live| live.set(0));
    ARMED.with(|armed| armed.set(true));
}

/// The bytes live on this thread since [`arm`].
fn live_bytes() -> i64 {
    LIVE.with(Cell::get)
}

// ---- the loop's ports ---------------------------------------------------------------------------

thread_local! {
    static NOW_MS: Cell<u64> = const { Cell::new(0) };
}

/// The monotonic clock of the test's thread, which the test moves.
#[derive(Clone, Copy)]
struct TestClock;

impl TestClock {
    fn set(milliseconds: u64) {
        NOW_MS.with(|now| now.set(milliseconds));
    }

    fn now_ms() -> u64 {
        NOW_MS.with(Cell::get)
    }
}

impl MonotonicClock for TestClock {
    fn now(&self) -> MonotonicInstant {
        MonotonicInstant::from_origin(StdDuration::from_millis(Self::now_ms()))
    }
}

/// A wait that never sleeps: the test runs one pass at a time.
#[derive(Clone, Copy)]
struct TestWait;

impl RunnerWait for TestWait {
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

/// The control link: the test queues request frames and takes the replies. Frames of up to 8,192
/// bytes, as the chip's USB link moves.
#[derive(Clone, Default)]
struct TestLink(Rc<RefCell<LinkState>>);

impl TestLink {
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

impl ControlLink for TestLink {
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

#[derive(Default)]
struct DogState {
    feeds: u32,
    /// The hardware intervals, in milliseconds, that the loader set.
    intervals: Vec<u32>,
}

/// A hardware watchdog that counts its feeds and keeps the intervals the loader set, with the C6's
/// range: 1 to 60,000 ms (specification 34 TIME-04).
#[derive(Clone)]
struct Dog(Rc<RefCell<DogState>>);

impl HardwareWatchdog for Dog {
    fn feed_interval(&self) -> TimeSpan {
        TimeSpan::new(StdDuration::from_millis(1_000))
    }

    fn feed(&mut self) {
        self.0.borrow_mut().feeds += 1;
    }
}

impl CycleWatchdog for Dog {
    fn apply_cycle_timeout_ms(&mut self, cycle_timeout_ms: u32) -> bool {
        let interval = watchdog_timeout_ms(cycle_timeout_ms);
        let in_range = (1..=60_000).contains(&interval);
        if in_range {
            self.0.borrow_mut().intervals.push(interval);
        }
        in_range
    }
}

/// A flash from address 0 to the end of the 8 MB module, erased, with the C6's units.
struct RamFlash {
    bytes: Vec<u8>,
}

impl RawFlash for RamFlash {
    fn read_at(&mut self, address: u32, bytes: &mut [u8]) -> Result<(), FlashError> {
        let start = address as usize;
        let stored = self
            .bytes
            .get(start..start + bytes.len())
            .ok_or(FlashError::Read)?;
        bytes.copy_from_slice(stored);
        Ok(())
    }

    fn program_at(&mut self, address: u32, bytes: &[u8]) -> Result<(), FlashError> {
        let start = address as usize;
        let stored = self
            .bytes
            .get_mut(start..start + bytes.len())
            .ok_or(FlashError::Program)?;
        for (cell, byte) in stored.iter_mut().zip(bytes) {
            *cell &= byte;
        }
        Ok(())
    }

    fn erase_at(&mut self, address: u32) -> Result<(), FlashError> {
        let start = address as usize;
        let unit = self
            .bytes
            .get_mut(start..start + 4_096)
            .ok_or(FlashError::Erase)?;
        unit.fill(0xFF);
        Ok(())
    }
}

/// The process I/O of a program: it holds the claims on its pins, and frees them when it drops.
struct Lines {
    _claims: Vec<PinClaim>,
}

impl ProcessIo for Lines {
    fn read_inputs(&mut self, _image: &mut [u8]) -> Result<(), RuntimeError> {
        Ok(())
    }

    fn write_outputs(&mut self, _image: &[u8]) -> Result<(), RuntimeError> {
        Ok(())
    }

    fn write_safe_outputs(&mut self, _image: &[u8]) -> Result<(), RuntimeError> {
        Ok(())
    }

    fn driver_health(&self) -> Vec<IoDriverStatus> {
        Vec::new()
    }
}

/// The chip's hardware as the loader sees it: the GPIO pins, claimed through the same pin checks and
/// claim set as the C6's, and the real flash owner, which hands the checkpoint slots to one writer at a
/// time.
struct Hardware {
    owner: FlashOwner<RamFlash>,
    claims: PinClaims,
}

impl ProgramHardware for Hardware {
    type Io = Lines;

    fn claim_lines(&mut self, config: &Configuration) -> Result<Lines, String> {
        let (inputs, outputs) = record_lines(config, "C6")?;
        let reserved = CHIP_GPIO_RESERVED.map(GpioPin::from_number);
        let mut claimed = Vec::new();
        let names = inputs
            .iter()
            .map(|line| line.line.as_str())
            .chain(outputs.iter().map(|line| line.line.as_str()));
        for name in names {
            let claim = check_pin(name, CHIP_GPIO_PINS, &reserved)
                .and_then(|pin| self.claims.claim(pin))
                .map_err(|error| {
                    format!("the installed application's GPIO lines cannot be claimed: {error}")
                })?;
            claimed.push(claim);
        }
        Ok(Lines { _claims: claimed })
    }

    fn checkpoint_flash(&mut self) -> Result<Box<dyn CheckpointFlash>, String> {
        let writer = self
            .owner
            .take_checkpoint()
            .map_err(|error| format!("the chip's checkpoint flash is not available: {error:?}"))?
            .ok_or_else(|| {
                String::from("the chip's checkpoint flash is not available: it is still in use")
            })?;
        Ok(Box::new(writer))
    }
}

thread_local! {
    /// The bytes of heap that are free now, which a test sets. Far more than any program of these
    /// tests unless a test lowers it.
    static ROOM: Cell<usize> = const { Cell::new(1 << 20) };
}

/// The room the heap has for a container and for the preparation of a program.
fn room() -> usize {
    ROOM.with(Cell::get)
}

/// The device's memory for the limits of a program in these tests: a 64 KiB region and the bring-up
/// heap.
const MEMORY: DeviceMemory = DeviceMemory {
    region_bytes: 64 * 1024,
    heap_bytes: 72 * 1024,
};

type TestRunner =
    Runner<TestLink, TestWait, ProgramCycle<TestClock, Lines>, Platform<Dog>, TestClock>;

/// One chip: the loop over the C6's loader, program memory and flash layers.
struct Chip {
    runner: TestRunner,
    link: TestLink,
    claims: PinClaims,
    dog: Dog,
}

impl Chip {
    /// A chip that booted without a program, as a chip with an erased store does, with the loader of the
    /// C6 and an empty flash.
    fn without_program() -> Self {
        Self::booted(None)
    }

    /// A chip as [`Self::without_program`] builds it, whose firmware measured `program_memory` bytes for
    /// programs at boot (specification 34 MEM-12), or none.
    fn booted(program_memory: Option<usize>) -> Self {
        let link = TestLink::default();
        // The watchdog keeps the intervals the loader set; its list has room for them all, so that
        // the test's own bookkeeping does not grow the heap that the test measures.
        let dog = Dog(Rc::new(RefCell::new(DogState {
            feeds: 0,
            intervals: Vec::with_capacity(64),
        })));
        let claims = PinClaims::default();
        let mut owner = FlashOwner::new(RamFlash {
            bytes: vec![0xFF; 0x80_0000],
        });
        let program_flash = owner
            .take_program()
            .expect("the program slots have a geometry")
            .expect("the program slots are handed out");
        let reader = owner
            .checkpoint_reader()
            .expect("the checkpoint slots have a geometry");
        let store = ProgramStore {
            flash: Box::new(program_flash),
            program_budget: PROGRAM_STEP_BYTES,
        };
        let hardware = Hardware {
            owner,
            claims: claims.clone(),
        };
        let loader = Esp32Loader::new(hardware, TestClock, MEMORY, room);
        let mut platform = Platform::new(
            dog.clone(),
            SmolStr::new_static("Controller"),
            status_settings(),
            false,
            trust_runtime_core::deployment::ControlMode::Production,
            EngineLiveValues::empty(),
            LastReset {
                cause: ResetCauseName::PowerOn,
                text: SmolStr::default(),
                boot_count: 1,
            },
            runtime_id([0x20, 0x6E, 0xF1, 0x17, 0x04, 0xC0]),
        );
        if let Some(bytes) = program_memory {
            platform.set_program_memory(bytes);
        }
        let runner = Runner::stopped(
            link.clone(),
            TestWait,
            platform,
            TestClock,
            SmolStr::new_static("no TRSTA002 container in either slot"),
            200,
        )
        .with_transfers(transfer_session(1))
        .with_program_store(store, install_check(Box::new(reader), MEMORY, room))
        .with_program_loader(loader, None);
        Self {
            runner,
            link,
            claims,
            dog,
        }
    }

    /// Sends `request`, runs one pass and returns the one reply.
    fn call(&mut self, request: &Value) -> Value {
        self.link.send(request);
        self.runner.run_pass().expect("the test wait does not fail");
        let mut replies = self.link.take_replies();
        assert_eq!(replies.len(), 1, "one reply to {request}: {replies:?}");
        replies.remove(0)
    }

    /// Moves the clock by `by` milliseconds and runs one pass.
    fn pass(&mut self, by: u64) {
        TestClock::set(TestClock::now_ms() + by);
        self.runner.run_pass().expect("the test wait does not fail");
    }

    /// Uploads `container` as one transfer in parts of 4,096 bytes, asks for its install and runs passes
    /// until the loop answers, and returns that answer. The clock does not move.
    fn install(&mut self, container: &[u8]) -> Value {
        let begin = self.call(&json!({
            "id": 1,
            "type": "transfer.begin",
            "params": {"bytes": container.len(), "sha256": hex(&sha256(container))},
        }));
        let transfer = begin["result"]["transfer"]
            .as_str()
            .expect("begin names the transfer")
            .to_string();
        for (index, part) in container.chunks(4_096).enumerate() {
            let reply = self.call(&json!({
                "id": 2 + index,
                "type": "transfer.part",
                "params": {
                    "transfer": transfer,
                    "offset": index * 4_096,
                    "data": base64(part),
                    "crc32": format!("{:08x}", crc32::checksum(part)),
                },
            }));
            assert_eq!(reply["ok"], json!(true), "part {index} is stored: {reply}");
        }
        self.link.send(&json!({
            "id": 900,
            "type": "program.install",
            "params": {"transfer": transfer},
        }));
        for _ in 0..1_000 {
            self.runner.run_pass().expect("the test wait does not fail");
            if let Some(reply) = self.link.take_replies().pop() {
                return reply;
            }
        }
        panic!("the install was not answered within 1,000 passes");
    }

    /// `shutdown`, then a pass at the program's next boundary, which takes the stop.
    fn stop(&mut self) {
        let reply = self.call(&json!({"id": 901, "type": "shutdown"}));
        assert_eq!(reply["result"], json!({"status": "stopping"}));
        self.pass(10);
        assert_eq!(self.runner.resource_state(), ResourceState::Stopped);
    }

    /// `restart` in `mode`: the loop answers `restart queued` and loads the installed container.
    fn restart(&mut self, mode: &str) -> Value {
        self.call(&json!({"id": 902, "type": "restart", "params": {"mode": mode}}))
    }

    /// Runs `scans` passes of 10 ms each.
    fn scan(&mut self, scans: u32) {
        for _ in 0..scans {
            self.pass(10);
        }
    }
}

// ---- containers and requests --------------------------------------------------------------------

fn hex(bytes: &[u8]) -> String {
    bytes.iter().map(|byte| format!("{byte:02x}")).collect()
}

/// Standard base64 with padding (RFC 4648), which `transfer.part` carries its bytes in.
fn base64(bytes: &[u8]) -> String {
    const ALPHABET: &[u8; 64] = b"ABCDEFGHIJKLMNOPQRSTUVWXYZabcdefghijklmnopqrstuvwxyz0123456789+/";
    let mut text = String::new();
    for chunk in bytes.chunks(3) {
        let word = (u32::from(chunk[0]) << 16)
            | (chunk.get(1).map_or(0, |byte| u32::from(*byte)) << 8)
            | chunk.get(2).map_or(0, |byte| u32::from(*byte));
        let sextet = |shift: u32| char::from(ALPHABET[((word >> shift) & 63) as usize]);
        text.push(sextet(18));
        text.push(sextet(12));
        text.push(if chunk.len() > 1 { sextet(6) } else { '=' });
        text.push(if chunk.len() > 2 { sextet(0) } else { '=' });
    }
    text
}

/// The `gpio` driver of a record with the given input and output pins, in `io.toml` order.
fn gpio(inputs: &[&str], outputs: &[&str]) -> IoDriver {
    let address = |text: &str| IoAddress::parse(text).expect("an address");
    IoDriver {
        name: SmolStr::new_static("gpio"),
        enabled: true,
        inputs: inputs
            .iter()
            .enumerate()
            .map(|(index, pin)| GpioInput {
                address: address(&format!("%IX0.{index}")),
                line: SmolStr::new(pin),
                invert: false,
                debounce_ms: 0,
            })
            .collect(),
        outputs: outputs
            .iter()
            .enumerate()
            .map(|(index, pin)| GpioOutput {
                address: address(&format!("%QX0.{index}")),
                line: SmolStr::new(pin),
                invert: false,
                initial: false,
            })
            .collect(),
    }
}

/// The `TRSTA002` container of [`PROGRAM`] with the configuration `configuration`, as a client uploads
/// it: the device replaces the sequence 0.
fn container(configuration: Configuration) -> Vec<u8> {
    let record = DeploymentRecord {
        application: ApplicationIdentity {
            program_sha256: sha256(PROGRAM),
            program_bytes: u32::try_from(PROGRAM.len()).expect("a short program"),
            bytecode_version: BytecodeVersion::SOURCE_FREE_V2,
            bytecode_flags: 1,
            configuration_sha256: [0; 32],
            retain_schema_sha256: [0; 32],
        },
        contracts: Contracts {
            scheduling_profile: SchedulingProfile::COOPERATIVE_RESOURCE_V1,
            numeric_contract: NumericContract::V1,
            timing_evidence: TimingEvidence::UNQUALIFIED,
            capabilities: Capabilities::NONE,
        },
        limits: RequestedLimits::from_preparation(&PreparationLimits::default()),
        process_image: ProcessImageSummary::default(),
        tasks: Vec::new(),
        imports: Vec::new(),
        configuration,
    }
    .encode()
    .expect("the record encodes");
    encode_sequenced(0, PROGRAM, &record).expect("the container encodes")
}

/// A configuration that keeps retained values, so that every program takes the checkpoint flash.
fn retaining(drivers: Vec<IoDriver>) -> Configuration {
    Configuration {
        retain_mode: RetainMode::File,
        retain_save_interval: Duration::ZERO,
        io_drivers: drivers,
        ..Configuration::default()
    }
}

fn pins(chip: &Chip, names: &[&str]) -> Vec<bool> {
    names
        .iter()
        .map(|name| chip.claims.holds(GpioPin::parse(name).expect("a pin name")))
        .collect()
}

// ---- the tests ----------------------------------------------------------------------------------

#[test]
fn install_and_restart_rounds_leave_the_heap_where_one_round_left_it() {
    // Specification 34 STATE-08 and Appendix D.2: a `restart` that loads a newer container frees the
    // program that ended (its module, its services, its engine state, its lines and its checkpoint
    // handle) before it builds the next, so the heap after any number of rounds is where one round
    // left it. A round stops the program, installs the container over the link (a transfer in parts, a
    // commit in steps, a reply after the last one) and restarts warm; the container keeps retained
    // values, so every program takes the checkpoint flash the one before it held, and claims GPIO18,
    // which the one before it held. The heap is read after the stop of each round, with a program that
    // has run eight scans in memory, as the bytes live on this thread since the test began. Eight
    // rounds must leave exactly the bytes the first left. Wrong behaviour caught: a module or services
    // that are leaked per program (the `Box::leak` this design replaces would add the size of a
    // prepared module in every round), a program that is not dropped before the next is built, a
    // checkpoint writer or event store that grows, a transfer or container buffer that is kept.
    arm();
    TestClock::set(0);
    let mut chip = Chip::without_program();
    let configuration = || retaining(vec![gpio(&["GPIO9"], &["GPIO18"])]);

    // Room for every reading, so that the test's own bookkeeping does not grow the heap it measures.
    let mut after_round = Vec::with_capacity(16);
    for round in 0..8_u64 {
        let installed = chip.install(&container(configuration()));
        assert_eq!(installed["ok"], json!(true), "round {round}: {installed}");
        assert_eq!(installed["result"]["sequence"], json!(round + 1));
        let restarted = chip.restart("warm");
        assert_eq!(
            restarted["result"],
            json!({"status": "restart queued"}),
            "round {round}"
        );
        assert_eq!(
            chip.runner.resource_state(),
            ResourceState::Running,
            "round {round}"
        );
        chip.scan(8);
        chip.stop();
        after_round.push(live_bytes());
    }
    assert_eq!(
        pins(&chip, &["GPIO9", "GPIO18", "GPIO19"]),
        [true, true, false],
        "the program in memory holds its pins"
    );
    let first = after_round[0];
    for (round, heap) in after_round.iter().enumerate() {
        assert_eq!(
            *heap, first,
            "the heap after round {round} is {heap} bytes, where round 0 left {first}: {after_round:?}"
        );
    }
}

#[test]
fn the_new_program_claims_the_pins_the_old_one_released() {
    // Specification 34 STATE-08 and TARGET-01: the lines of the program that ended go back to the chip's
    // pins when it is dropped, before the next program claims its own. The first container names GPIO9
    // as an input and GPIO18 as an output; the second names GPIO18 again as an output and GPIO19 as
    // another. After the first load the claims are GPIO9 and GPIO18; after the second GPIO18 and GPIO19,
    // GPIO9 free. A third container names GPIO12, which the C6 reserves for USB: its load is refused
    // with the pin's reason, the program that ended is gone, and no pin is held, not even GPIO19 or
    // GPIO18 that the lines before the bad one claimed. The fourth container names GPIO18 again and
    // loads. Wrong behaviour caught: a pin that is not given back (the second load fails with `GPIO18
    // is already claimed by another line`), a release that frees a pin the program still holds, a
    // failed load that keeps the pins of the lines it claimed before the bad one.
    arm();
    TestClock::set(0);
    let mut chip = Chip::without_program();
    let all = ["GPIO9", "GPIO18", "GPIO19"];

    let first = chip.install(&container(retaining(vec![gpio(&["GPIO9"], &["GPIO18"])])));
    assert_eq!(first["ok"], json!(true), "{first}");
    chip.restart("cold");
    assert_eq!(chip.runner.resource_state(), ResourceState::Running);
    assert_eq!(pins(&chip, &all), [true, true, false]);

    chip.stop();
    let second = chip.install(&container(retaining(vec![gpio(
        &[],
        &["GPIO18", "GPIO19"],
    )])));
    assert_eq!(second["ok"], json!(true), "{second}");
    chip.restart("cold");
    assert_eq!(
        chip.runner.resource_state(),
        ResourceState::Running,
        "GPIO18 was claimed again: {:?}",
        chip.runner.last_error()
    );
    assert_eq!(pins(&chip, &all), [false, true, true]);

    chip.stop();
    let third = chip.install(&container(retaining(vec![gpio(
        &[],
        &["GPIO19", "GPIO12"],
    )])));
    assert_eq!(third["ok"], json!(true), "{third}");
    chip.restart("cold");
    assert_eq!(chip.runner.resource_state(), ResourceState::Faulted);
    assert_eq!(
        chip.runner.last_error(),
        Some(RuntimeError::ControlError(SmolStr::new(
            "no installed program: the installed application's GPIO lines cannot be claimed: GPIO12 is reserved by the firmware"
        )))
    );
    assert_eq!(
        pins(&chip, &all),
        [false, false, false],
        "a failed load holds no pin"
    );

    // A faulted resource takes no install; a reset would be needed. The program store still holds the
    // bad container as the newest, and the next restart tries it again with the same result.
    chip.restart("cold");
    assert_eq!(chip.runner.resource_state(), ResourceState::Faulted);
    assert_eq!(pins(&chip, &all), [false, false, false]);
}

#[test]
fn a_record_whose_watchdog_is_in_the_chips_range_sets_the_hardware_watchdog() {
    // Specification 34 TIME-04 and STATE-08: the hardware watchdog interval follows the record's cycle
    // watchdog, the larger of 4 s and the timeout plus 2 s, when a program is loaded in place. A record
    // with the cycle watchdog enabled at 5,000 ms sets 7,000 ms; the next record, with the watchdog
    // disabled, sets 4,000 ms. A record whose timeout (59,000 ms) gives an interval above the C6's
    // 60,000 ms is refused with the watchdog's reason, holds no pin, and leaves the interval as it was.
    // The loader feeds the hardware watchdog while it loads. Wrong behaviour caught: an interval taken
    // from the first program for the rest of the boot; a timeout outside the range that is applied;
    // lines claimed before the timeout was checked; a load that never feeds the watchdog.
    arm();
    TestClock::set(0);
    let mut chip = Chip::without_program();
    let with_watchdog = |timeout_ms: i64, drivers: Vec<IoDriver>| Configuration {
        watchdog: WatchdogPolicy {
            enabled: true,
            timeout: Duration::from_millis(timeout_ms),
            ..WatchdogPolicy::default()
        },
        io_drivers: drivers,
        ..Configuration::default()
    };

    let installed = chip.install(&container(with_watchdog(
        5_000,
        vec![gpio(&[], &["GPIO18"])],
    )));
    assert_eq!(installed["ok"], json!(true), "{installed}");
    let feeds_before = chip.dog.0.borrow().feeds;
    chip.restart("cold");
    assert_eq!(chip.runner.resource_state(), ResourceState::Running);
    assert_eq!(chip.dog.0.borrow().intervals, [7_000]);
    assert!(
        chip.dog.0.borrow().feeds >= feeds_before + 3,
        "the load fed the watchdog between its steps"
    );

    chip.stop();
    let installed = chip.install(&container(Configuration::default()));
    assert_eq!(installed["ok"], json!(true), "{installed}");
    chip.restart("cold");
    assert_eq!(chip.dog.0.borrow().intervals, [7_000, 4_000]);

    chip.stop();
    let installed = chip.install(&container(with_watchdog(
        59_000,
        vec![gpio(&[], &["GPIO18"])],
    )));
    assert_eq!(installed["ok"], json!(true), "{installed}");
    chip.restart("cold");
    assert_eq!(chip.runner.resource_state(), ResourceState::Faulted);
    assert_eq!(
        chip.runner.last_error(),
        Some(RuntimeError::ControlError(SmolStr::new(
            "no installed program: the installed application's watchdog timeout is outside the C6's range"
        )))
    );
    assert_eq!(
        chip.dog.0.borrow().intervals,
        [7_000, 4_000],
        "the interval did not change"
    );
    assert_eq!(pins(&chip, &["GPIO18"]), [false], "and no line was claimed");
}

#[test]
fn the_c6_refuses_an_install_whose_dry_run_does_not_fit_the_heap() {
    // Specification 34 §11.2 and §11.3 (UPDATE-06; the follow-up of slice S4-6): the check of an install
    // prepares the new program with the limits that the heap allows now, its room less the C6's reserve,
    // before the shared dry run that prepares it with the limits of its record, so a program that cannot
    // fit is refused cleanly instead of resetting the chip. The new program keeps retained values but
    // the log holds no committed checkpoint, so the reserve is the working memory, 8 KiB, and no
    // payload. With the room at the reserve plus 10,000 bytes, 10,000 remain, which allow no call
    // level, so the container of the A4 program (6,412 bytes) is refused with the numbers inside the
    // check's error, and not a byte of the program store is written (the commit comes after the
    // checks); with the room back to 1 MiB the same container installs. Wrong behaviour caught: an
    // install that is checked with the hosted limits of its record only, which on the C6's heap
    // resets the chip; a refusal that leaves the chip unable to install again; a check that does not
    // use the C6's reserve.
    arm();
    TestClock::set(0);
    let mut chip = Chip::without_program();
    let configuration = || retaining(vec![gpio(&["GPIO9"], &["GPIO18"])]);

    ROOM.with(|room| room.set(ONLINE_CHANGE_WORKING_MEMORY_BYTES + 10_000));
    let refused = chip.install(&container(configuration()));
    assert_eq!(refused["ok"], json!(false), "{refused}");
    assert_eq!(
        refused["error"],
        json!(format!(
            "the retained-value check could not run: control error 'insufficient memory: a program \
             of 6412 bytes needs more than the 10000 bytes of heap that remain for it ({} bytes are \
             free, {} are kept in reserve)'",
            ONLINE_CHANGE_WORKING_MEMORY_BYTES + 10_000,
            ONLINE_CHANGE_WORKING_MEMORY_BYTES
        ))
    );
    assert_eq!(PROGRAM.len(), 6_412, "the fixture program");

    ROOM.with(|room| room.set(1 << 20));
    let installed = chip.install(&container(configuration()));
    assert_eq!(installed["ok"], json!(true), "{installed}");
    assert_eq!(
        installed["result"]["sequence"],
        json!(1),
        "the refusal wrote nothing: the first commit takes the first sequence"
    );
}

#[test]
fn the_c6_refuses_an_install_whose_memory_figure_exceeds_its_program_memory() {
    // Specification 34 MEM-12 and §5.1.1: the C6 measures the memory it has for programs at boot, and
    // its install refuses, before any write, a program whose memory figure is more, naming both
    // numbers. The new program keeps retained values, so the capture of them counts. The figure is the
    // one preparation computes for the A4 program with the limits of its record, the default limits.
    // A chip whose memory equals the figure installs the same container. Wrong behaviour caught: an
    // install that skips the check, so that the chip runs out of heap when it loads the program; a
    // refusal that writes the program store; and a figure equal to the memory refused.
    let needed = PreparedModule::from_bytes(PROGRAM, PreparationLimits::default())
        .expect("the A4 program prepares")
        .memory_figure()
        .total(true);
    let configuration = || retaining(vec![gpio(&["GPIO9"], &["GPIO18"])]);
    arm();
    TestClock::set(0);
    let mut tight = Chip::booted(Some(needed - 1));
    let refused = tight.install(&container(configuration()));
    assert_eq!(refused["ok"], json!(false), "{refused}");
    assert_eq!(
        refused["error"],
        json!(format!(
            "deployment refused: the program needs {needed} bytes of memory, the device has {} for \
             programs",
            needed - 1
        ))
    );
    let capabilities = tight.call(&json!({"id": 3, "type": "capabilities"}));
    assert_eq!(
        capabilities["result"]["program_memory_bytes"],
        json!(needed - 1),
        "the chip reports the memory it measured"
    );

    let mut exact = Chip::booted(Some(needed));
    let installed = exact.install(&container(configuration()));
    assert_eq!(installed["ok"], json!(true), "{installed}");
    assert_eq!(
        installed["result"]["sequence"],
        json!(1),
        "the first commit takes the first sequence"
    );
}

#[test]
fn the_loop_takes_the_heap_its_type_gives_beside_any_program() {
    // Specification 34 MEM-12 and Appendix D.2: the C6 keeps for its resource loop the bytes that
    // `Runner::heap_bytes` gives for the loop's type, the link's frame and the event store at the C6's
    // maximum of 200 events, with the program loader and the retained-value check that the firmware
    // gives the loop, beside the memory it has for programs. Building the loop of a chip that boots
    // without a program, boxed as the firmware keeps it, takes exactly those bytes. The wrong
    // behaviour caught: a loop that takes more than the chip keeps for it, so that a program at the
    // edge of the memory for programs would leave the chip no room for its loop.
    let link = TestLink::default();
    let dog = Dog(Rc::new(RefCell::new(DogState {
        feeds: 0,
        intervals: Vec::with_capacity(64),
    })));
    let mut owner = FlashOwner::new(RamFlash {
        bytes: vec![0xFF; 0x80_0000],
    });
    let program_flash = owner
        .take_program()
        .expect("the program slots have a geometry")
        .expect("the program slots are handed out");
    let reader = owner
        .checkpoint_reader()
        .expect("the checkpoint slots have a geometry");
    let store = ProgramStore {
        flash: Box::new(program_flash),
        program_budget: PROGRAM_STEP_BYTES,
    };
    let hardware = Hardware {
        owner,
        claims: PinClaims::default(),
    };
    let loader = Esp32Loader::new(hardware, TestClock, MEMORY, room);
    let check = install_check(Box::new(reader), MEMORY, room);
    let platform = Platform::new(
        dog,
        SmolStr::new_static("Controller"),
        status_settings(),
        false,
        trust_runtime_core::deployment::ControlMode::Production,
        EngineLiveValues::empty(),
        LastReset {
            cause: ResetCauseName::PowerOn,
            text: SmolStr::default(),
            boot_count: 1,
        },
        runtime_id([0x20, 0x6E, 0xF1, 0x17, 0x04, 0xC0]),
    );
    let expected = TestRunner::heap_bytes(link.max_frame_len(), 200)
        + std::mem::size_of_val(&loader)
        + std::mem::size_of_val(&check);
    arm();
    let runner = Box::new(
        Runner::stopped(
            link,
            TestWait,
            platform,
            TestClock,
            SmolStr::new_static("no TRSTA002 container in either slot"),
            200,
        )
        .with_transfers(transfer_session(1))
        .with_program_store(store, check)
        .with_program_loader(loader, None),
    );
    let taken = live_bytes();
    drop(runner);
    assert_eq!(
        taken,
        bytes(expected),
        "the loop takes the {expected} bytes the chip keeps for it"
    );
}
