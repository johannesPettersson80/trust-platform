//! A compiled program's retained values through the chip's checkpoint log (specification 44 §11;
//! specification 34 §10; specification 11 §6.7), with a stand-in module written against the kernel
//! interface over the RAM checkpoint flash of the engine cycle's retain tests.
//!
//! The image: `MAIN` in the task `Fast` of 10 ms; its body counts the retained global
//! `Batches : USINT := 3`, byte 0 of the retain area, once per activation.

use alloc::boxed::Box;
use alloc::vec;
use alloc::vec::Vec;
use core::cell::Cell;

use trust_plc_abi::kernel::{Entry, WITHOUT_DEBUGGER};
use trust_plc_abi::module::run;
use trust_plc_abi::{Cx, AREA_RETAIN};
use trust_runtime_core::error::RuntimeError;
use trust_runtime_core::native::image::{
    type_kind, AreaRecord, CodeImage, EntryKind, EntryRecord, FileRecord, ImageWriter,
    IoBindRecord, MemoryFigure, RetainRecord, SectionId, StmtRecord, StringPool, SymbolRecord,
    Target, TaskRecord, TypeRecord, AREA_HAS_INIT, AREA_RETAINED, NONE, SYMBOL_PROGRAM,
    SYMBOL_RETAINED,
};
use trust_runtime_core::native::program::{admit, Device, ModuleCode, NativeProgram};
use trust_runtime_core::ports::{CheckpointStore, MonotonicClock, MonotonicInstant};
use trust_runtime_core::retain::checkpoint::CheckpointIdentity;
use trust_runtime_core::retain::RestartMode;
use trust_runtime_core::value::{Duration, Value};
use trust_runtime_core::vm::CycleHooks;
use trust_runtime_core::watchdog::{FaultPolicy, RetainMode, WatchdogPolicy};

use super::CompiledCycle;
use crate::runner::cycle::{ResourceCycle, RetainSource};
use crate::runner::engine::{CycleSettings, ProcessIo};
use crate::runner::retain_contract_tests::{f407, RamFlash};
use crate::runner::{newest_checkpoint, RetainConfig};
use crate::status::IoDriverStatus;

const MS: i64 = 1_000_000;

#[cfg(target_arch = "x86_64")]
extern "sysv64" fn counting(cx: *mut Cx) -> u32 {
    run(cx, |cx, areas| {
        let view: &[Cell<u8>; 8] = areas.view(cx, AREA_RETAIN)?;
        view[0].set(view[0].get().wrapping_add(1));
        Ok(())
    })
}

#[cfg(not(target_arch = "x86_64"))]
extern "C" fn counting(cx: *mut Cx) -> u32 {
    run(cx, |cx, areas| {
        let view: &[Cell<u8>; 8] = areas.view(cx, AREA_RETAIN)?;
        view[0].set(view[0].get().wrapping_add(1));
        Ok(())
    })
}

#[cfg(target_arch = "x86_64")]
extern "sysv64" fn nothing(cx: *mut Cx) -> u32 {
    run(cx, |_, _| Ok(()))
}

#[cfg(not(target_arch = "x86_64"))]
extern "C" fn nothing(cx: *mut Cx) -> u32 {
    run(cx, |_, _| Ok(()))
}

/// The stand-in module: code offset 0 counts `Batches`, 32 does nothing.
struct Module;

impl ModuleCode for Module {
    fn entry(&self, offset: u32) -> Option<Entry> {
        match offset {
            0 => Some(Entry::from_fn(counting)),
            32 => Some(Entry::from_fn(nothing)),
            _ => None,
        }
    }
}

/// The image of `MAIN` and the retained `Batches`.
fn image() -> Vec<u8> {
    let mut strings = StringPool::new();
    let fast = strings.add("Fast");
    let mut writer = ImageWriter::new(Target::this_cpu().unwrap_or(Target::X86_64), 1);
    writer.module(CodeImage {
        code: vec![0; 64],
        rodata: Vec::new(),
        rodata_distance: 64,
    });
    let entry = |kind: EntryKind, code_offset: u32| EntryRecord {
        kind: kind as u32,
        index: 0,
        code_offset,
    };
    writer.table(
        &[
            entry(EntryKind::ProgramBody, 0),
            entry(EntryKind::ColdInit, 32),
            entry(EntryKind::WarmInit, 32),
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
            area(2, 0, 0),
            area(3, 0, 0),
            area(4, 0, 0),
            area(5, 8, AREA_RETAINED | AREA_HAS_INIT),
            area(6, 0, 0),
        ],
        &[],
    );
    writer.section(SectionId::Init, vec![3, 0, 0, 0, 0, 0, 0, 0]);
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
    name_tables(&mut writer, &mut strings);
    writer.section(SectionId::Strings, strings.into_bytes());
    writer.finish().expect("the image builds")
}

/// The image's symbols `Batches` and `MAIN`, its one type, the retain map of `Batches`, its file
/// and its memory figure.
fn name_tables(writer: &mut ImageWriter, strings: &mut StringPool) {
    let symbol = |canonical: u32, display: u32, area: u8, flags: u8, size: u32| SymbolRecord {
        canonical,
        display,
        type_index: 0,
        area,
        flags,
        offset: 0,
        size,
        parent: NONE,
        pou: NONE,
    };
    let batches = symbol(
        strings.add("BATCHES"),
        strings.add("Batches"),
        5,
        SYMBOL_RETAINED,
        1,
    );
    let main = strings.add("MAIN");
    writer.table::<IoBindRecord>(&[], &[]);
    writer.table(&[batches, symbol(main, main, 6, SYMBOL_PROGRAM, 0)], &[]);
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
    writer.table::<StmtRecord>(&[], &[]);
    writer.table(
        &[RetainRecord {
            symbol: 0,
            offset: 0,
            size: 1,
        }],
        &[],
    );
    writer.table(
        &[FileRecord {
            label: strings.add("main.st"),
            source_offset: NONE,
            source_length: 0,
        }],
        &[],
    );
    let figure = MemoryFigure {
        areas: 8,
        stack: 64,
        total: 72,
        ..MemoryFigure::default()
    };
    writer.section(SectionId::MemFig, figure.bytes().to_vec());
}

/// Process I/O without lines.
struct NoLines;

impl ProcessIo for NoLines {
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

/// A clock that stays at its start; no scan of these tests has a deadline.
struct Still;

impl MonotonicClock for Still {
    fn now(&self) -> MonotonicInstant {
        MonotonicInstant::from_micros(0)
    }
}

/// An observer that sees nothing.
struct Quiet;

impl CycleHooks for Quiet {}

/// The program started cold, as a power-on starts it, with the checkpoint store over `flash` and a
/// save every `save_interval`.
fn power_on<'i>(
    bytes: &'i [u8],
    flash: &RamFlash,
    save_interval: Duration,
) -> CompiledCycle<'i, Still, NoLines, Module> {
    let device = Device {
        target: Target::this_cpu().unwrap_or(Target::X86_64),
        program_memory: 1_000,
        task_stack: 4_096,
        max_statements: 1_000,
    };
    let program = NativeProgram::new(
        admit(bytes, &device).expect("the image is admitted"),
        Module,
        &device,
        &WITHOUT_DEBUGGER,
    )
    .expect("the program is built");
    let settings = CycleSettings {
        cycle_interval: Duration::from_millis(10),
        watchdog: WatchdogPolicy::default(),
        fault_policy: FaultPolicy::Halt,
        safe_outputs: Vec::new(),
    };
    CompiledCycle::start(program, NoLines, settings, Still)
        .expect("the program starts")
        .with_retain(RetainConfig {
            mode: RetainMode::File,
            store: CheckpointStore {
                flash: Box::new(flash.clone()),
                program_budget: 64,
            },
            identity: CheckpointIdentity {
                application: [0x11; 16],
                schema: [0x22; 16],
            },
            save_interval,
            image_bytes: 256,
        })
        .expect("the checkpoint log opens")
}

/// The scans at `times` milliseconds, each followed by the persister steps a loop makes between
/// two scans.
fn scans(cycle: &mut CompiledCycle<'_, Still, NoLines, Module>, times: &[i64]) {
    for &time in times {
        cycle
            .run_scan(Duration::from_nanos(time * MS), None, &mut Quiet)
            .expect("the scan runs");
        for _ in 0..64 {
            cycle.step_retain();
        }
    }
}

/// The value of `Batches`.
fn batches(cycle: &CompiledCycle<'_, Still, NoLines, Module>) -> Option<Value> {
    cycle.program().value("BATCHES")
}

/// A stop saves the retained values the program holds, and the next power-on restores them by name
/// through the retain map with a warm start, reporting nothing; a cold restart gives the starting
/// value again (IEC 61131-3 §6.5.6; specification 11 §6.7). The save interval is an hour, so only
/// the stop saves. Catches a stop that saves nothing, which the M1 cycle did, and a warm start that
/// keeps the cold start's 3.
#[test]
fn a_stop_saves_and_the_next_power_on_restores() {
    let bytes = image();
    let flash = RamFlash::new(f407());
    let mut cycle = power_on(&bytes, &flash, Duration::from_secs(3600));
    // Activations at 10, 20 and 30 ms count 3 to 6.
    scans(&mut cycle, &[0, 10, 20, 30]);
    assert_eq!(batches(&cycle), Some(Value::USInt(6)));
    cycle.save_retain().expect("the stop saves");
    drop(cycle);

    let mut next = power_on(&bytes, &flash, Duration::from_secs(3600));
    assert_eq!(batches(&next), Some(Value::USInt(3)));
    let events = next
        .restart(RestartMode::Warm, RetainSource::Checkpoint)
        .expect("the warm start restores");
    assert_eq!(events, []);
    assert_eq!(batches(&next), Some(Value::USInt(6)));
    next.restart(RestartMode::Cold, RetainSource::Checkpoint)
        .expect("the cold restart");
    assert_eq!(batches(&next), Some(Value::USInt(3)));
}

/// A due save captures the values at the output sample and writes the checkpoint between scans,
/// under the key `Batches`, so a power cut after it loses nothing that was saved (specification 34
/// §10). Every scan is due with an interval of zero. Catches a capture that is never offered, and
/// a checkpoint under the canonical name `BATCHES` instead of the declared one.
#[test]
fn a_due_save_survives_a_power_cut() {
    let bytes = image();
    let flash = RamFlash::new(f407());
    let mut cycle = power_on(&bytes, &flash, Duration::ZERO);
    scans(&mut cycle, &[0, 10, 20]);
    assert_eq!(batches(&cycle), Some(Value::USInt(5)));
    // The power is cut: nothing is saved at a stop.
    drop(cycle);

    let snapshot = newest_checkpoint(&mut flash.clone())
        .expect("the log reads")
        .expect("a checkpoint was written");
    assert_eq!(snapshot.values().len(), 1);
    assert_eq!(snapshot.values().get("Batches"), Some(&Value::USInt(5)));
    let mut next = power_on(&bytes, &flash, Duration::ZERO);
    next.restart(RestartMode::Warm, RetainSource::Checkpoint)
        .expect("the warm start restores");
    assert_eq!(batches(&next), Some(Value::USInt(5)));
}
