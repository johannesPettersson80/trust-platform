//! The interface's layouts and the module side's hook, budget and fault records (specification 44
//! §7.2, §7.3, §8.2 to §8.6, §13.2 and Appendix A).

use core::cell::Cell;
use core::mem::{align_of, size_of};
use core::sync::atomic::{AtomicU32, Ordering};

use crate::kernel::WITHOUT_DEBUGGER;
use crate::module::{run, stmt, tick};
use crate::{
    plain_struct, Cx, FaultCode, KernelApi, RefValue, Str, WStr, AREA_GLOBALS, AREA_INSTANCES,
};

/// A reference is 16 bytes on every target (§7.3): a layout of 24 or 12 would put the next
/// variable at another offset than the symbol table names.
#[test]
fn a_reference_is_sixteen_bytes() {
    assert_eq!(size_of::<RefValue>(), 16);
    assert_eq!(align_of::<RefValue>(), 4);
    assert_eq!(size_of::<Cell<RefValue>>(), 16);
}

/// `STRING[n]` is a 2-byte length and `n` bytes, rounded up to an even size (§6.2, §7.2): 18 for
/// `STRING[16]`, 8 for `STRING[5]` (2 + 5 = 7, rounded up), 82 for `STRING[80]`; `WSTRING[3]` is
/// 2 + 6 = 8. A length of 4 bytes, or no rounding, would break every array of strings.
#[test]
fn strings_lie_as_length_and_units() {
    assert_eq!(size_of::<Str<16>>(), 18);
    assert_eq!(size_of::<Str<5>>(), 8);
    assert_eq!(size_of::<Str<80>>(), 82);
    assert_eq!(align_of::<Str<80>>(), 2);
    assert_eq!(size_of::<WStr<3>>(), 8);
    assert_eq!(size_of::<[Str<5>; 3]>(), 24);
}

/// The fault codes are numbered from 1 in the order of Appendix A, and a number reads back as its
/// code: a kernel that numbered them differently would report a division by zero as an overflow.
#[test]
fn fault_codes_are_numbered_from_one_in_appendix_order() {
    for (index, code) in FaultCode::ALL.iter().enumerate() {
        assert_eq!(*code as u32, u32::try_from(index).unwrap() + 1);
        assert_eq!(FaultCode::from_raw(*code as u32), Some(*code));
    }
    assert_eq!(FaultCode::DivisionByZero as u32, 1);
    assert_eq!(FaultCode::InfiniteLoop as u32, 13);
    assert_eq!(FaultCode::ModuleInternal as u32, 16);
    assert_eq!(FaultCode::from_raw(0), None);
    assert_eq!(FaultCode::from_raw(17), None);
}

plain_struct! {
    struct Globals {
        counter: Cell<i32>,
        level: Cell<f32>,
        name: Str<6>,
    }
}

/// An area of 8-byte words, as a kernel aligns its areas.
fn area_words<const N: usize>() -> [u64; N] {
    [0; N]
}

fn context() -> Cx {
    Cx::new(&WITHOUT_DEBUGGER)
}

/// An entry reads and writes its area through the typed view at the declared offsets, and returns 0
/// (§8.3): the counter at offset 0, the level at offset 4, the name's length at offset 8.
#[test]
fn an_entry_writes_its_area_through_the_view() {
    let mut words = area_words::<4>();
    let mut cx = context();
    cx.set_area(AREA_GLOBALS, words.as_mut_ptr().cast(), 32);
    cx.budget = 10;
    let result = run(&raw mut cx, |cx, areas| {
        let globals: &Globals = areas.view(cx, AREA_GLOBALS)?;
        stmt(cx, 1)?;
        globals.counter.set(globals.counter.get() + 41);
        globals.level.set(2.5);
        globals.name.len.set(3);
        Ok(())
    });
    assert_eq!(result, 0);
    let bytes: [u8; 32] = unsafe_free_bytes(&words);
    assert_eq!(i32::from_ne_bytes(bytes[0..4].try_into().unwrap()), 41);
    assert_eq!(f32::from_ne_bytes(bytes[4..8].try_into().unwrap()), 2.5);
    assert_eq!(u16::from_ne_bytes(bytes[8..10].try_into().unwrap()), 3);
    assert_eq!(cx.stmt, 1);
    assert_eq!(cx.budget, 9);
}

fn unsafe_free_bytes(words: &[u64; 4]) -> [u8; 32] {
    let mut bytes = [0u8; 32];
    for (index, word) in words.iter().enumerate() {
        bytes[index * 8..index * 8 + 8].copy_from_slice(&word.to_ne_bytes());
    }
    bytes
}

/// A view larger than its area is refused with `ModuleInternal` and the area's numbers, never read
/// out of bounds: the globals need 16 bytes (4 + 4 + 2 + 6), the area has 12.
#[test]
fn a_view_larger_than_its_area_is_a_fault() {
    let mut words = area_words::<4>();
    let mut cx = context();
    cx.set_area(AREA_GLOBALS, words.as_mut_ptr().cast(), 12);
    let result = run(&raw mut cx, |cx, areas| {
        let _: &Globals = areas.view(cx, AREA_GLOBALS)?;
        Ok(())
    });
    assert_eq!(result, FaultCode::ModuleInternal as u32);
    assert_eq!(cx.fault.code, FaultCode::ModuleInternal as u32);
    assert_eq!(cx.fault.detail, [i64::from(AREA_GLOBALS), 16, 12]);
}

/// The budget counts every statement and every loop iteration, and an activation that uses it up
/// ends with `InfiniteLoop` at the statement that found it empty (§8.4): with a budget of 5, a loop
/// whose body is one statement runs two whole iterations (4 counts) and the third iteration's tick
/// takes the last; its statement then finds the budget empty.
#[test]
fn the_budget_ends_an_endless_loop() {
    let mut cx = context();
    cx.budget = 5;
    let iterations = Cell::new(0u32);
    let result = run(&raw mut cx, |cx, _| loop {
        tick(cx)?;
        stmt(cx, 7)?;
        iterations.set(iterations.get() + 1);
    });
    assert_eq!(result, FaultCode::InfiniteLoop as u32);
    assert_eq!(iterations.get(), 2);
    assert_eq!(cx.fault.code, FaultCode::InfiniteLoop as u32);
    assert_eq!(cx.fault.stmt, 7);
    assert_eq!(cx.budget, 0);
}

static STOPS: AtomicU32 = AtomicU32::new(0);

#[cfg(target_arch = "x86_64")]
extern "sysv64" fn counting_stop(_cx: *mut Cx, stmt: u32) -> u32 {
    STOPS.fetch_add(stmt, Ordering::Relaxed);
    0
}
#[cfg(not(target_arch = "x86_64"))]
extern "C" fn counting_stop(_cx: *mut Cx, stmt: u32) -> u32 {
    STOPS.fetch_add(stmt, Ordering::Relaxed);
    0
}

#[cfg(target_arch = "x86_64")]
extern "sysv64" fn aborting_stop(_cx: *mut Cx, _stmt: u32) -> u32 {
    FaultCode::ModuleInternal as u32
}
#[cfg(not(target_arch = "x86_64"))]
extern "C" fn aborting_stop(_cx: *mut Cx, _stmt: u32) -> u32 {
    FaultCode::ModuleInternal as u32
}

static COUNTING: KernelApi = KernelApi {
    stop: counting_stop,
    debug_write: WITHOUT_DEBUGGER.debug_write,
};

static ABORTING: KernelApi = KernelApi {
    stop: aborting_stop,
    debug_write: WITHOUT_DEBUGGER.debug_write,
};

/// While armed, a statement whose bit is set calls the kernel's stop with its id, and the others do
/// not (§13.2): bits for statements 3 and 9 set (byte 0 bit 3, byte 1 bit 1), statements 3, 4 and 9
/// run, so the stop sees 3 + 9 = 12. Unarmed, no statement stops.
#[test]
fn a_set_breakpoint_bit_calls_the_kernel() {
    let bits = [0b0000_1000u8, 0b0000_0010];
    let mut cx = Cx::new(&COUNTING);
    cx.budget = 100;
    cx.breakpoints = bits.as_ptr();
    cx.breakpoint_bytes = 2;
    cx.armed = 1;
    STOPS.store(0, Ordering::Relaxed);
    let result = run(&raw mut cx, |cx, _| {
        stmt(cx, 3)?;
        stmt(cx, 4)?;
        stmt(cx, 9)?;
        Ok(())
    });
    assert_eq!(result, 0);
    assert_eq!(STOPS.load(Ordering::Relaxed), 12);
    cx.armed = 0;
    let result = run(&raw mut cx, |cx, _| stmt(cx, 3));
    assert_eq!(result, 0);
    assert_eq!(STOPS.load(Ordering::Relaxed), 12);
}

/// A stop that returns a fault code ends the scan with it.
#[test]
fn a_stop_can_end_the_scan() {
    let bits = [0b0000_0001u8];
    let mut cx = Cx::new(&ABORTING);
    cx.budget = 100;
    cx.breakpoints = bits.as_ptr();
    cx.breakpoint_bytes = 1;
    cx.armed = 1;
    let reached = Cell::new(false);
    let result = run(&raw mut cx, |cx, _| {
        stmt(cx, 0)?;
        reached.set(true);
        Ok(())
    });
    assert_eq!(result, FaultCode::ModuleInternal as u32);
    assert!(!reached.get());
}

/// A kernel of another interface version is refused before the body runs.
#[test]
fn another_interface_version_is_refused() {
    let mut cx = context();
    cx.version = 2;
    let ran = Cell::new(false);
    let result = run(&raw mut cx, |_, _| {
        ran.set(true);
        Ok(())
    });
    assert_eq!(result, FaultCode::ModuleInternal as u32);
    assert!(!ran.get());
    assert_eq!(cx.fault.detail[..2], [2, 1]);
}

/// The other areas are untouched: an area id outside 1 to 7 is refused.
#[test]
fn an_unknown_area_id_is_a_fault() {
    let mut cx = context();
    let result = run(&raw mut cx, |cx, areas| {
        let _: &Cell<u8> = areas.view(cx, 8)?;
        Ok(())
    });
    assert_eq!(result, FaultCode::ModuleInternal as u32);
    let result = run(&raw mut cx, |cx, areas| {
        let _: &[Cell<u8>; 0] = areas.view(cx, AREA_INSTANCES)?;
        Ok(())
    });
    assert_eq!(result, 0);
}
