//! Debug data types.

#![allow(missing_docs)]

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

/// The hit-count condition of a breakpoint: the shared definition, so the rule exists once
/// (specification 13, "Runtime debug state, mutation, and projection invariants").
pub use trust_runtime_services::debug::HitCondition;

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
    /// Parser-owned registry for anonymous types referenced by expressions.
    /// This remains detached from the installed schema and lives as long as
    /// the compiled breakpoint or logpoint.
    pub type_registry: Option<std::sync::Arc<TypeRegistry>>,
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
            type_registry: None,
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
    /// Storage interpretation of the installed artifact that produced this snapshot.
    pub storage_abi: crate::value::StorageAbi,
    /// Variable storage snapshot.
    pub storage: VariableStorage,
    /// Current runtime time.
    pub now: Duration,
}

impl DebugSnapshot {
    /// Resolve scratch-only expressions before queueing a paused assignment.
    /// Snapshot writes remain presentation; the owner rechecks the destination.
    pub fn resolve_lvalue_expressions(
        &mut self,
        target: LValue,
        registry: &TypeRegistry,
        stdlib: Option<&StandardLibrary>,
        profile: &DateTimeProfile,
        frame_id: Option<FrameId>,
    ) -> Result<LValue, RuntimeError> {
        target.resolve_expressions(&mut |expr| {
            self.evaluate_expression(expr, registry, stdlib, profile, frame_id)
        })
    }

    /// Evaluate an expression against this paused-state snapshot.
    pub fn evaluate_expression(
        &mut self,
        expr: &Expr,
        registry: &TypeRegistry,
        stdlib: Option<&StandardLibrary>,
        profile: &DateTimeProfile,
        frame_id: Option<FrameId>,
    ) -> Result<Value, RuntimeError> {
        let storage_abi = self.storage_abi;
        let eval = |storage: &mut VariableStorage| {
            let instance_id = storage.current_frame().and_then(|frame| frame.instance_id);
            crate::helper_eval::eval_storage_expr_with_abi(
                storage,
                registry,
                storage_abi,
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
        let storage_abi = self.storage_abi;
        let read = |storage: &mut VariableStorage| {
            let instance_id = storage.current_frame().and_then(|frame| frame.instance_id);
            crate::helper_eval::read_storage_lvalue_with_abi(
                storage,
                registry,
                storage_abi,
                profile,
                instance_id,
                target,
            )
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
        let storage_abi = self.storage_abi;
        let write = |storage: &mut VariableStorage| {
            let instance_id = storage.current_frame().and_then(|frame| frame.instance_id);
            crate::helper_eval::write_storage_lvalue_with_abi(
                storage,
                registry,
                storage_abi,
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

// Runtime events are shared runtime code (spec 34 ARCH-11).
pub use trust_runtime_services::events::RuntimeEvent;

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

#[cfg(test)]
#[path = "types/contract_tests.rs"]
mod contract_tests;
