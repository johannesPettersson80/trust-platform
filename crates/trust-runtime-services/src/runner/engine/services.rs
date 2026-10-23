//! The engine's execution services on a device (specification 34, §5.2, §7.1 and TIME-03): the
//! scan deadline on the platform's monotonic clock, which the debugger holds of the scan move
//! forward (specification 11 §6.6; specification 34 STATE-07), and the statement hook that holds
//! the scan for the debugger (specification 13, "Safe Points").

use alloc::vec::Vec;
use core::cell::Cell;

use smol_str::SmolStr;
use trust_runtime_core::error::RuntimeError;
use trust_runtime_core::memory::VariableStorage;
use trust_runtime_core::ports::{MonotonicClock, MonotonicInstant, TimeSpan};
use trust_runtime_core::value::{self, DateTimeValue};
use trust_runtime_core::vm::{ExecutionServices, PreparedModule, StatementFrame};

use super::held_time::HeldTime;
use crate::debug::LineSources;
use crate::runner::debugger::{Boundary, StatementStart};
use crate::runner::hold::{HeldView, HoldHandle};
use crate::status::IoDriverStatus;

/// The admission refusal of `CURRENT_DT` without a wall clock, the engine's own text.
const NO_WALL_CLOCK: &str = "CURRENT_DT requires an admitted platform wall clock";

/// The engine's execution services on a device: the scan deadline, armed for each scan on the
/// platform's monotonic clock and moved forward by the debugger holds that run after it was armed,
/// the statement hook that holds the scan, and no wall clock.
///
/// A device has no wall clock until slice T6 gives it the platform's `WallClock`, so the
/// composition is logical-only: the engine refuses a program that calls `CURRENT_DT` when it
/// constructs the program (spec 34 §5.2), and the replies carry `null` for wall-clock times
/// (TARGET-06). It has no retain store until slice C5.
///
/// The statement hook asks the resource loop, which hands its pass to the cycle when it starts
/// ([`ResourceCycle::attach_hold`](crate::runner::ResourceCycle::attach_hold)), and finds a
/// statement start in the prepared module that the composition names
/// ([`EngineCycle::with_debugger`](crate::runner::engine::EngineCycle::with_debugger)), which also
/// gives it the program's line sources. Until both are in place the hook does nothing.
pub struct DeviceServices<C> {
    clock: C,
    deadline: Cell<Option<MonotonicInstant>>,
    holds: HeldTime,
    /// The holds that had run when the deadline was armed, so that only later holds move it.
    holds_at_arm: Cell<TimeSpan>,
    /// Whether the statement hook is live for the scan that runs: the platform enables debugging
    /// and the debugger can find statement starts. With it off, the hook returns after this check
    /// (specification 11 §6.9: debug hooks are side-effect-free when disabled).
    debugging: Cell<bool>,
    /// The prepared module whose `LINE_TABLE` names the statement starts (specification 12 §6.13).
    statements: Cell<Option<&'static PreparedModule>>,
    /// The module's `LINE_TABLE` and file labels, which place breakpoints and name files; none for
    /// a module that has no `LINE_TABLE`.
    sources: Cell<Option<&'static LineSources>>,
    /// The resource loop's pass for a held scan.
    hold: Cell<Option<HoldHandle>>,
    /// The health of the process I/O, which the cycle publishes after it reads the inputs while
    /// debugging is live, for a hold to report (the cycle is borrowed whole by the scan).
    drivers: Cell<Vec<IoDriverStatus>>,
    /// The platform's count of its allocator operations, which the engine reads around a scan
    /// (specification 34, "The allocation count"); `None` on a device that does not count.
    allocator_operations: Option<fn() -> u32>,
}

impl<C> DeviceServices<C> {
    /// Services on `clock`, with no deadline armed and no debugger attached.
    #[must_use]
    pub const fn new(clock: C) -> Self {
        Self {
            clock,
            deadline: Cell::new(None),
            holds: HeldTime::new(),
            holds_at_arm: Cell::new(TimeSpan::ZERO),
            debugging: Cell::new(false),
            statements: Cell::new(None),
            sources: Cell::new(None),
            hold: Cell::new(None),
            drivers: Cell::new(Vec::new()),
            allocator_operations: None,
        }
    }

    /// These services with the platform's count of its allocator operations, `operations`, in 32
    /// bits that wrap, which must allocate nothing.
    #[must_use]
    pub fn with_allocator_operations(mut self, operations: Option<fn() -> u32>) -> Self {
        self.allocator_operations = operations;
        self
    }

    /// Names the prepared module whose statement starts the hook finds, and the sources of its
    /// `LINE_TABLE`, when it has one.
    pub(super) fn attach_statements(
        &self,
        prepared: &'static PreparedModule,
        sources: Option<&'static LineSources>,
    ) {
        self.statements.set(Some(prepared));
        self.sources.set(sources);
    }

    /// The prepared module and the line sources that a debugger was attached with, or none when no
    /// debugger is attached.
    pub(super) fn debug_attachment(
        &self,
    ) -> Option<(&'static PreparedModule, Option<&'static LineSources>)> {
        self.statements
            .get()
            .map(|prepared| (prepared, self.sources.get()))
    }

    /// Gives the hook the resource loop's pass, replacing an earlier one.
    pub(super) fn attach_hold(&self, hold: HoldHandle) {
        self.hold.set(Some(hold));
    }

    /// The resource loop's pass, if the loop attached one.
    fn hold_port(&self) -> Option<HoldHandle> {
        let hold = self.hold.take();
        let port = hold.clone();
        self.hold.set(hold);
        port
    }

    /// Where the cycle publishes the health of the process I/O for a hold, while the hook is live.
    pub(super) fn driver_health_cell(&self) -> Option<&Cell<Vec<IoDriverStatus>>> {
        self.debugging.get().then_some(&self.drivers)
    }

    /// A scan begins: the hook is live for it when the module has a statement index to find
    /// statement starts in (a module without a `LINE_TABLE` cannot be debugged, and keeps its
    /// register tier), the loop's pass is attached and the platform enables debugging. The loop
    /// also forgets the ordinary stops of the last scan (specification 13, "Breakpoint
    /// evaluation, source resolution, and hook contract").
    pub(super) fn scan_begins(&self) {
        let live = self
            .statements
            .get()
            .is_some_and(PreparedModule::has_statement_index)
            && self
                .hold_port()
                .is_some_and(|hold| HoldHandle::scan_begins(&hold));
        self.debugging.set(live);
    }

    /// The scan ends: the hook is no longer live.
    pub(super) fn scan_ends(&self) {
        self.debugging.set(false);
    }
}

impl<C: MonotonicClock> DeviceServices<C> {
    /// Arms the scan deadline at `deadline` on the clock, or disarms it with `None`. The holds that
    /// run after this call move the deadline forward; the holds before it do not.
    pub(super) fn arm(&self, deadline: Option<MonotonicInstant>) {
        self.holds_at_arm
            .set(self.holds.held_at(|| self.clock.now()));
        self.deadline.set(deadline);
    }

    /// A debugger hold starts now (specification 11 §6.9; specification 34 STATE-07).
    pub fn hold_started(&self) {
        self.holds.start(self.clock.now());
    }

    /// The debugger hold in progress ends now. Its length moves the armed deadline forward by the
    /// same amount, as trust-runtime's `deadline_with_debug_pause` does (specification 11 §6.6).
    pub fn hold_ended(&self) {
        self.holds.end(self.clock.now());
    }

    /// The debugger holds that have run on the device's clock: the completed holds, and the hold in
    /// progress up to now (specification 11 §6.6).
    pub(super) fn held_time(&self) -> TimeSpan {
        self.holds.held_at(|| self.clock.now())
    }

    /// The statement hook while the debugger is live: the work of
    /// [`ExecutionServices::on_statement_with_frame_checked`] past its first check, kept out of line
    /// so that the check alone is inlined into the dispatch loop.
    #[inline(never)]
    fn at_statement(
        &self,
        pou_id: u32,
        code_offset: usize,
        depth: u32,
        storage: &VariableStorage,
        logical_time: value::Duration,
        frame: StatementFrame<'_>,
    ) -> Result<(), RuntimeError> {
        let Some(prepared) = self.statements.get() else {
            return Ok(());
        };
        let code_offset =
            u32::try_from(code_offset).map_err(|_| RuntimeError::InvalidExecutionState)?;
        let Some(span) = prepared.statement_start(pou_id, code_offset) else {
            return Ok(());
        };
        let Some(hold) = self.hold_port() else {
            return Ok(());
        };
        let at = StatementStart { depth, span, frame };
        if hold.at_statement(&at)? == Boundary::Run {
            return Ok(());
        }
        let view = HeldView {
            storage,
            frame,
            prepared,
            sources: self.sources.get(),
            logical_time,
            drivers: &self.drivers,
        };
        self.hold_started();
        let outcome = hold.hold(&at, &view);
        self.hold_ended();
        outcome
    }
}

impl<C: MonotonicClock> ExecutionServices for DeviceServices<C> {
    /// Whether the statement hook is live: the debugger needs the canonical stack path, which calls
    /// the hook at every instruction, instead of the register tier, which calls it at block starts.
    #[inline]
    fn debug_active(&self) -> bool {
        self.debugging.get()
    }

    /// The statement hook acts only while the debugger is attached.
    fn observes_statements(&self) -> bool {
        self.debugging.get()
    }

    /// The statement hook acts only where the module's statement index names a statement start
    /// (`at_statement`).
    fn hook_acts_at_statement_starts(&self) -> bool {
        true
    }

    /// The platform's count, read through the function the device gave.
    fn allocator_operations(&self) -> Option<u32> {
        self.allocator_operations.map(|operations| operations())
    }

    fn allocator_operations_reader(&self) -> Option<fn() -> u32> {
        self.allocator_operations
    }

    /// The statement hook (specification 13, "Safe Points"). At the first instruction of a
    /// statement, found in the module's `LINE_TABLE`, the resource loop decides whether the
    /// scan stops: at a breakpoint, a pending pause or a step's boundary. If it does, the scan
    /// waits here, in the hold pass of the loop, until the debugger releases it; the time it waits
    /// is held time (specification 11 §6.6).
    ///
    /// No borrow of the loop outlives a call of the pass, and none is held while the hook waits.
    ///
    /// The engine calls the hook at every statement start that the module's statement index names
    /// (`hook_acts_at_statement_starts`). With the debugger off it is one check, inlined into the
    /// engine's dispatch loop, which a chip runs from RAM, so a scan without a debugger makes no
    /// call into this code (specification 11 §6.9).
    #[allow(clippy::inline_always)]
    #[inline(always)]
    fn on_statement_with_frame_checked(
        &self,
        pou_id: u32,
        code_offset: usize,
        depth: u32,
        storage: &VariableStorage,
        logical_time: value::Duration,
        frame: StatementFrame<'_>,
    ) -> Result<(), RuntimeError> {
        if !self.debugging.get() {
            return Ok(());
        }
        self.at_statement(pou_id, code_offset, depth, storage, logical_time, frame)
    }

    /// Whether the clock has reached the armed deadline, moved forward by the holds that have run
    /// since it was armed. A hold in progress counts as held, so it is never an overrun. Never
    /// while none is armed.
    fn deadline_exceeded(&self) -> bool {
        let Some(deadline) = self.deadline.get() else {
            return false;
        };
        let now = self.clock.now();
        let held = self
            .holds
            .held_at(|| now)
            .saturating_sub(self.holds_at_arm.get());
        now >= deadline.saturating_add(held)
    }

    fn has_wall_clock(&self) -> bool {
        false
    }

    /// Never called: the engine refuses a program that calls `CURRENT_DT` before it runs.
    fn current_dt(&self) -> Result<DateTimeValue, RuntimeError> {
        Err(RuntimeError::ProfileUnsupported(SmolStr::new_static(
            NO_WALL_CLOCK,
        )))
    }
}
