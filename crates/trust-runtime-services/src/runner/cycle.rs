//! The installed program as the resource loop drives it: one scan at a time, its faults, its
//! outputs at a stop, its retained values, its restarts, its task timing and its I/O drivers'
//! health. [`EngineCycle`](crate::runner::engine::EngineCycle) implements [`ResourceCycle`] on
//! the shared engine with the platform's process I/O.

use alloc::vec::Vec;

use smol_str::SmolStr;
use trust_runtime_core::error::RuntimeError;
use trust_runtime_core::memory::Generation;
use trust_runtime_core::ports::{MonotonicInstant, TimeSpan};
use trust_runtime_core::retain::RestartMode;
use trust_runtime_core::value::Duration;
use trust_runtime_core::vm::{CycleHooks, PreparedModule, RetainApplyEvent};
use trust_runtime_core::watchdog::{FaultPolicy, WatchdogPolicy};

use super::hold::HoldHandle;
use crate::status::IoDriverStatus;

/// Where a warm restart finds the retained values.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum RetainSource {
    /// The values the program holds now: an automatic restart after a fault, which trust-runtime's
    /// scheduler makes in place.
    Memory,
    /// The durable retain checkpoint, as a new trust-runtime process starts: a `restart` request
    /// and power-on.
    Checkpoint,
}

/// One configured task of the installed resource, as events and metrics name it.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct TaskDescriptor {
    /// Task name.
    pub name: SmolStr,
    /// Task priority; 0 is the highest.
    pub priority: u32,
}

/// The tasks of resource `resource_index` of `prepared`, in declaration order, so that the task
/// index of a cycle phase selects its task; empty for a resource the module does not have.
#[must_use]
pub fn resource_tasks(prepared: &PreparedModule, resource_index: usize) -> Vec<TaskDescriptor> {
    let strings = prepared.strings();
    prepared
        .resources()
        .get(resource_index)
        .map(|resource| {
            resource
                .tasks
                .iter()
                .map(|task| TaskDescriptor {
                    name: usize::try_from(task.name_idx)
                        .ok()
                        .and_then(|index| strings.get(index))
                        .map(SmolStr::new)
                        .unwrap_or_default(),
                    priority: task.priority,
                })
                .collect()
        })
        .unwrap_or_default()
}

/// The installed program of one resource, as the resource loop runs it (specification 34,
/// STATE-08).
///
/// Times passed in are logical times, which the loop counts from the program's start on the
/// platform's monotonic clock; deadlines are readings of that clock. The loop itself restarts the
/// task timing after every restart and at power-on ([`Self::reset_task_timing`]).
pub trait ResourceCycle {
    /// The configured tasks of the installed resource, in declaration order: the task index of a
    /// cycle phase indexes this list ([`resource_tasks`] reads them from the prepared module).
    fn tasks(&self) -> Vec<TaskDescriptor>;

    /// The resource's cycle interval: the next scan is due this long after the previous one
    /// started, as trust-runtime's scheduler paces its cycles.
    fn cycle_interval(&self) -> Duration;

    /// The watchdog policy that supervises each scan.
    fn watchdog(&self) -> WatchdogPolicy;

    /// The fault policy that follows a failed scan.
    fn fault_policy(&self) -> FaultPolicy;

    /// The debugger holds that have run on this cycle since it started, on the monotonic clock. A
    /// hold still in progress counts up to now (specification 11 §6.6; specification 34 STATE-07).
    /// The loop takes the holds of a scan as the difference of two readings, as trust-runtime takes
    /// its pause time, so a hold never reads as an overrun. Every implementor says what it holds.
    fn held_time(&self) -> TimeSpan;

    /// Runs one scan of the installed program at logical time `now` with
    /// `RuntimeState::execute_cycle_with_hooks`.
    ///
    /// The implementation
    /// - arms the engine's execution deadline at `deadline`, when there is one, so the engine
    ///   ends a scan still running then with `ExecutionTimeout`, and disarms it after the scan;
    /// - passes the engine hooks that sample the inputs and publish the outputs at their phases
    ///   (`CycleHooks::publish_outputs`) and forward every other callback to `observer`, the
    ///   loop's record of the scan: `has_program_hooks`, `program_started`, `program_finished`
    ///   and `on_phase`. They answer `has_output_hooks` true, as `observer` does, so the engine
    ///   checks the deadline before it publishes and restores the outputs it staged when the
    ///   publication fails, as trust-runtime's hosted cycle does. One without I/O passes
    ///   `observer` itself.
    ///
    /// # Errors
    ///
    /// The [`RuntimeError`] of a scan that failed.
    fn run_scan(
        &mut self,
        now: Duration,
        deadline: Option<MonotonicInstant>,
        observer: &mut dyn CycleHooks,
    ) -> Result<(), RuntimeError>;

    /// Latches `fault` in the engine: no further scan runs and no normal outputs are committed
    /// until a restart (STATE-04).
    fn latch_fault(&mut self, fault: RuntimeError);

    /// Drives every output to its safe value, trust-runtime's `[[io.safe_state]]`: the stop
    /// output policy (STATE-02), applied on a stop and on a fault whose policy asks for it. Every
    /// driver is attempted even when one fails.
    ///
    /// # Errors
    ///
    /// The first failure among the drivers.
    fn apply_safe_outputs(&mut self) -> Result<(), RuntimeError>;

    /// Saves the retained values durably, as trust-runtime saves them when it stops.
    ///
    /// # Errors
    ///
    /// The [`RuntimeError`] of a save that did not complete; the previous checkpoint stays.
    fn save_retain(&mut self) -> Result<(), RuntimeError>;

    /// Advances a pending retained checkpoint by at most one persister step, between this cycle's
    /// scans and with the platform's per-step program budget (specification 34 §10).
    ///
    /// The resource loop calls it while it otherwise waits for the next release, never inside a
    /// scan, so an erase that outlasts the time before the next release only makes that scan late,
    /// which the loop's overrun accounting reports. A failed step latches a retain-store failure
    /// that the next scan fails with before physical outputs. The default does nothing: a
    /// composition without a checkpoint store.
    fn step_retain(&mut self) {}

    /// Starts the installed program again, `mode` cold or warm with the retained values of
    /// `source`, and returns the retained values it dropped or migrated. A failed restart leaves
    /// the program as it was.
    ///
    /// # Errors
    ///
    /// The [`RuntimeError`] of a restart that did not complete.
    fn restart(
        &mut self,
        mode: RestartMode,
        source: RetainSource,
    ) -> Result<Vec<RetainApplyEvent>, RuntimeError>;

    /// Restarts the task timing at logical time `now`, as `RuntimeState::reset_task_timing`
    /// does: every task counts its next activation from `now`, which becomes the program's
    /// current time.
    fn reset_task_timing(&mut self, now: Duration);

    /// The installation generation the program's state is in (specification 34 MEM-07 and
    /// UPDATE-04). Every restart that commits installs the next one, and a restart that fails
    /// changes none.
    ///
    /// A request that names an instance by a bare identity belongs to the generation the Live Values
    /// carry when the request is accepted, and the scan that applies it refuses it when the program is
    /// in another generation. The loop therefore reads this after every restart that commits, and when
    /// it takes a program, and gives it to the Live Values before the next cycle
    /// ([`set_generation`](crate::runner::EngineLiveValues::set_generation); STATE-08).
    fn generation(&self) -> Generation;

    /// The health of the program's I/O drivers, in `io.toml` order, which `status` and `health`
    /// report before the drivers the platform runs itself.
    fn io_drivers(&self) -> Vec<IoDriverStatus>;

    /// Gives the cycle the loop's pass for a scan that a debugger holds (specification 13, "Safe
    /// Points"; specification 34, STATE-07). The loop calls it once, when it starts.
    ///
    /// A cycle whose execution services stop at statement starts calls the pass from its statement
    /// hook, at the first instruction of a statement, and lends it the halted state. It may keep the
    /// handle for the rest of the boot. The default ignores it: a cycle that never holds a scan.
    fn attach_hold(&mut self, _hold: HoldHandle) {}
}
