//! Explicit bring-up admission; logical cumulative charges are not heap limits.
use crate::board::Clock;
use core::cell::Cell;
use trust_runtime_core::{
    bytecode::ValidationLimits,
    error::RuntimeError,
    value::DateTimeValue,
    vm::{ExecutionServices, PreparationLimits},
};

pub fn limits() -> PreparationLimits {
    PreparationLimits {
        max_artifact_bytes: 16 * 1024,
        // These are cumulative logical charges across retired scratch, not a
        // claim of RAM availability. The allocator independently caps 72 KiB.
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
        max_construction_work: 50_000,
        max_call_depth: 4,
    }
}

/// Execution services over one board clock; the deadline is armed per scan.
pub struct Services<'a, C: Clock> {
    pub clock: &'a C,
    pub deadline: Cell<Option<u64>>,
}

impl<C: Clock> ExecutionServices for Services<'_, C> {
    fn deadline_exceeded(&self) -> bool {
        self.deadline
            .get()
            .is_some_and(|limit| self.clock.micros() >= limit)
    }
    fn has_wall_clock(&self) -> bool {
        false
    }
    fn current_dt(&self) -> Result<DateTimeValue, RuntimeError> {
        Err(RuntimeError::ProfileUnsupported(
            "no UTC clock in board bring-up".into(),
        ))
    }
}
