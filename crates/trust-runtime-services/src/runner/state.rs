//! What the resource loop keeps between passes and the dispatcher reaches through shared
//! references: the resource state with its fault, its pending restart and its queued commands,
//! and the metrics and events it records.
//!
//! Everything is single-owner. The cells take the place of trust-runtime's locks and its command
//! queue; a record that is in use when it is read is reported unavailable, as a poisoned lock is
//! on the host.

use alloc::collections::VecDeque;
use core::cell::{Cell, RefCell};

use trust_runtime_core::error::RuntimeError;
use trust_runtime_core::ports::{MonotonicClock, MonotonicInstant};
use trust_runtime_core::retain::RestartMode;
use trust_runtime_core::scheduler::ResourceState;

use crate::events::{store_event, RuntimeEvent, EVENT_STORE_CAPACITY};
use crate::metrics::{RuntimeMetrics, RuntimeMetricsSnapshot};
use crate::resource::commands::{
    running_command_effect, CommandClass, CommandSendError, RunningCommandEffect,
    RESOURCE_COMMAND_CAPACITY,
};

/// The resource state machine: Running, Paused, Stopped or Faulted, the fault that stopped the
/// resource, and the restart and the commands that wait for the loop.
pub(super) struct ResourceCell {
    state: Cell<ResourceState>,
    last_error: Cell<Option<RuntimeError>>,
    pending_restart: Cell<Option<RestartMode>>,
    /// Pause and resume commands that wait for the program's next boundary, as trust-runtime's
    /// bounded command queue holds them for its scan thread: how many there are, and the effect
    /// of the last pause or resume among them, which decides the state.
    queued: Cell<usize>,
    last_command: Cell<Option<RunningCommandEffect>>,
    /// The program has ended, as trust-runtime's process ends at a stop: the queue is closed, and
    /// a restart starts the program as a new process would.
    ended: Cell<bool>,
}

impl ResourceCell {
    /// A running resource without a fault.
    pub(super) fn running() -> Self {
        Self {
            state: Cell::new(ResourceState::Running),
            last_error: Cell::new(None),
            pending_restart: Cell::new(None),
            queued: Cell::new(0),
            last_command: Cell::new(None),
            ended: Cell::new(false),
        }
    }

    pub(super) fn state(&self) -> ResourceState {
        self.state.get()
    }

    pub(super) fn set_state(&self, state: ResourceState) {
        self.state.set(state);
    }

    pub(super) fn last_error(&self) -> Option<RuntimeError> {
        let error = self.last_error.take();
        let copy = error.clone();
        self.last_error.set(error);
        copy
    }

    pub(super) fn set_last_error(&self, error: Option<RuntimeError>) {
        self.last_error.set(error);
    }

    pub(super) fn update_pending_restart(&self, update: impl FnOnce(&mut Option<RestartMode>)) {
        let mut pending = self.pending_restart.get();
        update(&mut pending);
        self.pending_restart.set(pending);
    }

    pub(super) fn take_pending_restart(&self) -> Option<RestartMode> {
        self.pending_restart.take()
    }

    /// Queues a pause or resume for the program's next boundary, where trust-runtime's scan
    /// thread would take it at its next pass. The shared running-command rule decides its
    /// effect. A program that has ended takes no command, as a scan thread that has ended, and a
    /// full queue refuses one.
    pub(super) fn command(&self, class: CommandClass) -> Result<(), CommandSendError> {
        if self.ended.get() {
            return Err(CommandSendError::Closed);
        }
        let queued = self.queued.get();
        if queued >= RESOURCE_COMMAND_CAPACITY {
            return Err(CommandSendError::Full);
        }
        self.queued.set(queued + 1);
        let effect = running_command_effect(class);
        if effect != RunningCommandEffect::Apply {
            self.last_command.set(Some(effect));
        }
        Ok(())
    }

    /// Takes every queued command and returns the effect of the last pause or resume among them,
    /// the state that trust-runtime's scan thread leaves when it applies them in order.
    pub(super) fn take_commands(&self) -> Option<RunningCommandEffect> {
        self.queued.set(0);
        self.last_command.take()
    }

    pub(super) fn ended(&self) -> bool {
        self.ended.get()
    }

    /// The program has ended: the queued commands and a pending restart are dropped, as they are
    /// when trust-runtime's scan thread ends.
    pub(super) fn end(&self) {
        self.ended.set(true);
        self.queued.set(0);
        self.last_command.set(None);
        self.pending_restart.set(None);
    }

    /// The program runs again after it started as a new process would.
    pub(super) fn begin(&self) {
        self.ended.set(false);
        self.last_error.set(None);
        self.state.set(ResourceState::Running);
    }
}

/// The clock of the program's uptime: the platform's monotonic clock read from the program's
/// start, as a new trust-runtime process measures its uptime from its own start.
#[derive(Clone)]
pub(super) struct ProgramClock<C> {
    clock: C,
    start: MonotonicInstant,
}

impl<C: MonotonicClock> MonotonicClock for ProgramClock<C> {
    fn now(&self) -> MonotonicInstant {
        MonotonicInstant::from_origin(self.clock.now().saturating_since(self.start).as_duration())
    }
}

/// The metrics and the event store of the program.
pub(super) struct Records<C> {
    metrics: RefCell<RuntimeMetrics<ProgramClock<C>>>,
    events: RefCell<VecDeque<RuntimeEvent>>,
}

impl<C: MonotonicClock> Records<C> {
    /// Empty records of a program that started at `start` on `clock`.
    ///
    /// The event store takes its whole bound at once, so a heap that is too small for it fails
    /// here, at start, never in the middle of a scan (specification 34, ARCH-05).
    pub(super) fn new(clock: C, start: MonotonicInstant) -> Self {
        Self {
            metrics: RefCell::new(RuntimeMetrics::with_clock(ProgramClock { clock, start })),
            events: RefCell::new(VecDeque::with_capacity(EVENT_STORE_CAPACITY + 1)),
        }
    }

    /// Empties the records of a program that starts again at `start` as a new trust-runtime
    /// process: no events and new metrics, whose uptime counts from `start`. The event store
    /// keeps its memory.
    pub(super) fn restart(&self, clock: C, start: MonotonicInstant) {
        if let Ok(mut metrics) = self.metrics.try_borrow_mut() {
            metrics.restart(ProgramClock { clock, start });
        }
        if let Ok(mut events) = self.events.try_borrow_mut() {
            events.clear();
        }
    }

    /// Stores `event` as the newest event, within the store's bound.
    pub(super) fn push_event(&self, event: RuntimeEvent) {
        if let Ok(mut events) = self.events.try_borrow_mut() {
            store_event(&mut events, event);
        }
    }

    /// Records a measurement in the metrics.
    pub(super) fn record(&self, record: impl FnOnce(&mut RuntimeMetrics<ProgramClock<C>>)) {
        if let Ok(mut metrics) = self.metrics.try_borrow_mut() {
            record(&mut metrics);
        }
    }

    pub(super) fn metrics_snapshot(&self) -> Option<RuntimeMetricsSnapshot> {
        let metrics = self.metrics.try_borrow().ok()?;
        Some(metrics.snapshot())
    }

    pub(super) fn read_events<R>(
        &self,
        read: impl FnOnce(&VecDeque<RuntimeEvent>) -> R,
    ) -> Option<R> {
        let events = self.events.try_borrow().ok()?;
        Some(read(&events))
    }
}
