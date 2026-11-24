//! A cycle trace's recording allocates nothing in a scan (specification 48 RU-59, "Memory";
//! specification 34 MEM-09): it reserves its memory when it is armed, and every cycle's start state,
//! changes, activations, readings and images go into that memory.
#![allow(
    unsafe_code,
    reason = "the counting allocator is this test's meter, which must implement the unsafe GlobalAlloc trait"
)]

use std::alloc::{GlobalAlloc, Layout, System};
use std::cell::Cell;

use smol_str::SmolStr;
use trust_runtime_core::error::RuntimeError;
use trust_runtime_core::io_address::IoAddress;
use trust_runtime_core::native::image::Image;
use trust_runtime_core::native::reflect::Located;
use trust_runtime_core::native::variables::CompiledVariables;
use trust_runtime_core::trace::{ChangeRef, ChangeSource, IoArea};
use trust_runtime_core::value::{Duration, Value};
use trust_runtime_services::trace::{Budget, Goal, Recorder, Sizes};

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
struct Calls {
    alloc: u32,
    realloc: u32,
    dealloc: u32,
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

/// A program that gives, by name only, the bytes of the INT `Main.x`: 42.
struct Program;

impl CompiledVariables for Program {
    fn read_named(&self, name: &str) -> Result<Value, RuntimeError> {
        Err(RuntimeError::UndefinedVariable(name.into()))
    }

    fn read_named_bytes(
        &self,
        _name: &str,
        out: &mut [u8],
    ) -> Result<(usize, SmolStr), RuntimeError> {
        out[..2].copy_from_slice(&[0x2A, 0]);
        Ok((2, SmolStr::new_static("INT")))
    }

    fn write_named(&mut self, name: &str, _value: &Value) -> Result<(), RuntimeError> {
        Err(RuntimeError::UndefinedVariable(name.into()))
    }

    fn write_retained(&mut self, label: &str, _value: &Value) -> Result<(), RuntimeError> {
        Err(RuntimeError::UndefinedVariable(label.into()))
    }

    fn write_member(
        &mut self,
        _instance: u32,
        member: &str,
        _value: &Value,
    ) -> Result<(), RuntimeError> {
        Err(RuntimeError::UndefinedVariable(member.into()))
    }

    fn write_image(&mut self, _address: &IoAddress, _value: Value) -> Result<(), RuntimeError> {
        Err(RuntimeError::InvalidExecutionState)
    }

    fn program_image(&self) -> Image<'_> {
        unreachable!("the stand-in holds its variables by name, without a program image")
    }

    fn read_located(&self, _at: &Located) -> Result<Value, RuntimeError> {
        Err(RuntimeError::InvalidExecutionState)
    }

    fn write_located(&mut self, _at: &Located, _value: &Value) -> Result<(), RuntimeError> {
        Err(RuntimeError::InvalidExecutionState)
    }
}

#[test]
fn recording_cycles_allocates_nothing() {
    // Catches any growth of a reserved buffer in a scan: three cycles with the start state, a force,
    // variable and address writes, a write read back from the program, a method call,
    // activations, readings and both images.
    let sizes = Sizes {
        areas: [2, 4, 8, 16, 8, 32],
        tasks: 2,
        cycle_interval: Duration::from_millis(10),
        task_interval: None,
    };
    let mut recorder = Recorder::arm(Goal::Cycles(3), sizes, Budget::COMPUTER).expect("armed");
    let inputs = [1_u8, 0];
    let outputs = [0_u8, 0, 0x2A, 0];
    let meter = Armed::new();
    for cycle in 0..3_i64 {
        let now = Duration::from_millis(10 * cycle);
        if recorder.begin_cycle(now, 0) == trust_runtime_services::trace::Begin::First {
            for id in 1..=6 {
                if let Some(area) = recorder.start_area(id) {
                    area.fill(id);
                }
            }
            recorder.start_force(
                ChangeSource::ControlLink,
                &ChangeRef::VariableForce {
                    path: "Main.limit",
                    type_name: "REAL",
                    value: &[0, 0, 0xAC, 0x41],
                },
            );
        }
        recorder.change(
            ChangeSource::Screen,
            &ChangeRef::VariableWrite {
                path: "Main.x",
                type_name: "INT",
                value: &[0x2A, 0],
            },
        );
        recorder.change(
            ChangeSource::Mesh,
            &ChangeRef::AddressWrite {
                area: IoArea::Memory,
                bit: Some(3),
                offset: 1,
                bytes: &[1],
            },
        );
        recorder.change(
            ChangeSource::ControlStudio,
            &ChangeRef::MethodCall {
                instance: "Main.Heating",
                interface: "Plant.ISequence",
                method: "Start",
                inputs: &[("Setpoint", "21.5")],
            },
        );
        recorder.written(ChangeSource::ControlLink, "Main.x", &Program);
        recorder.activation(0, now.as_nanos());
        recorder.activation(1, now.as_nanos());
        recorder.wall_clock(1_795_000_000_000 + cycle);
        recorder.inputs(&inputs);
        recorder.outputs(&outputs);
        recorder.end_cycle(None);
    }
    let calls = COUNTED.with(Cell::get);
    drop(meter);
    assert_eq!(calls, Calls::ZERO);
    assert_eq!(recorder.cycles(), 3);
}
