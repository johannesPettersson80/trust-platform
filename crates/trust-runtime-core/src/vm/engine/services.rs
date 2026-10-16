//! Explicit platform services; physical deadlines never use PLC logical time.
use crate::{error::RuntimeError, value::DateTimeValue};

/// Optional services supplied by the runtime's platform composition.
pub trait ExecutionServices {
    /// Immutable optional-tier settings captured before initialization and all
    /// register-state reservations. This callback must not mutate environment.
    #[cfg(feature = "register-ir")]
    fn register_configuration(&self) -> crate::vm::register::RegisterExecutionConfiguration {
        crate::vm::register::RegisterExecutionConfiguration::default()
    }

    /// Whether instruction-level debugging requires the canonical stack path.
    fn debug_active(&self) -> bool {
        false
    }

    /// Observe an instruction boundary using read-only installed state.
    /// Hosted debugging/profiling adapters may publish snapshots or pause here.
    /// Writes still enter through the controller's checked management boundary.
    /// The callback must not reacquire this controller or wait for its write
    /// completion: it runs while the owner holds exclusive execution access.
    /// Supported debugger pauses require the host's existing pause-aware deadline
    /// accounting; bounded RUN profiles must not block on ordinary observers.
    fn on_statement(
        &self,
        _pou_id: u32,
        _code_offset: usize,
        _depth: u32,
        _storage: &crate::memory::VariableStorage,
        _logical_time: crate::value::Duration,
    ) {
    }

    /// Observe the active invocation without exposing mutable locals or storage.
    /// The view is borrowed only for this callback. Do not reacquire the controller
    /// or synchronously wait for a checked write to complete from this hook.
    /// Existing services continue to receive the old logical/storage callback.
    fn on_statement_with_frame(
        &self,
        pou_id: u32,
        code_offset: usize,
        depth: u32,
        storage: &crate::memory::VariableStorage,
        logical_time: crate::value::Duration,
        _frame: crate::vm::StatementFrame<'_>,
    ) {
        self.on_statement(pou_id, code_offset, depth, storage, logical_time);
    }

    /// Observe an instruction boundary with explicit adapter invariant failures.
    /// Existing infallible observers retain their callback through the default.
    /// Errors follow normal execution cleanup and fail-closed cycle handling;
    /// rejected management requests must be reported separately by their adapter.
    fn on_statement_with_frame_checked(
        &self,
        pou_id: u32,
        code_offset: usize,
        depth: u32,
        storage: &crate::memory::VariableStorage,
        logical_time: crate::value::Duration,
        frame: crate::vm::StatementFrame<'_>,
    ) -> Result<(), RuntimeError> {
        self.on_statement_with_frame(pou_id, code_offset, depth, storage, logical_time, frame);
        Ok(())
    }

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

/// Logical-time composition without wall-clock, retain-store or physical deadlines.
#[derive(Clone, Copy, Debug, Default)]
pub struct LogicalExecutionServices;
pub(super) static LOGICAL_ONLY: LogicalExecutionServices = LogicalExecutionServices;
impl ExecutionServices for LogicalExecutionServices {
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
