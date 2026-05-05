//! Debug data types.

#![allow(missing_docs)]

use smol_str::SmolStr;

use crate::error::RuntimeError;
use crate::memory::{FrameId, VariableStorage};
use crate::program_model::{Expr, LValue};
use crate::stdlib::StandardLibrary;
use crate::value::{DateTimeProfile, Duration, Value};

use trust_hir::types::TypeRegistry;

/// Source location for a statement or expression.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct SourceLocation {
    /// Source file identifier (per lowering pass).
    pub file_id: u32,
    /// Byte offset at the start of the statement.
    pub start: u32,
    /// Byte offset at the end of the statement.
    pub end: u32,
}

impl SourceLocation {
    /// Create a new source location from byte offsets.
    #[must_use]
    pub fn new(file_id: u32, start: u32, end: u32) -> Self {
        Self {
            file_id,
            start,
            end,
        }
    }
}

/// Hit count conditions for breakpoints.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum HitCondition {
    /// Break when hit count equals the target.
    Equal(u64),
    /// Break when hit count is at least the target.
    AtLeast(u64),
    /// Break when hit count is greater than the target.
    GreaterThan(u64),
}

impl HitCondition {
    /// Check whether the hit condition is satisfied.
    #[must_use]
    pub fn is_met(self, hits: u64) -> bool {
        match self {
            HitCondition::Equal(target) => hits == target,
            HitCondition::AtLeast(target) => hits >= target,
            HitCondition::GreaterThan(target) => hits > target,
        }
    }
}

/// Logpoint message fragments.
#[derive(Debug, Clone)]
pub enum LogFragment {
    /// Literal text.
    Text(String),
    /// Expression to evaluate.
    Expr(Expr),
}

/// Breakpoint definition with optional conditions.
#[derive(Debug, Clone)]
pub struct DebugBreakpoint {
    /// Resolved statement location for this breakpoint.
    pub location: SourceLocation,
    /// Optional condition expression evaluated at the statement boundary.
    pub condition: Option<Expr>,
    /// Optional hit count condition.
    pub hit_condition: Option<HitCondition>,
    /// Optional logpoint template fragments.
    pub log_message: Option<Vec<LogFragment>>,
    /// Current hit count for this breakpoint.
    pub hits: u64,
    /// Breakpoint generation (updated when setBreakpoints runs).
    pub generation: u64,
}

impl DebugBreakpoint {
    /// Create an unconditional breakpoint at a location.
    #[must_use]
    pub fn new(location: SourceLocation) -> Self {
        Self {
            location,
            condition: None,
            hit_condition: None,
            log_message: None,
            hits: 0,
            generation: 0,
        }
    }
}

/// Captured log output.
#[derive(Debug, Clone)]
pub struct DebugLog {
    /// Log message text.
    pub message: String,
    /// Optional source location for the log.
    pub location: Option<SourceLocation>,
}

/// Snapshot of runtime state at a stop.
#[derive(Debug, Clone)]
pub struct DebugSnapshot {
    /// Variable storage snapshot.
    pub storage: VariableStorage,
    /// Current runtime time.
    pub now: Duration,
}

impl DebugSnapshot {
    /// Evaluate an expression against this paused-state snapshot.
    pub fn evaluate_expression(
        &mut self,
        expr: &Expr,
        registry: &TypeRegistry,
        stdlib: Option<&StandardLibrary>,
        profile: &DateTimeProfile,
        frame_id: Option<FrameId>,
    ) -> Result<Value, RuntimeError> {
        let eval = |storage: &mut VariableStorage| {
            let instance_id = storage.current_frame().and_then(|frame| frame.instance_id);
            crate::helper_eval::eval_storage_expr_with_stdlib(
                storage,
                registry,
                profile,
                instance_id,
                stdlib,
                expr,
            )
        };

        if let Some(frame_id) = frame_id {
            self.storage
                .with_frame(frame_id, eval)
                .ok_or(RuntimeError::InvalidFrame(frame_id.0))?
        } else {
            eval(&mut self.storage)
        }
    }

    /// Read an lvalue against this paused-state snapshot.
    pub fn read_lvalue(
        &mut self,
        target: &LValue,
        registry: &TypeRegistry,
        profile: &DateTimeProfile,
        frame_id: Option<FrameId>,
    ) -> Result<Value, RuntimeError> {
        let read = |storage: &mut VariableStorage| {
            let instance_id = storage.current_frame().and_then(|frame| frame.instance_id);
            crate::helper_eval::read_storage_lvalue(storage, registry, profile, instance_id, target)
        };

        if let Some(frame_id) = frame_id {
            self.storage
                .with_frame(frame_id, read)
                .ok_or(RuntimeError::InvalidFrame(frame_id.0))?
        } else {
            read(&mut self.storage)
        }
    }

    /// Write an lvalue against this paused-state snapshot.
    pub fn write_lvalue(
        &mut self,
        target: &LValue,
        value: Value,
        registry: &TypeRegistry,
        profile: &DateTimeProfile,
        frame_id: Option<FrameId>,
    ) -> Result<(), RuntimeError> {
        let write = |storage: &mut VariableStorage| {
            let instance_id = storage.current_frame().and_then(|frame| frame.instance_id);
            crate::helper_eval::write_storage_lvalue(
                storage,
                registry,
                profile,
                instance_id,
                target,
                value.clone(),
            )
        };

        if let Some(frame_id) = frame_id {
            self.storage
                .with_frame(frame_id, write)
                .ok_or(RuntimeError::InvalidFrame(frame_id.0))?
        } else {
            write(&mut self.storage)
        }
    }
}

/// Runtime scheduling and diagnostic events.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum RuntimeEvent {
    /// Cycle start event.
    CycleStart {
        /// Cycle counter value.
        cycle: u64,
        /// Time at cycle start.
        time: Duration,
    },
    /// Cycle end event.
    CycleEnd {
        /// Cycle counter value.
        cycle: u64,
        /// Time at cycle end.
        time: Duration,
    },
    /// Task execution start.
    TaskStart {
        /// Task name.
        name: SmolStr,
        /// Task priority (0 is highest).
        priority: u32,
        /// Time at task start.
        time: Duration,
    },
    /// Task execution end.
    TaskEnd {
        /// Task name.
        name: SmolStr,
        /// Task priority (0 is highest).
        priority: u32,
        /// Time at task end.
        time: Duration,
    },
    /// Task missed one or more periodic activations.
    TaskOverrun {
        /// Task name.
        name: SmolStr,
        /// Missed activation count.
        missed: u64,
        /// Time when the overrun was detected.
        time: Duration,
    },
    /// Resource fault event.
    Fault {
        /// Fault message.
        error: String,
        /// Time when the fault was recorded.
        time: Duration,
    },
    /// Safe-state application failed while handling a resource fault.
    SafeStateFailed {
        /// Original fault message.
        root: String,
        /// Safe-state failure message.
        error: String,
        /// Time when the safe-state failure was recorded.
        time: Duration,
    },
    /// A retained value no longer has a matching retained global and was dropped.
    RetainOrphanDropped {
        /// Retained variable name.
        name: SmolStr,
        /// Time when the orphan was dropped.
        time: Duration,
    },
    /// A retained value was explicitly migrated to the current declared type.
    RetainMigrationApplied {
        /// Retained variable name.
        name: SmolStr,
        /// Migration detail.
        detail: String,
        /// Time when the migration was applied.
        time: Duration,
    },
    /// A control audit event could not be delivered to the configured audit sink.
    AuditDropped {
        /// Control request id associated with the dropped audit event.
        request_id: u64,
        /// Control request type associated with the dropped audit event.
        request_type: SmolStr,
        /// Send/write failure detail.
        error: String,
        /// Time when the audit drop was observed.
        time: Duration,
    },
    /// A request reached a surface that is disabled by feature/configuration.
    FeatureDisabled {
        /// Disabled feature name.
        feature: SmolStr,
        /// Optional request type that attempted to use the feature.
        request_type: Option<SmolStr>,
        /// Time when the disabled feature request was observed.
        time: Duration,
    },
}

/// Stop reason for debugger events.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum DebugStopReason {
    /// Paused due to a breakpoint.
    Breakpoint,
    /// Paused due to stepping.
    Step,
    /// Paused due to a user pause request.
    Pause,
    /// Paused due to stopOnEntry.
    Entry,
}

/// Notification emitted when execution stops.
#[derive(Debug, Clone)]
pub struct DebugStop {
    /// Reason for stopping.
    pub reason: DebugStopReason,
    /// Location where execution stopped (if known).
    pub location: Option<SourceLocation>,
    /// Thread/task id, if known.
    pub thread_id: Option<u32>,
    /// Breakpoint generation when the stop was emitted.
    pub breakpoint_generation: Option<u64>,
}
