//! Debug control and state.

#![allow(missing_docs)]

use std::collections::HashMap;
use std::sync::mpsc::Sender;
use std::sync::{Arc, Condvar, Mutex};

use smol_str::SmolStr;

use crate::io::{IoAddress, IoSnapshot};
use crate::memory::{FrameId, InstanceId};
use crate::program_model::{Expr, LValue};
use crate::value::Value;

use super::breakpoints::matches_breakpoint;
use super::hook::{DebugHook, DebugRuntimeContext};
use super::trace::trace_debug;
use super::{
    DebugBreakpoint, DebugLog, DebugSnapshot, DebugStop, DebugStopReason, RuntimeEvent,
    SourceLocation,
};

include!("control/types.rs");
include!("control/api.rs");
include!("control/hook.rs");

#[cfg(test)]
mod tests;
