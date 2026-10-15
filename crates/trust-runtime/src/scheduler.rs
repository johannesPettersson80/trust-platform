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

use command_queue::{command_channel, try_queue};
pub use command_queue::{CommandSendError, ReloadClaim, RESOURCE_COMMAND_CAPACITY};

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
