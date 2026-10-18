//! Spec 34 MEM-05 (fault records) and D-C3-2 (the 1,000-event store on every device): recording
//! a fault event keeps the engine's fixed `FaultRecord` and allocates nothing, because the
//! record is `Copy` and the store's buffer is already reserved.
#![cfg(feature = "full")]

use std::alloc::{GlobalAlloc, Layout, System};
use std::cell::Cell;
use std::collections::VecDeque;

use trust_runtime::debug::RuntimeEvent;
use trust_runtime::value::Duration;
use trust_runtime_core::error::{RuntimeError, StableErrorCode};
use trust_runtime_services::events::{store_event, EVENT_STORE_CAPACITY};

thread_local! {
    static ARMED: Cell<bool> = const { Cell::new(false) };
    static COUNTED: Cell<u64> = const { Cell::new(0) };
}

/// Count one allocator call on this thread while counting is armed. Only const-initialized
/// `Cell`s are touched, so counting never allocates, blocks or unwinds.
fn record() {
    let _ = ARMED.try_with(|armed| {
        if armed.get() {
            let _ = COUNTED.try_with(|counted| counted.set(counted.get().saturating_add(1)));
        }
    });
}

struct CountingAllocator;

// SAFETY: every operation forwards the caller's unchanged layout, pointer and new size to
// `System`; the counting touches only const-initialized thread-local `Cell`s.
unsafe impl GlobalAlloc for CountingAllocator {
    unsafe fn alloc(&self, layout: Layout) -> *mut u8 {
        record();
        // SAFETY: the caller supplies a valid allocation layout.
        unsafe { System.alloc(layout) }
    }

    unsafe fn realloc(&self, pointer: *mut u8, layout: Layout, size: usize) -> *mut u8 {
        record();
        // SAFETY: `System` returned `pointer` for `layout`; `size` is the caller's new size.
        unsafe { System.realloc(pointer, layout, size) }
    }

    unsafe fn dealloc(&self, pointer: *mut u8, layout: Layout) {
        record();
        // SAFETY: `pointer` and `layout` are the pair `System` returned.
        unsafe { System.dealloc(pointer, layout) }
    }
}

#[global_allocator]
static ALLOCATOR: CountingAllocator = CountingAllocator;

/// Spec 34 MEM-05 and D-C3-2: appending a fault event to the store allocates nothing, because the
/// event holds the `Copy` `FaultRecord` and shares the error's hosted payload without copying. It
/// would catch a fault event that carried an owned message, whose construction would allocate once
/// per recorded fault. It covers one fault of each kind that carries a hosted message: simulation,
/// resource panic, the four I/O kinds, the three retain kinds, invalid config, a read-only alias,
/// a retained subrange name and bytecode. Sharing a payload is an inline copy or a reference-count
/// increment; neither allocates.
#[test]
fn recording_a_fault_event_allocates_nothing() {
    let time = Duration::from_millis(4_036);
    let errors = [
        RuntimeError::SimulationFault("simulation fault message longer than inline".into()),
        RuntimeError::ResourcePanic("resource panic message longer than inline".into()),
        RuntimeError::IoDriver("i/o driver message longer than inline".into()),
        RuntimeError::IoTransport("i/o transport message longer than inline".into()),
        RuntimeError::IoAddress("%IX99.99".into()),
        RuntimeError::IoFreshness("%IX99.99".into()),
        RuntimeError::RetainStore("retain store message longer than inline".into()),
        RuntimeError::RetainCorruption("retain corruption message longer than inline".into()),
        RuntimeError::RetainMigration("retain migration message longer than inline".into()),
        RuntimeError::InvalidConfig("invalid config message longer than inline".into()),
        RuntimeError::ReadOnlyAlias("ReadOnlyAliasNameLongerThanInline".into()),
        RuntimeError::RetainSubrangeViolation {
            name: "RetainedNameLongerThanInline".into(),
            value: 5_000,
            lower: 0,
            upper: 1_000,
        },
        RuntimeError::Bytecode {
            code: StableErrorCode::VmStackUnderflow,
            detail: "bytecode detail message longer than inline".into(),
        },
    ];

    // Fill the store to its bound and empty it, so its buffer is already reserved.
    let mut store = VecDeque::new();
    for _ in 0..EVENT_STORE_CAPACITY {
        store_event(
            &mut store,
            RuntimeEvent::CycleStart {
                cycle: 0,
                time: Duration::ZERO,
            },
            EVENT_STORE_CAPACITY,
        );
    }
    assert_eq!(store.len(), EVENT_STORE_CAPACITY);
    store.clear();

    for error in &errors {
        let counted = {
            COUNTED.with(|counted| counted.set(0));
            ARMED.with(|armed| armed.set(true));
            store_event(
                &mut store,
                RuntimeEvent::fault(error, time),
                EVENT_STORE_CAPACITY,
            );
            ARMED.with(|armed| armed.set(false));
            COUNTED.with(Cell::get)
        };
        assert_eq!(
            counted, 0,
            "recording a fault event must not allocate; the record is Copy and the message is shared"
        );
        assert!(matches!(store.back(), Some(RuntimeEvent::Fault { .. })));
        store.clear();
    }
}

/// D-C3-2: report the RAM the 1,000-event store costs on this target. It pins the measured size
/// of one stored event, so it would catch an event that grew, and reports the per-event and total
/// bytes. The `Fault` variant carries the fixed `FaultRecord` and the shared message payload
/// (specification 34 MEM-05), so one event is 96 bytes on a 64-bit target.
#[test]
fn the_event_store_reports_its_ram_cost() {
    assert_eq!(EVENT_STORE_CAPACITY, 1_000);
    let per_event = std::mem::size_of::<RuntimeEvent>();
    let total_bytes = per_event * EVENT_STORE_CAPACITY;
    eprintln!("event-store-ram,per_event={per_event},total_bytes={total_bytes}");
    assert_eq!(per_event, 96, "one stored event is {per_event} bytes");
    assert_eq!(
        total_bytes, 96_000,
        "the store's 1,000 events are {total_bytes} bytes"
    );
}
