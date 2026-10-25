//! The installed program as the resource loop drives it: one scan at a time, its faults, its
//! outputs at a stop, its retained values, its restarts, its task timing and its I/O drivers'
//! health. [`CompiledCycle`](crate::runner::CompiledCycle) implements [`ResourceCycle`] for a
//! compiled program image with the platform's process I/O (specification 44 §8.7).

use alloc::vec::Vec;

use smol_str::SmolStr;
use trust_runtime_core::cycle::CycleHooks;
use trust_runtime_core::deployment::HangAction;
use trust_runtime_core::error::RuntimeError;
use trust_runtime_core::io_address::IoAddress;
use trust_runtime_core::memory::Generation;
use trust_runtime_core::ports::{MonotonicInstant, TimeSpan};
use trust_runtime_core::retain::{RestartMode, RetainApplyEvent};
use trust_runtime_core::value::{Duration, Value};
use trust_runtime_core::watchdog::{FaultPolicy, WatchdogPolicy};

use super::hold::HoldHandle;
use crate::cycle_step::{StepOutcome, StepRefusal, StepRequest};
use crate::debug::LineSources;
use crate::metrics::ProgramMemory;
use crate::status::IoDriverStatus;

/// What the deployment configures for the installed program besides its code (specification
/// 34, §5.1 and TARGET-08).
#[derive(Debug, Clone)]
pub struct CycleSettings {
    /// The resource's cycle interval.
    pub cycle_interval: Duration,
    /// The cycle watchdog of `[runtime.watchdog]`.
    pub watchdog: WatchdogPolicy,
    /// `runtime.watchdog.hang_action`: whether a scan that did not return, which a device's
    /// hardware watchdog ends with a reset, leaves the program faulted at the next boot (`fault`) or
    /// runs it again (`abort`), as trust-runtime's resource stays faulted or its service manager
    /// restarts it (specification 11, hosted hang supervision; specification 34 TARGET-08).
    pub hang_action: HangAction,
    /// What follows a failed scan, `[runtime.fault]`.
    pub fault_policy: FaultPolicy,
    /// The safe value of each output, `io.toml` `[[io.safe_state]]`, in its order. Without any,
    /// a stop or a fault drives no output, as on trust-runtime.
    pub safe_outputs: Vec<(IoAddress, Value)>,
}

/// What a debug read sees of an installed compiled program (specification 44 §13.2): its memory,
/// held with the frames of its call stack while the debugger holds the scan, and the sources of
/// its statement table.
#[derive(Clone, Copy)]
pub struct CompiledView<'a> {
    /// The program's memory.
    pub memory: &'a dyn trust_runtime_core::native::memory::ProgramMemory,
    /// The program's statement table and file labels; none for a release image.
    pub sources: Option<&'a LineSources>,
    /// Whether the program is a release image, built without statement hooks, which the debugger
    /// refuses (specification 44 §13.2).
    pub release: bool,
}

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

/// The installed program of one resource, as the resource loop runs it (specification 34,
/// STATE-08).
///
/// Times passed in are logical times, which the loop counts from the program's start on the
/// platform's monotonic clock; deadlines are readings of that clock. The loop itself restarts the
/// task timing after every restart and at power-on ([`Self::reset_task_timing`]).
pub trait ResourceCycle {
    /// The configured tasks of the installed resource, in declaration order: the task index of a
    /// cycle phase indexes this list.
    fn tasks(&self) -> Vec<TaskDescriptor>;

    /// The program instance each entry of the program's image runs, in the entries' order, by
    /// which the program calls are recorded (specification 11, runtime metrics): the entry index
    /// of a `ProgramStart` phase indexes this list. A cycle that reports no program phases has
    /// none.
    fn programs(&self) -> Vec<SmolStr> {
        Vec::new()
    }

    /// The resource's cycle interval: the next scan is due this long after the previous one
    /// started, as trust-runtime's scheduler paces its cycles.
    fn cycle_interval(&self) -> Duration;

    /// The watchdog policy that supervises each scan.
    fn watchdog(&self) -> WatchdogPolicy;

    /// The fault policy that follows a failed scan.
    fn fault_policy(&self) -> FaultPolicy;

    /// What follows a scan that did not return ([`CycleSettings::hang_action`]); `fault`, the
    /// default of `[runtime.watchdog]`, for a cycle that has none.
    fn hang_action(&self) -> HangAction {
        HangAction::Fault
    }

    /// The debugger holds that have run on this cycle since it started, on the monotonic clock. A
    /// hold still in progress counts up to now (specification 11 §6.6; specification 34 STATE-07).
    /// The loop takes the holds of a scan as the difference of two readings, as trust-runtime takes
    /// its pause time, so a hold never reads as an overrun. Every implementor says what it holds.
    fn held_time(&self) -> TimeSpan;

    /// Runs one scan of the installed program at logical time `now`. The scan ends with
    /// `ExecutionTimeout` when it still runs at `deadline`, when there is one; its process images are
    /// read from and written to the platform's drivers at their phases, and every boundary is given
    /// to `observer`, the loop's record of the scan (specification 44 §8.7).
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

    /// The allocator operations the thread that runs the scans has made so far, each allocation,
    /// reallocation and free once, as the platform counts them in 32 bits that wrap; `None` when it
    /// does not (specification 34, "The allocation count"). Reading it allocates nothing.
    fn allocator_operations(&self) -> Option<u32> {
        None
    }

    /// The allocator operations the program made itself in the last scan, its calls into the
    /// services not counted; `None` when they were not counted.
    fn last_scan_allocations(&self) -> Option<u64> {
        None
    }

    /// The memory figure of the installed program, and whether the cycle keeps its retained values
    /// (specification 44 §10.4), which `status` reports; `None` when the cycle does not know it.
    fn program_memory(&self) -> Option<ProgramMemory> {
        None
    }

    /// Latches `fault` in the program: no further scan runs and no normal outputs are committed
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

    /// Restarts the task timing at logical time `now`: every task counts its next activation from
    /// `now`, which becomes the program's current time. Before the program's first scan this
    /// registers the tasks, and an event input (`SINGLE`) that is TRUE then makes no rising edge;
    /// after it, the event inputs' last samples are cleared, so that an input held TRUE through a
    /// restart is a rising edge at the first scan after it (specification 10 §4.1.1).
    fn reset_task_timing(&mut self, now: Duration);

    /// The installation generation the program's state is in (specification 34 MEM-07 and
    /// UPDATE-04). Every restart that commits installs the next one, and a restart that fails
    /// changes none.
    ///
    /// A request that names an instance by a bare identity belongs to the generation the Live Values
    /// carry when the request is accepted, and the scan that applies it refuses it when the program is
    /// in another generation. The loop therefore reads this after every restart that commits, and when
    /// it takes a program, and gives it to the Live Values before the next cycle (STATE-08).
    fn generation(&self) -> Generation;

    /// The health of the program's I/O drivers, in `io.toml` order, which `status` and `health`
    /// report before the drivers the platform runs itself.
    fn io_drivers(&self) -> Vec<IoDriverStatus>;

    /// Runs one stepped scan of the stopped program (`cycle.step`, specification 11 §6.8.1): the
    /// restart the request asks for, at logical time `restart_at`; the request's inputs; one scan at
    /// the request's time, its due tasks by priority and then its programs without a task; and the
    /// request's watches. The scan neither reads nor drives the drivers, captures no retained value
    /// and is seen by no observer; its outputs are published into the output image only. Its error is
    /// in the outcome and is not latched. `deadline` bounds the scan as the watchdog bounds every
    /// scan.
    ///
    /// # Errors
    ///
    /// The [`StepRefusal`] of a name that names no single variable, before anything changes; of a
    /// restart that failed; of a value that its variable or address does not take; or of a watched
    /// value that cannot be read.
    fn step(
        &mut self,
        request: &StepRequest,
        restart_at: Duration,
        deadline: Option<MonotonicInstant>,
    ) -> Result<StepOutcome, StepRefusal>;

    /// Gives the cycle the loop's pass for a scan that a debugger holds (specification 13, "Safe
    /// Points"; specification 34, STATE-07). The loop calls it once, when it starts.
    ///
    /// A cycle whose program stops at statement starts calls the pass from its statement hook and
    /// lends it the halted state. It may keep the handle for the rest of the boot. The default
    /// ignores it: a cycle that never holds a scan.
    fn attach_hold(&mut self, _hold: HoldHandle) {}

    /// The compiled program as a debug read sees it between scans, or none for a cycle that runs
    /// no compiled program or has no debugger. The default has none.
    fn compiled_view(&self) -> Option<CompiledView<'_>> {
        None
    }
}
