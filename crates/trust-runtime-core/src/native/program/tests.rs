//! The kernel's side of a compiled program (specification 44 §8, §10): admission, the starting
//! memory, the scan's task schedule, the process image bindings and the faults, with stand-in
//! modules written against the interface.

use alloc::vec;
use alloc::vec::Vec;
use core::cell::Cell;

use trust_plc_abi::kernel::{Entry, WITHOUT_DEBUGGER};
use trust_plc_abi::module::{fault, run};
use trust_plc_abi::{Cx, FaultCode, AREA_INSTANCES};

use super::*;
use crate::native::image::{
    type_kind, CodeImage, FileRecord, ImageWriter, MemoryFigure, RetainRecord, SectionId,
    StmtRecord, StringPool, TypeRecord, AREA_HAS_INIT, SYMBOL_PROGRAM,
};

const MS: i64 = 1_000_000;

/// The program `MAIN`: byte 0 counts its scans, byte 1 is the count's lowest bit.
fn count(cx: &mut Cx, view: &[Cell<u8>; 8]) {
    view[0].set(view[0].get().wrapping_add(1));
    view[1].set(view[0].get() & 1);
    let _ = cx;
}

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
    count(cx, view);
    Ok(())
});

entry!(dividing_by_zero, |cx, _| Err(fault(
    cx,
    FaultCode::DivisionByZero,
    7,
    0,
    0
)));

entry!(nothing, |_, _| Ok(()));

/// The module of the tests: code offset 0 counts, 16 divides by zero, 32 does nothing.
struct StandIn;

impl ModuleCode for StandIn {
    fn entry(&self, offset: u32) -> Option<Entry> {
        match offset {
            0 => Some(Entry::from_fn(counting)),
            16 => Some(Entry::from_fn(dividing_by_zero)),
            32 => Some(Entry::from_fn(nothing)),
            _ => None,
        }
    }
}

/// An image of `MAIN` in a 10 ms task, its byte 1 bound to `%QX0.1`, whose body is at
/// `body`, with memory figure `total` and stack bound `stack`.
fn image(body: u32, total: u64, stack: u64, target: Target) -> Vec<u8> {
    let mut strings = StringPool::new();
    let main = strings.add("MAIN");
    let count_name = strings.add("MAIN.COUNT");
    let out = strings.add("MAIN.OUT");
    let fast = strings.add("Fast");
    let label = strings.add("main.st");
    let mut writer = ImageWriter::new(target, 1);
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
    writer.table(
        &[IoBindRecord {
            symbol: 2,
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
    writer.table(
        &[
            symbol(main, 0, 8, SYMBOL_PROGRAM, NONE),
            symbol(count_name, 0, 1, 0, 0),
            symbol(out, 1, 1, 0, 0),
        ],
        &[],
    );
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
            total,
            stack,
            ..MemoryFigure::default()
        }
        .bytes()
        .to_vec(),
    );
    writer.section(SectionId::Strings, strings.into_bytes());
    writer.finish().unwrap()
}

fn device() -> Device {
    Device {
        target: Target::this_cpu().unwrap_or(Target::X86_64),
        program_memory: 1000,
        task_stack: 4096,
        max_statements: 1000,
    }
}

/// A device refuses an image for another CPU, of another interface, needing more memory or more
/// stack than it has, each with both numbers (specification 44 §10.2).
#[test]
fn a_device_refuses_what_it_cannot_run() {
    let device = device();
    let other = if device.target == Target::Rv32imac {
        Target::X86_64
    } else {
        Target::Rv32imac
    };
    assert!(matches!(
        admit(&image(0, 100, 50, other), &device),
        Err(Refusal::Target { .. })
    ));
    assert_eq!(
        admit(&image(0, 5000, 50, device.target), &device).err(),
        Some(Refusal::Memory {
            needs: 5000,
            has: 1000
        })
    );
    assert_eq!(
        admit(&image(0, 100, 8192, device.target), &device).err(),
        Some(Refusal::Stack {
            needs: 8192,
            has: 4096
        })
    );
    let mut interface = image(0, 100, 50, device.target);
    interface[18] = 2;
    let end = HEADER + 16 * u32::from_le_bytes(interface[112..116].try_into().unwrap()) as usize;
    let crc = crate::crc32::checksum(&interface[12..end]);
    interface[8..12].copy_from_slice(&crc.to_le_bytes());
    assert_eq!(
        admit(&interface, &device).err(),
        Some(Refusal::Interface { image: 2 })
    );
    assert!(admit(&image(0, 100, 50, device.target), &device).is_ok());
}

const HEADER: usize = crate::native::image::HEADER_BYTES;

/// A cold start copies the starting memory (the count starts at 5); a scan runs a due task's
/// program and publishes its bound output: at 10 ms the count is 6 and bit 1 of `%QB0` is 0, at
/// 20 ms 7 and 1, at 25 ms the task is not due and nothing changes.
#[test]
fn a_scan_runs_the_due_tasks_and_publishes_the_bound_outputs() {
    let device = device();
    let bytes = image(0, 100, 50, device.target);
    let image = admit(&bytes, &device).unwrap();
    let mut program = NativeProgram::new(image, StandIn, &device, &WITHOUT_DEBUGGER).unwrap();
    program.cold_start(Duration::ZERO).unwrap();
    let mut byte = [0u8; 2];
    program.read(AREA_INSTANCES, 0, &mut byte);
    assert_eq!(byte, [5, 0]);
    let scan = |program: &mut NativeProgram<'_, StandIn>, ms: i64| {
        program
            .scan(Duration::from_nanos(ms * MS), &mut NoHooks)
            .unwrap();
        let mut bytes = [0u8; 2];
        program.read(AREA_INSTANCES, 0, &mut bytes);
        (bytes[0], program.outputs[0])
    };
    assert_eq!(scan(&mut program, 10), (6, 0));
    assert_eq!(scan(&mut program, 20), (7, 0b10));
    assert_eq!(scan(&mut program, 25), (7, 0b10));
    assert_eq!(scan(&mut program, 30), (8, 0));
}

/// A fault ends the scan with its runtime error and publishes nothing.
#[test]
fn a_module_fault_ends_the_scan() {
    let device = device();
    let bytes = image(16, 100, 50, device.target);
    let image = admit(&bytes, &device).unwrap();
    let mut program = NativeProgram::new(image, StandIn, &device, &WITHOUT_DEBUGGER).unwrap();
    program.cold_start(Duration::ZERO).unwrap();
    program.outputs[0] = 0xAA;
    let result = program.scan(Duration::from_nanos(10 * MS), &mut NoHooks);
    assert_eq!(result, Err(RuntimeError::DivisionByZero));
    assert_eq!(program.outputs[0], 0xAA);
    assert_eq!(program.last_fault().detail[0], 7);
}

/// Each fault code maps to its runtime error with its numbers.
#[test]
fn fault_codes_map_to_runtime_errors() {
    let record = FaultRecord {
        code: FaultCode::IndexOutOfBounds as u32,
        stmt: 3,
        detail: [13, 1, 12],
    };
    assert_eq!(
        fault_error(&record, FaultCode::IndexOutOfBounds as u32, 10),
        RuntimeError::IndexOutOfBounds {
            index: 13,
            lower: 1,
            upper: 12
        }
    );
    assert!(matches!(
        fault_error(&record, FaultCode::InfiniteLoop as u32, 10),
        RuntimeError::ExecutionLimit { limit: 10, .. }
    ));
}
