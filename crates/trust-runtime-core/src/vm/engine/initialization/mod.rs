//! Transactional lifecycle actions and executable default recipes.
use super::super::{
    construction::values::{construct_intrinsic_value, ValueConstructionContext},
    dispatch::execute_initializer_body,
    module::VmModule,
};
use super::*;
use crate::bytecode::{InitializationPhase, StorageOwner, StorageRole};
use crate::memory::MemoryLocation;

mod construction;
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
        self.charge_work_units(1)?;
        if depth as usize >= self.prepared.limits.max_call_depth {
            return Err(super::super::VmTrap::CallStackOverflow.into_runtime_error());
        }
        let entry = self
            .prepared
            .initializers
            .entries
            .get(id as usize)
            .cloned()
            .ok_or_else(|| invalid_bytecode("unknown initializer"))?;
        let context = entry.context_initializer_idx.unwrap_or(id);
        let result_type = self
            .prepared
            .vm
            .ref_type(entry.result_ref_idx)
            .ok_or_else(|| invalid_bytecode("untyped initializer result"))?;
        let code_end = (entry.code_offset as usize)
            .checked_add(entry.code_length as usize)
            .ok_or(RuntimeError::Overflow)?;
        let (seed, backups) = self.initializer_seed(&entry, instance, lexical.as_deref())?;
        let writable_instances = backups.iter().map(|(id, _)| *id).collect();
        let result = self.storage.reserve_execution_frame()?;
        let mut slot = vec![seed];
        self.storage.suspend_execution_frame(result, &mut slot)?;
        self.lifetimes.live_activations.push(result);
        let lexical_frame = lexical.as_ref().and_then(|frame| frame.activation);
        self.construction.initializers.push(ActiveInitializer {
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
                .and_then(|index| self.prepared.vm.consts.get(index as usize))
                .cloned();
            constant
                .map_or_else(
                    || {
                        self.with_construction_frame(&mut frame, |context| {
                            construct_intrinsic_value(context, result_type)
                        })
                    },
                    Ok,
                )
                .and_then(|value| {
                    if context_store_result(self, result, value) {
                        Ok(())
                    } else {
                        Err(RuntimeError::InvalidExecutionState)
                    }
                })
        } else {
            execute_initializer_body(self, &self.prepared.vm, &mut frame, depth)
        };
        if let Some(lexical) = lexical {
            lexical.locals = frame.locals;
        }
        let value = outcome.and_then(|()| {
            let mut values = Vec::new();
            self.storage.resume_execution_frame(result, &mut values)?;
            values.pop().ok_or(RuntimeError::InvalidExecutionState)
        });
        let destination = self
            .construction
            .initializers
            .iter()
            .rev()
            .nth(1)
            .map(|parent| parent.result)
            .or(lexical_frame)
            .or_else(|| {
                instance.and_then(|id| {
                    self.lifetimes
                        .instance_lifetimes
                        .get(&id)
                        .copied()
                        .flatten()
                })
            });
        let originally_owned = self
            .lifetimes
            .owned_instances
            .get(&result)
            .cloned()
            .unwrap_or_default();
        let value = value.and_then(|value| {
            self.promote_constructed_instances(&value, result, destination)?;
            self.check_value_lifetime(&value, destination)?;
            Ok(value)
        });
        if value.is_err() {
            self.restore_initializer_instances(backups);
            for instance in originally_owned {
                if self.lifetimes.instance_lifetimes.contains_key(&instance) {
                    self.lifetimes
                        .instance_lifetimes
                        .insert(instance, Some(result));
                }
            }
        }
        self.construction.initializers.pop();
        self.storage.release_execution_frame(result);
        self.lifetimes
            .live_activations
            .retain(|candidate| *candidate != result);
        self.remove_owned_instances(result);
        value
    }

    pub(in crate::vm::engine) fn with_construction_frame<T>(
        &mut self,
        frame: &mut VmFrame,
        action: impl FnOnce(&mut Self) -> Result<T, RuntimeError>,
    ) -> Result<T, RuntimeError> {
        let locals = core::mem::take(&mut frame.locals);
        let mut context = frame.clone();
        context.locals = locals;
        self.construction.frames.push(context);
        let result = action(self);
        if let Some(context) = self.construction.frames.pop() {
            frame.locals = context.locals;
        }
        result
    }
}

fn context_store_result(state: &mut EngineState<'_>, frame: FrameId, value: Value) -> bool {
    state
        .storage
        .write_by_ref_parts(MemoryLocation::Local(frame), 0, &[], value)
}

#[cfg(test)]
mod tests;
