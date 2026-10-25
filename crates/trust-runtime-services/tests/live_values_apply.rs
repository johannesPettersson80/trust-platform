//! The shared Live Values apply of a compiled program allocates nothing (specification 11, "Live
//! Values"; specification 34 §8 and MEM-04; specification 44 §13.3).
//!
//! The test calls the shared apply where the loop calls it, at the cycle start and around the
//! process images, against a stand-in program that keeps each value in place, so that every
//! allocation it counts is the apply's own. The order of the applies and their refusals are proven
//! by the apply's own tests (`live_values/apply/compiled_tests.rs`).
#![allow(
    unsafe_code,
    reason = "the counting allocator is this test's meter, which must implement the unsafe GlobalAlloc trait"
)]
use std::alloc::{GlobalAlloc, Layout, System};
use std::cell::Cell;

use trust_runtime_core::error::RuntimeError;
use trust_runtime_core::io_address::IoAddress;
use trust_runtime_core::memory::{Generation, IoArea};
use trust_runtime_core::native::variables::CompiledVariables;
use trust_runtime_core::value::{Duration, Value};
use trust_runtime_services::live_values::apply::{self, WriteSink};
use trust_runtime_services::live_values::queues::{
    ForcedVarTarget, LiveValueQueues, PendingVarTarget,
};

/// A sink that throws its reports away.
struct NoReport;

impl WriteSink for NoReport {
    fn refused(&mut self, _target: &str, _error: &RuntimeError, _time: Duration) {}
}

/// A compiled program's variables as the apply reaches them: the `INT` global `probe_depth` and the
/// output bit `%QX0.0`, each kept in place, so that a write allocates nothing. Anything else is
/// refused.
struct Program {
    probe_depth: Value,
    led: Value,
}

impl CompiledVariables for Program {
    fn read_named(&self, name: &str) -> Result<Value, RuntimeError> {
        match name {
            "probe_depth" => Ok(self.probe_depth.clone()),
            _ => Err(RuntimeError::UndefinedVariable(name.into())),
        }
    }

    fn write_named(&mut self, name: &str, value: &Value) -> Result<(), RuntimeError> {
        match name {
            "probe_depth" => {
                self.probe_depth = value.clone();
                Ok(())
            }
            _ => Err(RuntimeError::UndefinedVariable(name.into())),
        }
    }

    fn write_retained(&mut self, label: &str, _value: &Value) -> Result<(), RuntimeError> {
        Err(RuntimeError::UndefinedVariable(label.into()))
    }

    fn write_member(
        &mut self,
        instance: u32,
        _member: &str,
        _value: &Value,
    ) -> Result<(), RuntimeError> {
        Err(RuntimeError::UndefinedVariable(
            format!("{instance}").into(),
        ))
    }

    fn write_image(&mut self, address: &IoAddress, value: Value) -> Result<(), RuntimeError> {
        if address.area == IoArea::Output && address.byte == 0 && address.bit == 0 {
            self.led = value;
            Ok(())
        } else {
            Err(RuntimeError::IoAddress("outside the image".into()))
        }
    }
}

/// Allocator calls counted while an apply runs: allocation, reallocation and deallocation.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
struct Calls {
    alloc: u64,
    realloc: u64,
    dealloc: u64,
}

impl Calls {
    const ZERO: Self = Self {
        alloc: 0,
        realloc: 0,
        dealloc: 0,
    };
}

thread_local! {
    static ARMED: Cell<bool> = const { Cell::new(false) };
    static COUNTED: Cell<Calls> = const { Cell::new(Calls::ZERO) };
}

/// Count one call on this thread while it is armed. Only const-initialized `Cell`s are touched,
/// so counting never allocates, blocks or unwinds.
fn record(call: fn(&mut Calls)) {
    let _ = ARMED.try_with(|armed| {
        if armed.get() {
            let _ = COUNTED.try_with(|counted| {
                let mut calls = counted.get();
                call(&mut calls);
                counted.set(calls);
            });
        }
    });
}

struct CountingAllocator;

// SAFETY: every operation forwards the caller's unchanged layout, pointer and new size to
// `System`; the counting touches only const-initialized thread-local `Cell`s.
unsafe impl GlobalAlloc for CountingAllocator {
    unsafe fn alloc(&self, layout: Layout) -> *mut u8 {
        record(|calls| calls.alloc = calls.alloc.saturating_add(1));
        // SAFETY: the caller supplies a valid allocation layout.
        unsafe { System.alloc(layout) }
    }

    unsafe fn realloc(&self, pointer: *mut u8, layout: Layout, size: usize) -> *mut u8 {
        record(|calls| calls.realloc = calls.realloc.saturating_add(1));
        // SAFETY: `System` returned `pointer` for `layout`; `size` is the caller's new size.
        unsafe { System.realloc(pointer, layout, size) }
    }

    unsafe fn dealloc(&self, pointer: *mut u8, layout: Layout) {
        record(|calls| calls.dealloc = calls.dealloc.saturating_add(1));
        // SAFETY: `pointer` and `layout` are the pair `System` returned.
        unsafe { System.dealloc(pointer, layout) }
    }
}

#[global_allocator]
static ALLOCATOR: CountingAllocator = CountingAllocator;

/// Counting on this thread from creation until drop, starting from zero.
struct Armed;

impl Armed {
    fn new() -> Self {
        COUNTED.with(|counted| counted.set(Calls::ZERO));
        ARMED.with(|armed| armed.set(true));
        Self
    }
}

impl Drop for Armed {
    fn drop(&mut self) {
        ARMED.with(|armed| armed.set(false));
    }
}

/// The calls counted since the last take.
fn take_counted() -> Calls {
    COUNTED.with(|counted| counted.replace(Calls::ZERO))
}

/// The result of `apply` and the allocator calls it made on this thread.
fn counted<R>(apply: impl FnOnce() -> R) -> (R, Calls) {
    let armed = Armed::new();
    let result = apply();
    let calls = take_counted();
    drop(armed);
    (result, calls)
}

/// Spec 34 §8 and MEM-04 (bounded-c: an in-scan hook allocates nothing): applying the queued
/// writes and the active forces allocates nothing while no request is refused.
///
/// The queues hold scalar values and apply in place: the write queue is drained without a new
/// vector, each force stays in its queue, and no target text is built. A snapshot clone, a
/// `Vec`-returning drain, or an eagerly built label would show up as one of the counts below. The
/// write of `INT#2` and then the force of `INT#3` reach `probe_depth` at the cycle start, and the
/// force of `TRUE` reaches `%QX0.0` before the outputs.
#[test]
fn applying_writes_and_forces_allocates_nothing_when_nothing_is_refused() {
    let mut queues = LiveValueQueues::<u32>::new();
    queues
        .try_upsert_var_write(
            PendingVarTarget::Global("probe_depth".into()),
            Value::Int(2),
        )
        .expect("the write queue has room");
    queues
        .try_upsert_forced_var(ForcedVarTarget::Global("probe_depth".into()), Value::Int(3))
        .expect("the force queue has room");
    queues
        .try_upsert_forced_io(
            IoAddress::parse("%QX0.0").expect("a valid output address"),
            Value::Bool(true),
        )
        .expect("the force queue has room");
    let mut program = Program {
        probe_depth: Value::Int(1),
        led: Value::Bool(false),
    };
    let now = Duration::from_millis(10);

    let (result, cycle_start) = counted(|| {
        apply::compiled_cycle_start(
            &mut queues,
            Generation::FIRST,
            &mut program,
            now,
            &mut NoReport,
        )
    });
    result.expect("the cycle start refuses nothing");
    let (result, after_inputs) = counted(|| {
        apply::compiled_images(
            &mut queues,
            &[IoArea::Input, IoArea::Memory],
            &mut program,
            now,
            &mut NoReport,
        )
    });
    result.expect("the input image refuses nothing");
    let (result, before_outputs) = counted(|| {
        apply::compiled_images(
            &mut queues,
            &[IoArea::Output],
            &mut program,
            now,
            &mut NoReport,
        )
    });
    result.expect("the output image refuses nothing");

    assert_eq!(cycle_start, Calls::ZERO, "the cycle-start apply allocated");
    assert_eq!(after_inputs, Calls::ZERO, "the input-image apply allocated");
    assert_eq!(
        before_outputs,
        Calls::ZERO,
        "the output-image apply allocated"
    );
    assert_eq!(program.probe_depth, Value::Int(3));
    assert_eq!(program.led, Value::Bool(true));
    assert!(queues.pending_var_writes.is_empty());
    assert_eq!(queues.forced_vars.len(), 1, "the force stays in its queue");
    assert_eq!(queues.forced_io.len(), 1, "the force stays in its queue");
}
