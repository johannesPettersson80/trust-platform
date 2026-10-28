//! The kernel's call of a tool entry between scans (specification 44 §13.3), with stand-in modules
//! written against the interface: refused while a scan runs, undone when it uses its allowance up,
//! and the program's fault when the method faults.

use core::cell::Cell;

use trust_plc_abi::kernel::{Entry, WITHOUT_DEBUGGER};
use trust_plc_abi::module::{fault, run, tick};
use trust_plc_abi::{Cx, FaultCode, AREA_INSTANCES};

use super::*;
use crate::native::image::{EntryKind, EntryRecord, Target};
use crate::native::program::tests::{output_bit, writer_with};
use crate::native::program::{admit, Device, NoHooks};

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

// The program body: one more in byte 0 of the instances, `MAIN.COUNT`.
entry!(counting, |cx, areas| {
    let view: &[Cell<u8>; 8] = areas.view(cx, AREA_INSTANCES)?;
    view[0].set(view[0].get().wrapping_add(1));
    Ok(())
});

entry!(nothing, |_, _| Ok(()));

// A method with a loop of three passes, each taking one from the allowance before it counts.
entry!(counting_three_passes, |cx, areas| {
    let view: &[Cell<u8>; 8] = areas.view(cx, AREA_INSTANCES)?;
    for _ in 0..3 {
        tick(cx)?;
        view[0].set(view[0].get().wrapping_add(1));
    }
    Ok(())
});

// A method that counts once and then divides by zero.
entry!(counting_then_dividing, |cx, areas| {
    let view: &[Cell<u8>; 8] = areas.view(cx, AREA_INSTANCES)?;
    view[0].set(view[0].get().wrapping_add(1));
    Err(fault(cx, FaultCode::DivisionByZero, 7, 0, 0))
});

/// The module of the tests: code offset 0 is the body, 32 the cold-start initialization, 16 and 48
/// the two methods.
struct StandIn;

impl ModuleCode for StandIn {
    fn entry(&self, offset: u32) -> Option<Entry> {
        match offset {
            0 => Some(Entry::from_fn(counting)),
            16 => Some(Entry::from_fn(counting_three_passes)),
            32 => Some(Entry::from_fn(nothing)),
            48 => Some(Entry::from_fn(counting_then_dividing)),
            _ => None,
        }
    }
}

/// Entry 2 counts three passes, entry 3 counts once and divides by zero.
const THREE_PASSES: usize = 2;
const DIVIDING: usize = 3;

fn device() -> Device {
    Device {
        target: Target::this_cpu().unwrap_or(Target::X86_64),
        program_memory: 1000,
        task_stack: 4096,
        max_statements: 1000,
        trusted_keys: &[],
    }
}

/// The image of the program tests' `MAIN` in a 10 ms task, `MAIN.COUNT` starting at 5, with the
/// two tool entries after its body and its cold-start initialization.
fn image_bytes() -> Vec<u8> {
    let mut writer = writer_with(0, 100, 50, device().target, output_bit(), 0, 0);
    let entry = |kind: EntryKind, code_offset| EntryRecord {
        kind: kind as u32,
        index: 0,
        code_offset,
    };
    writer.table(
        &[
            entry(EntryKind::ProgramBody, 0),
            entry(EntryKind::ColdInit, 32),
            entry(EntryKind::Method, 16),
            entry(EntryKind::Method, 48),
        ],
        &[],
    );
    writer.finish().unwrap()
}

fn count<C: ModuleCode>(program: &NativeProgram<'_, C>) -> u8 {
    let mut byte = [0u8];
    assert!(program.read(AREA_INSTANCES, 0, &mut byte));
    byte[0]
}

/// Specification 44 §13.3: a tool calls a method between scans, never while a scan runs, whose
/// memory and context are in use; refused, the call looks nothing up, writes nothing and runs
/// nothing. A scan that has ended leaves the program to the calls between scans. Catches a call
/// that enters the module or writes its block while a scan uses them, and a scan that leaves the
/// program refusing calls once it ended.
#[test]
fn a_call_while_a_scan_runs_is_refused_and_changes_nothing() {
    let device = device();
    let bytes = image_bytes();
    let image = admit(&bytes, &device).unwrap();
    let mut program = NativeProgram::new(image, StandIn, &device, &WITHOUT_DEBUGGER).unwrap();
    program.cold_start(Duration::ZERO).unwrap();
    assert_eq!(count(&program), 5);
    // As the program is while its scan runs: the entry's call is refused, and so is a whole
    // method call, before it looks its path up (which names nothing here) or writes its block.
    program.scanning = true;
    let during_a_scan = Err(MethodCallError::Refused(RuntimeError::ControlError(
        DURING_A_SCAN.into(),
    )));
    assert_eq!(
        program.call_tool_entry(THREE_PASSES, 10, Duration::ZERO),
        during_a_scan.clone()
    );
    assert_eq!(
        program
            .call_method("NOWHERE", "IAny", "Run", &[], 10, Duration::ZERO)
            .map(|(_, used)| used),
        during_a_scan
    );
    assert_eq!(count(&program), 5);
    program.scanning = false;
    // A scan counts once (6); the scan has ended, and the call's three passes count to 9.
    program
        .scan(Duration::from_nanos(10 * MS), &mut NoHooks)
        .unwrap();
    assert!(!program.scanning);
    assert_eq!(
        program.call_tool_entry(THREE_PASSES, 10, Duration::ZERO),
        Ok(3)
    );
    assert_eq!(count(&program), 9);
}

/// Specification 44 §13.3: a call that uses its allowance up is refused and undone, the program's
/// memory as it was before the call; the same call with the allowance it needs runs and returns
/// the passes it used. With an allowance of 2 the method counts twice (5 to 7) and the third pass
/// finds none left: the count is 5 again. With 3 it counts to 8 and used 3. Catches the stores of
/// a call cut short that stay, a call cut short that faults the program, and a wrong count of the
/// passes used.
#[test]
fn a_call_that_uses_its_allowance_up_is_undone() {
    let device = device();
    let bytes = image_bytes();
    let image = admit(&bytes, &device).unwrap();
    let mut program = NativeProgram::new(image, StandIn, &device, &WITHOUT_DEBUGGER).unwrap();
    program.cold_start(Duration::ZERO).unwrap();
    let refused = program.call_tool_entry(THREE_PASSES, 2, Duration::ZERO);
    assert!(
        matches!(
            refused,
            Err(MethodCallError::Refused(RuntimeError::ExecutionLimit {
                used: 2,
                limit: 2,
                ..
            }))
        ),
        "{refused:?}"
    );
    assert_eq!(count(&program), 5);
    assert_eq!(
        program.call_tool_entry(THREE_PASSES, 3, Duration::ZERO),
        Ok(3)
    );
    assert_eq!(count(&program), 8);
}

/// Specification 44 §13.3, §8.6: a fault of the method is the program's fault, and the stores of
/// what ran before it stay, as at a faulted scan: the method counts 5 to 6 and divides by zero.
/// An allowance of 0 runs nothing, and an entry that is no tool entry, the program's body, is not
/// called. Catches a fault reported as a refusal, which would leave the resource running on a
/// state the fault left, a fault that is undone, and a body called as a method.
#[test]
fn a_fault_of_the_method_is_the_programs_and_only_a_tool_entry_is_called() {
    let device = device();
    let bytes = image_bytes();
    let image = admit(&bytes, &device).unwrap();
    let mut program = NativeProgram::new(image, StandIn, &device, &WITHOUT_DEBUGGER).unwrap();
    program.cold_start(Duration::ZERO).unwrap();
    assert!(matches!(
        program.call_tool_entry(THREE_PASSES, 0, Duration::ZERO),
        Err(MethodCallError::Refused(RuntimeError::ExecutionLimit {
            limit: 0,
            ..
        }))
    ));
    assert_eq!(
        program.call_tool_entry(0, 10, Duration::ZERO),
        Err(MethodCallError::Refused(
            RuntimeError::InvalidExecutionState
        ))
    );
    assert_eq!(count(&program), 5);
    assert_eq!(
        program.call_tool_entry(DIVIDING, 10, Duration::ZERO),
        Err(MethodCallError::Fault(RuntimeError::DivisionByZero))
    );
    assert_eq!(count(&program), 6);
    assert_eq!(program.last_fault().detail[0], 7);
}
