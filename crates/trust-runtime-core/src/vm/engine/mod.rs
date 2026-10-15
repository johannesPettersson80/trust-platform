//! Single-owner state executing a prepared source-free artifact.
use super::{
    dispatch::ExecutionBuffers, edge::EdgeInputTransaction, module::invalid_bytecode,
    PreparedModule, VmFrame,
};
use crate::bytecode::{InitializationOnce, InitializerEntry, ResourceEntry};
use crate::error::RuntimeError;
use crate::memory::{FrameId, InstanceId, VariableStorage};
use crate::task::TaskState;
use crate::value::{DateTimeProfile, Duration, Value, ValueRef};
use alloc::{
    boxed::Box,
    collections::{BTreeMap, BTreeSet},
    vec,
    vec::Vec,
};

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
mod services;
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

/// Reusable execution buffers and per-operation resource accounting.
struct ExecutionResources {
    buffers: Vec<ExecutionBuffers>,
    ready_tasks: Vec<crate::cycle::ReadyTask>,
    constructed_values: usize,
    work_budget: crate::vm::budget::ExecutionBudget,
    construction_bytes: core::cell::Cell<usize>,
    output_journal: Option<output_transaction::SavedDestinations>,
    #[cfg(feature = "register-ir")]
    register: register::RegisterExecutionState,
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
    profile: DateTimeProfile,
    now: Duration,
    resource: ResourceEntry,
    resource_index: usize,
    tasks: Vec<TaskState>,
    fault: Option<RuntimeError>,
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
            false,
            None,
            None,
            Duration::ZERO,
        )
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
        after_restart: bool,
        now: Duration,
    ) -> Box<EngineData> {
        #[cfg(not(feature = "register-ir"))]
        let _ = services;
        Box::new(EngineData {
            construction_complete: false,
            resources: ExecutionResources {
                buffers: Vec::new(),
                ready_tasks: Vec::new(),
                constructed_values: 0,
                // TIME-09: construction is one entry under its own allowance.
                work_budget: crate::vm::budget::ExecutionBudget::new(
                    prepared.limits.max_construction_work,
                ),
                construction_bytes: core::cell::Cell::new(0),
                output_journal: None,
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
            storage: VariableStorage::new(),
            profile: DateTimeProfile::default(),
            now,
            tasks: vec![TaskState::new(now); resource.tasks.len()],
            resource,
            resource_index,
            fault: None,
        })
    }

    fn build(
        prepared: &'a PreparedModule,
        resource_index: usize,
        services: &'a dyn ExecutionServices,
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
        // Calls and staging results each have their own admitted depth bound.
        let frame_capacity = prepared
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
            .reserve_execution_frames(frame_capacity)?;
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
        if let Some(images) = prior_images {
            state.copy_process_images(images)?;
        }
        state.reserve_type_operations()?;
        state.construct_resource(retained)?;
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
            self.data.fault = Some(error.clone());
        }
        result
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
        super::dispatch::execute_pou_stack_for_engine_entry(
            self,
            module,
            pou,
            instance,
            0,
            crate::vm::budget::ExecutionEntry::Nested,
        )
    }
}

#[cfg(test)]
mod budget_tests;
#[cfg(test)]
mod continuation_tests;

#[cfg(test)]
mod restart_tests;
#[cfg(test)]
mod retain_apply_tests;
#[cfg(test)]
mod stdlib_text_work_tests;
#[cfg(test)]
mod storage_tests;

#[cfg(test)]
mod activation_registry_tests;
#[cfg(test)]
mod bounded_identity_sets_tests;
#[cfg(test)]
mod call_path_activation_tests;
#[cfg(test)]
mod work_allowance_continuation_tests;
