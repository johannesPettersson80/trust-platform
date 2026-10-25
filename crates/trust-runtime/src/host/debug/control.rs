//! The debugger's control of a resource (specification 11 §6.9, §6.13; specification 44 §8.8): the
//! Live Values' queued writes and active forces, the runtime events and I/O snapshots for the
//! clients that stream them, and the debugger of the compiled program the resource runs.

#![allow(missing_docs)]

use std::collections::VecDeque;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::mpsc::Sender;
use std::sync::{Arc, Mutex, MutexGuard};

use smol_str::SmolStr;

use crate::error::RuntimeError;
use crate::io::{IoAddress, IoSnapshot};
use crate::memory::{FrameId, Generation, InstanceId};
use crate::runtime::CompiledDebugger;
use crate::value::{Duration, Value};

#[cfg(test)]
use trust_runtime_services::live_values::PendingVarWrite;
use trust_runtime_services::live_values::{
    ForcedSnapshot, ForcedVarTarget, LiveValueQueues, PendingVarTarget, RequestQueueFull,
};

use super::{DebugSnapshot, RuntimeEvent};

include!("control/types.rs");
include!("control/api.rs");

#[cfg(test)]
mod tests;
