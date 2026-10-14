//! The `parallel-pid-v1` initialization adapter (specification 35, C0 and C1; annex E6.4). Ported from `pidInitialization.ts`
//! (`pidInitializationAdapter`). The decision itself is `parallel_pid_contract` in `transfer.rs`, which the transfer needs
//! too, so this module names it for the PID initialization layer and adds nothing to it.

use super::transfer::{parallel_pid_contract, ParallelPidContract};
use crate::failure::Failure;
use crate::loop_definition::LoopDefinition;

/// The `parallel-pid-v1` contract of a loop: its integral and derivative states and its setpoint weights, which an Auto
/// commissioning of a PI or PID needs (`pidInitializationAdapter`).
///
/// # Errors
/// The loop has no `parallel-pid-v1` initialization, no PID law or no setpoint. Auto is then blocked, and the next action is
/// to choose a method (C0).
pub fn pid_initialization_adapter(
    loop_definition: &LoopDefinition,
) -> Result<ParallelPidContract<'_>, Failure> {
    parallel_pid_contract(loop_definition)
}

#[cfg(test)]
#[path = "pid_initialization_tests.rs"]
mod tests;
