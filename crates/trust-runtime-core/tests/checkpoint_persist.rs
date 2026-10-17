//! The stepwise persister of a chip's retained values (specification 34 §10; C5-3): a due changed
//! image is written in steps and committed, an unchanged image is skipped, a newer due image waits
//! for a pending checkpoint and is not lost, `finish` completes the pending checkpoint and the
//! newest image, a failed step latches a retain-store failure once and keeps the last checkpoint,
//! and a power cut after every step recovers the last committed payload.
//!
//! The flash is simulated (`checkpoint_log/flash.rs`) with a chip's program and erase units, so a
//! cut can stop a program or an erase at any byte.

// The shared simulated flash carries helpers (`stick`, `flip`, `disturb_after`, ...) that the
// sibling `checkpoint_log` test uses; this test uses the rest, so the shared module is complete but
// not every helper is reachable here.
#[path = "checkpoint_log/flash.rs"]
#[allow(
    dead_code,
    reason = "the shared simulated flash's other helpers are used by the sibling checkpoint_log test"
)]
mod flash;

use std::alloc::{GlobalAlloc, Layout, System};
use std::cell::Cell;

use flash::{Cut, EraseCut, Op, SimFlash};
use trust_runtime_core::error::RuntimeError;
use trust_runtime_core::retain::capture::RetainCapture;
use trust_runtime_core::retain::checkpoint::{
    CheckpointFlash, CheckpointIdentity, CheckpointLog, CheckpointPersister, FlashGeometry,
    RecordInfo, Step,
};
use trust_runtime_core::value::{Duration, Value};

/// The application identity every checkpoint of these tests is written for.
const APPLICATION: [u8; 16] = [0x11; 16];
/// The retain-schema identity every checkpoint of these tests is written for.
const SCHEMA: [u8; 16] = [0x22; 16];
/// The identity every checkpoint of these tests is written for.
const IDENTITY: CheckpointIdentity = CheckpointIdentity {
    application: APPLICATION,
    schema: SCHEMA,
};

/// The admitted retain image of these tests: larger than any staged image below.
const IMAGE_BYTES: usize = 256;

/// A persisted checkpoint as recovery returns it.
type Recovered = (RecordInfo, Vec<u8>);

/// The STM32F407's checkpoint slots: one 16 KiB erase unit each (specification 34 §10.1).
fn f407() -> FlashGeometry {
    FlashGeometry::new(4, 16_384, 16_384).expect("the STM32F407's checkpoint geometry")
}

/// The ESP32-C6's checkpoint slots: four 4 KiB erase units each (specification 34 §10.1).
fn c6() -> FlashGeometry {
    FlashGeometry::new(4, 4_096, 16_384).expect("the ESP32-C6's checkpoint geometry")
}

fn geometries() -> [FlashGeometry; 2] {
    [f407(), c6()]
}

/// The STRN v3 payload of one retained `DINT` named `counter`: the entry count, the name length and
/// name, the `DINT` value tag 4, and the value (specification 11 §6.7).
fn image(counter: i32) -> Vec<u8> {
    let mut bytes = Vec::new();
    bytes.extend_from_slice(&1_u32.to_le_bytes());
    bytes.extend_from_slice(&7_u32.to_le_bytes());
    bytes.extend_from_slice(b"counter");
    bytes.push(4);
    bytes.extend_from_slice(&counter.to_le_bytes());
    bytes
}

/// A capture with a 100 ms save interval whose cadence starts at zero.
fn new_capture() -> RetainCapture {
    let mut capture = RetainCapture::new();
    capture.configure(Some(Duration::from_millis(100)), Duration::ZERO);
    capture
}

/// Stage `counter` as the capture's image, marked changed so the next due time saves it.
fn stage(capture: &mut RetainCapture, counter: i32) {
    capture.mark_dirty();
    capture
        .refill(|payload| payload.entry("counter", &Value::DInt(counter)))
        .expect("stage the retained image");
}

/// A persister over `flash`, as a boot would open it.
fn persister(flash: &mut SimFlash) -> CheckpointPersister<&mut SimFlash> {
    let log = CheckpointLog::open(flash).expect("open the checkpoint log");
    CheckpointPersister::new(log, IDENTITY, IMAGE_BYTES)
}

/// Run the pending checkpoint to its commit, one step per call.
fn drain<F: CheckpointFlash>(persister: &mut CheckpointPersister<F>, budget: usize) -> RecordInfo {
    loop {
        match persister.step(budget).expect("step the checkpoint") {
            Step::Pending => {}
            Step::Committed(record) => return record,
            Step::Idle => panic!("no checkpoint was pending"),
        }
    }
}

/// Power `flash` on and read the newest committed checkpoint through a fresh persister, as a boot
/// does (specification 34 §10, RETAIN-02).
fn recover(flash: &mut SimFlash) -> Option<Recovered> {
    let mut persister = persister(flash);
    let mut buffer = vec![0; 16 * 1024];
    let record = persister
        .recover(&mut buffer)
        .expect("recover the checkpoint");
    record.map(|record| {
        let len = usize::try_from(record.payload_len).expect("a payload length");
        (record, buffer[..len].to_vec())
    })
}

/// Write one checkpoint of `counter` at once, as a stop save does, and recover it.
fn write(flash: &mut SimFlash, counter: i32) -> Recovered {
    {
        let mut capture = new_capture();
        stage(&mut capture, counter);
        let mut persister = persister(flash);
        persister
            .finish(&mut capture, Duration::from_millis(100))
            .expect("write the checkpoint");
    }
    recover(flash).expect("the written checkpoint recovers")
}

/// Seed `flash` with `count` committed checkpoints of `len` bytes, as earlier runs would leave them.
fn seed(flash: &mut SimFlash, count: usize, len: usize) {
    let mut log = CheckpointLog::open(flash).expect("open the checkpoint log");
    for tag in 0..count {
        let tag = u8::try_from(tag).expect("a small tag");
        let payload = vec![tag; len];
        log.append(IDENTITY, &payload)
            .expect("seed a committed checkpoint");
    }
}

/// The steps a full write of `counter` from `start` takes at `budget`, as one flash operation each,
/// and the record it commits.
fn operations(start: &SimFlash, counter: i32, budget: usize) -> (Vec<Op>, RecordInfo) {
    let mut flash = start.clone();
    flash.record_operations();
    let record = {
        let mut capture = new_capture();
        stage(&mut capture, counter);
        let mut persister = persister(&mut flash);
        persister
            .offer(&mut capture, Duration::from_millis(100))
            .expect("offer the due image");
        drain(&mut persister, budget)
    };
    (flash.operations().to_vec(), record)
}

/// Cut power at every step of the write of `counter` from `start`, then power the flash again. After
/// every cut, recovery returns exactly the previous checkpoint, never a partial one, and the log
/// then writes and recovers `next`.
fn sweep(start: &SimFlash, counter: i32, budget: usize, next: i32) -> usize {
    let previous = recover(&mut start.clone()).expect("the start state recovers");
    let (ops, _) = operations(start, counter, budget);
    let mut cuts = 0;
    let mut at = 0;
    for op in &ops {
        let variants: [(bool, EraseCut); 2] = if op.erase {
            [(true, EraseCut::LeaveOld), (true, EraseCut::Zero)]
        } else {
            [(true, EraseCut::LeaveOld), (false, EraseCut::LeaveOld)]
        };
        for (partial, erase) in variants {
            let mut flash = start.clone();
            flash.cut_at(Cut {
                after: at,
                partial,
                erase,
            });
            let mut committed = None;
            {
                let mut capture = new_capture();
                stage(&mut capture, counter);
                let mut persister = persister(&mut flash);
                persister
                    .offer(&mut capture, Duration::from_millis(100))
                    .expect("offer the due image");
                loop {
                    match persister.step(budget) {
                        Ok(Step::Pending) => {}
                        Ok(Step::Committed(record)) => {
                            committed = Some(record);
                            break;
                        }
                        Ok(Step::Idle) => panic!("the write ended without a step"),
                        Err(error) => {
                            assert!(matches!(&error, RuntimeError::RetainStore(_)), "{error}");
                            break;
                        }
                    }
                }
            }
            assert!(
                committed.is_none(),
                "{at}: the write committed although power was cut"
            );
            assert!(
                !flash.powered(),
                "{at}: the write failed without a power cut"
            );
            flash.power_cycle();
            assert_eq!(
                recover(&mut flash).expect("recovery after a cut"),
                previous,
                "{at}: recovery must return the previous checkpoint"
            );
            let written = write(&mut flash, next);
            assert_eq!(
                written.1,
                image(next),
                "{at}: the next checkpoint must recover"
            );
            cuts += 1;
        }
        at += u64::from(op.len);
    }
    cuts
}

#[test]
fn a_due_changed_image_is_written_in_steps_and_committed() {
    for geometry in geometries() {
        let mut flash = SimFlash::new(geometry);
        flash.record_operations();
        let mut capture = new_capture();
        stage(&mut capture, 1);
        let (record, steps) = {
            let mut persister = persister(&mut flash);
            // 99 ms into a 100 ms interval: not due, so nothing starts and no step runs.
            persister
                .offer(&mut capture, Duration::from_millis(99))
                .expect("offer before the interval");
            assert!(
                !persister.is_writing(),
                "{geometry:?}: a save before its interval started"
            );
            assert_eq!(persister.step(64).expect("idle step"), Step::Idle);
            // Due at 100 ms: the changed image starts a checkpoint.
            persister
                .offer(&mut capture, Duration::from_millis(100))
                .expect("offer the due image");
            assert!(
                persister.is_writing(),
                "{geometry:?}: a due changed image did not start"
            );
            let mut steps: usize = 0;
            let record = loop {
                steps += 1;
                match persister.step(64).expect("step the checkpoint") {
                    Step::Pending => {}
                    Step::Committed(record) => break record,
                    Step::Idle => panic!("the checkpoint ended without committing"),
                }
            };
            (record, steps)
        };
        assert_eq!(record.sequence, 1, "{geometry:?}: the first sequence");
        assert_eq!(record.identity, IDENTITY, "{geometry:?}: the identity");
        assert_eq!(record.payload_len, 20, "{geometry:?}: the payload length");
        // One log step per call, and one flash operation per step: no call runs two steps.
        assert_eq!(
            flash.operations().len(),
            steps,
            "{geometry:?}: one flash operation per step"
        );
        let recovered = recover(&mut flash).expect("the committed checkpoint recovers");
        assert_eq!(recovered.0.sequence, 1);
        assert_eq!(recovered.0.identity, IDENTITY);
        assert_eq!(recovered.1, image(1));
    }
}

#[test]
fn an_unchanged_image_is_not_written() {
    for geometry in geometries() {
        let mut flash = SimFlash::new(geometry);
        flash.record_operations();
        let mut capture = new_capture();
        stage(&mut capture, 1);
        {
            let mut persister = persister(&mut flash);
            persister
                .offer(&mut capture, Duration::from_millis(100))
                .expect("offer the first image");
            let first = drain(&mut persister, 64);
            assert_eq!((first.sequence, first.payload_len), (1, 20));
            // The next due save stages the same image: it takes the cadence without a write.
            stage(&mut capture, 1);
            persister
                .offer(&mut capture, Duration::from_millis(200))
                .expect("offer the unchanged image");
            assert!(
                !persister.is_writing(),
                "{geometry:?}: an unchanged image started a write"
            );
            assert_eq!(persister.step(64).expect("step"), Step::Idle);
            // A stop with the same image writes nothing either.
            persister
                .finish(&mut capture, Duration::from_millis(300))
                .expect("finish the unchanged image");
            assert!(!persister.is_writing());
        }
        // Only the first checkpoint's steps ran: an erase unit, the slot header, the record header,
        // the payload and the commit. An extra append would add three more operations.
        assert_eq!(
            u32::try_from(flash.operations().len()).expect("an operation count"),
            geometry.erase_units() + 4,
            "{geometry:?}: an unchanged image programmed or erased flash"
        );
        let recovered = recover(&mut flash).expect("the checkpoint recovers");
        assert_eq!(
            recovered.0.sequence, 1,
            "{geometry:?}: an unchanged image appended a record"
        );
        assert_eq!(recovered.1, image(1));
    }
}

#[test]
fn a_due_save_during_a_pending_write_waits_and_is_not_lost() {
    for geometry in geometries() {
        let mut flash = SimFlash::new(geometry);
        let mut capture = new_capture();
        stage(&mut capture, 1);
        {
            let mut persister = persister(&mut flash);
            persister
                .offer(&mut capture, Duration::from_millis(100))
                .expect("offer the first image");
            assert_eq!(persister.step(64).expect("step"), Step::Pending);
            // A newer image becomes due while the first checkpoint is still being written.
            stage(&mut capture, 2);
            persister
                .offer(&mut capture, Duration::from_millis(200))
                .expect("offer while writing");
            assert!(
                persister.is_writing(),
                "{geometry:?}: the newer save did not wait for the pending checkpoint"
            );
            let first = drain(&mut persister, 64);
            assert_eq!(first.sequence, 1, "{geometry:?}: the first sequence");
            // The pending checkpoint committed the first image.
            let mut buffer = vec![0; 16 * 1024];
            let committed = persister
                .recover(&mut buffer)
                .expect("recover the first checkpoint")
                .expect("a committed checkpoint");
            assert_eq!(committed.sequence, 1);
            // The newest staged image is offered again and written: it was not lost.
            persister
                .offer(&mut capture, Duration::from_millis(200))
                .expect("offer the newest image");
            assert!(
                persister.is_writing(),
                "{geometry:?}: the newest image was not offered again"
            );
            let second = drain(&mut persister, 64);
            assert_eq!(second.sequence, 2, "{geometry:?}: the second sequence");
        }
        let recovered = recover(&mut flash).expect("the newest checkpoint recovers");
        assert_eq!(recovered.0.sequence, 2, "{geometry:?}: the newest sequence");
        assert_eq!(recovered.1, image(2));
    }
}

#[test]
fn finish_completes_the_pending_write_and_the_newest_image() {
    for geometry in geometries() {
        let mut flash = SimFlash::new(geometry);
        let mut capture = new_capture();
        stage(&mut capture, 1);
        {
            let mut persister = persister(&mut flash);
            persister
                .offer(&mut capture, Duration::from_millis(100))
                .expect("offer the first image");
            assert_eq!(persister.step(64).expect("step"), Step::Pending);
            stage(&mut capture, 2);
            // A stop: the pending checkpoint completes, then the newest image is written at once.
            persister
                .finish(&mut capture, Duration::from_millis(200))
                .expect("finish at the stop");
            assert!(!persister.is_writing());
        }
        let recovered = recover(&mut flash).expect("the newest checkpoint recovers");
        assert_eq!(
            recovered.0.sequence, 2,
            "{geometry:?}: both checkpoints must be committed"
        );
        assert_eq!(recovered.1, image(2));

        // A stop without a pending checkpoint writes the newest image, and a second stop with the
        // same image writes nothing.
        let mut capture = new_capture();
        stage(&mut capture, 3);
        let (first, second) = {
            let mut persister = persister(&mut flash);
            persister
                .finish(&mut capture, Duration::from_millis(100))
                .expect("finish the newest image");
            let mut buffer = vec![0; 16 * 1024];
            let first = persister
                .recover(&mut buffer)
                .expect("recover")
                .expect("a checkpoint");
            stage(&mut capture, 3);
            persister
                .finish(&mut capture, Duration::from_millis(200))
                .expect("finish the unchanged image");
            let second = persister
                .recover(&mut buffer)
                .expect("recover")
                .expect("a checkpoint");
            (first, second)
        };
        assert_eq!(first.sequence, 3);
        assert_eq!(
            second.sequence, 3,
            "{geometry:?}: a stop with an unchanged image appended a record"
        );
    }
}

#[test]
fn a_failed_step_latches_retain_store_once_and_keeps_the_last_checkpoint() {
    for geometry in geometries() {
        let mut flash = SimFlash::new(geometry);
        let last = write(&mut flash, 1);
        assert_eq!((last.0.sequence, last.1), (1, image(1)));
        // The next checkpoint appends after the committed one; cut power before its first program.
        flash.cut_at(Cut {
            after: 0,
            partial: false,
            erase: EraseCut::LeaveOld,
        });
        let mut capture = new_capture();
        stage(&mut capture, 2);
        let mut persister = persister(&mut flash);
        persister
            .offer(&mut capture, Duration::from_millis(100))
            .expect("offer the next image");
        assert!(persister.is_writing());
        let failure = persister
            .step(64)
            .expect_err("the cut must fail the first step");
        assert_eq!(
            failure,
            RuntimeError::RetainStore("checkpoint flash program failed".into()),
            "{geometry:?}: the log's reason is latched"
        );
        assert!(
            !persister.is_writing(),
            "{geometry:?}: the failed checkpoint is still pending"
        );
        // A latched failure stops further saves.
        persister
            .offer(&mut capture, Duration::from_millis(200))
            .expect("offer after the failure");
        assert!(
            !persister.is_writing(),
            "{geometry:?}: a save started after the latched failure"
        );
        assert_eq!(
            persister.take_failure(),
            Some(failure),
            "{geometry:?}: the latched failure is handed out"
        );
        assert_eq!(
            persister.take_failure(),
            None,
            "{geometry:?}: the failure is handed out once"
        );
        drop(persister);
        flash.power_cycle();
        let recovered = recover(&mut flash).expect("the last checkpoint recovers");
        assert_eq!(
            (recovered.0.sequence, recovered.1),
            (1, image(1)),
            "{geometry:?}: the newest committed checkpoint stays readable"
        );
    }
}

#[test]
fn a_cut_during_any_step_recovers_the_previous_checkpoint() {
    for geometry in geometries() {
        // A write appended after a committed checkpoint, with no erase.
        let mut start = SimFlash::new(geometry);
        let previous = write(&mut start, 1);
        assert_eq!((previous.0.sequence, previous.1), (1, image(1)));
        assert_eq!(
            sweep(&start, 2, 64, 3),
            2 * 3,
            "{geometry:?}: three steps of an append"
        );

        // A rotation: two 8100-byte records fill the active slot to 32 bytes, too few for the next
        // 20-byte checkpoint, so the write erases the other slot.
        let mut start = SimFlash::new(geometry);
        seed(&mut start, 2, 8_100);
        assert_eq!(
            recover(&mut start.clone()),
            Some((
                RecordInfo {
                    sequence: 2,
                    identity: IDENTITY,
                    payload_len: 8_100,
                },
                vec![1; 8_100],
            )),
        );
        let steps = usize::try_from(geometry.erase_units()).expect("the erase units") + 4;
        assert_eq!(
            sweep(&start, 2, 64, 3),
            2 * steps,
            "{geometry:?}: an erase unit and four program steps of a rotation"
        );
    }
}

thread_local! {
    static COUNTING: Cell<bool> = const { Cell::new(false) };
    static ALLOCATIONS: Cell<u64> = const { Cell::new(0) };
}

/// The system allocator, counting the allocations of a thread while it counts.
struct CountingAllocator;

// SAFETY: every call goes unchanged to the system allocator; the thread-local cells neither
// allocate nor panic.
unsafe impl GlobalAlloc for CountingAllocator {
    unsafe fn alloc(&self, layout: Layout) -> *mut u8 {
        let _ = COUNTING.try_with(|counting| {
            if counting.get() {
                let _ = ALLOCATIONS.try_with(|count| count.set(count.get() + 1));
            }
        });
        // SAFETY: the caller's layout is passed on unchanged.
        unsafe { System.alloc(layout) }
    }

    unsafe fn dealloc(&self, pointer: *mut u8, layout: Layout) {
        // SAFETY: the pointer and layout come from `alloc` above.
        unsafe { System.dealloc(pointer, layout) }
    }
}

#[global_allocator]
static ALLOCATOR: CountingAllocator = CountingAllocator;

/// The allocations `run` makes on this thread.
fn allocations(run: impl FnOnce()) -> u64 {
    ALLOCATIONS.with(|count| count.set(0));
    COUNTING.with(|counting| counting.set(true));
    run();
    COUNTING.with(|counting| counting.set(false));
    ALLOCATIONS.with(Cell::get)
}

#[test]
fn the_persister_allocates_nothing_after_construction() {
    for geometry in geometries() {
        let mut flash = SimFlash::new(geometry);
        // Construction reserves the payload buffer and the capture's staging buffer; the first
        // accepted image also gives the capture its second staging buffer.
        let mut persister = persister(&mut flash);
        let mut capture = new_capture();
        stage(&mut capture, 1);
        persister
            .offer(&mut capture, Duration::from_millis(100))
            .expect("offer the first image");
        drain(&mut persister, 64);

        let mut buffer = vec![0; 16 * 1024];
        let count = allocations(|| {
            for counter in 2..6 {
                stage(&mut capture, counter);
                persister
                    .offer(
                        &mut capture,
                        Duration::from_millis(100 + 100 * i64::from(counter)),
                    )
                    .expect("offer a due changed image");
                let record = drain(&mut persister, 64);
                assert_eq!(
                    record.sequence,
                    u64::try_from(counter).expect("a positive sequence")
                );
                let recovered = persister
                    .recover(&mut buffer)
                    .expect("recover")
                    .expect("a checkpoint");
                assert_eq!(recovered.payload_len, 20);
            }
        });
        assert_eq!(
            count, 0,
            "{geometry:?}: the persister allocated after construction"
        );
    }
}
