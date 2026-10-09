//! Resource scheduling utilities and clocks.

#![allow(missing_docs)]

use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Arc, Condvar, Mutex};
use std::thread;

use indexmap::IndexMap;
use smol_str::SmolStr;

use crate::error::RuntimeError;
use crate::value::Duration;
use crate::value::Value;
use crate::Runtime;
use crate::RuntimeMetadata;

pub use trust_runtime_core::scheduler::ResourceState;

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

#[cfg(test)]
mod tests;

#[cfg(test)]
mod simulated_time_tests;
