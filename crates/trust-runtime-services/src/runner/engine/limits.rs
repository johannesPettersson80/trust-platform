//! Admission on a device (specification 34, LOAD-04, MEM-02 and §6.4): the preparation limits
//! that follow from the device's application region and heap.

use trust_runtime_core::bytecode::ValidationLimits;
use trust_runtime_core::vm::{PreparationLimits, VM_MAX_CALL_DEPTH};

/// The memory a device gives the installed program.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct DeviceMemory {
    /// Bytes of the application region that holds the program (the chip file's `[layout]
    /// application`).
    pub region_bytes: usize,
    /// Bytes of the heap in which the program is prepared, constructed and run.
    pub heap_bytes: usize,
}

/// The heap of the M2E bring-up runs: in it both chips prepared, constructed and ran the A4
/// programs under the limits that `device_limits` gives for this heap (spec 34 Appendix D).
const MEASURED_HEAP_BYTES: u64 = 72 * 1024;

/// The call depth of the bring-up runs, at which both chips ran the A4 programs and the board
/// probe's four nested calls in the 72 KiB heap.
const MEASURED_CALL_DEPTH: u64 = 4;

/// The preparation limits of a device with `memory`.
///
/// - The program can be no longer than its region.
/// - The byte and value-count limits are the bring-up limits for a 72 KiB heap, scaled to the
///   device's heap: the preparation and construction charges, the validation scratch, the
///   constructed values and the process images all come from the heap.
/// - The call depth comes from the heap in the same way (MEM-02): each level reserves memory
///   when the program is constructed, for its frame, its operands and a pending call. It is 4
///   at 72 KiB, `4 × heap / 72 KiB` in general, and never more than trust-runtime's 1,024.
///   Nested initialization of function-block instances still recurses on the native stack, by
///   about 3.9 KB per level on the STM32F4, so the main stack must hold the admitted depth.
/// - The work limits are trust-runtime's: they measure what the program does, not memory, and
///   the scan deadline bounds a scan's time as on a computer (TIME-08).
///
/// A program that needs more is refused at admission with the limit it exceeds (ARCH-13),
/// before it can exhaust the device's memory.
#[must_use]
pub fn device_limits(memory: DeviceMemory) -> PreparationLimits {
    let host = PreparationLimits::default();
    let heap = memory.heap_bytes;
    PreparationLimits {
        max_artifact_bytes: memory.region_bytes,
        max_preparation_bytes: scaled_to(heap, 512 * 1024),
        max_preparation_work: host.max_preparation_work,
        validation: ValidationLimits {
            max_scratch_bytes: scaled_to(heap, 48 * 1024),
            max_work: host.validation.max_work,
        },
        max_construction_values: scaled_to(heap, 4096),
        max_construction_bytes: scaled_to(heap, 128 * 1024),
        max_process_image_bytes: scaled_to(heap, 64),
        max_work: host.max_work,
        max_construction_work: host.max_construction_work,
        max_call_depth: scaled_to(heap, MEASURED_CALL_DEPTH).min(VM_MAX_CALL_DEPTH),
    }
}

/// The bring-up limit `measured`, set for a 72 KiB heap, scaled to a heap of `heap_bytes`.
fn scaled_to(heap_bytes: usize, measured: u64) -> usize {
    let heap_bytes = u64::try_from(heap_bytes).unwrap_or(u64::MAX);
    usize::try_from(heap_bytes.saturating_mul(measured) / MEASURED_HEAP_BYTES).unwrap_or(usize::MAX)
}
