//! Transactional lifecycle actions and executable default recipes.
use super::super::{
    construction::values::{construct_intrinsic_value, ValueConstructionContext},
    dispatch::execute_initializer_body,
    module::VmModule,
};
use super::*;
use crate::bytecode::{
    InitializationPhase, InitializerBodyKind, StorageOwner, StorageRole, TypeData,
};
use crate::memory::MemoryLocation;
use crate::vm::context::ReferenceContext;

#[cfg(test)]
mod cleanup_tests;
mod construction;
mod frame_defaults;
mod opcodes;
mod staging;
mod values;

impl EngineState<'_> {
    pub(in crate::vm::engine) fn evaluate_initializer(
        &mut self,
        id: u32,
        mut lexical: Option<&mut VmFrame>,
        instance: Option<InstanceId>,
        depth: u32,
    ) -> Result<Value, RuntimeError> {
        #[cfg(all(feature = "std", feature = "test-support"))]
        let _timing = crate::vm::diagnostic_timing::begin(
            crate::vm::diagnostic_timing::Phase::InitializerEvaluate,
        );
        if depth as usize >= self.prepared.limits.max_call_depth {
            return Err(super::super::VmTrap::CallStackOverflow.into_runtime_error());
        }
        let entry = self
            .prepared
            .initializers
            .entries
            .get(id as usize)
            .cloned()
            .ok_or_else(|| {
                invalid_bytecode(smol_str::SmolStr::new_static("unknown initializer"))
            })?;
        let context = entry.context_initializer_idx.unwrap_or(id);
        let result_type = self
            .prepared
            .vm
            .ref_type(entry.result_ref_idx)
            .ok_or_else(|| {
                invalid_bytecode(smol_str::SmolStr::new_static("untyped initializer result"))
            })?;
        let Some(code_end) = (entry.code_offset as usize).checked_add(entry.code_length as usize)
        else {
            return Err(RuntimeError::Overflow);
        };
        #[cfg(all(feature = "std", feature = "test-support"))]
        let branch_metadata = initializer_branch_metadata(self.prepared, id, &entry, result_type);
        // An admitted literal default cannot create an instance or live
        // reference. It needs the charged independent copy and lifetime/depth
        // walk, but no execution frame, staged identity or dispatcher entry.
        if entry.stage == crate::bytecode::InitializationStage::Default
            && entry.code_length == 0
            && let Some(value) = entry
                .declaration_idx
                .and_then(|index| self.prepared.layout.entries.get(index as usize))
                .and_then(|declaration| declaration.default_const_idx)
                .and_then(|index| self.prepared.vm.consts.get(index as usize))
        {
            #[cfg(all(feature = "std", feature = "test-support"))]
            log_initializer_branch(
                branch_metadata,
                crate::vm::diagnostic_timing::InitializerBranch::LiteralDefaultFastPath,
            );
            self.before_value_clone(value)?;
            let value = value.clone();
            let destination = self
                .data
                .construction
                .initializers
                .last()
                .map(|parent| parent.result)
                .or_else(|| lexical.as_ref().and_then(|frame| frame.activation))
                .or_else(|| {
                    instance
                        .and_then(|id| self.data.lifetimes.instance_lifetimes.get(&id).flatten())
                });
            self.check_value_lifetime(&value, destination)?;
            return Ok(value);
        }
        if entry.stage == crate::bytecode::InitializationStage::Default
            && entry.code_length == 0
            && entry.body_kind == InitializerBodyKind::Action
            && entry.context_initializer_idx.is_none()
            && entry.recipe_type_id.is_none()
            && entry.recipe_member_idx.is_none()
            && entry
                .declaration_idx
                .and_then(|index| self.prepared.layout.entries.get(index as usize))
                .is_some_and(|declaration| declaration.default_const_idx.is_none())
            && direct_scalar_intrinsic_default(&self.prepared.vm, result_type)
        {
            #[cfg(all(feature = "std", feature = "test-support"))]
            log_initializer_branch(
                branch_metadata,
                crate::vm::diagnostic_timing::InitializerBranch::DirectScalarIntrinsicFastPath,
            );
            // The shared constructor retains type, budget and lifetime checks;
            // scalar defaults need no staged identity or temporary frame.
            let prepared = self.prepared;
            return construct_intrinsic_value(&prepared.vm.types, self, result_type);
        }
        let staging::InitializerSeed {
            value: seed,
            backups,
            writable_instances,
        } = self.initializer_seed(&entry, instance, lexical.as_deref())?;
        let result = self.data.storage.reserve_execution_frame()?;
        let mut slot = vec![seed];
        self.data
            .storage
            .suspend_execution_frame(result, &mut slot)?;
        self.data.storage.activate_execution_frame(result)?;
        let lexical_frame = lexical.as_ref().and_then(|frame| frame.activation);
        self.data.construction.initializers.push(ActiveInitializer {
            id,
            context,
            entry: entry.clone(),
            result,
            lexical_frame,
            instance,
            depth,
            writable_instances,
        });
        let template = entry
            .owner_pou_id
            .and_then(|owner| self.prepared.vm.pou(owner));
        let mut frame = VmFrame {
            parameter_values_present: Vec::new(),
            activation: lexical_frame,
            pou_id: entry.owner_pou_id,
            return_pc: usize::MAX,
            code_start: entry.code_offset as usize,
            code_end,
            local_ref_start: template.map_or(0, |pou| pou.local_ref_start),
            local_ref_count: template.map_or(0, |pou| pou.local_ref_count),
            locals: lexical
                .as_mut()
                .map_or_else(Vec::new, |frame| core::mem::take(&mut frame.locals)),
            runtime_instance: instance,
            instance_owner: template.and_then(|pou| pou.primary_instance_owner),
        };
        let outcome = if entry.code_length == 0 {
            let constant = entry
                .declaration_idx
                .and_then(|index| self.prepared.layout.entries.get(index as usize))
                .and_then(|declaration| declaration.default_const_idx)
                .and_then(|index| self.prepared.vm.consts.get(index as usize));
            let value = match constant {
                Some(value) => {
                    #[cfg(all(feature = "std", feature = "test-support"))]
                    log_initializer_branch(
                        branch_metadata,
                        crate::vm::diagnostic_timing::InitializerBranch::StagedConstant,
                    );
                    self.before_value_clone(value).map(|()| value.clone())
                }
                None => self.with_construction_frame(&mut frame, |context| {
                    #[cfg(all(feature = "std", feature = "test-support"))]
                    log_initializer_branch(
                        branch_metadata,
                        crate::vm::diagnostic_timing::InitializerBranch::StagedIntrinsic,
                    );
                    let prepared = context.prepared;
                    construct_intrinsic_value(&prepared.vm.types, context, result_type)
                }),
            };
            value.and_then(|value| {
                if context_store_result(self, result, value) {
                    Ok(())
                } else {
                    Err(RuntimeError::InvalidExecutionState)
                }
            })
        } else {
            #[cfg(all(feature = "std", feature = "test-support"))]
            log_initializer_branch(
                branch_metadata,
                crate::vm::diagnostic_timing::InitializerBranch::InitializerBody,
            );
            execute_initializer_body(self, &self.prepared.vm, &mut frame, depth)
        };
        if let Some(lexical) = lexical {
            lexical.locals = frame.locals;
        }
        self.finish_initializer(outcome, result, lexical_frame, instance, backups)
    }

    // Keep finalization's temporaries out of the evaluator frame that remains
    // live across synchronous initializer/default-recipe dispatcher entries.
    #[inline(never)]
    fn finish_initializer(
        &mut self,
        outcome: Result<(), RuntimeError>,
        result: FrameId,
        lexical_frame: Option<FrameId>,
        instance: Option<InstanceId>,
        backups: Vec<(InstanceId, crate::memory::InstanceData)>,
    ) -> Result<Value, RuntimeError> {
        let value = outcome.and_then(|()| {
            let mut values = Vec::new();
            self.data
                .storage
                .resume_execution_frame(result, &mut values)?;
            values
                .pop()
                .ok_or_else(|| RuntimeError::InvalidExecutionState)
        });
        let destination = self
            .data
            .construction
            .initializers
            .iter()
            .rev()
            .nth(1)
            .map(|parent| parent.result)
            .or(lexical_frame)
            .or_else(|| {
                instance.and_then(|id| self.data.lifetimes.instance_lifetimes.get(&id).flatten())
            });
        let mut originally_owned = Vec::new();
        let value = value.and_then(|value| {
            originally_owned = self.snapshot_owned_instances(result)?;
            self.promote_constructed_instances(&value, result, destination)?;
            self.check_value_lifetime(&value, destination)?;
            Ok(value)
        });
        if value.is_err() {
            self.restore_initializer_instances(backups);
            for instance in originally_owned {
                if self
                    .data
                    .lifetimes
                    .instance_lifetimes
                    .contains_key(&instance)
                {
                    self.data
                        .lifetimes
                        .instance_lifetimes
                        .restore(instance, Some(result));
                }
            }
        }
        self.data.construction.initializers.pop();
        self.data.storage.release_execution_frame(result);
        self.remove_owned_instances_unchecked(result);
        value
    }

    fn snapshot_owned_instances(&self, owner: FrameId) -> Result<Vec<InstanceId>, RuntimeError> {
        let count = self.data.lifetimes.owned_instances.count(owner);
        let bytes = count
            .checked_mul(core::mem::size_of::<InstanceId>())
            .ok_or_else(|| RuntimeError::Overflow)?;
        self.charge_allocation_bytes(bytes)?;
        let mut snapshot = Vec::new();
        snapshot
            .try_reserve_exact(count)
            .map_err(|_| RuntimeError::PreparationLimit)?;
        self.data
            .lifetimes
            .owned_instances
            .copy_snapshot(owner, &mut snapshot);
        Ok(snapshot)
    }

    pub(in crate::vm::engine) fn with_construction_frame<T>(
        &mut self,
        frame: &mut VmFrame,
        action: impl FnOnce(&mut Self) -> Result<T, RuntimeError>,
    ) -> Result<T, RuntimeError> {
        let locals = core::mem::take(&mut frame.locals);
        let mut context = frame.clone();
        context.locals = locals;
        self.data.construction.frames.push(context);
        let result = action(self);
        if let Some(context) = self.data.construction.frames.pop() {
            frame.locals = context.locals;
        }
        result
    }
}

#[cfg(all(feature = "std", feature = "test-support"))]
fn initializer_branch_metadata(
    prepared: &PreparedModule,
    id: u32,
    entry: &crate::bytecode::InitializerEntry,
    result_type: u32,
) -> Option<crate::vm::diagnostic_timing::InitializerBranchMetadata> {
    if !crate::vm::diagnostic_timing::initializer_branch_log_enabled() {
        return None;
    }
    let declaration = entry
        .declaration_idx
        .and_then(|index| prepared.layout.entries.get(index as usize));
    let primitive_id = prepared
        .vm
        .types
        .entries
        .get(result_type as usize)
        .and_then(|entry| match &entry.data {
            TypeData::Primitive { prim_id, .. } => Some(*prim_id),
            _ => None,
        });
    Some(crate::vm::diagnostic_timing::InitializerBranchMetadata {
        initializer_id: id,
        declaration_idx: entry.declaration_idx,
        declaration_role: declaration.map(|declaration| declaration.role),
        result_type_id: result_type,
        primitive_id,
        stage: entry.stage,
        body_kind: entry.body_kind,
        code_length: entry.code_length,
        default_const_idx: declaration.and_then(|declaration| declaration.default_const_idx),
    })
}

#[cfg(all(feature = "std", feature = "test-support"))]
fn log_initializer_branch(
    metadata: Option<crate::vm::diagnostic_timing::InitializerBranchMetadata>,
    branch: crate::vm::diagnostic_timing::InitializerBranch,
) {
    if let Some(metadata) = metadata {
        crate::vm::diagnostic_timing::record_initializer_branch(metadata, branch);
    }
}

fn direct_scalar_intrinsic_default(module: &VmModule, type_id: u32) -> bool {
    matches!(
        module
            .types
            .entries
            .get(type_id as usize)
            .map(|entry| &entry.data),
        Some(TypeData::Primitive {
            prim_id: 1..=23 | 26 | 27,
            max_length: 0,
        })
    )
}

fn context_store_result(state: &mut EngineState<'_>, frame: FrameId, value: Value) -> bool {
    state
        .data
        .storage
        .write_by_ref_parts(MemoryLocation::Local(frame), 0, &[], value)
}

#[cfg(test)]
mod call_path_literal_default_tests;
#[cfg(test)]
mod call_path_nesting_tests;
#[cfg(test)]
mod default_constant_tests;
#[cfg(test)]
mod tests;
#[cfg(test)]
mod work_allowance_default_tests;
