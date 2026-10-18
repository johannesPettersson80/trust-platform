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
//! refused write is reported through the runner's [`Records`] as a `WriteRejected` event, as
//! trust-runtime reports it.

use alloc::string::ToString;
use alloc::vec::Vec;
use core::cell::{Cell, RefCell};

use smol_str::SmolStr;
use trust_runtime_core::error::RuntimeError;
use trust_runtime_core::io_address::IoAddress;
use trust_runtime_core::io_image::PreparedIoBinding;
use trust_runtime_core::memory::{Generation, InstanceId, IoArea};
use trust_runtime_core::ports::MonotonicClock;
use trust_runtime_core::value::{Duration, Value};
use trust_runtime_core::vm::{CycleAccess, CycleHooks, CyclePhase, PreparedModule};

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
