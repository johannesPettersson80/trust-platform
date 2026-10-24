//! The compiled program's cycle (specification 44 §8.7, specification 34 §8): the scan's order, its
//! outputs, its faults, the safe outputs, the deadline and the restarts, with a stand-in module
//! written against the kernel interface.
//!
//! The image: `MAIN` in the task `Fast` of 10 ms at priority 1. Its body counts its scans in byte 0
//! of the instance area, starting from 5, and copies the count's lowest bit to byte 1, which is
//! bound to `%QX0.1`. A second image's body divides by zero.

use alloc::format;
use alloc::rc::Rc;
use alloc::string::String;
use alloc::vec;
use alloc::vec::Vec;
use core::cell::{Cell, RefCell};

use trust_plc_abi::kernel::{Entry, WITHOUT_DEBUGGER};
use trust_plc_abi::module::{fault, run, stmt};
use trust_plc_abi::{Cx, FaultCode, Frame, KernelApi, AREA_INSTANCES};
use trust_runtime_core::error::RuntimeError;
use trust_runtime_core::io_address::IoAddress;
use trust_runtime_core::io_image::FlatImages;
use trust_runtime_core::memory::Generation;
use trust_runtime_core::native::image::{
    type_kind, AreaRecord, CodeImage, EntryKind, EntryRecord, FileRecord, ImageWriter,
    IoBindRecord, MemoryFigure, RetainRecord, SectionId, StmtRecord, StringPool, SymbolRecord,
    Target, TaskRecord, TypeRecord, AREA_HAS_INIT, NONE, SYMBOL_PROGRAM,
};
use trust_runtime_core::native::memory::{read_place, symbol_place};
use trust_runtime_core::native::program::{admit, Device, ModuleCode, NativeProgram};
use trust_runtime_core::native::variables::{CompiledAccessPoint, CompiledVariables};
use trust_runtime_core::native::KERNEL_FUNCTIONS;
use trust_runtime_core::ports::{MonotonicClock, MonotonicInstant};
use trust_runtime_core::retain::RestartMode;
use trust_runtime_core::value::{Duration, Value};
use trust_runtime_core::vm::{CycleHooks, CyclePhase};
use trust_runtime_core::watchdog::{FaultPolicy, WatchdogPolicy};

use super::CompiledCycle;
use crate::cycle_step::{StepOutcome, StepRefusal, StepRequest};
use crate::runner::cycle::{ResourceCycle, RetainSource, TaskDescriptor};
use crate::runner::debugger::{Boundary, StatementStart, StopPoints};
use crate::runner::engine::{CycleSettings, ProcessIo};
use crate::runner::hold::{CompiledHeld, HeldView, HoldHandle, HoldPass};
use crate::status::IoDriverStatus;

const MS: i64 = 1_000_000;

macro_rules! entry {
    ($name:ident, $body:expr) => {
        #[cfg(target_arch = "x86_64")]
        extern "sysv64" fn $name(cx: *mut Cx) -> u32 {
            run(cx, $body)
        }
        #[cfg(not(target_arch = "x86_64"))]
        extern "C" fn $name(cx: *mut Cx) -> u32 {
            run(cx, $body)
        }
    };
}

entry!(counting, |cx, areas| {
    let view: &[Cell<u8>; 8] = areas.view(cx, AREA_INSTANCES)?;
    view[0].set(view[0].get().wrapping_add(1));
    view[1].set(view[0].get() & 1);
    Ok(())
});

entry!(dividing_by_zero, |cx, _| Err(fault(
    cx,
    FaultCode::DivisionByZero,
    0,
    0,
    0
)));

entry!(nothing, |_, _| Ok(()));

// The counting body of a debug image: it links its frame, as a POU of a debug image does
// (specification 44 §8.2, §13.2), and calls the statement hook before each of its two statements,
// ids 0 and 1.
entry!(stopping, |cx, areas| {
    let view: &[Cell<u8>; 8] = areas.view(cx, AREA_INSTANCES)?;
    let link = Frame {
        caller: cx.frame,
        pou: 0,
        call_stmt: cx.stmt,
        locals: core::ptr::null(),
        instance: view.as_ptr().cast::<u8>(),
    };
    cx.frame = &raw const link;
    let result = (|| {
        stmt(cx, 0)?;
        view[0].set(view[0].get().wrapping_add(1));
        stmt(cx, 1)?;
        view[1].set(view[0].get() & 1);
        Ok(())
    })();
    cx.frame = link.caller;
    result
});

/// The stand-in module: code offset 0 counts, 16 divides by zero, 32 does nothing, 48 counts as
/// a debug image does.
pub(super) struct StandIn;

impl ModuleCode for StandIn {
    fn entry(&self, offset: u32) -> Option<Entry> {
        match offset {
            0 => Some(Entry::from_fn(counting)),
            16 => Some(Entry::from_fn(dividing_by_zero)),
            32 => Some(Entry::from_fn(nothing)),
            48 => Some(Entry::from_fn(stopping)),
            _ => None,
        }
    }
}

/// The image of `MAIN` whose body is at code offset `body`.
fn image(body: u32) -> Vec<u8> {
    image_of(body, false)
}

/// The image of `MAIN` whose body is at code offset `body`; a `debug` image has the statement table
/// of the debug body's two statements, on lines 1 and 2 of `main.st`, and the POU symbol `@MAIN`
/// (specification 44 §5.7, §5.9).
pub(super) fn image_of(body: u32, debug: bool) -> Vec<u8> {
    image_with(body, debug, false)
}

/// The image of [`image_of`], as a release image when `release` is set: one built without statement
/// hooks (specification 44 §13.2).
pub(super) fn image_with(body: u32, debug: bool, release: bool) -> Vec<u8> {
    let mut strings = StringPool::new();
    let fast = strings.add("Fast");
    let mut writer = ImageWriter::new(
        Target::this_cpu().unwrap_or(Target::X86_64),
        u16::try_from(trust_plc_abi::INTERFACE_VERSION).unwrap(),
    );
    writer.release = release;
    writer.module(CodeImage {
        code: vec![0; 64],
        rodata: Vec::new(),
        rodata_distance: 64,
    });
    writer.table(
        &[
            EntryRecord {
                kind: EntryKind::ProgramBody as u32,
                index: 0,
                code_offset: body,
            },
            EntryRecord {
                kind: EntryKind::ColdInit as u32,
                index: 0,
                code_offset: 32,
            },
        ],
        &[],
    );
    let area = |id: u8, length: u64, flags: u32| AreaRecord {
        id: u32::from(id),
        length,
        alignment: 8,
        flags,
        init_offset: 0,
    };
    writer.table(
        &[
            area(1, 0, 0),
            area(2, 1, 0),
            area(3, 0, 0),
            area(4, 0, 0),
            area(5, 0, 0),
            area(6, 8, AREA_HAS_INIT),
        ],
        &[],
    );
    writer.section(SectionId::Init, vec![5, 0, 0, 0, 0, 0, 0, 0]);
    writer.table(
        &[TaskRecord {
            name: fast,
            interval_ns: 10 * MS as u64,
            priority: 1,
            single: NONE,
            program_count: 1,
            program_first: 0,
            flags: 0,
        }],
        &0u32.to_le_bytes(),
    );
    name_tables(&mut writer, &mut strings, debug);
    writer.section(SectionId::Strings, strings.into_bytes());
    writer.finish().expect("the image builds")
}

/// The image's binding of `MAIN.OUT` to `%QX0.1`, its symbols, its one type, its file and its
/// memory figure: areas 72, stack 64 and bookkeeping 24 bytes, 160 in all.
fn name_tables(writer: &mut ImageWriter, strings: &mut StringPool, debug: bool) {
    let pou = strings.add("@MAIN");
    let main = strings.add("MAIN");
    let count = strings.add("MAIN.COUNT");
    let out = strings.add("MAIN.OUT");
    let label = strings.add("main.st");
    // A debug image's POU symbol `@MAIN` sorts first and moves the others one on.
    let first = u32::from(debug);
    writer.table(
        &[IoBindRecord {
            symbol: first + 2,
            area: 2,
            size: 0,
            byte: 0,
            bit: 1,
            flags: 0,
        }],
        &[],
    );
    let symbol = |canonical, offset, size, flags, parent| SymbolRecord {
        canonical,
        display: canonical,
        type_index: 0,
        area: 6,
        flags,
        offset,
        size,
        parent,
        pou: NONE,
    };
    let mut symbols = Vec::new();
    if debug {
        symbols.push(SymbolRecord {
            area: 0,
            size: 0,
            pou: 0,
            ..symbol(pou, 0, 0, 0, NONE)
        });
    }
    symbols.extend([
        // The program root carries the instance's number, 0 (specification 44 §5.7).
        SymbolRecord {
            pou: 0,
            ..symbol(main, 0, 8, SYMBOL_PROGRAM, NONE)
        },
        symbol(count, 0, 1, 0, first),
        symbol(out, 1, 1, 0, first),
    ]);
    writer.table(&symbols, &[]);
    writer.table(
        &[TypeRecord {
            kind: type_kind::USINT,
            name: 0,
            size: 1,
            alignment: 1,
            detail: 0,
            count: 0,
        }],
        &[],
    );
    let statement = |line| StmtRecord {
        file: 0,
        pou: 0,
        start_line: line,
        start_column: 1,
        end_line: line,
        end_column: 20,
    };
    if debug {
        writer.table(&[statement(1), statement(2)], &[]);
    } else {
        writer.table::<StmtRecord>(&[], &[]);
    }
    writer.table::<RetainRecord>(&[], &[]);
    writer.table(
        &[FileRecord {
            label,
            source_offset: NONE,
            source_length: 0,
        }],
        &[],
    );
    writer.section(
        SectionId::MemFig,
        MemoryFigure {
            areas: 72,
            stack: 64,
            bookkeeping: 24,
            total: 160,
            ..MemoryFigure::default()
        }
        .bytes()
        .to_vec(),
    );
}

/// What the stand-in drivers saw: each output image they drove and each safe output image.
#[derive(Default)]
pub(super) struct Driven {
    outputs: Vec<Vec<u8>>,
    safe: Vec<Vec<u8>>,
}

/// Drivers with no input, which record what they drive.
pub(super) struct Drivers(Rc<RefCell<Driven>>);

impl ProcessIo for Drivers {
    fn read_inputs(&mut self, _image: &mut [u8]) -> Result<(), RuntimeError> {
        Ok(())
    }

    fn write_outputs(&mut self, image: &[u8]) -> Result<(), RuntimeError> {
        self.0.borrow_mut().outputs.push(image.to_vec());
        Ok(())
    }

    fn write_safe_outputs(&mut self, image: &[u8]) -> Result<(), RuntimeError> {
        self.0.borrow_mut().safe.push(image.to_vec());
        Ok(())
    }

    fn driver_health(&self) -> Vec<IoDriverStatus> {
        Vec::new()
    }
}

/// A clock that reads what the test sets, in microseconds.
#[derive(Clone)]
pub(super) struct Clock(Rc<Cell<u64>>);

impl MonotonicClock for Clock {
    fn now(&self) -> MonotonicInstant {
        MonotonicInstant::from_micros(self.0.get())
    }
}

/// An observer that records each phase with its logical time and the output image it was given.
#[derive(Default)]
struct Phases(Vec<(CyclePhase, Duration, Vec<u8>)>);

impl CycleHooks for Phases {
    fn on_compiled_phase(
        &mut self,
        phase: CyclePhase,
        now: Duration,
        images: &FlatImages,
    ) -> Result<(), RuntimeError> {
        self.0.push((phase, now, images.outputs.clone()));
        Ok(())
    }
}

fn settings(safe_outputs: Vec<(IoAddress, Value)>) -> CycleSettings {
    CycleSettings {
        cycle_interval: Duration::from_millis(10),
        watchdog: WatchdogPolicy::default(),
        fault_policy: FaultPolicy::Halt,
        safe_outputs,
    }
}

pub(super) type Cycle<'i> = CompiledCycle<'i, Clock, Drivers, StandIn>;

/// The cycle of the image `bytes`, started, with its drivers' record and its clock.
fn cycle(
    bytes: &[u8],
    safe_outputs: Vec<(IoAddress, Value)>,
) -> (Cycle<'_>, Rc<RefCell<Driven>>, Rc<Cell<u64>>) {
    cycle_with(bytes, safe_outputs, &WITHOUT_DEBUGGER)
}

/// The cycle of the image `bytes`, started, its program calling the kernel functions `api`.
pub(super) fn cycle_with<'b>(
    bytes: &'b [u8],
    safe_outputs: Vec<(IoAddress, Value)>,
    api: &'static KernelApi,
) -> (Cycle<'b>, Rc<RefCell<Driven>>, Rc<Cell<u64>>) {
    let device = Device {
        target: Target::this_cpu().unwrap_or(Target::X86_64),
        program_memory: 1_000,
        task_stack: 4_096,
        max_statements: 1_000,
    };
    let admitted = admit(bytes, &device).expect("the image is admitted");
    let program =
        NativeProgram::new(admitted, StandIn, &device, api).expect("the program is built");
    let driven = Rc::new(RefCell::new(Driven::default()));
    let time = Rc::new(Cell::new(0));
    let cycle = CompiledCycle::start(
        program,
        Drivers(Rc::clone(&driven)),
        settings(safe_outputs),
        Clock(Rc::clone(&time)),
    )
    .expect("the program starts");
    (cycle, driven, time)
}

#[test]
fn a_scan_runs_the_due_task_and_then_drives_its_outputs() {
    // Specification 34 CYCLE-01 to CYCLE-06 and specification 10 §4: the task of 10 ms is not due at
    // its start, 0 ms, and is due at 10 ms; the drivers drive the output image after the program
    // ran, once per scan. The count starts at 5: the first activation makes 6 (bit 1 clear), the
    // second 7 (bit 1 set, `%QX0.1` = 0b10). Catches outputs driven before the task, a task run at
    // its start, and a scan that drives nothing.
    let bytes = image(0);
    let (mut cycle, driven, _) = cycle(&bytes, Vec::new());
    assert_eq!(
        cycle.tasks(),
        [TaskDescriptor {
            name: "Fast".into(),
            priority: 1,
        }]
    );
    let mut phases = Phases::default();
    for time in [0, 10, 20] {
        cycle
            .run_scan(Duration::from_millis(time), None, &mut phases)
            .expect("the scan runs");
    }
    assert_eq!(driven.borrow().outputs, [vec![0], vec![0], vec![0b10]]);
    assert_eq!(cycle.program().value("MAIN.COUNT"), Some(Value::USInt(7)));
}

#[test]
fn the_observer_sees_the_phases_of_an_engine_scan_in_order() {
    // Specification 44 §8.7: a compiled program's scan reports the engine's phases in the engine's
    // order, the due task by its index, with the scan's logical time; the output image the observer
    // sees after the output sample is the one the scan made. Catches a task numbered by the image
    // instead of the cycle, phases out of order, and a snapshot of the previous scan.
    let bytes = image(0);
    let (mut cycle, _, _) = cycle(&bytes, Vec::new());
    let mut phases = Phases::default();
    for time in [10, 20] {
        cycle
            .run_scan(Duration::from_millis(time), None, &mut phases)
            .expect("the scan runs");
    }
    let second: Vec<_> = phases.0[7..]
        .iter()
        .map(|(phase, now, outputs)| (*phase, now.as_nanos() / MS, outputs.clone()))
        .collect();
    assert_eq!(
        second,
        [
            (CyclePhase::CycleStart, 20, vec![0]),
            (CyclePhase::BeforeInputSample, 20, vec![0]),
            (CyclePhase::TaskStart(0), 20, vec![0]),
            (CyclePhase::TaskEnd(0), 20, vec![0]),
            (CyclePhase::BeforeOutputSample, 20, vec![0b10]),
            (CyclePhase::AfterOutputSample, 20, vec![0b10]),
            (CyclePhase::CycleEnd, 20, vec![0b10]),
        ]
    );
}

/// An observer that records each boundary of a scan, its phases and its Live Values access points,
/// and the output image of each scan's snapshot (`io.read`), and writes through the access points as
/// Live Values do: `MAIN.COUNT` := 40 at the cycle start of the scan at 10 ms, and `%QX0.1` := TRUE
/// before the outputs of the scan at 20 ms.
#[derive(Default)]
struct LiveWrites(Vec<String>, Vec<Vec<u8>>);

impl CycleHooks for LiveWrites {
    fn on_compiled_phase(
        &mut self,
        phase: CyclePhase,
        _now: Duration,
        images: &FlatImages,
    ) -> Result<(), RuntimeError> {
        self.0.push(format!("{phase:?}"));
        if phase == CyclePhase::BeforeOutputSample {
            self.1.push(images.outputs.clone());
        }
        Ok(())
    }

    fn on_compiled_access(
        &mut self,
        point: CompiledAccessPoint,
        now: Duration,
        variables: &mut dyn CompiledVariables,
    ) -> Result<(), RuntimeError> {
        self.0.push(format!("access {point:?}"));
        match (point, now.as_nanos() / MS) {
            (CompiledAccessPoint::CycleStart, 10) => {
                variables.write_named("MAIN.COUNT", &Value::USInt(40))
            }
            (CompiledAccessPoint::BeforeOutputs, 20) => {
                variables.write_image(&IoAddress::parse("%QX0.1")?, Value::Bool(true))
            }
            _ => Ok(()),
        }
    }
}

#[test]
fn live_values_reach_a_compiled_scan_at_the_cycle_start_after_the_inputs_and_before_the_outputs() {
    // Specification 44 §13.3: Live Values reach a compiled program's variables at the cycle start,
    // before the program runs; its input and marker images after the drivers read the inputs,
    // before the program samples them; and its output image after the scan, before the outputs
    // reach the drivers. `MAIN.COUNT` := 40 at the cycle start of the scan at 10 ms makes the
    // count 41, odd, so `%QX0.1` is set (0b10) where the count of 6 would leave it clear; at 20 ms
    // the count of 42 clears it, and the write of TRUE before the outputs reaches the drivers
    // (0b10). The snapshot that `io.read` reads shows the output image the drivers drive, the
    // written bit too (0b10 at 20 ms), as an engine's snapshot shows a forced output (C6 bench,
    // 24 October 2026: a forced `%QX0.1` read back FALSE). Catches a write the scan does not see,
    // an output write the scan overwrites, an access point at another boundary, and a snapshot
    // taken from the program's output area rather than the driven image.
    let bytes = image(0);
    let (mut cycle, driven, _) = cycle(&bytes, Vec::new());
    let mut live = LiveWrites::default();
    for time in [0, 10, 20] {
        cycle
            .run_scan(Duration::from_millis(time), None, &mut live)
            .expect("the scan runs");
    }
    assert_eq!(driven.borrow().outputs, [vec![0], vec![0b10], vec![0b10]]);
    assert_eq!(live.1, [vec![0], vec![0b10], vec![0b10]]);
    assert_eq!(cycle.program().value("MAIN.COUNT"), Some(Value::USInt(42)));
    let last = &live.0[live.0.len() - 10..];
    assert_eq!(
        last,
        [
            "CycleStart",
            "access CycleStart",
            "BeforeInputSample",
            "access AfterInputs",
            "TaskStart(0)",
            "TaskEnd(0)",
            "access BeforeOutputs",
            "BeforeOutputSample",
            "AfterOutputSample",
            "CycleEnd",
        ]
    );
}

#[test]
fn a_late_scan_reports_the_missed_activations() {
    // Specification 10 §4: a scan 25 ms after the last activation at 10 ms passed the deadlines of
    // 20 and 30 ms; one activation runs and one is missed, reported before the task starts. Catches
    // an overrun that is not reported or reported with the image's task number.
    let bytes = image(0);
    let (mut cycle, _, _) = cycle(&bytes, Vec::new());
    let mut phases = Phases::default();
    for time in [10, 35] {
        cycle
            .run_scan(Duration::from_millis(time), None, &mut phases)
            .expect("the scan runs");
    }
    assert!(phases.0.iter().any(|(phase, _, _)| *phase
        == CyclePhase::TaskOverrun {
            index: 0,
            missed: 1
        }));
}

#[test]
fn a_fault_ends_the_scan_before_any_output_and_stays_latched_until_a_restart() {
    // Specification 44 §8.6 and specification 34 STATE-04: a module's fault fails the scan with its
    // runtime error and no output is driven; once latched, every scan is refused with it, and a
    // restart clears it. Catches a fault whose outputs are driven, a latch that runs the next scan,
    // and a restart that keeps the latch.
    let bytes = image(16);
    let (mut cycle, driven, _) = cycle(&bytes, Vec::new());
    let mut phases = Phases::default();
    let error = cycle
        .run_scan(Duration::from_millis(10), None, &mut phases)
        .expect_err("the module faults");
    assert_eq!(error, RuntimeError::DivisionByZero);
    assert_eq!(driven.borrow().outputs, Vec::<Vec<u8>>::new());
    cycle.latch_fault(error.clone());
    assert_eq!(
        cycle.run_scan(Duration::from_millis(20), None, &mut phases),
        Err(error)
    );
    cycle
        .restart(RestartMode::Warm, RetainSource::Memory)
        .expect("the restart completes");
    assert_eq!(cycle.generation(), Generation(1));
    // After the restart the scan runs again, and faults again.
    assert_eq!(
        cycle.run_scan(Duration::from_millis(30), None, &mut phases),
        Err(RuntimeError::DivisionByZero)
    );
}

#[test]
fn a_scan_past_its_deadline_drives_no_output() {
    // Specification 34 TIME-04: the deadline is checked before the outputs are published; a scan
    // that ends after it fails with the execution timeout and drives nothing. Catches a deadline
    // that is never checked or checked after the outputs.
    let bytes = image(0);
    let (mut cycle, driven, time) = cycle(&bytes, Vec::new());
    time.set(5_000);
    let deadline = MonotonicInstant::from_micros(4_000);
    let mut phases = Phases::default();
    assert_eq!(
        cycle.run_scan(Duration::from_millis(10), Some(deadline), &mut phases),
        Err(RuntimeError::ExecutionTimeout)
    );
    assert_eq!(driven.borrow().outputs, Vec::<Vec<u8>>::new());
}

#[test]
fn the_safe_outputs_are_driven_from_the_output_image() {
    // Specification 34 STATE-02: a stop drives each output to its `[[io.safe_state]]` value. After a
    // scan set `%QX0.1`, the safe value FALSE clears it in the image the drivers get. Catches safe
    // values written somewhere the drivers do not read, and safe outputs that are not driven.
    let bytes = image(0);
    let address = IoAddress::parse("%QX0.1").expect("the address parses");
    let (mut cycle, driven, _) = cycle(&bytes, vec![(address, Value::Bool(false))]);
    let mut phases = Phases::default();
    for time in [10, 20] {
        cycle
            .run_scan(Duration::from_millis(time), None, &mut phases)
            .expect("the scan runs");
    }
    assert_eq!(driven.borrow().outputs.last(), Some(&vec![0b10]));
    cycle.apply_safe_outputs().expect("the safe outputs drive");
    assert_eq!(driven.borrow().safe, [vec![0]]);
}

#[test]
fn a_cold_restart_starts_from_the_starting_memory() {
    // Specification 44 §6.7 and IEC 61131-3 §6.5.6.2: a cold restart gives every variable its
    // starting value again. Two activations count from 5 to 7, the restart sets 5, and the next
    // generation is installed. Catches a restart that keeps the program's memory.
    let bytes = image(0);
    let (mut cycle, _, _) = cycle(&bytes, Vec::new());
    let mut phases = Phases::default();
    for time in [10, 20] {
        cycle
            .run_scan(Duration::from_millis(time), None, &mut phases)
            .expect("the scan runs");
    }
    assert_eq!(cycle.program().value("MAIN.COUNT"), Some(Value::USInt(7)));
    cycle
        .restart(RestartMode::Cold, RetainSource::Checkpoint)
        .expect("the restart completes");
    assert_eq!(cycle.program().value("MAIN.COUNT"), Some(Value::USInt(5)));
    assert_eq!(cycle.generation(), Generation(1));
}

#[test]
fn the_memory_figure_is_the_images() {
    // Specification 44 §10.4: `status` reports the image's figure: areas 72, stack 64 and
    // bookkeeping 24, 160 bytes in all, and no retain capture. Catches a figure computed on the
    // device or taken from another part.
    let bytes = image(0);
    let (cycle, _, _) = cycle(&bytes, Vec::new());
    let memory = cycle.program_memory().expect("the cycle knows its figure");
    assert_eq!(memory.figure.persistent_block, 72);
    assert_eq!(memory.figure.activation_stack, 64);
    assert_eq!(memory.figure.engine, 24);
    assert_eq!(memory.bytes(), 160);
}

/// The platform's allocator count of the test: it never changes.
fn operations() -> u32 {
    7
}

#[test]
fn a_scan_reports_the_allocations_of_its_program() {
    // Specification 34, "The allocation count": the cycle reads the platform's count around the
    // program's scan and reports the difference; with no allocation between the two readings the
    // scan reports 0, and the loop reads the count itself. Catches a cycle that reports no count,
    // or the count itself instead of the scan's difference.
    let bytes = image(0);
    let (cycle, _, _) = cycle(&bytes, Vec::new());
    let mut cycle = cycle.with_allocator_operations(operations);
    assert_eq!(cycle.last_scan_allocations(), None);
    cycle
        .run_scan(Duration::from_millis(10), None, &mut Phases::default())
        .expect("the scan runs");
    assert_eq!(cycle.last_scan_allocations(), Some(0));
    assert_eq!(cycle.allocator_operations(), Some(7));
}

/// The allocator count of the next test, which its observer raises.
static OBSERVED: core::sync::atomic::AtomicU32 = core::sync::atomic::AtomicU32::new(0);

fn observed_operations() -> u32 {
    OBSERVED.load(core::sync::atomic::Ordering::Relaxed)
}

/// An observer each of whose phases counts as one allocator operation.
struct Allocating;

impl CycleHooks for Allocating {
    fn on_compiled_phase(
        &mut self,
        _phase: CyclePhase,
        _now: Duration,
        _images: &FlatImages,
    ) -> Result<(), RuntimeError> {
        OBSERVED.fetch_add(1, core::sync::atomic::Ordering::Relaxed);
        Ok(())
    }
}

#[test]
fn the_observers_operations_are_not_the_programs() {
    // Specification 34, "The allocation count": a scan's count is the program's and the kernel's;
    // what the loop's observer allocates inside the scan, at the task phases, is the services'.
    // Each of the observer's seven phases here counts one operation and the program makes none:
    // the scan reports 0, while the platform's count rose by 7. Catches the observer's operations
    // at the task's start and end counted as the program's.
    let bytes = image(0);
    let (cycle, _, _) = cycle(&bytes, Vec::new());
    let mut cycle = cycle.with_allocator_operations(observed_operations);
    let start = observed_operations();
    cycle
        .run_scan(Duration::from_millis(10), None, &mut Allocating)
        .expect("the scan runs");
    assert_eq!(cycle.last_scan_allocations(), Some(0));
    assert_eq!(observed_operations() - start, 7);
}

/// A step of the fixture's program at `time` milliseconds, with `restart`, writing `inputs` and
/// reading `watch` and the output bit `%QX0.1`.
fn step(
    time: i64,
    restart: Option<RestartMode>,
    inputs: Vec<(&str, Value)>,
    watch: &[&str],
) -> StepRequest {
    StepRequest {
        time: Duration::from_millis(time),
        restart,
        inputs: inputs
            .into_iter()
            .map(|(name, value)| (name.into(), value))
            .collect(),
        direct_inputs: Vec::new(),
        watch: watch.iter().map(|name| (*name).into()).collect(),
        watch_direct: vec![(
            "%QX0.1".into(),
            IoAddress::parse("%QX0.1").expect("an output bit"),
        )],
        run: None,
    }
}

#[test]
fn a_step_restarts_writes_scans_and_reads_without_the_drivers() {
    // Specification 11 §6.8.1: a cold restart at time 0 gives the count its starting 5; the input
    // then writes 20; the scan at 10 ms runs the due task once, 21, whose lowest bit sets `OUT` and
    // `%QX0.1`. `OUT` alone names `MAIN.OUT`, as the one program declares it. The drivers neither
    // read nor drive anything, and the step latches nothing. Catches an input written before the
    // restart (the count would be 6), a scan at the restart's time (the task is not due at 0), a
    // step that drives the outputs, and a bare name that resolves to nothing.
    let bytes = image(0);
    let (mut cycle, driven, _) = cycle(&bytes, Vec::new());
    let outcome = cycle
        .step(
            &step(
                10,
                Some(RestartMode::Cold),
                vec![("MAIN.COUNT", Value::USInt(20))],
                &["MAIN.COUNT", "out"],
            ),
            Duration::ZERO,
            None,
        )
        .expect("the step runs");
    assert_eq!(
        outcome,
        StepOutcome {
            values: vec![
                ("MAIN.COUNT".into(), Value::USInt(21)),
                ("out".into(), Value::USInt(1)),
            ],
            direct: vec![("%QX0.1".into(), Value::Bool(true))],
            error: None,
        }
    );
    assert_eq!(
        driven.borrow().outputs,
        Vec::<Vec<u8>>::new(),
        "no driver was driven"
    );
    assert_eq!(
        cycle.generation(),
        Generation::FIRST.next().expect("a next generation")
    );
}

#[test]
fn a_name_that_names_nothing_refuses_the_step_before_anything_changes() {
    // Specification 11 §6.8.1: a name that names no variable refuses the whole step before the
    // restart, so the count the first scan made, 6, stays. Catches names resolved after the
    // restart.
    let bytes = image(0);
    let (mut cycle, _, _) = cycle(&bytes, Vec::new());
    cycle
        .run_scan(Duration::from_millis(10), None, &mut Phases::default())
        .expect("the scan runs");
    let refused = cycle.step(
        &step(
            20,
            Some(RestartMode::Cold),
            vec![("NOPE", Value::USInt(1))],
            &[],
        ),
        Duration::from_millis(10),
        None,
    );
    assert_eq!(
        refused,
        Err(StepRefusal::name(
            "NOPE",
            "names no variable of the compiled program"
        ))
    );
    assert_eq!(cycle.program().value("MAIN.COUNT"), Some(Value::USInt(6)));
}

#[test]
fn a_stepped_fault_is_reported_and_not_latched() {
    // Specification 11 §6.8.1: the fault of a stepped scan is in its outcome, and the next step runs
    // the next scan, which faults again on its own. Catches a step that latches its fault, after
    // which the next step would answer the latched fault without scanning.
    let bytes = image(16);
    let (mut cycle, driven, _) = cycle(&bytes, Vec::new());
    for (time, restart) in [(10, Some(RestartMode::Cold)), (20, None)] {
        let outcome = cycle
            .step(&step(time, restart, Vec::new(), &[]), Duration::ZERO, None)
            .expect("the step runs");
        assert_eq!(
            outcome.error,
            Some(RuntimeError::DivisionByZero),
            "{time} ms"
        );
    }
    assert_eq!(driven.borrow().outputs, Vec::<Vec<u8>>::new());
}

#[test]
fn a_step_runs_a_program_instance_by_the_number_its_root_symbol_carries() {
    // Specification 11 §6.8.1, `run`, and specification 44 §5.7: the body of `MAIN`, found by the
    // instance number its root symbol carries, runs once per step at time 0, when the task `Fast`
    // is not due: the count goes 5, 6, 7. The outputs are not published, so `%QX0.1` stays FALSE
    // although `OUT` is 1 after the second run. Catches a run that publishes the outputs, a
    // scheduler cycle in its place (the count would stay 5), and a name that is not refused.
    let bytes = image(0);
    let (mut cycle, _, _) = cycle(&bytes, Vec::new());
    let mut request = step(
        0,
        Some(RestartMode::Cold),
        Vec::new(),
        &["MAIN.COUNT", "OUT"],
    );
    request.run = Some("main".into());
    let first = cycle
        .step(&request, Duration::ZERO, None)
        .expect("the step runs");
    assert_eq!(first.values[0].1, Value::USInt(6));
    request.restart = None;
    let second = cycle
        .step(&request, Duration::ZERO, None)
        .expect("the step runs");
    assert_eq!(
        second,
        StepOutcome {
            values: vec![
                ("MAIN.COUNT".into(), Value::USInt(7)),
                ("OUT".into(), Value::USInt(1)),
            ],
            direct: vec![("%QX0.1".into(), Value::Bool(false))],
            error: None,
        }
    );
    request.run = Some("NOPE".into());
    assert_eq!(
        cycle.step(&request, Duration::ZERO, None),
        Err(StepRefusal::name("NOPE", "names no program instance"))
    );
}

/// A hold that holds at every stop it is asked about, reads `MAIN.COUNT` through the held memory,
/// and arms the statements `points` names; with debugging off it arms nothing.
struct FakeHold {
    enabled: bool,
    points: StopPoints,
    stops: RefCell<Vec<(u32, (u32, u32))>>,
    read: RefCell<Vec<Value>>,
}

impl FakeHold {
    fn new(enabled: bool, points: StopPoints) -> Rc<Self> {
        Rc::new(Self {
            enabled,
            points,
            stops: RefCell::new(Vec::new()),
            read: RefCell::new(Vec::new()),
        })
    }
}

impl HoldPass for FakeHold {
    fn scan_begins(&self) -> bool {
        self.enabled
    }

    fn at_statement(&self, at: &StatementStart<'_>) -> Result<Boundary, RuntimeError> {
        self.stops.borrow_mut().push((at.depth, at.span.start));
        Ok(Boundary::Hold)
    }

    fn hold(&self, _at: &StatementStart<'_>, _view: &HeldView<'_>) -> Result<(), RuntimeError> {
        Err(RuntimeError::InvalidExecutionState)
    }

    fn hold_compiled(
        &self,
        _at: &StatementStart<'_>,
        view: &CompiledHeld<'_>,
    ) -> Result<(), RuntimeError> {
        let image = view.memory.image();
        let (index, _) = image
            .find_symbol("MAIN.COUNT")
            .expect("MAIN.COUNT has a symbol");
        let (place, type_index) = symbol_place(&image, index)?;
        self.read
            .borrow_mut()
            .push(read_place(view.memory, place, type_index)?);
        Ok(())
    }

    fn stop_points(&self) -> StopPoints {
        self.points.clone()
    }

    fn cells_free(&self) -> bool {
        true
    }
}

/// An observer that writes `MAIN.COUNT` := 40 when a hold ends, as Live Values queued during the
/// hold reach the program then, and counts the holds that ended.
#[derive(Default)]
struct WriteAtHoldEnd(u32);

impl CycleHooks for WriteAtHoldEnd {
    fn on_compiled_access(
        &mut self,
        point: CompiledAccessPoint,
        _now: Duration,
        variables: &mut dyn CompiledVariables,
    ) -> Result<(), RuntimeError> {
        if point == CompiledAccessPoint::HoldEnd {
            self.0 += 1;
            variables.write_named("MAIN.COUNT", &Value::USInt(40))?;
        }
        Ok(())
    }
}

/// The started cycle of the debug body with `hold` attached, its program calling the kernel's
/// functions, whose stop goes to the cycle.
fn held_cycle<'b>(bytes: &'b [u8], hold: &Rc<FakeHold>) -> (Cycle<'b>, Rc<RefCell<Driven>>) {
    let (mut cycle, driven, _) = cycle_with(bytes, Vec::new(), &KERNEL_FUNCTIONS);
    let handle: Rc<dyn HoldPass> = Rc::clone(hold) as Rc<dyn HoldPass>;
    cycle.attach_hold(HoldHandle(handle));
    (cycle, driven)
}

#[test]
fn a_compiled_scan_holds_at_its_breakpoint_with_the_held_memory() {
    // Specification 44 §13.2, worked by hand: the breakpoint on line 2 of `main.st` arms statement
    // 1 alone. The task is not due at 0 ms; at 10 ms the body counts 5 to 6 at statement 0 and stops
    // at statement 1, at call depth 1 (its own frame): the hold reads `MAIN.COUNT` = 6 through the
    // held memory. When the hold ends the observer writes `MAIN.COUNT` := 40, which statement 1
    // then sees: `MAIN.OUT` = 40 & 1 = 0, so `%QX0.1` stays clear, and the count is 40. Catches a
    // stop at an unarmed statement, a depth without the frame, a hold that reads another state
    // than the held one, and Live Values that reach the program after the statement ran.
    let bytes = image_of(48, true);
    let hold = FakeHold::new(
        true,
        StopPoints {
            every: false,
            starts: vec![(0, (2, 1))],
        },
    );
    let (mut cycle, driven) = held_cycle(&bytes, &hold);
    let mut observer = WriteAtHoldEnd::default();
    for time in [0, 10] {
        cycle
            .run_scan(Duration::from_millis(time), None, &mut observer)
            .expect("the scan runs");
    }
    assert_eq!(*hold.stops.borrow(), [(1, (2, 1))]);
    assert_eq!(*hold.read.borrow(), [Value::USInt(6)]);
    assert_eq!(observer.0, 1);
    assert_eq!(cycle.program().value("MAIN.COUNT"), Some(Value::USInt(40)));
    assert_eq!(driven.borrow().outputs, [vec![0], vec![0]]);
}

#[test]
fn a_pause_or_a_step_stops_at_every_statement() {
    // Specification 44 §13.2: while a pause waits or a step runs, every statement stops. At 10 ms
    // statement 0 stops with the count 5 and statement 1 with 6, both at depth 1 on lines 1 and 2.
    // Catches a pause that arms only the breakpoints' statements.
    let bytes = image_of(48, true);
    let hold = FakeHold::new(
        true,
        StopPoints {
            every: true,
            starts: Vec::new(),
        },
    );
    let (mut cycle, _) = held_cycle(&bytes, &hold);
    let mut phases = Phases::default();
    for time in [0, 10] {
        cycle
            .run_scan(Duration::from_millis(time), None, &mut phases)
            .expect("the scan runs");
    }
    assert_eq!(*hold.stops.borrow(), [(1, (1, 1)), (1, (2, 1))]);
    assert_eq!(*hold.read.borrow(), [Value::USInt(5), Value::USInt(6)]);
}

#[test]
fn a_scan_the_platform_does_not_debug_never_stops() {
    // Specification 11 §6.9: with debugging off no statement stops, whatever the debugger holds
    // armed, and the body counts 5 to 6. Catches stops while debugging is disabled.
    let bytes = image_of(48, true);
    let hold = FakeHold::new(
        false,
        StopPoints {
            every: true,
            starts: Vec::new(),
        },
    );
    let (mut cycle, _) = held_cycle(&bytes, &hold);
    let mut phases = Phases::default();
    for time in [0, 10] {
        cycle
            .run_scan(Duration::from_millis(time), None, &mut phases)
            .expect("the scan runs");
    }
    assert!(hold.stops.borrow().is_empty());
    assert_eq!(cycle.program().value("MAIN.COUNT"), Some(Value::USInt(6)));
}

#[test]
fn the_sources_of_a_debug_image_place_a_breakpoint_by_its_statement_table() {
    // Specification 44 §13.1: a breakpoint on line 2 of `main.st` resolves to the statement that
    // starts there, as the interpreter's resolver resolves it on `LINE_TABLE`; the program's view
    // carries the sources once the loop attached its hold. Catches a view without sources and a
    // table built in another order.
    let bytes = image_of(48, true);
    let hold = FakeHold::new(true, StopPoints::default());
    let (cycle, _) = held_cycle(&bytes, &hold);
    let view = cycle.compiled_view().expect("the cycle has a debugger");
    assert!(!view.release);
    let sources = view.sources.expect("a debug image has sources");
    assert_eq!(sources.label(0), Some("main.st"));
    let resolved = sources
        .resolve_breakpoint(0, 2, 1)
        .expect("line 2 has a statement");
    assert_eq!((resolved.line, resolved.column), (2, 1));
    assert_eq!(resolved.code_offset, 1);
}
