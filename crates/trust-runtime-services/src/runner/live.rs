//! The Live Values of a device's cycle (specification 11, "Live Values"; specification 34 §4.3.1
//! and §8; specification 44 §13.3).
//!
//! A device has one control surface, so it owns one set of [`LiveValueQueues`] and one published
//! I/O snapshot: [`DeviceLiveValues`]. The platform's `IoQueuePort` and `VariableQueuePort` answer
//! the `io.*`, `set` and `var.*` requests from it, and the runner applies the queues through
//! [`crate::live_values::apply`] at the boundaries of the compiled program's scan and publishes
//! the snapshot after each scan, at the same points as trust-runtime.
//!
//! The state is single-owner and holds no lock: a device's runner and its platform are one thread,
//! so a shared [`Rc`] takes the place of trust-runtime's `DebugControl` lock. A refused write is
//! reported through the runner's record store as a `WriteRejected` event, as trust-runtime reports
//! it.

mod snapshot;

use alloc::rc::Rc;
use alloc::string::ToString;
use alloc::vec::Vec;
use core::cell::{Cell, RefCell};

use smol_str::SmolStr;
use trust_runtime_core::cycle::{CycleHooks, CyclePhase};
use trust_runtime_core::error::RuntimeError;
use trust_runtime_core::io_address::IoAddress;
use trust_runtime_core::io_image::{FlatImages, PreparedIoBinding};
use trust_runtime_core::memory::{Generation, InstanceId, IoArea};
use trust_runtime_core::native::variables::{CompiledAccessPoint, CompiledVariables};
use trust_runtime_core::ports::MonotonicClock;
use trust_runtime_core::value::{Duration, Value};

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
use crate::live_values::var_requests::VariableQueuePort;
use crate::live_values::RequestQueueFull;

/// The Live Values of one device: the pending writes and active forces the platform queues, and
/// the I/O snapshot the runner publishes.
///
/// A handle: a clone shares the queues and the snapshot with the original. The platform keeps one
/// and answers the control requests from it, and the resource loop takes a clone when it starts,
/// so a scan applies the queues without borrowing the platform, which a debugger hold needs for
/// its idle work and its watchdog feed (specification 34, STATE-07). It is single-owner, so it
/// holds no lock; a chip keeps the Live Values of its program as long as the program lives.
#[derive(Clone)]
pub struct DeviceLiveValues {
    inner: Rc<LiveInner>,
}

/// The state that the handles of one [`DeviceLiveValues`] share.
struct LiveInner {
    /// The pending writes and active forces the control requests queue.
    queues: RefCell<LiveValueQueues<()>>,
    /// The installation generation an instance-targeted request is resolved in.
    generation: Cell<Generation>,
    /// The process-image bindings the snapshot renders, in declaration order.
    bindings: Vec<PreparedIoBinding>,
    /// The declared name of each binding, parallel to `bindings`.
    binding_names: Vec<Option<SmolStr>>,
    /// The published snapshot, once the first scan has published one; with a copy of the images,
    /// the snapshot that a request decoded from it last.
    published: RefCell<Option<IoSnapshot>>,
    /// Whether a scan publishes a copy of the flat images instead of the decoded snapshot
    /// (checklist P1-6).
    copies: bool,
    /// The copy that the last scan published, when it publishes one.
    images: RefCell<snapshot::PublishedImages>,
}

impl DeviceLiveValues {
    /// Empty Live Values of a platform that has no installed program yet: no bindings and nothing
    /// queued.
    #[must_use]
    pub fn empty() -> Self {
        Self::of(Vec::new(), Vec::new(), true)
    }

    /// The Live Values of a compiled program (specification 44 §13.3): the process-image bindings
    /// of its image, `bindings`, each named by the parallel `binding_names`, with nothing queued.
    /// The program's variables are reached by name through its symbol table.
    ///
    /// # Errors
    ///
    /// [`RuntimeError::InvalidExecutionState`] when `binding_names` is not parallel to `bindings`.
    pub fn for_compiled(
        bindings: Vec<PreparedIoBinding>,
        binding_names: Vec<Option<SmolStr>>,
    ) -> Result<Self, RuntimeError> {
        if binding_names.len() != bindings.len() {
            return Err(RuntimeError::InvalidExecutionState);
        }
        let copies = snapshot::copies_images(&bindings);
        Ok(Self::of(bindings, binding_names, copies))
    }

    fn of(
        bindings: Vec<PreparedIoBinding>,
        binding_names: Vec<Option<SmolStr>>,
        copies: bool,
    ) -> Self {
        Self {
            inner: Rc::new(LiveInner {
                queues: RefCell::new(LiveValueQueues::new()),
                generation: Cell::new(Generation::FIRST),
                bindings,
                binding_names,
                published: RefCell::new(None),
                copies,
                images: RefCell::default(),
            }),
        }
    }

    /// Clear every queued write and active force at a terminating boundary: safe state, a fault
    /// and a restart (specification 11 §6.9.2). The published snapshot stays, as trust-runtime
    /// keeps it.
    pub fn clear(&self) {
        self.inner.queues.borrow_mut().clear();
    }

    /// Records the installation generation of the program these Live Values belong to. A request that
    /// names an instance by a bare identity, as a control client does, belongs to the generation
    /// installed when the request is accepted, and the boundary that applies it refuses it when another
    /// generation is installed (specification 34 §4.3.1). A device gives the Live Values of an online
    /// change's candidate the generation of the candidate's state before the commit, so a force queued
    /// after the change names the installed generation. The resource loop gives them the generation of
    /// the program's state ([`ResourceCycle::generation`](super::ResourceCycle::generation)) when it
    /// takes a program and after every restart that commits (specification 34 STATE-08).
    pub fn set_generation(&self, generation: Generation) {
        self.inner.generation.set(generation);
    }

    /// The published I/O snapshot of a compiled program's last completed scan, from its process
    /// images `images`: the copy that a request decodes, or the snapshot decoded now when a binding
    /// is not a flat scalar.
    fn publish_images(&self, images: &FlatImages, scan: u64) -> Result<(), RuntimeError> {
        if self.inner.copies {
            let queues = self.inner.queues.borrow();
            self.inner.images.borrow_mut().record_images(
                images,
                scan,
                queues.forced_io.iter().map(|(address, _)| address),
            );
            return Ok(());
        }
        let mut published = self.inner.published.borrow_mut();
        let snapshot = published.get_or_insert_with(|| self.build_snapshot());
        snapshot.scan = Some(scan);
        snapshot.forced.clear();
        snapshot::fill(&self.inner.bindings, snapshot, |address| {
            images.read(address)
        })
    }

    /// The snapshot of every binding, with the value of each entry not sampled yet.
    fn build_snapshot(&self) -> IoSnapshot {
        let mut snapshot = IoSnapshot::default();
        for (binding, name) in self.inner.bindings.iter().zip(&self.inner.binding_names) {
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

impl IoQueuePort for DeviceLiveValues {
    fn io_snapshot(&self) -> IoSnapshotRead {
        if self.inner.copies {
            // The bindings are decoded from the copy the last scan published (checklist P1-6).
            let images = self.inner.images.borrow();
            if !images.published() {
                return IoSnapshotRead::Missing;
            }
            let mut published = self.inner.published.borrow_mut();
            let snapshot = published.get_or_insert_with(|| self.build_snapshot());
            return match images.decode_into(&self.inner.bindings, snapshot) {
                Ok(()) => IoSnapshotRead::Published(snapshot.clone()),
                Err(_) => IoSnapshotRead::Missing,
            };
        }
        match &*self.inner.published.borrow() {
            Some(snapshot) => IoSnapshotRead::Published(snapshot.clone()),
            None => IoSnapshotRead::Missing,
        }
    }

    fn enqueue_io_write(&self, address: IoAddress, value: Value) -> Result<(), RequestQueueFull> {
        self.inner
            .queues
            .borrow_mut()
            .try_enqueue_io_write(address, value)
    }

    fn force_io(&self, address: IoAddress, value: Value) -> Result<(), RequestQueueFull> {
        self.inner
            .queues
            .borrow_mut()
            .try_upsert_forced_io(address, value)
    }

    fn release_io(&self, address: &IoAddress) {
        self.inner.queues.borrow_mut().release_io(address);
    }
}

impl VariableQueuePort for DeviceLiveValues {
    fn generation(&self) -> Generation {
        self.inner.generation.get()
    }

    fn enqueue_var_write(
        &self,
        target: PendingVarTarget,
        value: Value,
    ) -> Result<(), RequestQueueFull> {
        self.inner
            .queues
            .borrow_mut()
            .try_upsert_var_write(target, value)
    }

    fn force_var(&self, target: ForcedVarTarget, value: Value) -> Result<(), RequestQueueFull> {
        self.inner
            .queues
            .borrow_mut()
            .try_upsert_forced_var(target, value)
    }

    fn release_global(&self, name: &str) {
        self.inner.queues.borrow_mut().clear_forced_vars(
            |target| matches!(target, ForcedVarTarget::Global(current) if current.as_str() == name),
        );
    }

    fn release_retain(&self, name: &str) {
        self.inner.queues.borrow_mut().clear_forced_vars(
            |target| matches!(target, ForcedVarTarget::Retain(current) if current.as_str() == name),
        );
    }

    fn release_instance(&self, instance: InstanceId, name: &str) {
        self.inner.queues.borrow_mut().clear_forced_vars(|target| {
            matches!(
                target,
                ForcedVarTarget::Instance(_, current_id, current_name)
                    if *current_id == instance && current_name.as_str() == name
            )
        });
    }

    fn forced_snapshot(&self) -> ForcedSnapshot {
        self.inner.queues.borrow().forced_snapshot()
    }
}

/// The runner's [`CycleHooks`] for one device scan: the shared Live Values applied at the scan's
/// boundaries and the snapshot published after the output sample, with every callback forwarded
/// to `observer`, the loop's own record of the scan.
///
/// A refused write is pushed into `records` as a `WriteRejected` event at the boundary that refused
/// it, as trust-runtime reports it.
pub(super) struct LiveValuesHooks<'a, 'o, C> {
    live: &'a DeviceLiveValues,
    records: &'a Records<C>,
    scan: u64,
    observer: &'a mut (dyn CycleHooks + 'o),
}

impl<'a, 'o, C: MonotonicClock> LiveValuesHooks<'a, 'o, C> {
    /// The Live Values of `live` and the scan number `scan`, over `observer`.
    pub(super) fn new(
        live: &'a DeviceLiveValues,
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
    /// The snapshot is published after the output sample.
    fn on_compiled_phase(
        &mut self,
        phase: CyclePhase,
        now: Duration,
        images: &FlatImages,
    ) -> Result<(), RuntimeError> {
        if phase == CyclePhase::AfterOutputSample {
            self.live.publish_images(images, self.scan)?;
        }
        self.observer.on_compiled_phase(phase, now, images)
    }

    /// The Live Values of a compiled program reach its variables through the symbol table
    /// (specification 44 §13.3): the queued variable writes and the variable forces at the cycle
    /// start, the queued image writes and the input and marker forces after the drivers read the
    /// inputs, and the variable forces again and the output forces before the outputs reach the
    /// drivers (specification 10 §4.7, steps 2 and 6).
    fn on_compiled_access(
        &mut self,
        point: CompiledAccessPoint,
        now: Duration,
        variables: &mut dyn CompiledVariables,
    ) -> Result<(), RuntimeError> {
        let live = self.live;
        {
            let mut queues = live.inner.queues.borrow_mut();
            if !queues.is_idle() {
                let generation = live.inner.generation.get();
                match point {
                    CompiledAccessPoint::CycleStart | CompiledAccessPoint::HoldEnd => {
                        apply::compiled_cycle_start(&mut queues, generation, variables, now, self)?;
                    }
                    CompiledAccessPoint::AfterInputs => apply::compiled_images(
                        &mut queues,
                        &[IoArea::Input, IoArea::Memory],
                        variables,
                        now,
                        self,
                    )?,
                    // The forces again after the scan, the variables' and the marker and output
                    // images', before the outputs reach the drivers (specification 10 §4.7, step 6).
                    CompiledAccessPoint::BeforeOutputs => {
                        apply::compiled_forces(&mut queues, generation, variables, now, self)?;
                        apply::compiled_images(
                            &mut queues,
                            &[IoArea::Memory, IoArea::Output],
                            variables,
                            now,
                            self,
                        )?;
                    }
                    // The task boundaries belong to the computer's services, not to Live Values.
                    CompiledAccessPoint::TaskStart(_) | CompiledAccessPoint::TaskEnd(_) => {}
                }
            }
        }
        self.observer.on_compiled_access(point, now, variables)
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
