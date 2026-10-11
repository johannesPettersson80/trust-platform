//! Explicit platform services; physical deadlines never use PLC logical time.
use crate::{error::RuntimeError, value::DateTimeValue};

/// Optional services supplied by the runtime's platform composition.
pub trait ExecutionServices {
    /// Check the platform's physical execution deadline in its own clock domain.
    fn deadline_exceeded(&self) -> bool;
    /// Whether this composition admits the UTC wall-clock native import.
    fn has_wall_clock(&self) -> bool;
    /// Whether the composition has explicitly configured a retain store.
    fn has_retain_store(&self) -> bool {
        false
    }
    /// Save or durably enqueue this cycle's snapshot according to the platform policy.
    /// Failure prevents output publication. A configured store must implement this hook.
    fn save_retain_snapshot(
        &self,
        _snapshot: &crate::retain::RetainSnapshot,
    ) -> Result<(), RuntimeError> {
        Err(RuntimeError::RetainStore(
            "retain store has no save implementation".into(),
        ))
    }
    /// Sample UTC wall time for CURRENT_DT, independently of scan logical time.
    fn current_dt(&self) -> Result<DateTimeValue, RuntimeError>;
}

pub(super) struct LogicalOnly;
pub(super) static LOGICAL_ONLY: LogicalOnly = LogicalOnly;
impl ExecutionServices for LogicalOnly {
    fn deadline_exceeded(&self) -> bool {
        false
    }
    fn has_wall_clock(&self) -> bool {
        false
    }
    fn current_dt(&self) -> Result<DateTimeValue, RuntimeError> {
        Err(RuntimeError::UndefinedFunction("CURRENT_DT".into()))
    }
}
