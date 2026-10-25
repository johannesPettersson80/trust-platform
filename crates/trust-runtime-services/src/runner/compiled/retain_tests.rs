//! A compiled program's retained values through the chip's checkpoint log (specification 44 §11;
//! specification 34 §10; specification 11 §6.7), with a stand-in module written against the kernel
//! interface over a RAM checkpoint flash.
//!
//! The image: `MAIN` in the task `Fast` of 10 ms; its body counts the retained global
//! `Batches : USINT := 3`, byte 0 of the retain area, once per activation.

use alloc::boxed::Box;
use alloc::rc::Rc;
use alloc::vec;
use alloc::vec::Vec;
use core::cell::{Cell, RefCell};

use trust_plc_abi::kernel::{Entry, WITHOUT_DEBUGGER};
use trust_plc_abi::module::run;
use trust_plc_abi::{Cx, AREA_RETAIN};
use trust_runtime_core::cycle::CycleHooks;
use trust_runtime_core::error::RuntimeError;
use trust_runtime_core::native::image::{
    type_kind, AreaRecord, CodeImage, EntryKind, EntryRecord, FileRecord, ImageWriter,
    IoBindRecord, MemoryFigure, RetainRecord, SectionId, StmtRecord, StringPool, SymbolRecord,
    Target, TaskRecord, TypeRecord, AREA_HAS_INIT, AREA_RETAINED, NONE, SYMBOL_PROGRAM,
    SYMBOL_RETAINED,
};
use trust_runtime_core::native::program::{admit, Device, ModuleCode, NativeProgram};
use trust_runtime_core::ports::{CheckpointStore, MonotonicClock, MonotonicInstant};
use trust_runtime_core::retain::checkpoint::{
    CheckpointFlash, CheckpointIdentity, FlashError, FlashGeometry, Slot,
};
use trust_runtime_core::retain::RestartMode;
use trust_runtime_core::value::{Duration, Value};
use trust_runtime_core::watchdog::{FaultPolicy, RetainMode, WatchdogPolicy};

use super::CompiledCycle;
use crate::runner::cycle::{CycleSettings, ResourceCycle, RetainSource};
use crate::runner::process_io::ProcessIo;
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
    let mut writer = ImageWriter::new(
        Target::this_cpu().unwrap_or(Target::X86_64),
        u16::try_from(trust_plc_abi::INTERFACE_VERSION).unwrap(),
    );
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
            key: strings.add("Batches"),
            area: 5,
            offset: 0,
            type_index: 0,
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

/// The checkpoint slots of these tests: one 16 KiB erase unit each, programmed in 4-byte units.
fn slots() -> FlashGeometry {
    FlashGeometry::new(4, 16_384, 16_384).expect("the checkpoint geometry")
}

/// A RAM checkpoint flash: erasing fills a unit with 0xFF and programming ANDs whole units, as the
/// real flash does. A clone shares the bytes, as the flash a power-on finds again.
#[derive(Clone)]
struct RamFlash {
    geometry: FlashGeometry,
    /// The two slots' bytes; a blank flash reads 0xFF.
    bytes: Rc<RefCell<Vec<u8>>>,
}

impl RamFlash {
    /// A blank flash of `geometry`.
    fn new(geometry: FlashGeometry) -> Self {
        Self {
            geometry,
            bytes: Rc::new(RefCell::new(vec![0xFF; 2 * geometry.slot_len() as usize])),
        }
    }

    /// Where `slot` starts in the two slots' bytes.
    fn slot_base(&self, slot: Slot) -> usize {
        match slot {
            Slot::A => 0,
            Slot::B => self.geometry.slot_len() as usize,
        }
    }
}

impl CheckpointFlash for RamFlash {
    fn geometry(&self) -> FlashGeometry {
        self.geometry
    }

    fn read(&mut self, slot: Slot, offset: u32, bytes: &mut [u8]) -> Result<(), FlashError> {
        let base = self.slot_base(slot) + offset as usize;
        bytes.copy_from_slice(&self.bytes.borrow()[base..base + bytes.len()]);
        Ok(())
    }

    fn program(&mut self, slot: Slot, offset: u32, bytes: &[u8]) -> Result<(), FlashError> {
        let base = self.slot_base(slot) + offset as usize;
        let mut flash = self.bytes.borrow_mut();
        for (index, byte) in bytes.iter().enumerate() {
            flash[base + index] &= *byte;
        }
        Ok(())
    }

    fn erase(&mut self, slot: Slot, offset: u32) -> Result<(), FlashError> {
        let base = self.slot_base(slot) + offset as usize;
        let end = base + self.geometry.erase_unit() as usize;
        self.bytes.borrow_mut()[base..end].fill(0xFF);
        Ok(())
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
        trusted_keys: &[],
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
        hang_action: trust_runtime_core::deployment::HangAction::Fault,
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
    let flash = RamFlash::new(slots());
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
    let flash = RamFlash::new(slots());
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
