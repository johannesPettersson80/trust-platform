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
use crate::CompiledRuntime;

pub use trust_runtime_core::scheduler::ResourceState;

use command_queue::{command_channel, try_queue};

/// The stack of a resource's scan thread: 2 MiB, whatever `RUST_MIN_STACK` says, so that a compiled
/// program's task stack of 1 MiB fits within it (specification 44 §8.5, NX-15).
const SCAN_THREAD_STACK_BYTES: usize = 2 << 20;
pub use command_queue::{CommandSendError, ReloadClaim, RESOURCE_COMMAND_CAPACITY};
// The supervisor's decisions are shared runtime code (spec 34 ARCH-11); the threads, waits and
// restarts below call them.
// The step's request, outcome and refusal travel in `ResourceCommand::StepCycle`, so a crate that
// serves resource commands names them from here.
pub use trust_runtime_services::cycle_step::{StepOutcome, StepRefusal, StepRequest};
use trust_runtime_services::resource::commands::{
    running_command_effect, CommandClass, RunningCommandEffect,
};
use trust_runtime_services::resource::supervisor::{
    automatic_restart_backoff, cycle_error_action, stop_outcome, watchdog_overrun_action,
    AutomaticRestartLimiter, CycleErrorAction, OverrunAction,
};

mod cycle_timing;
mod simulation_clock;

pub use self::simulation_clock::{
    simulation_warning, time_scale_not_whole_message, validate_time_scale, AchievedSpeedWindow,
    SimulationSpeed, SIMULATION_TIME_SCALE_MAX, SIMULATION_TIME_SCALE_MIN,
};

use self::cycle_timing::CycleTiming;

include!("scheduler/clock.rs");
include!("scheduler/model.rs");
include!("scheduler/runner_api.rs");
include!("scheduler/runner_loop.rs");
include!("scheduler/handle_shared.rs");

mod command_queue;
mod faulted;
mod stepped;
mod stopped;
mod stopping;
mod supervisor;

#[cfg(test)]
mod tests;

#[cfg(test)]
mod policy_update_tests;

#[cfg(test)]
mod simulated_time_tests;
