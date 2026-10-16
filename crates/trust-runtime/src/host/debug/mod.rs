//! Debugging and tracing support.

#![allow(missing_docs)]

mod breakpoints;
mod control;
pub mod dap;
mod execution_snapshot;
mod hook;
mod resolve;
mod trace;
mod types;

pub(crate) use control::PendingVarTarget;
pub use control::{
    ControlAction, ControlOutcome, DebugControl, DebugMode, ForcedVarTarget, RequestQueueFull,
    StepKind,
};
pub use dap::{DebugScope, DebugSource, DebugVariable, DebugVariableHandles, VariableHandle};
pub use hook::{DebugHook, DebugRuntimeContext, NoopDebugHook};
pub use resolve::{location_to_line_col, offset_to_line_col, resolve_breakpoint_location};
pub use types::{
    DebugBreakpoint, DebugLog, DebugSnapshot, DebugStop, DebugStopReason, HitCondition,
    LogFragment, RuntimeEvent, SourceLocation,
};
