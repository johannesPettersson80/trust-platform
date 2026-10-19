//! The program of the resource loop: the installed one, or why a device has none
//! (specification 34, STATE-08).

use alloc::format;
use alloc::vec::Vec;

use smol_str::SmolStr;
use trust_runtime_core::error::RuntimeError;
use trust_runtime_core::memory::Generation;
use trust_runtime_core::ports::{MonotonicInstant, TimeSpan};
use trust_runtime_core::retain::RestartMode;
use trust_runtime_core::value::Duration;
use trust_runtime_core::vm::{CycleHooks, RetainApplyEvent};
use trust_runtime_core::watchdog::{FaultPolicy, WatchdogPolicy};

use super::cycle::{ResourceCycle, RetainSource, TaskDescriptor};
use super::hold::HoldHandle;
use crate::status::IoDriverStatus;

/// The program the loop runs, or the reason a device has none: its application region holds no
/// program, or one that the device refused.
///
/// Without a program the resource stays Stopped and has no boundary, so the loop asks for no
/// scan, fault, stop or restart; the dispatcher refuses `restart` with the reason before one
/// could be asked. A missing program answers as a program with nothing to run, and the scan and
/// the restart, which would run one, fail with the reason.
pub(super) enum Program<Y> {
    /// The installed program.
    Installed(Y),
    /// No program is installed, for this reason.
    Missing(SmolStr),
}

impl<Y> Program<Y> {
    /// Why no program is installed, when none is.
    pub(super) fn missing(&self) -> Option<&str> {
        match self {
            Self::Installed(_) => None,
            Self::Missing(reason) => Some(reason.as_str()),
        }
    }
}

/// The failure of a scan or a restart that would need the missing program, and of a program that
/// cannot be loaded in place (specification 34 STATE-08).
pub(super) fn no_program(reason: &str) -> RuntimeError {
    RuntimeError::ControlError(SmolStr::new(format!("no installed program: {reason}")))
}

impl<Y: ResourceCycle> ResourceCycle for Program<Y> {
    fn tasks(&self) -> Vec<TaskDescriptor> {
        match self {
            Self::Installed(cycle) => cycle.tasks(),
            Self::Missing(_) => Vec::new(),
        }
    }

    fn cycle_interval(&self) -> Duration {
        match self {
            Self::Installed(cycle) => cycle.cycle_interval(),
            Self::Missing(_) => Duration::ZERO,
        }
    }

    fn watchdog(&self) -> WatchdogPolicy {
        match self {
            Self::Installed(cycle) => cycle.watchdog(),
            Self::Missing(_) => WatchdogPolicy::default(),
        }
    }

    fn fault_policy(&self) -> FaultPolicy {
        match self {
            Self::Installed(cycle) => cycle.fault_policy(),
            Self::Missing(_) => FaultPolicy::Halt,
        }
    }

    fn held_time(&self) -> TimeSpan {
        match self {
            Self::Installed(cycle) => cycle.held_time(),
            // A missing program runs no scan, so no scan is held.
            Self::Missing(_) => TimeSpan::ZERO,
        }
    }

    fn run_scan(
        &mut self,
        now: Duration,
        deadline: Option<MonotonicInstant>,
        observer: &mut dyn CycleHooks,
    ) -> Result<(), RuntimeError> {
        match self {
            Self::Installed(cycle) => cycle.run_scan(now, deadline, observer),
            Self::Missing(reason) => Err(no_program(reason)),
        }
    }

    fn latch_fault(&mut self, fault: RuntimeError) {
        if let Self::Installed(cycle) = self {
            cycle.latch_fault(fault);
        }
    }

    fn apply_safe_outputs(&mut self) -> Result<(), RuntimeError> {
        match self {
            Self::Installed(cycle) => cycle.apply_safe_outputs(),
            Self::Missing(_) => Ok(()),
        }
    }

    fn save_retain(&mut self) -> Result<(), RuntimeError> {
        match self {
            Self::Installed(cycle) => cycle.save_retain(),
            Self::Missing(_) => Ok(()),
        }
    }

    fn restart(
        &mut self,
        mode: RestartMode,
        source: RetainSource,
    ) -> Result<Vec<RetainApplyEvent>, RuntimeError> {
        match self {
            Self::Installed(cycle) => cycle.restart(mode, source),
            Self::Missing(reason) => Err(no_program(reason)),
        }
    }

    fn reset_task_timing(&mut self, now: Duration) {
        if let Self::Installed(cycle) = self {
            cycle.reset_task_timing(now);
        }
    }

    fn generation(&self) -> Generation {
        match self {
            Self::Installed(cycle) => cycle.generation(),
            // A missing program has no state to be in a generation; none follows it.
            Self::Missing(_) => Generation::FIRST,
        }
    }

    fn io_drivers(&self) -> Vec<IoDriverStatus> {
        match self {
            Self::Installed(cycle) => cycle.io_drivers(),
            Self::Missing(_) => Vec::new(),
        }
    }

    fn attach_hold(&mut self, hold: HoldHandle) {
        if let Self::Installed(cycle) = self {
            cycle.attach_hold(hold);
        }
    }
}
