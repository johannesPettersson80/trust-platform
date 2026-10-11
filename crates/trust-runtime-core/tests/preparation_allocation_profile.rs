//! Requested allocation payloads on the native target, not MCU heap/stack proof.
//! On i686 this uses 32-bit core layouts. System allocator overhead, LLFF
//! fragmentation, interrupt stack and hardware timing are deliberately excluded.
use std::alloc::{GlobalAlloc, Layout, System};
use std::cell::Cell;
use trust_runtime_core::{
    bytecode::ValidationLimits,
    value::Duration,
    vm::{PreparationLimits, PreparedModule},
};

#[derive(Clone, Copy, Debug, Default)]
struct Sample {
    live: usize,
    peak: usize,
    allocations: usize,
    invalid_accounting: bool,
}
thread_local! {
    static ENABLED: Cell<bool> = const { Cell::new(false) };
    static SAMPLE: Cell<Sample> = const { Cell::new(Sample {
        live: 0, peak: 0, allocations: 0, invalid_accounting: false,
    }) };
}
fn record(bytes: usize, allocation: bool) {
    let _ = ENABLED.try_with(|enabled| {
        if enabled.get() {
            let _ = SAMPLE.try_with(|sample| {
                let mut value = sample.get();
                let next = if allocation {
                    value.live.checked_add(bytes)
                } else {
                    value.live.checked_sub(bytes)
                };
                if let Some(next) = next {
                    value.live = next;
                } else {
                    value.invalid_accounting = true;
                }
                value.peak = value.peak.max(value.live);
                if allocation {
                    match value.allocations.checked_add(1) {
                        Some(count) => value.allocations = count,
                        None => value.invalid_accounting = true,
                    }
                }
                sample.set(value);
            });
        }
    });
}

struct MeasuredSystem;
// SAFETY: Unchanged pointer/layout pairs are delegated to System. Thread-local
// scalar bookkeeping does not allocate, block, or panic. Default realloc uses
// this allocator's alloc/dealloc, matching the firmware allocator's strategy.
unsafe impl GlobalAlloc for MeasuredSystem {
    unsafe fn alloc(&self, layout: Layout) -> *mut u8 {
        // SAFETY: GlobalAlloc caller supplies the valid allocation layout.
        let pointer = unsafe { System.alloc(layout) };
        if !pointer.is_null() {
            record(layout.size(), true);
        }
        pointer
    }
    unsafe fn dealloc(&self, pointer: *mut u8, layout: Layout) {
        record(layout.size(), false);
        // SAFETY: Original allocation pointer and layout are unchanged.
        unsafe { System.dealloc(pointer, layout) };
    }
}
#[global_allocator]
static ALLOCATOR: MeasuredSystem = MeasuredSystem;

struct Capture;
impl Capture {
    fn begin() -> Self {
        SAMPLE.with(|sample| sample.set(Sample::default()));
        ENABLED.with(|enabled| enabled.set(true));
        Self
    }
    fn phase() -> Sample {
        SAMPLE.with(|sample| {
            let previous = sample.get();
            sample.set(Sample {
                peak: previous.live,
                allocations: 0,
                ..previous
            });
            previous
        })
    }
}
impl Drop for Capture {
    fn drop(&mut self) {
        ENABLED.with(|enabled| enabled.set(false));
    }
}

#[test]
fn saved_artifacts_report_actual_preparation_instantiation_and_scan_allocations() {
    let limits = PreparationLimits {
        max_artifact_bytes: 16 * 1024,
        max_preparation_bytes: 512 * 1024,
        max_preparation_work: 1_000_000,
        validation: ValidationLimits {
            max_scratch_bytes: 48 * 1024,
            max_work: 500_000,
        },
        max_construction_values: 4096,
        max_construction_bytes: 128 * 1024,
        max_process_image_bytes: 64,
        max_work: 50_000,
        max_call_depth: 4,
    };
    for (name, bytes) in [
        (
            "main",
            &include_bytes!(
                "../../trust-runtime/tests/fixtures/portability/stbc-2.0/program-v2.stbc"
            )[..],
        ),
        (
            "numeric",
            &include_bytes!(
                "../../trust-runtime/tests/fixtures/portability/stbc-2.0/numeric-v2.stbc"
            )[..],
        ),
        (
            "gpio",
            &include_bytes!("../../trust-runtime/tests/fixtures/portability/f401/gpio.stbc")[..],
        ),
    ] {
        let capture = Capture::begin();
        let prepared = PreparedModule::from_bytes(bytes, limits).unwrap();
        let preparation = Capture::phase();
        let mut state = prepared.instantiate(0).unwrap();
        let instantiation = Capture::phase();
        for millis in (0..=1000).step_by(10) {
            state.execute_cycle(Duration::from_millis(millis)).unwrap();
        }
        let scans = Capture::phase();
        drop(state);
        drop(prepared);
        let retired = Capture::phase();
        drop(capture);
        eprintln!("allocation-profile,{name},pointer_bits={},prepare={preparation:?},instantiate={instantiation:?},scans={scans:?},retired={retired:?}", usize::BITS);
        for sample in [preparation, instantiation, scans, retired] {
            assert!(!sample.invalid_accounting);
            assert!(sample.peak >= sample.live);
        }
        assert!(preparation.allocations > 0);
        assert!(instantiation.allocations > 0);
        assert_eq!(retired.live, 0, "all tracked application state must retire");
    }
}

#[test]
fn fixed_section_reasons_borrow_their_static_message() {
    use trust_runtime_core::bytecode::{BytecodeError, RejectionReason};
    let capture = Capture::begin();
    let error = BytecodeError::from(RejectionReason::InitializerWriteOutsideStaging);
    let cloned = error.clone();
    let equal = error == cloned;
    drop(error);
    drop(cloned);
    let sample = Capture::phase();
    drop(capture);
    assert!(equal);
    assert!(!sample.invalid_accounting);
    assert_eq!(sample.allocations, 0);
    assert_eq!(sample.live, 0);
}

#[cfg(not(feature = "std"))]
#[test]
fn portable_diagnostic_construction_clone_and_equality_do_not_render_or_allocate() {
    use trust_runtime_core::{
        bytecode::{BytecodeError, SectionDiagnostic},
        error::{PreparationDiagnostic, RuntimeError},
    };
    let capture = Capture::begin();
    let error = RuntimeError::from(BytecodeError::section_diagnostic(
        SectionDiagnostic::ArrayCount {
            expected: 100,
            actual: 3,
        },
    ));
    let cloned = error.clone();
    let equal = error == cloned;
    let materialization = PreparationDiagnostic::ConstantTruncated {
        kind: "ARRAY child",
        length: true,
        need: 4,
        have: 2,
    }
    .into_runtime_error();
    let code = error.stable_code();
    let materialization_code = materialization.stable_code();
    drop(error);
    drop(cloned);
    drop(materialization);
    let sample = Capture::phase();
    drop(capture);
    assert!(equal);
    assert_eq!(
        code,
        trust_runtime_core::error::StableErrorCode::BytecodeInvalidSection
    );
    assert_eq!(
        materialization_code,
        trust_runtime_core::error::StableErrorCode::VmBytecodeDecode
    );
    assert!(!sample.invalid_accounting);
    assert_eq!(sample.allocations, 0);
    assert_eq!(sample.live, 0);
}
