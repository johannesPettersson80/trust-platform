//! Resource scheduling utilities and clocks.

#![allow(missing_docs)]

use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Arc, Condvar, Mutex};
use std::thread;

use indexmap::IndexMap;
use smol_str::SmolStr;

use crate::debug::DebugControl;
use crate::error::RuntimeError;
use crate::value::Duration;
use crate::value::Value;
use crate::Runtime;
use crate::RuntimeMetadata;

pub use trust_runtime_core::scheduler::ResourceState;

use command_queue::{command_channel, try_queue};
pub use command_queue::{CommandSendError, ReloadClaim, RESOURCE_COMMAND_CAPACITY};
// The supervisor's decisions are shared runtime code (spec 34 ARCH-11); the threads, waits and
// restarts below call them.
use trust_runtime_services::resource::commands::{
    running_command_effect, CommandClass, RunningCommandEffect,
};
use trust_runtime_services::resource::supervisor::{
    automatic_restart_backoff, cycle_error_action, stop_outcome, watchdog_overrun_action,
    AutomaticRestartLimiter, CycleErrorAction, OverrunAction,
};

include!("scheduler/clock.rs");
include!("scheduler/model.rs");
include!("scheduler/runner_api.rs");
include!("scheduler/runner_loop.rs");
include!("scheduler/handle_shared.rs");

mod command_queue;
mod faulted;
mod supervisor;

#[cfg(test)]
mod tests;

#[cfg(test)]
mod policy_update_tests;
