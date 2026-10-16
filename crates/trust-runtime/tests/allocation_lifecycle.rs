//! Spec 34 MEM-04, MEM-09, HOST-02, HOST-04, HOST-09 (section 4.3.1), UPDATE-01 and UPDATE-05:
//! counting-allocator evidence for the hosted `Runtime` across construction, scans, restarts,
//! online change, drop and the rare error paths.
//!
//! The counters see only the thread that enabled measurement, and each test holds `SERIAL` while
//! it measures, so other test threads cannot pollute them. Values are net live bytes and allocator
//! operations relative to `reset`. Hosted services may allocate on the control thread, so the scan
//! tests assert that live bytes do not grow and report allocations per cycle without asserting zero.
#![cfg(feature = "full")]

use std::alloc::{GlobalAlloc, Layout, System};
use std::cell::Cell;
use std::sync::atomic::{AtomicIsize, AtomicUsize, Ordering};
use std::sync::{Mutex, MutexGuard, PoisonError};

use trust_runtime::bytecode::BytecodeVersion;
use trust_runtime::error::RuntimeError;
use trust_runtime::harness::CompileSession;
use trust_runtime::value::{Duration, Value};
use trust_runtime::{RestartMode, Runtime};
use trust_runtime_core::vm::PreparationLimits;

/// Generous ceiling on the peak live bytes of one admission (HOST-02 allows backing storage at preparation).
const CONSTRUCTION_PEAK_BOUND_BYTES: isize = 16 * 1024 * 1024;
/// Generous ceiling on allocator calls during one admission.
const CONSTRUCTION_ALLOCATION_BOUND: usize = 200_000;
/// Generous ceiling on the peak live bytes of the first scan after admission (MEM-04, first invocation).
const FIRST_SCAN_PEAK_BOUND_BYTES: isize = 16 * 1024 * 1024;

static NET_BYTES: AtomicIsize = AtomicIsize::new(0);
static PEAK_NET_BYTES: AtomicIsize = AtomicIsize::new(0);
static ALLOCATIONS: AtomicUsize = AtomicUsize::new(0);
static DEALLOCATIONS: AtomicUsize = AtomicUsize::new(0);
/// The counters are process-wide, so each test holds this lock while it measures.
static SERIAL: Mutex<()> = Mutex::new(());

thread_local! {
    static MEASURING: Cell<bool> = const { Cell::new(false) };
}

struct MeasuredAllocator;

fn measuring() -> bool {
    MEASURING.try_with(Cell::get).unwrap_or(false)
}

fn record_allocation(bytes: usize) {
    let bytes = isize::try_from(bytes).unwrap_or(isize::MAX);
    let before = NET_BYTES.fetch_add(bytes, Ordering::SeqCst);
    let _ = PEAK_NET_BYTES.fetch_max(before.wrapping_add(bytes), Ordering::SeqCst);
    let _ = ALLOCATIONS.fetch_add(1, Ordering::SeqCst);
}

fn record_deallocation(bytes: usize) {
    let bytes = isize::try_from(bytes).unwrap_or(isize::MAX);
    let _ = NET_BYTES.fetch_sub(bytes, Ordering::SeqCst);
    let _ = DEALLOCATIONS.fetch_add(1, Ordering::SeqCst);
}

// SAFETY: every operation forwards the caller's unchanged layout and pointer to `System`. The
// bookkeeping uses only atomics and a const thread-local flag, so it neither allocates nor unwinds.
unsafe impl GlobalAlloc for MeasuredAllocator {
    unsafe fn alloc(&self, layout: Layout) -> *mut u8 {
        // SAFETY: the caller guarantees `layout` is a valid allocation request.
        let pointer = unsafe { System.alloc(layout) };
        if !pointer.is_null() && measuring() {
            record_allocation(layout.size());
        }
        pointer
    }

    unsafe fn dealloc(&self, pointer: *mut u8, layout: Layout) {
        if measuring() {
            record_deallocation(layout.size());
        }
        // SAFETY: the caller passes back the pointer and layout that this allocator returned.
        unsafe { System.dealloc(pointer, layout) }
    }
}

#[global_allocator]
static ALLOCATOR: MeasuredAllocator = MeasuredAllocator;

#[derive(Clone, Copy, Debug)]
struct Sample {
    net_bytes: isize,
    peak_bytes: isize,
    allocations: usize,
    deallocations: usize,
}

fn sample() -> Sample {
    Sample {
        net_bytes: NET_BYTES.load(Ordering::SeqCst),
        peak_bytes: PEAK_NET_BYTES.load(Ordering::SeqCst),
        allocations: ALLOCATIONS.load(Ordering::SeqCst),
        deallocations: DEALLOCATIONS.load(Ordering::SeqCst),
    }
}

/// Clears the counters. Call it while holding `serial()`.
fn reset() {
    NET_BYTES.store(0, Ordering::SeqCst);
    PEAK_NET_BYTES.store(0, Ordering::SeqCst);
    ALLOCATIONS.store(0, Ordering::SeqCst);
    DEALLOCATIONS.store(0, Ordering::SeqCst);
}

/// Runs `operation` with measurement enabled on this thread only. A value released after this
/// call returns is not measured, so release it inside another `measured` call.
fn measured<T>(operation: impl FnOnce() -> T) -> T {
    MEASURING.with(|flag| flag.set(true));
    let value = operation();
    MEASURING.with(|flag| flag.set(false));
    value
}

fn serial() -> MutexGuard<'static, ()> {
    SERIAL.lock().unwrap_or_else(PoisonError::into_inner)
}

/// Scalar arithmetic only.
const SCALAR: &str = r#"
VAR_GLOBAL count : DINT := DINT#0; END_VAR
PROGRAM Main
count := count + DINT#1;
END_PROGRAM
"#;

/// An array and a bounded string, updated on every scan.
const ARRAYS_AND_STRINGS: &str = r#"
VAR_GLOBAL
    values : ARRAY[1..8] OF DINT;
    label : STRING[16] := 'idle';
    checksum : DINT := DINT#0;
END_VAR
PROGRAM Main
VAR
    index : DINT := DINT#0;
END_VAR
FOR index := DINT#1 TO DINT#8 DO
    values[index] := values[index] + index;
END_FOR;
checksum := checksum + values[8];
label := '0123456789ABCDEF';
END_PROGRAM
"#;

/// A standard timer instance, nested function calls, an array and a string in one scan. A zero
/// `divisor` makes every scan fault with DivisionByZero.
const TIMERS_AND_CALLS: &str = r#"
VAR_GLOBAL
    count : DINT := DINT#0;
    total : DINT := DINT#0;
    divisor : DINT := DINT#1;
    quotient : DINT := DINT#0;
    setpoint : INT := INT#1;
    timed_done : BOOL := FALSE;
    values : ARRAY[1..8] OF DINT;
    label : STRING[16] := 'idle';
END_VAR
FUNCTION Twice : DINT
VAR_INPUT x : DINT; END_VAR
Twice := x + x;
END_FUNCTION
FUNCTION Quad : DINT
VAR_INPUT x : DINT; END_VAR
Quad := Twice(x := Twice(x := x));
END_FUNCTION
PROGRAM Main
VAR
    timer : TON;
    index : DINT := DINT#0;
END_VAR
count := count + DINT#1;
FOR index := DINT#1 TO DINT#8 DO
    values[index] := values[index] + Quad(x := index);
END_FOR;
total := total + values[1];
label := 'tick';
timer(IN := TRUE, PT := T#30ms);
timed_done := timer.Q;
quotient := DINT#100 / divisor;
END_PROGRAM
"#;

/// The representative applications, each with a global that changes on every scan.
const PROGRAMS: [(&str, &str, &str); 3] = [
    ("scalar", SCALAR, "count"),
    ("arrays_and_strings", ARRAYS_AND_STRINGS, "checksum"),
    ("timers_and_calls", TIMERS_AND_CALLS, "count"),
];

/// Wraps the application in one resource with a 10 ms periodic task.
fn build(body: &str) -> Vec<u8> {
    let source = format!(
        "{body}\nCONFIGURATION Conf RESOURCE Controller ON PLC TASK Tick (INTERVAL := T#10ms, PRIORITY := 0); PROGRAM Plant WITH Tick : Main; END_RESOURCE END_CONFIGURATION"
    );
    CompileSession::from_source(source)
        .build_bytecode_module_for_version(BytecodeVersion::SOURCE_FREE)
        .expect("accepted ST source")
        .encode()
        .expect("encode STBC")
}

/// Runs `count` scans; each advances the logical clock by 10 ms first.
fn run_cycles(runtime: &mut Runtime, count: usize) -> Result<(), String> {
    for _ in 0..count {
        runtime.advance_time(Duration::from_millis(10));
        runtime.execute_cycle().map_err(|error| error.to_string())?;
    }
    Ok(())
}

/// One restart and replacement round. Every step must succeed and leave a runnable controller.
fn lifecycle_round(runtime: &mut Runtime, replacement: &[u8]) -> Result<(), String> {
    runtime
        .restart(RestartMode::Warm)
        .map_err(|error| error.to_string())?;
    run_cycles(runtime, 1)?;
    runtime
        .restart(RestartMode::Cold)
        .map_err(|error| error.to_string())?;
    run_cycles(runtime, 1)?;
    runtime
        .apply_online_change_bytes(replacement, None)
        .map(|_| ())
        .map_err(|error| error.to_string())?;
    run_cycles(runtime, 1)
}

/// One scan that must fault with DivisionByZero, then the owner's fault clear.
fn faulting_scan_and_clear(runtime: &mut Runtime) -> bool {
    runtime.advance_time(Duration::from_millis(10));
    let division_fault = runtime.execute_cycle().err() == Some(RuntimeError::DivisionByZero);
    let latched = runtime.faulted();
    runtime.clear_fault();
    division_fault && latched
}

/// A checked write of a BOOL into an INT destination, which must be refused.
fn rejected_write(runtime: &mut Runtime) -> bool {
    runtime.write_global("setpoint", Value::Bool(true)).err() == Some(RuntimeError::TypeMismatch)
}

/// A candidate that names a resource absent from its bytecode. It is refused after preparation.
fn rejected_candidate(runtime: &mut Runtime, replacement: &[u8]) -> bool {
    let cycles = runtime.cycle_counter();
    let refused = matches!(
        runtime.apply_online_change_bytes(replacement, Some("Missing")),
        Err(RuntimeError::InvalidBytecodeMetadata(_))
    );
    refused && runtime.cycle_counter() == cycles && !runtime.faulted()
}

/// Spec 34 HOST-02 and MEM-04: admitting each representative application allocates its backing
/// storage, which is recorded and kept under generous constants. A warm-up admission first, so
/// process-wide first-use caches are not charged to the application.
#[test]
fn construction_allocations_are_recorded_and_bounded() {
    let _serial = serial();
    for (name, body, _) in PROGRAMS {
        let bytes = build(body);
        let limits = PreparationLimits::default();
        drop(Runtime::from_bytes(&bytes, limits).expect("warm-up admission"));
        reset();
        let runtime = measured(|| Runtime::from_bytes(&bytes, limits)).expect("admit application");
        let construction = sample();
        drop(runtime);
        eprintln!(
            "allocation-evidence,construction,program={name},allocations={},deallocations={},net_bytes={},peak_bytes={}",
            construction.allocations,
            construction.deallocations,
            construction.net_bytes,
            construction.peak_bytes
        );
        assert!(
            construction.allocations > 0,
            "{name}: the counting allocator must observe the admission"
        );
        assert!(
            construction.net_bytes > 0,
            "{name}: an installed controller retains backing storage"
        );
        assert!(
            construction.peak_bytes <= CONSTRUCTION_PEAK_BOUND_BYTES,
            "{name}: construction peak {} exceeds {}",
            construction.peak_bytes,
            CONSTRUCTION_PEAK_BOUND_BYTES
        );
        assert!(
            construction.allocations <= CONSTRUCTION_ALLOCATION_BOUND,
            "{name}: {} allocator calls exceed {}",
            construction.allocations,
            CONSTRUCTION_ALLOCATION_BOUND
        );
    }
}

/// Spec 34 MEM-04 (first invocation) and HOST-04: the first scan of a freshly admitted controller,
/// with no warm-up of its own, is measured and reported, and its peak stays under a generous
/// ceiling. A separate warm-up controller first initializes process-wide caches.
#[test]
fn first_scan_after_admission_is_measured_and_bounded() {
    let _serial = serial();
    for (name, body, witness) in PROGRAMS {
        let bytes = build(body);
        let limits = PreparationLimits::default();
        let mut warm = Runtime::from_bytes(&bytes, limits).expect("warm-up admission");
        run_cycles(&mut warm, 1).expect("warm-up scan");
        drop(warm);
        let mut runtime = Runtime::from_bytes(&bytes, limits).expect("admit application");
        let witness_before = runtime.storage().get_global(witness).cloned();
        reset();
        measured(|| run_cycles(&mut runtime, 1)).expect("first scan");
        let first = sample();
        let witness_after = runtime.storage().get_global(witness).cloned();
        eprintln!(
            "allocation-evidence,first_scan,program={name},allocations={},deallocations={},net_bytes={},peak_bytes={}",
            first.allocations, first.deallocations, first.net_bytes, first.peak_bytes
        );
        assert!(
            first.peak_bytes <= FIRST_SCAN_PEAK_BOUND_BYTES,
            "{name}: first-scan peak {} exceeds {}",
            first.peak_bytes,
            FIRST_SCAN_PEAK_BOUND_BYTES
        );
        assert_ne!(
            witness_before, witness_after,
            "{name}: the first scan must run the program"
        );
        assert!(!runtime.faulted(), "{name}: the first scan must not fault");
    }
}

/// Spec 34 MEM-04 (steady-state component; a warmed-up loop alone does not satisfy MEM-04, so
/// first invocation, rare branches, fault paths and retirement are covered by the other tests
/// here): after 100 warm-up scans, 1,000 further scans of each representative application leave
/// live bytes unchanged. Allocations per cycle are reported, not asserted, because hosted services
/// may allocate on the control thread.
#[test]
fn steady_state_scans_leave_live_bytes_unchanged_after_warm_up() {
    let _serial = serial();
    for (name, body, witness) in PROGRAMS {
        let bytes = build(body);
        let mut runtime =
            Runtime::from_bytes(&bytes, PreparationLimits::default()).expect("admit application");
        reset();
        measured(|| run_cycles(&mut runtime, 100)).expect("warm-up scans");
        let before = sample();
        let cycles_before = runtime.cycle_counter();
        let witness_before = runtime.storage().get_global(witness).cloned();
        measured(|| run_cycles(&mut runtime, 1000)).expect("steady-state scans");
        let after = sample();
        let witness_after = runtime.storage().get_global(witness).cloned();
        eprintln!(
            "allocation-evidence,steady_state,program={name},cycles=1000,allocations_per_cycle={:.2},deallocations_per_cycle={:.2},net_bytes={},peak_bytes={}",
            (after.allocations - before.allocations) as f64 / 1000.0,
            (after.deallocations - before.deallocations) as f64 / 1000.0,
            after.net_bytes - before.net_bytes,
            after.peak_bytes,
        );
        assert_eq!(
            after.net_bytes, before.net_bytes,
            "{name}: live bytes grew across 1,000 steady-state scans"
        );
        assert_eq!(runtime.cycle_counter(), cycles_before + 1000);
        assert_ne!(
            witness_before, witness_after,
            "{name}: the scans must run the program"
        );
        assert!(
            !runtime.faulted(),
            "{name}: steady-state scans must not fault"
        );
    }
}

/// Spec 34 MEM-04 and UPDATE-05: 100 rounds of warm restart, cold restart and online change leave
/// live bytes at the level reached after two warm-up rounds, so neither restarts nor generation
/// replacements retain storage.
#[test]
fn restarts_and_online_changes_repeated_100_times_return_live_bytes_to_the_same_level() {
    let _serial = serial();
    let first = build(TIMERS_AND_CALLS);
    let second =
        build(&TIMERS_AND_CALLS.replace("count := count + DINT#1;", "count := count + DINT#10;"));
    let mut runtime =
        Runtime::from_bytes(&first, PreparationLimits::default()).expect("admit application");
    reset();
    for _ in 0..2 {
        measured(|| lifecycle_round(&mut runtime, &second)).expect("warm-up round");
    }
    let before = sample();
    for round in 0..100 {
        let replacement = if round % 2 == 0 { &first } else { &second };
        measured(|| lifecycle_round(&mut runtime, replacement)).expect("lifecycle round");
    }
    let after = sample();
    eprintln!(
        "allocation-evidence,lifecycle,rounds=100,allocations={},deallocations={},net_bytes={},peak_bytes={}",
        after.allocations - before.allocations,
        after.deallocations - before.deallocations,
        after.net_bytes - before.net_bytes,
        after.peak_bytes,
    );
    assert_eq!(
        after.net_bytes, before.net_bytes,
        "live bytes must return to the post-warm-up level after 100 lifecycle rounds"
    );
    assert!(
        !runtime.faulted(),
        "every lifecycle round must leave the controller runnable"
    );
}

/// Spec 34 MEM-04 and UPDATE-05: dropping an installed controller releases everything its admission
/// and scans allocated, so live bytes return to the level before construction.
#[test]
fn dropping_the_runtime_returns_live_bytes_to_the_pre_construction_baseline() {
    let _serial = serial();
    let bytes = build(TIMERS_AND_CALLS);
    let limits = PreparationLimits::default();
    let mut warm = Runtime::from_bytes(&bytes, limits).expect("warm-up admission");
    run_cycles(&mut warm, 10).expect("warm-up scans");
    drop(warm);
    reset();
    let mut runtime = measured(|| Runtime::from_bytes(&bytes, limits)).expect("admit application");
    measured(|| run_cycles(&mut runtime, 10)).expect("scans before drop");
    let held = sample();
    measured(|| drop(runtime));
    let dropped = sample();
    eprintln!(
        "allocation-evidence,drop,held_net_bytes={},released_allocations={},net_bytes_after_drop={}",
        held.net_bytes,
        dropped.deallocations - held.deallocations,
        dropped.net_bytes,
    );
    assert!(
        held.net_bytes > 0,
        "the installed controller holds storage before drop"
    );
    assert_eq!(
        dropped.net_bytes, 0,
        "dropping the controller must release every allocation made since the baseline"
    );
}

/// Spec 34 MEM-04 (fault paths) and STATE-04: a scan that faults with DivisionByZero, followed by
/// the owner's fault clear, repeated 100 times, keeps live bytes at the level reached after two
/// warm-up faults.
#[test]
fn faulting_scans_repeated_100_times_do_not_grow_live_bytes() {
    let _serial = serial();
    let bytes = build(TIMERS_AND_CALLS);
    let mut runtime =
        Runtime::from_bytes(&bytes, PreparationLimits::default()).expect("admit application");
    run_cycles(&mut runtime, 10).expect("healthy scans");
    runtime
        .write_global("divisor", Value::DInt(0))
        .expect("a zero divisor is a valid DINT value");
    reset();
    for _ in 0..2 {
        assert!(
            measured(|| faulting_scan_and_clear(&mut runtime)),
            "warm-up scan must fault with DivisionByZero and latch"
        );
    }
    let before = sample();
    for _ in 0..100 {
        assert!(
            measured(|| faulting_scan_and_clear(&mut runtime)),
            "each scan must fault with DivisionByZero and latch"
        );
    }
    let after = sample();
    eprintln!(
        "allocation-evidence,faulting_scan,repetitions=100,allocations={},deallocations={},net_bytes={}",
        after.allocations - before.allocations,
        after.deallocations - before.deallocations,
        after.net_bytes - before.net_bytes,
    );
    assert_eq!(
        after.net_bytes, before.net_bytes,
        "faulting scans must not grow live bytes"
    );
    assert!(
        !runtime.faulted(),
        "the owner's clear leaves the controller runnable"
    );
}

/// Spec 34 §4.3.1 (checked writes) and the rejected-request rule: a BOOL refused by an INT
/// destination leaves the destination unchanged and is not a PLC fault. 100 refusals keep live
/// bytes at the post-warm-up level.
#[test]
fn rejected_writes_repeated_100_times_do_not_grow_live_bytes() {
    let _serial = serial();
    let bytes = build(TIMERS_AND_CALLS);
    let mut runtime =
        Runtime::from_bytes(&bytes, PreparationLimits::default()).expect("admit application");
    run_cycles(&mut runtime, 10).expect("healthy scans");
    reset();
    for _ in 0..2 {
        assert!(
            measured(|| rejected_write(&mut runtime)),
            "warm-up write must be refused with TypeMismatch"
        );
    }
    let before = sample();
    for _ in 0..100 {
        assert!(
            measured(|| rejected_write(&mut runtime)),
            "each write must be refused with TypeMismatch"
        );
    }
    let after = sample();
    eprintln!(
        "allocation-evidence,rejected_write,repetitions=100,allocations={},deallocations={},net_bytes={}",
        after.allocations - before.allocations,
        after.deallocations - before.deallocations,
        after.net_bytes - before.net_bytes,
    );
    assert_eq!(
        runtime.storage().get_global("setpoint"),
        Some(&Value::Int(1)),
        "a refused write must keep the destination's value"
    );
    assert_eq!(
        after.net_bytes, before.net_bytes,
        "refused writes must not grow live bytes"
    );
    assert!(
        !runtime.faulted(),
        "a refused management write is not a PLC execution fault"
    );
}

/// Spec 34 HOST-09 and UPDATE-01: a candidate refused during preparation leaves the installed
/// controller executable and unchanged. 100 refusals keep live bytes at the post-warm-up level and
/// never change the cycle counter.
#[test]
fn rejected_candidates_repeated_100_times_do_not_grow_live_bytes() {
    let _serial = serial();
    let bytes = build(TIMERS_AND_CALLS);
    let mut runtime =
        Runtime::from_bytes(&bytes, PreparationLimits::default()).expect("admit application");
    run_cycles(&mut runtime, 10).expect("healthy scans");
    reset();
    for _ in 0..2 {
        assert!(
            measured(|| rejected_candidate(&mut runtime, &bytes)),
            "warm-up candidate must be refused"
        );
    }
    let before = sample();
    for _ in 0..100 {
        assert!(
            measured(|| rejected_candidate(&mut runtime, &bytes)),
            "each candidate must be refused and leave the cycle counter unchanged"
        );
    }
    let after = sample();
    eprintln!(
        "allocation-evidence,rejected_candidate,repetitions=100,allocations={},deallocations={},net_bytes={}",
        after.allocations - before.allocations,
        after.deallocations - before.deallocations,
        after.net_bytes - before.net_bytes,
    );
    run_cycles(&mut runtime, 1).expect("the installed controller still runs");
    assert_eq!(
        after.net_bytes, before.net_bytes,
        "refused candidates must not grow live bytes"
    );
}
