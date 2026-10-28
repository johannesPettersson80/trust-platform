//! The debugger's control of a resource (specification 11 §6.9, §6.13; specification 44 §8.8): the
//! Live Values' queued writes and active forces, the runtime events and I/O snapshots for the
//! clients that stream them, and the debugger of the compiled program the resource runs; and
//! Control Studio's services that the scan feeds (specification 35): parameter sets, controller
//! transfers, recordings, signal inspection and the experiments' scan observer.

#![allow(missing_docs)]

use std::collections::VecDeque;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::mpsc::Sender;
use std::sync::{Arc, Mutex, MutexGuard};

use smol_str::SmolStr;

use crate::error::RuntimeError;
use crate::io::{IoAddress, IoSnapshot};
use crate::memory::{FrameId, Generation, InstanceId};
use crate::runtime::{CompiledDebugger, CompiledState, Located, ProgramRead, ProgramWrite};
use crate::value::{Duration, Value};

#[cfg(test)]
use trust_runtime_services::live_values::PendingVarWrite;
use trust_runtime_services::live_values::{
    ForcedSnapshot, ForcedVar, ForcedVarTarget, LiveValueQueues, PendingVarTarget, RequestQueueFull,
};

use super::{DebugSnapshot, RuntimeEvent};

#[path = "control/recording.rs"]
mod recording;
pub(crate) use recording::numeric as numeric_control_value;
#[path = "control/parameters.rs"]
mod parameters;
#[path = "control/program_facts.rs"]
mod program_facts;
#[path = "control/signals.rs"]
mod signals;
#[path = "control/symbol_path.rs"]
mod symbol_path;
#[path = "control/transfer.rs"]
mod transfer;
pub(crate) use parameters::resolve_path as resolve_control_parameter_path;
pub(crate) use parameters::{ControlPidOutcome, ControlPidView, ControlPidWrite};
pub(crate) use program_facts::{
    parameter_writable as control_parameter_writable, program_tasks as control_program_tasks,
};
pub use recording::{IoRecordingResult, IoRecordingSpec};
pub(crate) use symbol_path::number_as as control_number_as;

include!("control/types.rs");
include!("control/api.rs");

#[cfg(test)]
mod tests;
