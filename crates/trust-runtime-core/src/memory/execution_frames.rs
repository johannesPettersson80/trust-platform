//! Ownership transfer for locals of suspended bytecode activations.
use super::*;
use crate::error::RuntimeError;

impl VariableStorage {
    /// Reserve activation metadata before RUN, including initializer result frames.
    pub fn reserve_execution_frames(&mut self, capacity: usize) -> Result<(), RuntimeError> {
        if capacity < self.execution_frames.len() {
            return Err(RuntimeError::InvalidExecutionState);
        }
        self.execution_frames.reserve_capacity(capacity)?;
        self.execution_frame_limit = Some(capacity);
        Ok(())
    }

    /// Bytes reserved for activation identity, ownership and local-slot metadata,
    /// excluding the local payload vectors themselves.
    pub fn execution_frame_reservation_charge(capacity: usize) -> Option<usize> {
        capacity.checked_mul(core::mem::size_of::<(FrameId, ExecutionActivation)>())
    }

    /// Reserved execution-frame identities, active or not.
    pub fn execution_frame_count(&self) -> usize {
        self.execution_frames.len()
    }

    pub(crate) fn execution_frame_capacity(&self) -> usize {
        self.execution_frames.capacity()
    }

    fn execution_activation(&self, id: FrameId) -> Option<&ExecutionActivation> {
        if self
            .execution_frames
            .last()
            .is_some_and(|(last, _)| *last == id)
        {
            self.execution_frames
                .last()
                .map(|(_, activation)| activation)
        } else {
            self.execution_frames.get(&id)
        }
    }

    fn execution_activation_mut(&mut self, id: FrameId) -> Option<&mut ExecutionActivation> {
        if self
            .execution_frames
            .last()
            .is_some_and(|(last, _)| *last == id)
        {
            self.execution_frames.get_mut_last(&id)
        } else {
            self.execution_frames.get_mut(&id)
        }
    }

    /// Reserve an identity without activating it or reusing a completed ID.
    pub fn reserve_execution_frame(&mut self) -> Result<FrameId, RuntimeError> {
        let Some(next) = self.next_frame_id.checked_add(1) else {
            return Err(RuntimeError::Overflow);
        };
        let id = FrameId(self.next_frame_id);
        if self
            .execution_frame_limit
            .is_some_and(|limit| self.execution_frames.len() >= limit)
        {
            return Err(crate::vm::VmTrap::CallStackOverflow.into_runtime_error());
        }
        self.execution_frames
            .try_append(id, ExecutionActivation::default())?;
        self.next_frame_id = next;
        Ok(id)
    }

    /// Mark a reserved identity as part of the live activation order.
    pub(crate) fn activate_execution_frame(&mut self, id: FrameId) -> Result<(), RuntimeError> {
        let Some((last_id, _)) = self.execution_frames.last() else {
            return Err(RuntimeError::InvalidExecutionState);
        };
        if *last_id == id {
            let Some(activation) = self.execution_frames.get_mut_last(&id) else {
                return Err(RuntimeError::InvalidExecutionState);
            };
            if activation.active {
                return Err(RuntimeError::InvalidExecutionState);
            }
            activation.active = true;
            return Ok(());
        }
        let Some(existing) = self.execution_activation(id) else {
            return Err(RuntimeError::InvalidExecutionState);
        };
        if existing.active || *last_id < id {
            return Err(RuntimeError::InvalidExecutionState);
        }
        let latest_active = self
            .execution_frames
            .as_slice()
            .iter()
            .rev()
            .find(|(_, candidate)| candidate.active)
            .map(|(candidate, _)| *candidate);
        if latest_active.is_some_and(|latest| latest >= id) {
            return Err(RuntimeError::InvalidExecutionState);
        }
        let Some(activation) = self.execution_frames.get_mut(&id) else {
            return Err(RuntimeError::InvalidExecutionState);
        };
        activation.active = true;
        Ok(())
    }

    /// Whether a reserved identity has entered the engine's live activation order.
    pub(crate) fn execution_activation_is_live(&self, id: FrameId) -> bool {
        self.execution_activation(id)
            .is_some_and(|activation| activation.active)
    }

    pub(crate) fn execution_activation_count(&self) -> usize {
        self.execution_frames
            .values()
            .filter(|activation| activation.active)
            .count()
    }

    #[cfg(test)]
    pub(crate) fn execution_frame_owner_count(&self) -> usize {
        self.execution_frames
            .values()
            .filter(|activation| activation.owner.is_some())
            .count()
    }

    pub(crate) fn last_execution_activation(&self) -> Option<FrameId> {
        self.execution_frames
            .as_slice()
            .iter()
            .rev()
            .find(|(_, activation)| activation.active)
            .map(|(id, _)| *id)
    }

    pub(crate) fn previous_execution_activation(&self, id: FrameId) -> Option<FrameId> {
        let entries = self.execution_frames.as_slice();
        let index = entries
            .binary_search_by(|(candidate, _)| candidate.cmp(&id))
            .ok()?;
        if !entries[index].1.active {
            return None;
        }
        entries[..index]
            .iter()
            .rev()
            .find(|(_, activation)| activation.active)
            .map(|(candidate, _)| *candidate)
    }

    /// Bind an activated ordinary frame to its admitted POU and runtime instance.
    pub(crate) fn set_execution_frame_owner(
        &mut self,
        id: FrameId,
        pou_id: u32,
        runtime_instance: Option<InstanceId>,
    ) -> Result<(), RuntimeError> {
        let activation = self.execution_activation_mut(id);
        let Some(activation) = activation else {
            return Err(RuntimeError::InvalidExecutionState);
        };
        if !activation.active || activation.owner.is_some() {
            return Err(RuntimeError::InvalidExecutionState);
        }
        activation.owner = Some(ExecutionFrameOwner {
            pou_id,
            runtime_instance,
        });
        Ok(())
    }

    pub(crate) fn execution_frame_owner(&self, id: FrameId) -> Option<ExecutionFrameOwner> {
        self.execution_activation(id)?.owner
    }

    /// Project suspended activations that have ordinary POU owner metadata.
    pub(crate) fn suspended_execution_activations(
        &self,
    ) -> impl Iterator<Item = (FrameId, ExecutionFrameOwner, &[Value])> {
        self.execution_frames
            .as_slice()
            .iter()
            .filter_map(|(id, activation)| {
                if !activation.active {
                    return None;
                }
                let owner = activation.owner?;
                let locals = activation.locals.as_deref()?;
                Some((*id, owner, locals))
            })
    }

    /// Transfer ownership of suspended locals without copying values or references.
    pub fn suspend_execution_frame(
        &mut self,
        id: FrameId,
        locals: &mut Vec<Value>,
    ) -> Result<(), RuntimeError> {
        let Some(slot) = self.execution_activation_mut(id) else {
            return Err(RuntimeError::InvalidExecutionState);
        };
        if slot.locals.is_some() {
            return Err(RuntimeError::InvalidExecutionState);
        }
        slot.locals = Some(core::mem::take(locals));
        Ok(())
    }

    /// Restore the same local vector, including mutations through ancestor references.
    pub fn resume_execution_frame(
        &mut self,
        id: FrameId,
        locals: &mut Vec<Value>,
    ) -> Result<(), RuntimeError> {
        if !locals.is_empty() {
            return Err(RuntimeError::InvalidExecutionState);
        }
        let Some(slot) = self.execution_activation_mut(id) else {
            return Err(RuntimeError::InvalidExecutionState);
        };
        let Some(suspended) = slot.locals.take() else {
            return Err(RuntimeError::InvalidExecutionState);
        };
        *locals = suspended;
        Ok(())
    }

    /// Borrow suspended locals for bounded snapshot accounting.
    pub fn suspended_execution_values(&self) -> impl Iterator<Item = &Value> {
        self.execution_frames
            .values()
            .flat_map(|activation| activation.locals.as_ref())
            .flat_map(|values| values.iter())
    }

    /// Invalidate all references to a completed activation.
    pub fn release_execution_frame(&mut self, id: FrameId) -> bool {
        self.execution_frames
            .remove_last(&id)
            .or_else(|| self.execution_frames.remove(&id))
            .is_some()
    }

    /// Whether an identity still has a reserved execution-frame slot.
    /// A reservation can exist before the engine activates the invocation.
    pub fn execution_frame_is_live(&self, id: FrameId) -> bool {
        self.execution_activation(id).is_some()
    }

    pub(super) fn local_slot(&self, id: FrameId, offset: usize) -> Option<&Value> {
        if let Some(activation) = self.execution_activation(id) {
            return activation.locals.as_ref()?.get(offset);
        }
        self.frames
            .iter()
            .find(|frame| frame.id == id)
            .and_then(|frame| frame.variables.get_index(offset).map(|(_, value)| value))
    }

    pub(super) fn local_slot_mut(&mut self, id: FrameId, offset: usize) -> Option<&mut Value> {
        if self.execution_activation(id).is_some() {
            return self
                .execution_activation_mut(id)?
                .locals
                .as_mut()?
                .get_mut(offset);
        }
        self.frames
            .iter_mut()
            .find(|frame| frame.id == id)
            .and_then(|frame| {
                frame
                    .variables
                    .get_index_mut(offset)
                    .map(|(_, value)| value)
            })
    }
}

#[cfg(test)]
#[path = "execution_frames/tests.rs"]
mod tests;
