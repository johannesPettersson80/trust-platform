//! Admission on a device (specification 34, LOAD-04, MEM-02, MEM-03 and §6.4): the preparation
//! limits that follow from the device's application region, its heap and its main stack.

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
    /// The call levels the device's main stack holds ([`DeviceStack::call_depth`]).
    pub call_depth: usize,
}

/// A device's main stack and what one call level takes of it (specification 34 MEM-03; owner,
/// 21 October 2026, "Stack decides"). Nested initialization of function-block instances recurses on
/// the native stack, one recursion per call level, so the main stack, not the heap, decides how
/// many call levels a device admits.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct DeviceStack {
    /// Bytes of the main-stack window that the device paints and measures.
    pub window_bytes: usize,
    /// Bytes the deepest path takes before its first call level, measured on the bench: the
    /// high-water of a program's install less what its call levels add.
    pub base_bytes: usize,
    /// Bytes each call level adds to that path, measured on the bench.
    pub level_bytes: usize,
}

impl DeviceStack {
    /// The headroom the deepest path keeps below the window's bottom (Appendix D.2).
    pub const HEADROOM_BYTES: usize = 2 * 1024;

    /// The call levels the window holds with the headroom: `(window − headroom − base) / level`,
    /// never more than trust-runtime's 1,024, and none when the window does not hold the base.
    #[must_use]
    pub const fn call_depth(self) -> usize {
        if self.level_bytes == 0 {
            return 0;
        }
        let room = self
            .window_bytes
            .saturating_sub(Self::HEADROOM_BYTES)
            .saturating_sub(self.base_bytes);
        let levels = room / self.level_bytes;
        if levels < VM_MAX_CALL_DEPTH {
            levels
        } else {
            VM_MAX_CALL_DEPTH
        }
    }
}

/// The heap of the M2E bring-up runs: in it both chips prepared and ran the A4 programs under
/// the preparation limits that `device_limits` gives for this heap (spec 34 Appendix D).
const MEASURED_HEAP_BYTES: u64 = 72 * 1024;

/// The preparation limits of a device with `memory`.
///
/// - The program can be no longer than its region.
/// - The preparation bytes and the validation scratch are the bring-up limits for a 72 KiB heap,
///   scaled to the device's heap: preparing a program takes them from the heap before its memory
///   figure is known.
/// - The constructed values, the construction bytes and the process images are trust-runtime's
///   (owner, 20 October 2026, "Let memory decide"): the program's memory figure, which admission
///   compares with the memory the device has for programs, decides what fits (MEM-12).
/// - The call depth is the device's ([`DeviceMemory::call_depth`]): the levels its main stack
///   holds (MEM-03), never more than trust-runtime's 1,024.
/// - The work limits are trust-runtime's: they measure what the program does, not memory, and
///   the scan deadline bounds a scan's time as on a computer (TIME-08).
///
/// A program that needs more is refused at admission with the limit it exceeds (ARCH-13),
/// before it can exhaust the device's memory or its stack.
#[must_use]
pub fn device_limits(memory: DeviceMemory) -> PreparationLimits {
    let host = PreparationLimits::default();
    let heap = memory.heap_bytes;
    PreparationLimits {
        max_artifact_bytes: memory.region_bytes,
        max_preparation_bytes: scaled_to(heap, 512 * 1024),
        validation: ValidationLimits {
            max_scratch_bytes: scaled_to(heap, 48 * 1024),
            max_work: host.validation.max_work,
        },
        max_call_depth: memory.call_depth.min(VM_MAX_CALL_DEPTH),
        ..host
    }
}

/// The bring-up limit `measured`, set for a 72 KiB heap, scaled to a heap of `heap_bytes`.
fn scaled_to(heap_bytes: usize, measured: u64) -> usize {
    let heap_bytes = u64::try_from(heap_bytes).unwrap_or(u64::MAX);
    usize::try_from(heap_bytes.saturating_mul(measured) / MEASURED_HEAP_BYTES).unwrap_or(usize::MAX)
}
