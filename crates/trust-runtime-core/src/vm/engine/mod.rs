//! Single-owner state executing a prepared source-free artifact.
use super::{
    dispatch::{execute_pou_stack_with_locals, ExecutionBuffers},
    edge::EdgeInputTransaction,
    module::invalid_bytecode,
    PreparedModule, VmFrame,
};
use crate::bytecode::{InitializationOnce, InitializerEntry, ResourceEntry};
use crate::error::RuntimeError;
use crate::memory::{FrameId, InstanceId, VariableStorage};
use crate::task::TaskState;
use crate::value::{DateTimeProfile, Duration, Value, ValueRef};
use alloc::{
    collections::{BTreeMap, BTreeSet},
    vec,
    vec::Vec,
};

mod access;
mod allocation;
mod api;
mod assignment;
mod context;
mod cycle;
mod identities;
mod initialization;
mod ordered;
use identities::{
    ActivationPous, InstanceLifetimes, InstanceSet, InstanceTemplates, OwnedInstances,
    RetainedGlobals,
};
mod keys;
use keys::{root_index, LifecycleMarks};
mod destinations;
mod io;
mod output_transaction;
mod path_write;
mod policy;
mod readonly;
mod reference_types;
mod restart;
mod retained_graph;
mod services;
pub use api::RuntimeState;
pub use services::ExecutionServices;

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
    live_activations: Vec<FrameId>,
    activation_pous: ActivationPous,
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
    constructed_values: usize,
    work_budget: crate::vm::budget::ExecutionBudget,
    construction_bytes: core::cell::Cell<usize>,
    output_journal: Option<output_transaction::SavedDestinations>,
}

/// Mutable runtime state tied to one immutable prepared application.
/// A failed construction returns no state; replacement can therefore be prepared
/// beside a running application and installed only after successful construction.
pub(super) struct EngineState<'a> {
    resources: ExecutionResources,
    images: ProcessImages,
    construction: ConstructionState,
    lifetimes: LifetimeState,
    prepared: &'a PreparedModule,
    services: &'a dyn ExecutionServices,
    storage: VariableStorage,
    profile: DateTimeProfile,
    now: Duration,
    resource: ResourceEntry,
    resource_index: usize,
    tasks: Vec<TaskState>,
    fault: Option<RuntimeError>,
}

impl<'a> EngineState<'a> {
    fn new(
        prepared: &'a PreparedModule,
        resource_index: usize,
        services: &'a dyn ExecutionServices,
    ) -> Result<Self, RuntimeError> {
        Self::build(
            prepared,
            resource_index,
            services,
            false,
            None,
            Duration::ZERO,
        )
    }

    fn build(
        prepared: &'a PreparedModule,
        resource_index: usize,
        services: &'a dyn ExecutionServices,
        after_restart: bool,
        retained: Option<&EngineState<'_>>,
        now: Duration,
    ) -> Result<Self, RuntimeError> {
        prepared.validate_services(services)?;
        let resource = prepared
            .resources
            .resources
            .get(resource_index)
            .cloned()
            .ok_or_else(|| {
                invalid_bytecode(smol_str::SmolStr::new_static("unknown resource index"))
            })?;
        let mut state = Self {
            resources: ExecutionResources {
                buffers: Vec::new(),
                constructed_values: 0,
                work_budget: crate::vm::budget::ExecutionBudget::new(prepared.limits.max_work),
                construction_bytes: core::cell::Cell::new(0),
                output_journal: None,
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
                retained_globals: RetainedGlobals::default(),
            },
            lifetimes: LifetimeState {
                live_activations: Vec::new(),
                activation_pous: ActivationPous::default(),
                instance_lifetimes: InstanceLifetimes::default(),
                owned_instances: OwnedInstances::default(),
            },
            prepared,
            services,
            storage: VariableStorage::new(),
            profile: DateTimeProfile::default(),
            now,
            tasks: vec![TaskState::new(now); resource.tasks.len()],
            resource,
            resource_index,
            fault: None,
        };
        state.check_entry_deadline()?;
        // Calls and staging results each have their own admitted depth bound.
        let frame_capacity = prepared
            .limits
            .max_call_depth
            .checked_mul(2)
            .ok_or(RuntimeError::Overflow)?;
        let frame_bytes = VariableStorage::execution_frame_reservation_charge(frame_capacity)
            .ok_or(RuntimeError::Overflow)?;
        state.charge_allocation_bytes(frame_bytes)?;
        state.storage.reserve_execution_frames(frame_capacity)?;
        let activation_bytes = frame_capacity
            .checked_mul(core::mem::size_of::<FrameId>())
            .ok_or(RuntimeError::Overflow)?;
        state.charge_sorted_growth(Ok((activation_bytes, 1)))?;
        state
            .lifetimes
            .live_activations
            .try_reserve_exact(frame_capacity)
            .map_err(|_| RuntimeError::Overflow)?;
        // Ordinary POU identities are bounded by call depth; instance owners may
        // also be initializer result frames at the same depth.
        state.charge_sorted_growth(
            state
                .lifetimes
                .activation_pous
                .reservation_demand(prepared.limits.max_call_depth),
        )?;
        state
            .lifetimes
            .activation_pous
            .reserve_capacity(prepared.limits.max_call_depth)?;
        state.charge_sorted_growth(
            state
                .lifetimes
                .owned_instances
                .reservation_demand(frame_capacity),
        )?;
        state
            .lifetimes
            .owned_instances
            .reserve_capacity(frame_capacity)?;
        state.construct_resource(retained)?;
        state.check_entry_deadline()?;
        Ok(state)
    }

    /// Inspect state without exposing a second mutable execution path.
    pub fn storage(&self) -> &VariableStorage {
        &self.storage
    }
    /// Borrow the full input image to sample the platform's inputs before a scan.
    pub fn inputs_mut(&mut self) -> &mut [u8] {
        &mut self.images.inputs
    }
    /// Read the output image after a successful scan.
    pub fn outputs(&self) -> &[u8] {
        &self.images.outputs
    }
    /// Current latched resource fault.
    pub fn fault(&self) -> Option<&RuntimeError> {
        self.fault.as_ref()
    }
    /// Task scheduling state, including the nominal deadline and dropped activations.
    pub fn task_states(&self) -> &[TaskState] {
        &self.tasks
    }

    /// Execute one cooperative resource cycle at an explicitly sampled logical time.
    /// The platform owns physical sleeping, I/O drivers and independent supervision.
    pub fn execute_cycle(&mut self, now: Duration) -> Result<(), RuntimeError> {
        if self.fault.is_some() {
            return Err(RuntimeError::ResourceFaulted);
        }
        if now < self.now {
            return Err(RuntimeError::InvalidConfig(
                "logical time moved backwards".into(),
            ));
        }
        self.now = now;
        self.resources
            .work_budget
            .reset(self.prepared.limits.max_work);
        self.resources.construction_bytes.set(0);
        self.resources.constructed_values = 0;
        let result = self
            .check_entry_deadline()
            .and_then(|()| self.execute_cycle_inner());
        if let Err(error) = &result {
            self.fault = Some(error.clone());
        }
        result
    }

    fn execute_pou(&mut self, pou: u32, instance: Option<InstanceId>) -> Result<(), RuntimeError> {
        let module = &self.prepared.vm;
        let edge = EdgeInputTransaction::begin(self, module, pou, instance)?;
        let result = execute_pou_stack_with_locals(
            self,
            module,
            pou,
            instance,
            None,
            false,
            0,
            crate::vm::budget::ExecutionEntry::Nested,
        );
        if let Some(edge) = edge {
            edge.restore(self);
        }
        result.map(|_| ())
    }
}

#[cfg(test)]
mod budget_tests;
#[cfg(test)]
mod continuation_tests;

#[cfg(test)]
mod storage_tests;
