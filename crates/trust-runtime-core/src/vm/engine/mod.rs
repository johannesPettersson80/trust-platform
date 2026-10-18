//! Single-owner state executing a prepared source-free artifact.
use super::call::context::VmEdgeInput;
use super::call::scratch::CallScratch;
use super::{
    dispatch::ExecutionBuffers, edge::EdgeInputTransaction, module::invalid_bytecode,
    PreparedModule, VmFrame,
};
use crate::bytecode::{InitializationOnce, InitializerEntry, ResourceEntry};
use crate::error::RuntimeError;
use crate::fault::{FaultRecord, FaultSite, NO_TASK};
use crate::memory::cell_access::CellArm;
use crate::memory::{FrameId, Generation, InstanceId, VariableStorage};
use crate::task::TaskState;
use crate::value::{DateTimeProfile, Duration, Value, ValueRef};
use alloc::{
    boxed::Box,
    collections::{BTreeMap, BTreeSet},
    vec,
    vec::Vec,
};
use smol_str::SmolStr;

mod access;
mod retain_apply;
pub use retain_apply::RetainApplyEvent;
mod allocation;
mod api;
mod assignment;
mod context;
mod cycle;
mod cycle_port;
mod entry;
#[cfg(feature = "std")]
mod owned;
#[cfg(feature = "register-ir")]
mod register;
pub use cycle_port::{CycleAccess, CycleHooks, CyclePhase};
mod identities;
mod initialization;
mod ordered;
use identities::{
    InstanceLifetimes, InstanceSet, InstanceTemplates, OwnedInstances, RetainedGlobals,
};
mod keys;
use keys::{root_index, LifecycleMarks};
mod destinations;
mod io;
mod literal_admission;
mod output_transaction;
mod path_write;
mod policy;
mod readonly;
#[cfg(feature = "std")]
mod reconfiguration;
mod reference_types;
mod restart;
#[cfg(feature = "std")]
pub use reconfiguration::ResourceReconfiguration;
mod retained_graph;
mod scratch;
mod services;
mod store_access;
pub use api::RuntimeState;
#[cfg(feature = "std")]
pub use owned::OwnedRuntimeState;
pub use services::{ExecutionServices, LogicalExecutionServices};

struct ActiveInitializer {
    id: u32,
    context: u32,
    entry: InitializerEntry,
    result: FrameId,
    lexical_frame: Option<FrameId>,
    instance: Option<InstanceId>,
    depth: u32,
    writable_instances: InstanceSet,
}

/// Activation ownership and reference lifetime tracking.
struct LifetimeState {
    instance_lifetimes: InstanceLifetimes,
    owned_instances: OwnedInstances,
}

/// Declaration, instance and initializer lifecycle bookkeeping.
struct ConstructionState {
    roots: Vec<Option<InstanceId>>,
    claimed_roots: BTreeSet<u32>,
    instance_templates: InstanceTemplates,
    initialized_instances: InstanceSet,
    initialized_declarations: LifecycleMarks,
    once: LifecycleMarks,
    after_restart: bool,
    initializers: Vec<ActiveInitializer>,
    frames: Vec<VmFrame>,
    types: Vec<(u32, super::construction::values::ValueOperation)>,
    type_operations: Vec<u8>,
    retained_globals: RetainedGlobals,
}

/// Published process images and hierarchical binding values.
struct ProcessImages {
    inputs: Vec<u8>,
    outputs: Vec<u8>,
    memory: Vec<u8>,
    hierarchical: crate::collections::OrderedMap<crate::io_image::IoAddressKey, Value>,
}

/// Reusable execution buffers and per-operation resource accounting. Construction reserves
/// the buffers, frame locals, journal and staging that every scan reuses (spec 34 MEM-09).
struct ExecutionResources {
    buffers: Vec<ExecutionBuffers>,
    frame_locals: scratch::FrameLocals,
    /// Reusable edge-input plan of one call, empty between calls (spec 34 MEM-09).
    edge_inputs: Vec<VmEdgeInput>,
    /// Reusable originals of one edge-affected call, empty between calls: member ordinals with
    /// their values (C1-9d2a, R2).
    edge_restores: Vec<(usize, Value)>,
    /// Reserved initializer, frame and lifetime-check bookkeeping (spec 34 MEM-09).
    bookkeeping: scratch::BookkeepingScratch,
    /// Reusable per-depth call-binding scratch (spec 34 MEM-09; census C1, C3, C4, C5, C7).
    call_scratch: CallScratch,
    ready_tasks: Vec<crate::cycle::ReadyTask>,
    constructed_values: usize,
    work_budget: crate::vm::budget::ExecutionBudget,
    construction_bytes: core::cell::Cell<usize>,
    output_journal: Option<output_transaction::SavedDestinations>,
    /// The output copy-back journal kept between transactions.
    spare_journal: output_transaction::SavedDestinations,
    /// Process-image staging, empty between scans.
    staging: io::staging::ImageStaging,
    #[cfg(feature = "register-ir")]
    register: register::RegisterExecutionState,
}

/// The store the engine reads variables through (specification 34 §6.2; C1-9b). `Values` reads
/// `EngineData::storage`, today's `Value` storage. `Cells` reads the cell arm, which holds its own
/// `Value` half for locals and per-call instances. Only `Values` is built until C1-9d.
#[allow(
    dead_code,
    reason = "C1-9d builds the cell arm at construction; until then only the tests build it"
)]
enum EngineStore {
    /// Today's `Value` storage, `EngineData::storage`.
    Values,
    /// The cell arm.
    Cells(Box<CellArm>),
}

/// Owned mutable application data. Metadata and platform services are borrowed
/// only while an operation executes, allowing hosted ownership without a
/// self-reference or a second variable store.
struct EngineData {
    construction_complete: bool,
    resources: ExecutionResources,
    images: ProcessImages,
    construction: ConstructionState,
    lifetimes: LifetimeState,
    storage: VariableStorage,
    store: EngineStore,
    profile: DateTimeProfile,
    now: Duration,
    resource: ResourceEntry,
    resource_index: usize,
    tasks: Vec<TaskState>,
    fault: Option<RuntimeError>,
    /// The fixed-size record latched with this fault (specification 34 MEM-05, STATE-04).
    fault_record: Option<FaultRecord>,
    /// Site of the innermost instruction the last dispatch entry failed at; absent when the
    /// entry succeeded or no instruction raised the fault.
    fault_site: Option<FaultSite>,
    /// Task index running at this point of the cycle; [`NO_TASK`] between tasks and in
    /// background programs.
    task: u16,
}

/// Temporary execution context for one authoritative mutable state.
pub(super) struct EngineState<'a> {
    data: &'a mut EngineData,
    prepared: &'a PreparedModule,
    services: &'a dyn ExecutionServices,
}

impl<'a> EngineState<'a> {
    fn engineering_operation<T>(
        &mut self,
        action: impl FnOnce(&mut EngineState<'_>) -> Result<T, RuntimeError>,
    ) -> Result<T, RuntimeError> {
        self.data
            .resources
            .work_budget
            .reset(self.prepared.limits.max_work);
        self.data.resources.construction_bytes.set(0);
        self.data.resources.constructed_values = 0;
        self.check_entry_deadline()?;
        // Completion is checked before each operation's final publication.
        // A fallible sample after commit cannot turn a successful write into rejection.
        action(self)
    }

    fn construct_data(
        prepared: &'a PreparedModule,
        resource_index: usize,
        services: &'a dyn ExecutionServices,
    ) -> Result<Box<EngineData>, RuntimeError> {
        Self::build(
            prepared,
            resource_index,
            services,
            Generation::FIRST,
            false,
            None,
            None,
            Duration::ZERO,
        )
    }

    /// The generation a restart or replacement of this state constructs. An
    /// exhausted counter refuses it before anything changes (spec 34 §11.3).
    fn next_generation(&self) -> Result<Generation, RuntimeError> {
        self.data
            .storage
            .generation()
            .next()
            .ok_or(RuntimeError::Overflow)
    }

    // Construct the engine record in its own short frame. Building it in place in
    // `build` kept 544 bytes on the stack under every nested initializer during
    // construction (F401 headroom); the boxed record is reached through a pointer.
    #[inline(never)]
    fn new_data(
        prepared: &PreparedModule,
        services: &dyn ExecutionServices,
        resource: ResourceEntry,
        resource_index: usize,
        generation: Generation,
        after_restart: bool,
        now: Duration,
    ) -> Box<EngineData> {
        #[cfg(not(feature = "register-ir"))]
        let _ = services;
        let mut storage = VariableStorage::new();
        // Every reference construction creates belongs to this generation.
        storage.set_generation(generation);
        Box::new(EngineData {
            construction_complete: false,
            resources: ExecutionResources {
                buffers: Vec::new(),
                frame_locals: scratch::FrameLocals::default(),
                edge_inputs: Vec::new(),
                edge_restores: Vec::new(),
                bookkeeping: scratch::BookkeepingScratch::default(),
                call_scratch: CallScratch::default(),
                ready_tasks: Vec::new(),
                constructed_values: 0,
                // TIME-09: construction is one entry under its own allowance.
                work_budget: crate::vm::budget::ExecutionBudget::new(
                    prepared.limits.max_construction_work,
                ),
                construction_bytes: core::cell::Cell::new(0),
                output_journal: None,
                spare_journal: output_transaction::SavedDestinations::new(),
                staging: io::staging::ImageStaging::default(),
                #[cfg(feature = "register-ir")]
                register: register::RegisterExecutionState::new(services.register_configuration()),
            },
            images: ProcessImages {
                inputs: vec![0; resource.inputs_size as usize],
                outputs: vec![0; resource.outputs_size as usize],
                memory: vec![0; resource.memory_size as usize],
                hierarchical: crate::collections::OrderedMap::default(),
            },
            construction: ConstructionState {
                roots: vec![None; prepared.roots.entries.len()],
                claimed_roots: BTreeSet::new(),
                instance_templates: InstanceTemplates::default(),
                initialized_instances: InstanceSet::default(),
                initialized_declarations: LifecycleMarks::default(),
                once: LifecycleMarks::default(),
                after_restart,
                initializers: Vec::new(),
                frames: Vec::new(),
                types: Vec::new(),
                type_operations: Vec::new(),
                retained_globals: RetainedGlobals::default(),
            },
            lifetimes: LifetimeState {
                instance_lifetimes: InstanceLifetimes::default(),
                owned_instances: OwnedInstances::default(),
            },
            storage,
            store: EngineStore::Values,
            profile: DateTimeProfile::default(),
            now,
            tasks: vec![TaskState::new(now); resource.tasks.len()],
            resource,
            resource_index,
            fault: None,
            fault_record: None,
            fault_site: None,
            task: NO_TASK,
        })
    }

    #[allow(clippy::too_many_arguments)]
    fn build(
        prepared: &'a PreparedModule,
        resource_index: usize,
        services: &'a dyn ExecutionServices,
        generation: Generation,
        after_restart: bool,
        retained: Option<&EngineState<'_>>,
        prior_images: Option<&ProcessImages>,
        now: Duration,
    ) -> Result<Box<EngineData>, RuntimeError> {
        prepared.validate_services(services)?;
        let resource = prepared
            .resources
            .resources
            .get(resource_index)
            .cloned()
            .ok_or_else(|| {
                invalid_bytecode(smol_str::SmolStr::new_static("unknown resource index"))
            })?;
        let mut data = Self::new_data(
            prepared,
            services,
            resource,
            resource_index,
            generation,
            after_restart,
            now,
        );
        let mut state = Self::borrow(&mut data, prepared, services);
        state.check_entry_deadline()?;
        #[cfg(feature = "register-ir")]
        state.prepare_register_state()?;
        let ready_capacity = state.data.resource.tasks.len();
        let ready_bytes = ready_capacity
            .checked_mul(core::mem::size_of::<crate::cycle::ReadyTask>())
            .ok_or(RuntimeError::Overflow)?;
        state.charge_allocation_bytes(ready_bytes)?;
        state
            .data
            .resources
            .ready_tasks
            .try_reserve_exact(ready_capacity)
            .map_err(|_| super::VmTrap::BudgetExceeded.into_runtime_error())?;
        // Calls and staging results each have their own reserved depth bound: the
        // module's static call depth, capped at the admitted ceiling (spec 34 MEM-09).
        let frame_capacity = prepared
            .reserved_call_levels()
            .checked_mul(2)
            .ok_or(RuntimeError::Overflow)?;
        // The runtime depth check stays at the admitted ceiling even though the reservation
        // follows the static call depth, which is never larger (spec 34 MEM-02).
        let frame_depth_check = prepared
            .limits
            .max_call_depth
            .checked_mul(2)
            .ok_or(RuntimeError::Overflow)?;
        let frame_bytes = VariableStorage::execution_frame_reservation_charge(frame_capacity)
            .ok_or(RuntimeError::Overflow)?;
        // One registry owns suspended storage, active ancestry and POU metadata.
        // Both admissions precede allocation; no discarded parallel arrays are charged.
        state.charge_allocation_bytes(frame_bytes)?;
        state
            .data
            .storage
            .reserve_execution_frames_with_depth_check(frame_capacity, frame_depth_check)?;
        state.charge_sorted_growth(
            state
                .data
                .lifetimes
                .owned_instances
                .reservation_demand(frame_capacity),
        )?;
        state
            .data
            .lifetimes
            .owned_instances
            .reserve_capacity(frame_capacity)?;
        state.reserve_execution_scratch()?;
        if let Some(images) = prior_images {
            state.copy_process_images(images)?;
        }
        state.reserve_type_operations()?;
        state.construct_resource(retained)?;
        // A variable located at %M starts with its initial value (or, after a warm restart, its
        // retained one). The first scan loads the marker image into the bound variables, so the
        // image must hold that value rather than zero or the carried-over image. Restoration
        // that follows construction publishes again (IEC 61131-3 Ed.3 6.5.6; specification 11 6.7).
        state.publish_marker_image()?;
        state.check_entry_deadline()?;
        state.data.construction_complete = true;
        Ok(data)
    }

    fn borrow<'b>(
        data: &'b mut EngineData,
        prepared: &'b PreparedModule,
        services: &'b dyn ExecutionServices,
    ) -> EngineState<'b> {
        EngineState {
            data,
            prepared,
            services,
        }
    }

    #[cfg(test)]
    fn outputs(&self) -> &[u8] {
        &self.data.images.outputs
    }

    /// Execute one cooperative resource cycle at an explicitly sampled logical time.
    /// The platform owns physical sleeping, I/O drivers and independent supervision.
    pub fn execute_cycle(&mut self, now: Duration) -> Result<(), RuntimeError> {
        self.execute_cycle_with_hooks(now, &mut cycle_port::NoopCycleHooks)
    }

    fn execute_cycle_with_hooks(
        &mut self,
        now: Duration,
        hooks: &mut dyn CycleHooks,
    ) -> Result<(), RuntimeError> {
        if self.data.fault.is_some() {
            return Err(RuntimeError::ResourceFaulted);
        }
        if now < self.data.now {
            return Err(RuntimeError::InvalidConfig(
                "logical time moved backwards".into(),
            ));
        }
        self.data.now = now;
        self.data.task = NO_TASK;
        self.data
            .resources
            .work_budget
            .reset(self.prepared.limits.max_work);
        self.data.resources.construction_bytes.set(0);
        self.data.resources.constructed_values = 0;
        let result = self
            .check_entry_deadline()
            .and_then(|()| self.execute_cycle_inner(hooks));
        if let Err(error) = &result {
            self.latch_fault_record(error);
            self.data.fault = Some(error.clone());
        }
        result
    }

    /// Latch the fixed-size record of a fault with the site of the failing instruction, the
    /// task that ran and this scan's logical time (specification 34 MEM-05, STATE-04). The
    /// record is `Copy` and built from the error by reference, so nothing is cloned and
    /// nothing is allocated.
    fn latch_fault_record(&mut self, error: &RuntimeError) {
        let site = self.data.fault_site.unwrap_or(FaultSite::NONE);
        self.data.fault_record = Some(FaultRecord::latch(
            error,
            site,
            self.data.task,
            self.data.now,
        ));
    }

    fn execute_pou(&mut self, pou: u32, instance: Option<InstanceId>) -> Result<(), RuntimeError> {
        let module = &self.prepared.vm;
        let edge = EdgeInputTransaction::begin(self, module, pou, instance)?;
        let result = self.execute_prepared_pou(module, pou, instance);
        if let Some(edge) = edge {
            edge.restore(self);
        }
        result
    }

    fn execute_prepared_pou(
        &mut self,
        module: &super::module::VmModule,
        pou: u32,
        instance: Option<InstanceId>,
    ) -> Result<(), RuntimeError> {
        let mut fault_site = None;
        let result = super::dispatch::execute_pou_stack_for_engine_entry(
            self,
            module,
            pou,
            instance,
            0,
            crate::vm::budget::ExecutionEntry::Nested,
            &mut fault_site,
        );
        // Only the failing entry's site belongs to the fault: a later adapter, deadline or
        // output failure must not inherit the last instruction a program reached.
        self.data.fault_site = if result.is_err() { fault_site } else { None };
        result
    }
}

#[cfg(test)]
mod budget_tests;
#[cfg(test)]
mod continuation_tests;

#[cfg(test)]
mod generation_tests;
#[cfg(test)]
mod restart_tests;
#[cfg(test)]
mod retain_apply_tests;
#[cfg(test)]
mod stdlib_text_work_tests;
#[cfg(test)]
mod storage_tests;
#[cfg(test)]
mod store_access_tests;

#[cfg(test)]
mod activation_registry_tests;
#[cfg(test)]
mod bounded_identity_sets_tests;
#[cfg(test)]
mod call_path_activation_tests;
#[cfg(all(test, feature = "register-ir"))]
mod work_allowance_continuation_tests;
