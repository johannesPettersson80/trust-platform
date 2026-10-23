//! Exclusive owner operation changing resources without reconstructing PLC values.
use super::{ExecutionServices, OwnedRuntimeState, PreparedModule};
use crate::{
    bytecode::ResourceEntry, error::RuntimeError, io_address::IoAddress, memory::IoArea,
    task::TaskState, value::Value, vm::prepared::PreparationBudget,
};
use alloc::{sync::Arc, vec::Vec};

/// Exclusively borrowed, admitted resource candidate. Dropping it changes no PLC
/// values, images, task timing or fault. Commit is the only publication point.
pub struct ResourceReconfiguration<'a, S: ExecutionServices> {
    owner: &'a mut OwnedRuntimeState<S>,
    prepared: Arc<PreparedModule>,
    resource: ResourceEntry,
    resource_index: usize,
    tasks: Option<Vec<TaskState>>,
    ready: Vec<crate::cycle::ReadyTask>,
    images: [Vec<u8>; 3],
    budget: PreparationBudget,
}
impl<S: ExecutionServices> OwnedRuntimeState<S> {
    /// Stage task/image changes against exactly the same executable and storage
    /// contract. No initializers execute and no live object graph is copied.
    pub fn prepare_resource_reconfiguration(
        &mut self,
        prepared: Arc<PreparedModule>,
        resource_index: usize,
        reset_task_timing: bool,
    ) -> Result<ResourceReconfiguration<'_, S>, RuntimeError> {
        let mut budget = PreparationBudget::new(self.prepared.limits);
        // Existing admission accounts every expanded constant and table node;
        // this bounds the exact identity comparison without charging RUN fuel.
        budget.charge(0, self.prepared.preparation_usage().work)?;
        if !same_execution(&self.prepared, &prepared) {
            return Err(RuntimeError::InvalidConfig(
                "resource update changes executable or construction identity".into(),
            ));
        }
        prepared.validate_services(&self.services)?;
        if self.services.deadline_exceeded() {
            return Err(RuntimeError::ExecutionTimeout);
        }
        let resource = prepared
            .resources()
            .get(resource_index)
            .ok_or(RuntimeError::InvalidExecutionState)?;
        if !reset_task_timing && resource.tasks != self.data.resource.tasks {
            return Err(RuntimeError::InvalidConfig(
                "binding-only update changes tasks".into(),
            ));
        }
        let image_sizes = [
            resource.inputs_size as usize,
            resource.outputs_size as usize,
            resource.memory_size as usize,
        ];
        let task_bytes = resource
            .tasks
            .len()
            .checked_mul(
                core::mem::size_of::<TaskState>()
                    + core::mem::size_of::<crate::cycle::ReadyTask>()
                    + core::mem::size_of::<crate::bytecode::TaskEntry>(),
            )
            .ok_or(RuntimeError::Overflow)?;
        let mut allocation = task_bytes;
        for task in &resource.tasks {
            allocation = allocation
                .checked_add(
                    task.program_name_idx
                        .len()
                        .checked_add(task.fb_ref_idx.len())
                        .and_then(|n| n.checked_mul(core::mem::size_of::<u32>()))
                        .ok_or(RuntimeError::Overflow)?,
                )
                .ok_or(RuntimeError::Overflow)?;
        }
        for size in image_sizes {
            allocation = allocation.checked_add(size).ok_or(RuntimeError::Overflow)?;
        }
        budget.charge(allocation, allocation)?;
        let mut images = [Vec::new(), Vec::new(), Vec::new()];
        for ((target, size), old) in images.iter_mut().zip(image_sizes).zip([
            self.data.images.inputs.as_slice(),
            self.data.images.outputs.as_slice(),
            self.data.images.memory.as_slice(),
        ]) {
            target
                .try_reserve_exact(size)
                .map_err(|_| RuntimeError::PreparationLimit)?;
            target.resize(size, 0);
            let shared = size.min(old.len());
            target[..shared].copy_from_slice(&old[..shared]);
        }
        let mut tasks = Vec::new();
        tasks
            .try_reserve_exact(resource.tasks.len())
            .map_err(|_| RuntimeError::PreparationLimit)?;
        tasks.resize(resource.tasks.len(), TaskState::new(self.data.now));
        if reset_task_timing {
            for (state, task) in tasks.iter_mut().zip(&resource.tasks) {
                if let Some(name_idx) = task.single_name_idx {
                    let name = &prepared.vm.strings[name_idx as usize];
                    let (_, declaration) = prepared
                        .global(name)
                        .ok_or(RuntimeError::InvalidExecutionState)?;
                    state.last_single = match self
                        .data
                        .storage
                        .read_global_slot_by_offset(declaration.slot as usize)
                    {
                        Some(Value::Bool(value)) => *value,
                        _ => return Err(RuntimeError::TypeMismatch),
                    };
                }
            }
        }

        let mut ready = Vec::new();
        ready
            .try_reserve_exact(resource.tasks.len())
            .map_err(|_| RuntimeError::PreparationLimit)?;
        let resource = resource.clone();
        Ok(ResourceReconfiguration {
            owner: self,
            prepared,
            resource,
            resource_index,
            tasks: reset_task_timing.then_some(tasks),
            ready,
            images,
            budget,
        })
    }
}
impl<S: ExecutionServices> ResourceReconfiguration<'_, S> {
    /// Qualify retained safe-output policy without applying its values.
    pub fn validate_image_write(
        &mut self,
        address: &IoAddress,
        value: Value,
    ) -> Result<(), RuntimeError> {
        crate::io_image::validate_process_image_address(address)?;
        if address.path.len() > 1 {
            self.budget.charge(0, self.prepared.io_bindings.len())?;
            let binding = self
                .prepared
                .io_bindings
                .iter()
                .find(|binding| binding.address == *address)
                .ok_or_else(|| {
                    RuntimeError::InvalidIoAddress("unbound hierarchical image".into())
                })?;
            let bytes = match &value {
                Value::String(v) => v.len(),
                Value::WString(v) => v.len(),
                _ => 0,
            };
            self.budget.charge(bytes, bytes)?;
            binding
                .codec
                .encode(binding.codec.decode(value)?, address.size)?;
        } else {
            let area = match address.area {
                IoArea::Input => 0,
                IoArea::Output => 1,
                IoArea::Memory => 2,
            };
            let end = address
                .flat_byte_end()?
                .ok_or(RuntimeError::InvalidExecutionState)? as usize;
            if end > self.images[area].len() {
                return Err(RuntimeError::InvalidIoAddress(
                    "address exceeds admitted process image".into(),
                ));
            }
            let width = end
                .checked_sub(address.byte as usize)
                .ok_or(RuntimeError::Overflow)?;
            self.budget.charge(
                width
                    .checked_add(core::mem::size_of::<u32>())
                    .ok_or(RuntimeError::Overflow)?,
                width,
            )?;
            let mut window = Vec::new();
            window
                .try_reserve_exact(width)
                .map_err(|_| RuntimeError::PreparationLimit)?;
            window.resize(width, 0);
            let mut local = address.clone();
            local.byte = 0;
            local.path.clear();
            local.path.push(0);
            crate::io_image::write_flat_image(&mut window, &local, value)?;
        }
        Ok(())
    }
    /// Publish after all external schema work succeeds. The mutable borrow bars
    /// any intervening scan/image write, so staged image intersections stay current.
    pub fn commit(self) -> Result<(), RuntimeError> {
        if self.owner.services.deadline_exceeded() {
            return Err(RuntimeError::ExecutionTimeout);
        }
        let [inputs, outputs, memory] = self.images;
        self.owner.data.images.inputs = inputs;
        self.owner.data.images.outputs = outputs;
        self.owner.data.images.memory = memory;
        if let Some(tasks) = self.tasks {
            self.owner.data.tasks = tasks;
        }
        self.owner.data.resources.ready_tasks = self.ready;
        self.owner.data.resource = self.resource;
        self.owner.data.resource_index = self.resource_index;
        self.owner.prepared = self.prepared;
        Ok(())
    }
}

fn same_execution(a: &PreparedModule, b: &PreparedModule) -> bool {
    let x = &a.vm;
    let y = &b.vm;
    a.limits == b.limits
        && a.layout == b.layout
        && a.roots == b.roots
        && a.initializers == b.initializers
        && a.access == b.access
        && a.recipes == b.recipes
        && a.method_owners == b.method_owners
        && a.requires_wall_clock == b.requires_wall_clock
        && x.version == y.version
        && x.storage_abi == y.storage_abi
        && x.code == y.code
        && x.types == y.types
        && x.refs == y.refs
        && x.strings.len() <= y.strings.len()
        && x.strings.iter().zip(&y.strings).all(|(x, y)| x == y)
        && x.consts.len() == y.consts.len()
        && x.consts
            .iter()
            .zip(&y.consts)
            .all(|(x, y)| constant_equal(x, y))
        && x.pou_by_id == y.pou_by_id
        && x.program_ids == y.program_ids
        && x.function_block_ids == y.function_block_ids
        && x.class_ids == y.class_ids
        && x.parent_pou_ids == y.parent_pou_ids
        && x.interface_type_ids_by_pou == y.interface_type_ids_by_pou
        && x.pou_params == y.pou_params
        && x.pou_has_return_slot == y.pou_has_return_slot
        && x.method_table_by_owner == y.method_table_by_owner
        && x.ref_types == y.ref_types
        && x.instruction_budget == y.instruction_budget
        && {
            #[cfg(feature = "hir")]
            {
                x.function_ids == y.function_ids && x.debug_map == y.debug_map
            }
            #[cfg(not(feature = "hir"))]
            {
                true
            }
        }
}
fn constant_equal(a: &Value, b: &Value) -> bool {
    match (a, b) {
        (Value::Real(a), Value::Real(b)) => a.to_bits() == b.to_bits(),
        (Value::LReal(a), Value::LReal(b)) => a.to_bits() == b.to_bits(),
        (Value::Array(a), Value::Array(b)) => {
            a.dimensions() == b.dimensions()
                && a.elements().len() == b.elements().len()
                && a.elements()
                    .iter()
                    .zip(b.elements())
                    .all(|(a, b)| constant_equal(a, b))
        }
        (Value::Struct(a), Value::Struct(b)) => {
            a.type_name == b.type_name
                && a.fields().len() == b.fields().len()
                && a.fields()
                    .iter()
                    .zip(b.fields())
                    .all(|((an, av), (bn, bv))| an == bn && constant_equal(av, bv))
        }
        _ => a == b,
    }
}
