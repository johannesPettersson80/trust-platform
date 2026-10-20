//! The reserve of an online change on the C6 (specification 34 §11.3, UPDATE-06 and Appendix D.2),
//! read from the numbers of its refusals. The C6's loader keeps the working memory,
//! `ONLINE_CHANGE_WORKING_MEMORY_BYTES` (8,192 bytes), free for every change, and, when the record of
//! the running program keeps retained values, the payload of the newest committed checkpoint, which
//! the change reads whole into the heap. The heap has 20,000 bytes free in these tests, which allow no
//! call level once the reserve is taken (a call level needs 18,432 bytes of the heap that remains), so
//! the board probe (3,388 bytes) is refused with the numbers of the change, and the text names the
//! reserve that the loader kept: `<r> bytes are free, <k> are kept in reserve`.

#[path = "online_change/support.rs"]
mod support;

use std::cell::Cell;

use smol_str::SmolStr;
use support::{booted, configuration, Dog, Hardware, TestClock, PROBE, PROGRAM};
use trust_platform_esp32::loader::{
    program_memory_bytes, Esp32Loader, ONLINE_CHANGE_WORKING_MEMORY_BYTES,
};
use trust_platform_esp32::profile::runtime_id;
use trust_platform_esp32::Platform;
use trust_runtime_core::bytecode::{BytecodeModule, ValidationLimits};
use trust_runtime_core::deployment::{Configuration, ControlMode};
use trust_runtime_core::retain::checkpoint::{CheckpointFlash, FlashError, FlashGeometry, Slot};
use trust_runtime_core::retain::{RestartMode, RetainSnapshot};
use trust_runtime_core::value::Value;
use trust_runtime_core::watchdog::RetainMode;
use trust_runtime_services::capabilities::{LastReset, ResetCauseName};
use trust_runtime_services::chip::platform::status_settings;
use trust_runtime_services::runner::engine::DeviceMemory;
use trust_runtime_services::runner::{EngineLiveValues, Loaded, ProgramLoader};

thread_local! {
    /// The bytes of heap that are free now, which a test sets.
    static ROOM: Cell<usize> = const { Cell::new(1 << 20) };
}

fn room() -> usize {
    ROOM.with(Cell::get)
}

const MEMORY: DeviceMemory = DeviceMemory {
    region_bytes: 64 * 1024,
    heap_bytes: 136 * 1024,
};

/// The bytes of heap that are free when a change is taken in these tests.
const FREE: usize = 20_000;

/// The largest payload that a checkpoint slot of the C6 takes (specification 34 §10.1).
const LARGEST_PAYLOAD: usize = 16_300;

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

/// A checkpoint flash whose slots cannot be read: the record header of the newest checkpoint cannot
/// say how long its payload is.
struct Unreadable(FlashGeometry);

impl CheckpointFlash for Unreadable {
    fn geometry(&self) -> FlashGeometry {
        self.0
    }

    fn read(&mut self, _slot: Slot, _offset: u32, _bytes: &mut [u8]) -> Result<(), FlashError> {
        Err(FlashError::Read)
    }

    fn program(&mut self, _slot: Slot, _offset: u32, _bytes: &[u8]) -> Result<(), FlashError> {
        Err(FlashError::Program)
    }

    fn erase(&mut self, _slot: Slot, _offset: u32) -> Result<(), FlashError> {
        Err(FlashError::Erase)
    }
}

/// A snapshot of `names`, each a DINT, and the length of its payload.
fn snapshot(names: &[String]) -> (RetainSnapshot, usize) {
    let mut snapshot = RetainSnapshot::default();
    for name in names {
        snapshot.insert(name.as_str(), Value::DInt(2));
    }
    let mut payload = Vec::new();
    snapshot
        .encode_snapshot_payload_into(&mut payload)
        .expect("the snapshot encodes");
    (snapshot, payload.len())
}

fn names(count: usize) -> Vec<String> {
    (0..count)
        .map(|index| format!("@program/Plant/value{index}"))
        .collect()
}

/// The refusal that the board probe meets as a change of the A4 program, with 20,000 bytes of heap
/// free, from a chip whose flash holds `checkpoints`, written in this order, whose running program has
/// the record `config`. `reader` replaces the loader's handle on the checkpoint slots when it is given.
fn refusal(
    config: Configuration,
    checkpoints: &[&RetainSnapshot],
    reader: Option<Box<dyn CheckpointFlash>>,
) -> String {
    let (mut hardware, own) = Hardware::with_reader();
    for checkpoint in checkpoints {
        hardware.save_checkpoint(checkpoint);
    }
    let mut loader = Esp32Loader::new(hardware, TestClock, MEMORY, room, reader.unwrap_or(own));
    let mut platform = platform();
    ROOM.with(|room| room.set(1 << 20));
    let Loaded {
        cycle: mut running, ..
    } = loader
        .load(&mut platform, booted(PROGRAM, config), RestartMode::Cold)
        .expect("the A4 program loads");
    ROOM.with(|room| room.set(FREE));
    loader
        .prepare_change(&mut platform, &mut running, PROBE.to_vec())
        .err()
        .expect("20,000 bytes of heap leave nothing for a candidate")
}

/// The text of the refusal of the board probe when `reserve` bytes of the 20,000 free are kept.
fn probe_refused_with(reserve: usize) -> String {
    format!(
        "insufficient memory: a change of 3388 bytes needs more than the {} bytes of heap that \
         remain for it ({FREE} bytes are free, {reserve} are kept in reserve)",
        FREE.saturating_sub(reserve)
    )
}

#[test]
fn the_reserve_holds_the_checkpoint_only_when_the_record_keeps_retained_values() {
    // Specification 34 §11.3 and Appendix D.2: a change reads the newest checkpoint into the heap only
    // when the record of the running program keeps retained values, so only then does the reserve
    // hold its payload. The flash holds a checkpoint of three values. A record that keeps no retained
    // values reserves the working memory alone, 8,192 bytes, though the checkpoint is there; a record
    // that keeps them reserves 8,192 bytes and the length of the checkpoint's payload; a record that
    // keeps them over a log with no committed checkpoint has nothing to read, and reserves 8,192
    // bytes. Wrong behaviour caught: a reserve that always holds the largest payload (24,492 bytes
    // for the record that keeps nothing, as the first bench build reserved 24,576), which refuses a
    // program that fits; a reserve that forgets the payload when the record keeps retained values.
    assert_eq!(PROBE.len(), 3_388, "the board probe");
    let (saved, payload) = snapshot(&names(3));
    assert!(payload > 0 && payload < 200, "a short payload: {payload}");

    let none = refusal(configuration(RetainMode::None, Vec::new()), &[&saved], None);
    assert_eq!(none, probe_refused_with(ONLINE_CHANGE_WORKING_MEMORY_BYTES));

    let keeps = refusal(configuration(RetainMode::File, Vec::new()), &[&saved], None);
    assert_eq!(
        keeps,
        probe_refused_with(ONLINE_CHANGE_WORKING_MEMORY_BYTES + payload)
    );

    let empty = refusal(configuration(RetainMode::File, Vec::new()), &[], None);
    assert_eq!(
        empty,
        probe_refused_with(ONLINE_CHANGE_WORKING_MEMORY_BYTES)
    );
}

#[test]
fn the_reserve_holds_the_newest_checkpoints_payload_not_the_largest() {
    // Specification 34 §10.1 and §11.3: the reserve holds the bytes that the change will really read,
    // the payload of the newest committed checkpoint, which its record header gives, not the largest
    // payload a slot takes (16,300 bytes) and not the longest payload ever written. The flash holds
    // a checkpoint of forty values and, written after it, one of three: the newest is the short one,
    // so the reserve is 8,192 bytes and its length. Wrong behaviour caught: a reserve of the largest
    // payload; a reserve of the first or the longest payload in the log instead of the newest.
    let (long, long_payload) = snapshot(&names(40));
    let (short, short_payload) = snapshot(&names(3));
    assert!(short_payload < long_payload && long_payload < LARGEST_PAYLOAD);

    let refused = refusal(
        configuration(RetainMode::File, Vec::new()),
        &[&long, &short],
        None,
    );
    assert_eq!(
        refused,
        probe_refused_with(ONLINE_CHANGE_WORKING_MEMORY_BYTES + short_payload)
    );
    assert_ne!(
        refused,
        probe_refused_with(ONLINE_CHANGE_WORKING_MEMORY_BYTES + long_payload),
        "not the longest payload"
    );

    // Written the other way round, the newest is the long one.
    let refused = refusal(
        configuration(RetainMode::File, Vec::new()),
        &[&short, &long],
        None,
    );
    assert_eq!(
        refused,
        probe_refused_with(ONLINE_CHANGE_WORKING_MEMORY_BYTES + long_payload)
    );
}

#[test]
fn the_reserve_holds_the_largest_payload_when_the_checkpoint_header_cannot_say() {
    // Specification 34 §11.3: when the record header of the newest checkpoint cannot be read, the
    // change cannot know how many bytes it will read, so the reserve holds the largest payload that a
    // slot takes: 8,192 + 16,300 = 24,492 bytes, more than the 20,000 that are free, so nothing
    // remains. A record that keeps no retained values reads nothing, and its reserve is the same
    // 8,192 bytes though the flash cannot be read. Wrong behaviour caught: a header that cannot be
    // read counted as an empty log (a change that then reads the largest payload into a heap that
    // has no room for it); a reserve that is larger for a record that reads nothing.
    let geometry = Hardware::with_reader().1.geometry();
    assert_eq!(
        usize::try_from(geometry.max_payload_len()).expect("a short payload"),
        LARGEST_PAYLOAD
    );

    let keeps = refusal(
        configuration(RetainMode::File, Vec::new()),
        &[],
        Some(Box::new(Unreadable(geometry))),
    );
    assert_eq!(
        keeps,
        probe_refused_with(ONLINE_CHANGE_WORKING_MEMORY_BYTES + LARGEST_PAYLOAD)
    );
    assert!(keeps.contains("the 0 bytes of heap that remain"), "{keeps}");

    let none = refusal(
        configuration(RetainMode::None, Vec::new()),
        &[],
        Some(Box::new(Unreadable(geometry))),
    );
    assert_eq!(none, probe_refused_with(ONLINE_CHANGE_WORKING_MEMORY_BYTES));
}

#[test]
fn the_c6_words_a_validation_limit_as_insufficient_memory_with_the_chips_numbers() {
    // Specification 34 §11.3 and Appendix D.2: the candidate's limits are the device's limits for the
    // heap that remains after the reserve, and the validator's analysis storage is one of them. The
    // analysis of the A4 program peaks at a number of bytes that the test measures; a heap of three
    // halves of that peak less one leaves a scratch limit (two thirds of the heap) just below it. The
    // record keeps no retained values, so the reserve is the working memory, 8,192 bytes. The chip
    // refuses the A4 program with the numbers of the change, as it refuses any other limit, and not as
    // `invalid bytecode 'invalid section data: validation analysis storage limit exceeded'`. The
    // running program runs on. Wrong behaviour caught: the bench's refusal of a program that fits the
    // chip's memory but not the limits that the heap's room less the reserve allow.
    let peak = BytecodeModule::decode(PROGRAM)
        .expect("the A4 program decodes")
        .validated_with_limits(ValidationLimits::default())
        .expect("the A4 program validates")
        .stats()
        .peak_scratch_bytes;
    assert!(
        peak > 12_288,
        "the analysis of the A4 program peaks at {peak}"
    );
    let remains = 3 * (peak - 1) / 2;
    let free = remains + ONLINE_CHANGE_WORKING_MEMORY_BYTES;

    let (hardware, reader) = Hardware::with_reader();
    let mut loader = Esp32Loader::new(hardware, TestClock, MEMORY, room, reader);
    let mut platform = platform();
    ROOM.with(|room| room.set(1 << 20));
    let Loaded {
        cycle: mut running, ..
    } = loader
        .load(
            &mut platform,
            booted(PROGRAM, configuration(RetainMode::None, Vec::new())),
            RestartMode::Cold,
        )
        .expect("the A4 program loads");
    ROOM.with(|room| room.set(free));
    let refused = loader
        .prepare_change(&mut platform, &mut running, PROGRAM.to_vec())
        .err()
        .expect("the validator's limit refuses the A4 program");
    assert_eq!(
        refused,
        format!(
            "insufficient memory: a change of 6412 bytes needs more than the {remains} bytes of heap \
             that remain for it ({free} bytes are free, {ONLINE_CHANGE_WORKING_MEMORY_BYTES} are kept \
             in reserve)"
        )
    );
}

#[test]
fn the_c6_has_for_programs_its_free_heap_less_what_it_keeps_beside_them() {
    // Specification 34 MEM-12 and Appendix D.2: the C6's memory for programs is the heap's free bytes
    // before any program is loaded, with the installed program's bytes, which the load frees, less the
    // bytes of its resource loop, the working memory of an online change (8,192 bytes) and the largest
    // payload that a checkpoint slot takes (16,300 bytes: the checkpoint store's buffer, which a warm
    // start also reads the newest checkpoint into). Worked by hand with 120,000 bytes free, a program
    // of 6,412 bytes and a loop of 20,000: 120,000 + 6,412 - 20,000 - 8,192 - 16,300 = 81,920 bytes.
    // The wrong behaviour: the program's own bytes left out (a program that fits refused), or the
    // loop or the checkpoint store left out (a program admitted that the chip could not then run).
    assert_eq!(
        program_memory_bytes(120_000, 6_412, LARGEST_PAYLOAD, 20_000),
        81_920
    );
    // A heap that holds less than the chip keeps beside programs leaves no memory for programs.
    assert_eq!(program_memory_bytes(10_000, 0, LARGEST_PAYLOAD, 0), 0);
}
