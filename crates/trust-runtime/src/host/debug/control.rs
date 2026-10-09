//! Debug control and state.

#![allow(missing_docs)]

use std::collections::{HashMap, HashSet};
use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::mpsc::Sender;
use std::sync::{Arc, Condvar, Mutex};

use smol_str::SmolStr;

use crate::io::{IoAddress, IoSnapshot};
use crate::memory::{FrameId, InstanceId};
use crate::program_model::{Expr, LValue};
use crate::value::Value;

use super::breakpoints::{matches_breakpoint, BreakpointCycleKey};
use super::hook::{DebugHook, DebugRuntimeContext};
use super::trace::{trace_debug, trace_debug_with};
use super::{
    DebugBreakpoint, DebugLog, DebugSnapshot, DebugStop, DebugStopReason, RuntimeEvent,
    SourceLocation,
};

#[path = "control/recording.rs"]
mod recording;
pub(crate) use recording::numeric as numeric_control_value;
#[path = "control/parameters.rs"]
mod parameters;
#[path = "control/signals.rs"]
mod signals;
#[path = "control/symbol_path.rs"]
mod symbol_path;
#[path = "control/transfer.rs"]
mod transfer;
pub(crate) use parameters::resolve_path as resolve_control_parameter_path;
pub use recording::{IoRecordingResult, IoRecordingSpec};
pub(crate) use symbol_path::number_as as control_number_as;

include!("control/types.rs");
include!("control/api.rs");
include!("control/hook.rs");

#[cfg(test)]
mod tests;
