//! The engine's part of the memory figure (specification 34 MEM-12; checklist C1-21): the bytes of
//! heap that construction reserves for a prepared module's resource and that the engine then holds
//! while it runs, computed from the module as construction computes them. A scan allocates nothing
//! (ARCH-05), so these bytes stay what they are after construction.

use super::identities::instance_bookkeeping_bytes;
use super::scratch::{scratch_heap_parts, ReferenceBuffers};
use super::*;
use crate::bytecode::TaskEntry;
use crate::cycle::ReadyTask;
use crate::memory::cell_access::CellArm;
use crate::memory::regions::CellStorage;
use crate::task::TaskState;

/// The bytes that construction reserves, by the part of the memory figure they belong to.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub(crate) struct EngineParts {
    pub(crate) persistent_block: usize,
    pub(crate) activation_stack: usize,
    pub(crate) operand_stack: usize,
    pub(crate) engine: usize,
    pub(crate) retain_capture: usize,
}

/// The bytes construction reserves for resource 0 of `prepared`, the only resource a prepared
/// module admits.
///
/// # Errors
///
/// [`RuntimeError::Overflow`] when a part exceeds the address range, and
/// [`RuntimeError::InvalidExecutionState`] for a module without a resource.
pub(crate) fn engine_parts(prepared: &PreparedModule) -> Result<EngineParts, RuntimeError> {
    let overflow = || RuntimeError::Overflow;
    let add = |parts: &[usize]| {
        parts
            .iter()
            .try_fold(0usize, |sum, bytes| sum.checked_add(*bytes))
            .ok_or_else(overflow)
    };
    let resource = prepared
        .resources
        .resources
        .first()
        .ok_or(RuntimeError::InvalidExecutionState)?;
    let tasks = resource.tasks.len();
    let task_lists = resource
        .tasks
        .iter()
        .map(|task| task.program_name_idx.len() + task.fb_ref_idx.len())
        .sum::<usize>()
        * core::mem::size_of::<u32>();
    let images = add(&[
        resource.inputs_size as usize,
        resource.outputs_size as usize,
        resource.memory_size as usize,
    ])?;
    // Calls and staged results each take an execution frame on every reserved call level.
    let frame_capacity = prepared
        .reserved_call_levels()
        .checked_mul(2)
        .ok_or_else(overflow)?;
    let shape = crate::vm::prepared::operand_bounds::PathShape::of(prepared);
    let leaves = CellStorage::persistent_reference_leaves_of(prepared)?;
    let stored_references = if leaves == 0 {
        0
    } else {
        ReferenceBuffers::reservation_bytes(
            (leaves, shape.segments),
            (
                leaves.checked_mul(shape.indexes).ok_or_else(overflow)?,
                shape.subscripts,
            ),
        )
        .ok_or_else(overflow)?
    };
    let (store_persistent, store_activation) = CellStorage::heap_bytes(prepared)?;
    let scratch = scratch_heap_parts(prepared)?;
    #[cfg(feature = "register-ir")]
    let (register_buffers, register_telemetry) = prepared
        .register_plans
        .state_heap_bytes(&prepared.vm)
        .ok_or_else(overflow)?;
    #[cfg(not(feature = "register-ir"))]
    let (register_buffers, register_telemetry) = (0, 0);
    Ok(EngineParts {
        persistent_block: add(&[
            store_persistent,
            stored_references,
            crate::memory::cells::largest_text_edit(prepared).ok_or_else(overflow)?,
        ])?,
        activation_stack: add(&[
            store_activation,
            scratch.activation,
            VariableStorage::execution_frame_reservation_charge(frame_capacity)
                .ok_or_else(overflow)?,
            frame_capacity * core::mem::size_of::<(u32, Vec<u32>)>(),
        ])?,
        operand_stack: add(&[scratch.operands, register_buffers])?,
        engine: add(&[
            core::mem::size_of::<EngineData>(),
            core::mem::size_of::<CellArm>(),
            // The image staging, in a box of its own (checklist P1-6).
            core::mem::size_of::<io::staging::ImageStaging>(),
            images,
            prepared.roots.entries.len() * core::mem::size_of::<Option<InstanceId>>(),
            tasks * core::mem::size_of::<TaskState>(),
            tasks * core::mem::size_of::<TaskEntry>() + task_lists,
            tasks * core::mem::size_of::<ReadyTask>(),
            scratch.engine,
            // The copy of the output image that a publication to output hooks keeps (C1-19f).
            resource.outputs_size as usize,
            prepared.vm.types.entries.len().div_ceil(2),
            prepared.input_snapshot_bytes()?,
            instance_bookkeeping_bytes(&prepared.instance_bookkeeping()?).ok_or_else(overflow)?,
            register_telemetry,
        ])?,
        retain_capture: retain_capture::retained_payload_bound(prepared)
            .and_then(|bytes| bytes.checked_mul(2))
            .ok_or_else(overflow)?,
    })
}
