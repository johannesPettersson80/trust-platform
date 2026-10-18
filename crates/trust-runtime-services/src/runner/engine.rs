//! The installed program on the shared engine (specification 34, ARCH-11 and STATE-08): the
//! [`ResourceCycle`] of every device without an operating system, over the engine's
//! `RuntimeState`, and what the platform adapter supplies to it.
//!
//! - [`ProcessIo`]: the drivers of the process image. The cycle reads the inputs before the
//!   engine samples them, drives the outputs at the engine's publication point and the safe
//!   outputs of a stop or a fault, and reports the drivers' health in `status`.
//! - [`DeviceServices`]: the engine's execution services, with the scan deadline on the
//!   platform's monotonic clock and no wall clock until slice T6.
//! - [`device_limits`]: the admission limits of the device's application region and heap.
//!
//! trust-runtime on a computer runs the same engine through its own hosted cycle
//! (`runtime/installed_cycle.rs`); this cycle acts at the same phases and keeps the same output
//! and safe-state rules.
//!
//! The Live Values of a device's cycle (specification 11, "Live Values"; specification 34 §4.3.1
//! and §8).
//!
//! A device has one control surface, so it owns one set of [`LiveValueQueues`] and one published
//! I/O snapshot: [`EngineLiveValues`]. The platform's `IoQueuePort` and `VariableQueuePort` answer
//! the `io.*`, `set` and `var.*` requests from it, and the runner applies the queues through
//! [`crate::live_values::apply`] at the engine's phases and publishes the snapshot after each
//! scan, at the same points as trust-runtime's hosted cycle.
//!
//! The state is single-owner and holds no lock: a device's runner and its platform are one thread,
//! so a shared [`Rc`](alloc::rc::Rc) takes the place of trust-runtime's `DebugControl` lock. A
//! refused write is reported through the runner's record store as a `WriteRejected` event, as
//! trust-runtime reports it.

mod limits;
mod process_io;
mod services;

use alloc::string::ToString;
use alloc::vec::Vec;
use core::cell::{Cell, RefCell};

use smol_str::SmolStr;
use trust_runtime_core::error::RuntimeError;
use trust_runtime_core::io_address::IoAddress;
use trust_runtime_core::io_image::PreparedIoBinding;
use trust_runtime_core::memory::{Generation, InstanceId, IoArea};
use trust_runtime_core::ports::{MonotonicClock, MonotonicInstant};
use trust_runtime_core::retain::{RestartMode, RetainSnapshot};
use trust_runtime_core::value::{Duration, Value};
use trust_runtime_core::vm::{
    CycleAccess, CycleHooks, CyclePhase, PreparedModule, RetainApplyEvent, RuntimeState,
};
use trust_runtime_core::watchdog::{FaultPolicy, WatchdogPolicy};

pub use limits::{device_limits, DeviceMemory};
pub use process_io::ProcessIo;
pub use services::DeviceServices;

use super::cycle::{resource_tasks, ResourceCycle, RetainSource, TaskDescriptor};
use super::retain::{RetainConfig, RetainStore};
use super::state::Records;
use crate::events::RuntimeEvent;
use crate::live_values::apply::{self, WriteSink};
use crate::live_values::io_requests::{IoQueuePort, IoSnapshotRead};
use crate::live_values::io_snapshot::{
    value_type_name, IoSnapshot, IoSnapshotEntry, IoSnapshotValue,
};
use crate::live_values::queues::{
    ForcedSnapshot, ForcedVarTarget, LiveValueQueues, PendingVarTarget,
};
use crate::live_values::retained_names::{self, RetainedNames};
use crate::live_values::var_requests::VariableQueuePort;
use crate::live_values::RequestQueueFull;
use crate::status::IoDriverStatus;

/// The only resource a source-free composition admits (spec 34 §5.2).
const RESOURCE_INDEX: usize = 0;

/// What the deployment configures for the installed program besides its code (specification
/// 34, §5.1 and TARGET-08).
#[derive(Debug, Clone)]
pub struct CycleSettings {
    /// The resource's cycle interval.
    pub cycle_interval: Duration,
    /// The cycle watchdog of `[runtime.watchdog]`.
    pub watchdog: WatchdogPolicy,
    /// What follows a failed scan, `[runtime.fault]`.
    pub fault_policy: FaultPolicy,
    /// The safe value of each output, `io.toml` `[[io.safe_state]]`, in its order. Without any,
    /// a stop or a fault drives no output, as on trust-runtime.
    pub safe_outputs: Vec<(IoAddress, Value)>,
}

/// The installed program of a device: resource 0 of a prepared module on the shared engine,
/// with the platform's process I/O.
///
/// `'a` is the life of the prepared module and the execution services, which the engine state
/// borrows; a firmware keeps both for the rest of the boot (`'static`). `C` is the platform's
/// monotonic clock, `I` its process I/O.
///
/// A cycle built with [`Self::with_retain`] also holds the chip's checkpoint store
/// (`RetainStore`): it captures the retained values at the pre-output-sample checkpoint, the
/// resource loop advances a pending checkpoint between scans, a stop completes it, and a warm
/// restart from the checkpoint restores it. A cycle built with [`Self::start`] has no store: a stop
/// saves nothing, and a warm restart keeps the retained values the device holds in its memory.
pub struct EngineCycle<'a, C, I> {
    state: RuntimeState<'a>,
    prepared: &'a PreparedModule,
    services: &'a DeviceServices<C>,
    io: I,
    settings: CycleSettings,
    retain: Option<RetainStore>,
}

impl<'a, C: MonotonicClock, I: ProcessIo> EngineCycle<'a, C, I> {
    /// Starts the program of `prepared` as power-on starts it: the engine constructs resource 0
    /// with `services`, which is a cold start. The resource loop then starts its task timing.
    ///
    /// # Errors
    ///
    /// The [`RuntimeError`] of a program the device cannot start, for example one that calls
    /// `CURRENT_DT` on a device without a wall clock; the device then serves the link without a
    /// program ([`Runner::stopped`](crate::runner::Runner::stopped)).
    pub fn start(
        prepared: &'a PreparedModule,
        services: &'a DeviceServices<C>,
        io: I,
        settings: CycleSettings,
    ) -> Result<Self, RuntimeError> {
        Self::start_with(prepared, services, io, settings, None)
    }

    /// Starts the program as [`start`](Self::start) does, with the checkpoint store of `retain`
    /// (specification 34 §10): the persister opens the log over the platform's flash, so a control
    /// flash that cannot be read fails the start.
    ///
    /// # Errors
    ///
    /// The [`RuntimeError`] of [`start`](Self::start), or of a checkpoint log that cannot be
    /// opened.
    pub fn with_retain(
        prepared: &'a PreparedModule,
        services: &'a DeviceServices<C>,
        io: I,
        settings: CycleSettings,
        retain: RetainConfig,
    ) -> Result<Self, RuntimeError> {
        Self::start_with(prepared, services, io, settings, Some(retain))
    }

    /// The program `prepared` started cold on the engine, with an optional checkpoint store.
    fn start_with(
        prepared: &'a PreparedModule,
        services: &'a DeviceServices<C>,
        io: I,
        settings: CycleSettings,
        retain: Option<RetainConfig>,
    ) -> Result<Self, RuntimeError> {
        let state = prepared.instantiate_with_services(RESOURCE_INDEX, services)?;
        Ok(Self {
            state,
            prepared,
            services,
            io,
            settings,
            retain: retain.map(RetainStore::open).transpose()?,
        })
    }

    /// The installed program's engine state, for the tests that read the program's values.
    #[cfg(test)]
    pub(crate) fn state(&self) -> &RuntimeState<'a> {
        &self.state
    }
}

impl<C: MonotonicClock, I: ProcessIo> ResourceCycle for EngineCycle<'_, C, I> {
    fn tasks(&self) -> Vec<TaskDescriptor> {
        resource_tasks(self.prepared, RESOURCE_INDEX)
    }

    fn cycle_interval(&self) -> Duration {
        self.settings.cycle_interval
    }

    fn watchdog(&self) -> WatchdogPolicy {
        self.settings.watchdog
    }

    fn fault_policy(&self) -> FaultPolicy {
        self.settings.fault_policy
    }

    /// The deadline is armed for this scan only, so a stop's or a fault's safe outputs are
    /// never refused by an expired one.
    fn run_scan(
        &mut self,
        now: Duration,
        deadline: Option<MonotonicInstant>,
        observer: &mut dyn CycleHooks,
    ) -> Result<(), RuntimeError> {
        let mut hooks = ScanHooks {
            io: &mut self.io,
            retain: self.retain.as_mut(),
            observer,
        };
        self.services.arm(deadline);
        let result = self.state.execute_cycle_with_hooks(now, &mut hooks);
        self.services.arm(None);
        result
    }

    fn latch_fault(&mut self, fault: RuntimeError) {
        self.state.latch_fault(fault);
    }

    /// As trust-runtime applies `[[io.safe_state]]`: each safe value is written into the
    /// engine's output image through its checked image access, and the drivers then drive that
    /// image.
    fn apply_safe_outputs(&mut self) -> Result<(), RuntimeError> {
        if self.settings.safe_outputs.is_empty() {
            return Ok(());
        }
        for (address, value) in &self.settings.safe_outputs {
            self.state.write_image(address, value.clone())?;
        }
        self.io.write_safe_outputs(self.state.outputs())
    }

    /// Captures the values the program holds and completes the pending checkpoint and the newest
    /// image before the resource reports Stopped (specification 11 §6.7); a device without a
    /// checkpoint store saves nothing.
    fn save_retain(&mut self) -> Result<(), RuntimeError> {
        if let Some(retain) = self.retain.as_mut() {
            retain.save(&mut self.state)?;
        }
        Ok(())
    }

    /// Advances a pending checkpoint by at most one persister step, between scans (specification
    /// 34 §10); a device without a checkpoint store does nothing.
    fn step_retain(&mut self) {
        if let Some(retain) = self.retain.as_mut() {
            retain.step();
        }
    }

    /// A warm restart from the checkpoint decodes the newest committed checkpoint with the shared
    /// codec and applies it by name (specification 34 §10, specification 11 §6.7); a device
    /// without a checkpoint store loads the empty checkpoint, as trust-runtime loads it without
    /// one, and keeps the RETAIN variables the program holds in memory: after a stop, the values
    /// they had when it stopped (IEC 61131-3 §6.5.6.1).
    fn restart(
        &mut self,
        mode: RestartMode,
        source: RetainSource,
    ) -> Result<Vec<RetainApplyEvent>, RuntimeError> {
        match (mode, source) {
            (RestartMode::Warm, RetainSource::Checkpoint) => {
                let snapshot = match self.retain.as_mut() {
                    Some(retain) => retain.recover()?,
                    None => RetainSnapshot::default(),
                };
                self.state
                    .restart_with_retain_snapshot(RestartMode::Warm, &snapshot)
            }
            (mode, RetainSource::Memory | RetainSource::Checkpoint) => {
                self.state.restart(mode)?;
                Ok(Vec::new())
            }
        }
    }

    fn reset_task_timing(&mut self, now: Duration) {
        self.state.reset_task_timing(now);
    }

    fn io_drivers(&self) -> Vec<IoDriverStatus> {
        self.io.driver_health()
    }
}

/// The engine hooks of one scan: the process I/O at its phases, the retained capture at the
/// pre-output-sample checkpoint, and every callback to the loop's observer.
///
/// They keep the engine's default `has_output_hooks`, true, so the engine checks the scan's
/// deadline before it publishes the outputs and restores the image it staged when the drivers
/// fail, as trust-runtime's hosted cycle does.
struct ScanHooks<'h, 'o, I> {
    io: &'h mut I,
    retain: Option<&'h mut RetainStore>,
    observer: &'h mut (dyn CycleHooks + 'o),
}

impl<I: ProcessIo> CycleHooks for ScanHooks<'_, '_, I> {
    fn has_program_hooks(&self) -> bool {
        self.observer.has_program_hooks()
    }

    fn program_started(&mut self, name: &SmolStr) {
        self.observer.program_started(name);
    }

    fn program_finished(&mut self, name: &SmolStr) {
        self.observer.program_finished(name);
    }

    /// The inputs are read at `BeforeInputSample`, where trust-runtime's drivers fill the input
    /// image, after the observer has seen the phase; the retained values are captured and offered
    /// at `BeforeOutputSample`, where trust-runtime saves them (specification 11 §6.7). A retain
    /// failure there fails the scan before its outputs are published.
    fn on_phase(
        &mut self,
        phase: CyclePhase,
        access: &mut CycleAccess<'_, '_>,
    ) -> Result<(), RuntimeError> {
        self.observer.on_phase(phase, access)?;
        match phase {
            CyclePhase::BeforeInputSample => self.io.read_inputs(access.inputs_mut()?)?,
            CyclePhase::BeforeOutputSample => {
                if let Some(retain) = self.retain.as_mut() {
                    retain.capture(access)?;
                }
            }
            _ => {}
        }
        Ok(())
    }

    fn publish_outputs(&mut self, access: &CycleAccess<'_, '_>) -> Result<(), RuntimeError> {
        self.observer.publish_outputs(access)?;
        self.io.write_outputs(access.outputs())
    }
}

/// The Live Values of one device: the pending writes and active forces the platform queues, and
/// the I/O snapshot the runner publishes.
///
/// The composing platform holds it behind an [`Rc`](alloc::rc::Rc) shared with the runner. It is
/// single-owner, so it holds no lock; a chip keeps it for the rest of the boot.
pub struct EngineLiveValues {
    /// The pending writes and active forces the control requests queue.
    queues: RefCell<LiveValueQueues<()>>,
    /// The installation generation an instance-targeted request is resolved in.
    generation: Cell<Generation>,
    /// The retained labels of the installed program.
    names: RetainedNames,
    /// The admitted process-image bindings the snapshot renders, in declaration order.
    bindings: Vec<PreparedIoBinding>,
    /// The declared name of each binding, parallel to `bindings`.
    binding_names: Vec<Option<SmolStr>>,
    /// The published snapshot, once the first scan has published one.
    published: RefCell<Option<IoSnapshot>>,
}

impl EngineLiveValues {
    /// Empty Live Values of a platform that has no installed program yet: no bindings, no
    /// retained labels and nothing queued.
    #[must_use]
    pub fn empty() -> Self {
        Self {
            queues: RefCell::new(LiveValueQueues::new()),
            generation: Cell::new(Generation::FIRST),
            names: RetainedNames::default(),
            bindings: Vec::new(),
            binding_names: Vec::new(),
            published: RefCell::new(None),
        }
    }

    /// The Live Values of `prepared`: its retained labels and the process-image bindings the
    /// snapshot renders, with nothing queued.
    ///
    /// `binding_names` holds each binding's declared name, parallel to `prepared.io_bindings()`,
    /// so the snapshot renders the same `name` as trust-runtime's. A platform that does not name
    /// its bindings passes `None` for every entry.
    ///
    /// # Errors
    ///
    /// [`RuntimeError::InvalidExecutionState`] when an admitted index is out of range or
    /// `binding_names` is not parallel to the admitted bindings, and
    /// [`RuntimeError::InvalidConfig`] when two retained declarations project to one label.
    pub fn for_prepared(
        prepared: &PreparedModule,
        binding_names: Vec<Option<SmolStr>>,
    ) -> Result<Self, RuntimeError> {
        let names = retained_names::bind_retained_declarations(prepared)?;
        let bindings = prepared.io_bindings().to_vec();
        if binding_names.len() != bindings.len() {
            return Err(RuntimeError::InvalidExecutionState);
        }
        Ok(Self {
            queues: RefCell::new(LiveValueQueues::new()),
            generation: Cell::new(Generation::FIRST),
            names,
            bindings,
            binding_names,
            published: RefCell::new(None),
        })
    }

    /// Clear every queued write and active force at a terminating boundary: safe state, a fault
    /// and a restart (specification 11 §6.9.2). The published snapshot stays, as trust-runtime
    /// keeps it.
    pub fn clear(&self) {
        self.queues.borrow_mut().clear();
    }

    /// The published I/O snapshot of the last completed scan, updated in place.
    fn publish(&self, access: &mut CycleAccess<'_, '_>, scan: u64) -> Result<(), RuntimeError> {
        let needs_build = self.published.borrow().is_none();
        if needs_build {
            let snapshot = self.build_snapshot();
            *self.published.borrow_mut() = Some(snapshot);
        }
        let mut published = self.published.borrow_mut();
        let Some(snapshot) = published.as_mut() else {
            return Err(RuntimeError::InvalidExecutionState);
        };
        snapshot.scan = Some(scan);
        snapshot.forced.clear();
        for (address, _) in &self.queues.borrow().forced_io {
            snapshot.forced.push(address.clone());
        }
        // The entries were laid out in binding order, so the same order updates their values
        // without changing the snapshot's shape or allocating.
        let mut inputs = 0;
        let mut outputs = 0;
        let mut memory = 0;
        for binding in &self.bindings {
            let value = match access.read_image(&binding.address) {
                Ok(raw) => match binding.codec.decode(raw) {
                    Ok(value) => IoSnapshotValue::Value(value),
                    Err(error) => IoSnapshotValue::Error(error.to_string()),
                },
                Err(error) if apply::is_cycle_operational_error(&error) => return Err(error),
                Err(error) => IoSnapshotValue::Error(error.to_string()),
            };
            let entry = match binding.address.area {
                IoArea::Input => {
                    let entry = snapshot.inputs.get_mut(inputs);
                    inputs += 1;
                    entry
                }
                IoArea::Output => {
                    let entry = snapshot.outputs.get_mut(outputs);
                    outputs += 1;
                    entry
                }
                IoArea::Memory => {
                    let entry = snapshot.memory.get_mut(memory);
                    memory += 1;
                    entry
                }
            };
            entry.ok_or(RuntimeError::InvalidExecutionState)?.value = value;
        }
        Ok(())
    }

    /// The snapshot of every admitted binding, with the value of each entry not sampled yet.
    fn build_snapshot(&self) -> IoSnapshot {
        let mut snapshot = IoSnapshot::default();
        for (binding, name) in self.bindings.iter().zip(&self.binding_names) {
            let entry = IoSnapshotEntry {
                name: name.clone(),
                address: binding.address.clone(),
                value_type_name: binding_type_name(binding),
                value: IoSnapshotValue::Unresolved,
                source: None,
            };
            match binding.address.area {
                IoArea::Input => snapshot.inputs.push(entry),
                IoArea::Output => snapshot.outputs.push(entry),
                IoArea::Memory => snapshot.memory.push(entry),
            }
        }
        snapshot
    }
}

/// The IEC name of a binding's declared type, as trust-runtime's snapshot names it: the enum's
/// declared name when the binding is an enum, otherwise the scalar wire type's name.
fn binding_type_name(binding: &PreparedIoBinding) -> Option<SmolStr> {
    binding
        .codec
        .enum_type
        .as_ref()
        .map(|enum_type| enum_type.type_name.clone())
        .or_else(|| {
            binding
                .codec
                .wire_type
                .and_then(value_type_name)
                .map(SmolStr::new)
        })
}

impl IoQueuePort for EngineLiveValues {
    fn io_snapshot(&self) -> IoSnapshotRead {
        match &*self.published.borrow() {
            Some(snapshot) => IoSnapshotRead::Published(snapshot.clone()),
            None => IoSnapshotRead::Missing,
        }
    }

    fn enqueue_io_write(&self, address: IoAddress, value: Value) -> Result<(), RequestQueueFull> {
        self.queues
            .borrow_mut()
            .try_enqueue_io_write(address, value)
    }

    fn force_io(&self, address: IoAddress, value: Value) -> Result<(), RequestQueueFull> {
        self.queues
            .borrow_mut()
            .try_upsert_forced_io(address, value)
    }

    fn release_io(&self, address: &IoAddress) {
        self.queues.borrow_mut().release_io(address);
    }
}

impl VariableQueuePort for EngineLiveValues {
    fn generation(&self) -> Generation {
        self.generation.get()
    }

    fn enqueue_var_write(
        &self,
        target: PendingVarTarget,
        value: Value,
    ) -> Result<(), RequestQueueFull> {
        self.queues.borrow_mut().try_upsert_var_write(target, value)
    }

    fn force_var(&self, target: ForcedVarTarget, value: Value) -> Result<(), RequestQueueFull> {
        self.queues
            .borrow_mut()
            .try_upsert_forced_var(target, value)
    }

    fn release_global(&self, name: &str) {
        self.queues.borrow_mut().clear_forced_vars(
            |target| matches!(target, ForcedVarTarget::Global(current) if current.as_str() == name),
        );
    }

    fn release_retain(&self, name: &str) {
        self.queues.borrow_mut().clear_forced_vars(
            |target| matches!(target, ForcedVarTarget::Retain(current) if current.as_str() == name),
        );
    }

    fn release_instance(&self, instance: InstanceId, name: &str) {
        self.queues.borrow_mut().clear_forced_vars(|target| {
            matches!(
                target,
                ForcedVarTarget::Instance(_, current_id, current_name)
                    if *current_id == instance && current_name.as_str() == name
            )
        });
    }

    fn forced_snapshot(&self) -> ForcedSnapshot {
        self.queues.borrow().forced_snapshot()
    }
}

/// The runner's [`CycleHooks`] for one device scan: the shared Live Values applied at the engine's
/// phases and the snapshot published after the output sample, with every other callback forwarded
/// to `observer`, the loop's own record of the scan.
///
/// A refused write is pushed into `records` as a `WriteRejected` event at the phase that refused
/// it, as trust-runtime reports it.
pub(super) struct LiveValuesHooks<'a, 'o, C> {
    live: &'a EngineLiveValues,
    records: &'a Records<C>,
    scan: u64,
    observer: &'a mut (dyn CycleHooks + 'o),
}

impl<'a, 'o, C: MonotonicClock> LiveValuesHooks<'a, 'o, C> {
    /// The Live Values of `live` and the scan number `scan`, over `observer`.
    pub(super) fn new(
        live: &'a EngineLiveValues,
        records: &'a Records<C>,
        scan: u64,
        observer: &'a mut (dyn CycleHooks + 'o),
    ) -> Self {
        Self {
            live,
            records,
            scan,
            observer,
        }
    }
}

impl<C: MonotonicClock> CycleHooks for LiveValuesHooks<'_, '_, C> {
    fn has_program_hooks(&self) -> bool {
        self.observer.has_program_hooks()
    }

    fn program_started(&mut self, name: &SmolStr) {
        self.observer.program_started(name);
    }

    fn program_finished(&mut self, name: &SmolStr) {
        self.observer.program_finished(name);
    }

    fn has_output_hooks(&self) -> bool {
        self.observer.has_output_hooks()
    }

    /// The queued writes are applied at `CycleStart`, the queued I/O writes and then the forces at
    /// `BeforeInputSample`, and the forces again at `AfterOutputSample`, where the snapshot is
    /// published. The observer runs after each phase, so its events keep trust-runtime's order
    /// relative to a `WriteRejected`.
    fn on_phase(
        &mut self,
        phase: CyclePhase,
        access: &mut CycleAccess<'_, '_>,
    ) -> Result<(), RuntimeError> {
        let live = self.live;
        match phase {
            CyclePhase::CycleStart => {
                let mut queues = live.queues.borrow_mut();
                apply::cycle_start(
                    &mut *queues,
                    &live.names,
                    access,
                    self,
                    |_access, _target, _value| Err(RuntimeError::ReferenceLifetime),
                )?;
            }
            CyclePhase::BeforeInputSample => {
                let mut queues = live.queues.borrow_mut();
                apply::io_writes_and_forces(&mut *queues, &live.names, access, self)?;
            }
            CyclePhase::AfterOutputSample => {
                {
                    let mut queues = live.queues.borrow_mut();
                    apply::forces(&mut *queues, &live.names, access, self)?;
                }
                live.publish(access, self.scan)?;
            }
            _ => {}
        }
        self.observer.on_phase(phase, access)
    }

    fn publish_outputs(&mut self, access: &CycleAccess<'_, '_>) -> Result<(), RuntimeError> {
        self.observer.publish_outputs(access)
    }
}

impl<C: MonotonicClock> WriteSink for LiveValuesHooks<'_, '_, C> {
    fn refused(&mut self, target: &str, error: &RuntimeError, time: Duration) {
        self.records.push_event(RuntimeEvent::WriteRejected {
            target: SmolStr::new(target),
            error: error.to_string(),
            time,
        });
    }
}

#[cfg(test)]
mod contract_tests;
