//! The heap of an online change on the C6 (specification 34 §11.3, UPDATE-06 and Appendix D.2),
//! measured on the C6's own loader and program memory with a counting allocator, in the style of
//! `crates/trust-runtime/tests/device_link_heap.rs`. The allocator records the bytes live on the test's
//! thread, so the heap of each step is read exactly. The modules are the fixtures of the repository
//! (the A4 program and the board's other test programs); the bench projects are not in the repository.
//!
//! The host's pointers are 64 bits wide and the C6's are 32, so the host's figures are the larger
//! ones. What the host cannot measure, the fragmentation of the C6's first-fit heap, the reserve
//! covers with a margin of one more working memory; the bench on the board replaces both (checklist
//! S4, Q14).
//!
//! Run with `-- --nocapture` to see the figures.

#[path = "online_change/support.rs"]
mod support;

use std::alloc::{GlobalAlloc, Layout, System};
use std::cell::Cell;

use smol_str::SmolStr;
use support::{booted, configuration, Dog, Hardware, TestClock, PROGRAM};
use trust_platform_esp32::loader::{
    online_change_reserve, Esp32Loader, ONLINE_CHANGE_WORKING_MEMORY_BYTES,
};
use trust_platform_esp32::profile::{runtime_id, HEAP_CAPACITY_BYTES};
use trust_platform_esp32::Platform;
use trust_runtime_core::bytecode::{BytecodeModule, ValidationLimits};
use trust_runtime_core::deployment::ControlMode;
use trust_runtime_core::retain::{RestartMode, RetainSnapshot};
use trust_runtime_core::value::Value;
use trust_runtime_core::watchdog::RetainMode;
use trust_runtime_services::capabilities::{LastReset, ResetCauseName};
use trust_runtime_services::chip::platform::status_settings;
use trust_runtime_services::runner::engine::DeviceMemory;
use trust_runtime_services::runner::{Candidate, EngineLiveValues, Loaded, ProgramLoader};

thread_local! {
    static ARMED: Cell<bool> = const { Cell::new(false) };
    static LIVE: Cell<i64> = const { Cell::new(0) };
    static PEAK: Cell<i64> = const { Cell::new(0) };
}

/// Adds `change` to the bytes live on this thread while it is armed, and notes the most that were
/// live at once. Only const-initialized `Cell`s are touched, so counting never allocates, blocks or
/// unwinds.
fn record(change: i64) {
    let _ = ARMED.try_with(|armed| {
        if armed.get() {
            let _ = LIVE.try_with(|live| {
                let now = live.get().saturating_add(change);
                live.set(now);
                let _ = PEAK.try_with(|peak| peak.set(peak.get().max(now)));
            });
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
        // A reallocation holds both blocks while it copies, so the new one counts before the old
        // one is released.
        record(bytes(new_size));
        // SAFETY: `System` returned `pointer` for `layout`; `new_size` is the caller's new size.
        let moved = unsafe { System.realloc(pointer, layout, new_size) };
        record(-bytes(layout.size()));
        moved
    }
}

#[global_allocator]
static ALLOCATOR: CountingAllocator = CountingAllocator;

/// Starts counting the bytes live on this thread from zero.
fn arm() {
    LIVE.with(|live| live.set(0));
    PEAK.with(|peak| peak.set(0));
    ARMED.with(|armed| armed.set(true));
}

fn disarm() {
    ARMED.with(|armed| armed.set(false));
}

/// The bytes live on this thread since [`arm`].
fn live_bytes() -> i64 {
    LIVE.with(Cell::get)
}

/// Forgets the peak so far: the next peak is measured from the bytes live now.
fn reset_peak() {
    PEAK.with(|peak| peak.set(LIVE.with(Cell::get)));
}

fn peak_bytes() -> i64 {
    PEAK.with(Cell::get)
}

thread_local! {
    /// The bytes of heap that are free now: far more than any program here, so the measurement is
    /// not limited by it.
    static ROOM: Cell<usize> = const { Cell::new(1 << 20) };
}

fn room() -> usize {
    ROOM.with(Cell::get)
}

const MEMORY: DeviceMemory = DeviceMemory {
    region_bytes: 64 * 1024,
    heap_bytes: 136 * 1024,
};

/// One chip, built outside the measurement: its hardware, loader and platform.
struct Chip {
    loader: Esp32Loader<Hardware, TestClock>,
    platform: Platform<Dog>,
}

impl Chip {
    /// A chip whose checkpoint slots hold `checkpoint` as the newest checkpoint, when there is one.
    fn new(checkpoint: Option<&RetainSnapshot>) -> Self {
        let (mut hardware, reader) = Hardware::with_reader();
        if let Some(snapshot) = checkpoint {
            hardware.save_checkpoint(snapshot);
        }
        let platform = Platform::new(
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
        );
        Self {
            loader: Esp32Loader::new(hardware, TestClock, MEMORY, room, reader),
            platform,
        }
    }
}

/// What the heap did while the program `a` was changed to the program `b`.
#[derive(Debug)]
struct Change {
    /// The heap the running program takes, in bytes.
    running: i64,
    /// The upload of the request, in bytes.
    upload: i64,
    /// The heap the candidate takes when it is the only program, in bytes.
    candidate: i64,
    /// The part of the candidate that the running program already holds and the commit hands over
    /// instead of allocating again: the checkpoint store of a record that keeps retained values.
    shared: i64,
    /// The most bytes live at once during the change, above those live before the running program
    /// was loaded: the running program, the upload and what the change took beyond them.
    peak: i64,
    /// The bytes live after the commit and the end of the running program, above those live before
    /// the running program was loaded.
    after: i64,
}

impl Change {
    /// What the change needed at its peak beyond the two programs: the working memory that decoding,
    /// validating and constructing the candidate took while the running program and the candidate
    /// were both resident. A change that dropped the running program before it built the candidate
    /// would have a negative figure, since its peak would be one program.
    fn working_memory(&self) -> i64 {
        self.peak - (self.running + self.candidate - self.shared)
    }
}

/// The record of a program, which keeps retained values when `retains` is set.
fn record_of(retains: bool) -> trust_runtime_core::deployment::Configuration {
    let mode = if retains {
        RetainMode::File
    } else {
        RetainMode::None
    };
    configuration(mode, Vec::new())
}

/// The heap the program loaded alone takes, in bytes, on a fresh chip.
fn resident(
    program: &[u8],
    retains: bool,
    checkpoint: Option<&RetainSnapshot>,
) -> Result<i64, String> {
    let mut chip = Chip::new(checkpoint);
    arm();
    let before = live_bytes();
    let loaded = chip
        .loader
        .load(
            &mut chip.platform,
            booted(program, record_of(retains)),
            RestartMode::Cold,
        )
        .map_err(|error| format!("the program does not load: {error}"))?;
    let bytes = live_bytes() - before;
    drop(loaded);
    disarm();
    Ok(bytes)
}

/// Changes the program `a` to the program `b` on a fresh chip, and measures it. `checkpoint` is the
/// newest checkpoint of a record that keeps retained values, and none for one that keeps none.
fn measure(a: &[u8], b: &[u8], checkpoint: Option<&RetainSnapshot>) -> Result<Change, String> {
    let retains = checkpoint.is_some();
    let candidate = resident(b, retains, checkpoint)?;
    let shared = if retains {
        resident(a, true, checkpoint)? - resident(a, false, checkpoint)?
    } else {
        0
    };

    // The change: the running program, the upload, the candidate and the commit.
    let mut chip = Chip::new(checkpoint);
    arm();
    let before = live_bytes();
    let Loaded {
        cycle: mut running, ..
    } = chip
        .loader
        .load(
            &mut chip.platform,
            booted(a, record_of(retains)),
            RestartMode::Cold,
        )
        .map_err(|error| format!("the running program does not load: {error}"))?;
    let running_bytes = live_bytes() - before;
    let upload = b.to_vec();
    let upload_bytes = bytes(upload.len());
    reset_peak();
    let prepared = chip
        .loader
        .prepare_change(&mut chip.platform, &mut running, upload);
    let mut prepared: Candidate<_> = prepared?;
    chip.loader
        .commit_change(&mut chip.platform, &mut running, &mut prepared);
    let peak = peak_bytes() - before;
    drop(running);
    // The loop records the events of the retained values and drops them; the candidate's cycle and
    // Live Values are the program.
    let Candidate {
        cycle,
        live,
        events,
    } = prepared;
    drop(events);
    let after = live_bytes() - before;
    drop(live);
    drop(cycle);
    disarm();
    Ok(Change {
        running: running_bytes,
        upload: upload_bytes,
        candidate,
        shared,
        peak,
        after,
    })
}

/// The checkpoint that the A4 program's retained `activations` is saved in.
fn a4_checkpoint() -> RetainSnapshot {
    let mut snapshot = RetainSnapshot::default();
    snapshot.insert("@program/Plant/activations", Value::DInt(2));
    snapshot
}

/// The checkpoint that the retained global `total` of the `retained_blocks` program is saved in.
fn blocks_checkpoint() -> RetainSnapshot {
    let mut snapshot = RetainSnapshot::default();
    snapshot.insert("total", Value::DInt(2));
    snapshot
}

/// The program of the corpus whose retained globals hold a function-block instance, a reference and a
/// scalar: a change of it to itself is congruent and carries them live.
const BLOCKS: &[u8] =
    include_bytes!("../../trust-runtime/tests/fixtures/bounded-c/retained_blocks/program.stbc");

#[test]
fn an_online_changes_heap_peak_is_the_two_programs_and_the_upload_within_the_room() {
    // Specification 34 §11.3 (UPDATE-06) and Appendix D.2: while a change is prepared the heap holds
    // the running program and the candidate at once, with the upload until the candidate's module is
    // prepared. The working memory is what the change takes at its peak beyond the two programs: the
    // upload, the decoding and validation of the module and the retained steps. The loader keeps
    // `ONLINE_CHANGE_WORKING_MEMORY_BYTES` free for it always: twice the largest working memory
    // measured here, for the fragmentation of a first-fit heap that a host cannot measure, rounded up
    // to 8 KiB; and, when the record keeps retained values, the payload of the newest checkpoint,
    // which the retained steps read whole into the heap (the reserve is `online_change_reserve`).
    // After the commit the running program is dropped and the heap holds one program again: the bytes
    // live are those of the candidate loaded alone. The A4 program is changed to itself, which is
    // congruent, and to three of the repository's other programs, which are not, and the
    // `retained_blocks` program is changed to itself, a congruent change that copies a live
    // function-block instance and reference. Each is measured with a record that keeps no retained
    // values and with one that keeps them, whose newest checkpoint holds the running program's
    // retained value: that adds the decoded checkpoint, and moves the checkpoint store to the
    // candidate, so the store is held once.
    // Wrong behaviour caught: a working-memory part below twice the working memory (the chip would
    // run out of heap in the middle of a change, which resets it and stops the machine); a change that
    // drops the running program before it builds the candidate (a negative working memory: the heap
    // would never hold both, and UPDATE-06's overlap would go untested); a running program that
    // outlives the commit (the heap would hold both after it); a checkpoint store that is built again
    // for the candidate (16,332 bytes more at the peak and after the commit).
    type Checkpoint = fn() -> RetainSnapshot;
    let changes: [(&str, &[u8], &[u8], Checkpoint); 5] = [
        ("the A4 program", PROGRAM, PROGRAM, a4_checkpoint),
        (
            "numeric-v2",
            PROGRAM,
            include_bytes!(
                "../../trust-runtime/tests/fixtures/portability/stbc-2.0/numeric-v2.stbc"
            ),
            a4_checkpoint,
        ),
        (
            "bounded-c retain",
            PROGRAM,
            include_bytes!("../../trust-runtime/tests/fixtures/bounded-c/retain/program.stbc"),
            a4_checkpoint,
        ),
        (
            "bounded-c calls",
            PROGRAM,
            include_bytes!("../../trust-runtime/tests/fixtures/bounded-c/calls/program.stbc"),
            a4_checkpoint,
        ),
        ("the retained blocks", BLOCKS, BLOCKS, blocks_checkpoint),
    ];
    let mut worst = 0_i64;
    for kind in ["no retained values", "retained values"] {
        for (name, running, program, checkpoint) in changes {
            let saved = checkpoint();
            let record = (kind == "retained values").then_some(&saved);
            let change = measure(running, program, record)
                .unwrap_or_else(|reason| panic!("{kind}, to {name}: {reason}"));
            // The validator's analysis storage peaks at this many bytes, so the heap that remains for
            // the candidate must be at least three halves of it: the scratch limit is two thirds.
            let scratch = BytecodeModule::decode(program)
                .expect("the program decodes")
                .validated_with_limits(ValidationLimits::default())
                .expect("the program validates")
                .stats()
                .peak_scratch_bytes;
            println!(
                "online change heap: {kind}, to {name} ({} bytes): running {}, upload {}, candidate \
                 alone {}, shared {}, peak {}, working memory {}, after the commit {}, validation \
                 scratch {scratch}",
                program.len(),
                change.running,
                change.upload,
                change.candidate,
                change.shared,
                change.peak,
                change.working_memory(),
                change.after
            );
            worst = worst.max(change.working_memory());
            assert!(
                change.working_memory() >= 0,
                "{kind}, to {name}: the running program and the candidate were both resident at the \
                 peak, so the working memory of {} bytes is not negative",
                change.working_memory()
            );
            // A program that is built anew keeps bookkeeping of its initialization that a candidate
            // built from the running state does not (464 bytes less for the retained blocks), so the
            // heap after the change is the candidate's alone to within a kilobyte, against the 17 to
            // 63 KiB that a running program left behind would add.
            assert!(
                (change.after - change.candidate).abs() <= 1_024,
                "{kind}, to {name}: after the change the heap holds {} bytes, a candidate loaded alone \
                 holds {}: the running program is gone and the heap is one program again",
                change.after,
                change.candidate
            );
            assert!(
                change.peak < bytes(HEAP_CAPACITY_BYTES),
                "{kind}, to {name}: the peak of {} bytes fits the C6's heap of {HEAP_CAPACITY_BYTES} bytes",
                change.peak
            );
        }
    }
    // The working-memory part of the reserve is twice the largest working memory measured here, for
    // the fragmentation of a first-fit heap that the host cannot measure, rounded up to 8 KiB.
    let doubled = 2 * worst;
    println!(
        "online change heap: the largest working memory is {worst} bytes, twice it is {doubled} bytes; \
         the working-memory part of the reserve is {ONLINE_CHANGE_WORKING_MEMORY_BYTES} bytes"
    );
    assert!(
        doubled <= bytes(ONLINE_CHANGE_WORKING_MEMORY_BYTES),
        "the working-memory part of {ONLINE_CHANGE_WORKING_MEMORY_BYTES} bytes covers twice the largest \
         working memory ({worst} bytes), which is {doubled} bytes"
    );
    // A record that keeps no retained values reserves the working memory alone, and one that keeps
    // them adds the payload of the newest checkpoint, at most the largest payload that a slot takes.
    let (_, mut reader) = Hardware::with_reader();
    let largest = usize::try_from(reader.geometry().max_payload_len()).expect("a short payload");
    assert_eq!(
        online_change_reserve(false, &mut *reader),
        ONLINE_CHANGE_WORKING_MEMORY_BYTES
    );
    println!(
        "online change heap: the reserve is {ONLINE_CHANGE_WORKING_MEMORY_BYTES} bytes, and at most {} \
         bytes when the record keeps retained values",
        ONLINE_CHANGE_WORKING_MEMORY_BYTES + largest
    );
}

/// Where a loaded program's heap goes (for checklist C1, spec 34 MEM-12): the prepared module alone,
/// and the rest of the loaded program (its engine state, its execution services, its Live Values and
/// its records), for each fixture program. The figures are printed; only their order is asserted.
#[test]
fn where_a_loaded_programs_heap_goes() {
    let programs: [(&str, &[u8]); 4] = [
        ("the A4 program", PROGRAM),
        (
            "numeric-v2",
            include_bytes!(
                "../../trust-runtime/tests/fixtures/portability/stbc-2.0/numeric-v2.stbc"
            ),
        ),
        (
            "bounded-c calls",
            include_bytes!("../../trust-runtime/tests/fixtures/bounded-c/calls/program.stbc"),
        ),
        ("the retained blocks", BLOCKS),
    ];
    for (name, program) in programs {
        arm();
        let before = live_bytes();
        let decoded = BytecodeModule::decode(program).expect("the program decodes");
        let container = live_bytes() - before;
        drop(decoded);
        disarm();
        // Reserved before the meter is armed, so that recording a part allocates nothing.
        let mut parts = Vec::with_capacity(32);
        arm();
        let before = live_bytes();
        let prepared = trust_runtime_core::vm::PreparedModule::from_bytes(
            program,
            trust_runtime_services::runner::engine::device_limits(MEMORY),
        )
        .expect("the program prepares");
        let module = live_bytes() - before;
        let mut last = live_bytes();
        prepared.drop_in_parts(|part| {
            let now = live_bytes();
            if part != "start" {
                parts.push((part, last - now));
            }
            last = now;
        });
        disarm();
        for (part, bytes) in &parts {
            println!("footprint: {name}: {part}: {bytes} B");
        }
        let whole = resident(program, false, None).expect("the program loads");
        let figure = trust_runtime_core::vm::PreparedModule::from_bytes(
            program,
            trust_runtime_services::runner::engine::device_limits(MEMORY),
        )
        .expect("the program prepares")
        .memory_figure();
        println!(
            "footprint: {name}: {} B of bytecode; the decoded container alone {container} B; the \
             prepared module {module} B; the whole loaded program {whole} B, of which the state, \
             services and the rest {} B; the memory figure {} B, of which the engine {} B",
            program.len(),
            whole - module,
            figure.total(false),
            figure.total(false) - figure.program
        );
        assert!(module > 0 && whole > module, "{name}");
    }
}
