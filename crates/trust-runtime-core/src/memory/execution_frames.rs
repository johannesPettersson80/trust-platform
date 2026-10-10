//! Ownership transfer for locals of suspended bytecode activations.
use super::*;
use crate::error::RuntimeError;

impl VariableStorage {
    /// Reserve a live identity without reusing one from a completed activation.
    pub fn reserve_execution_frame(&mut self) -> Result<FrameId, RuntimeError> {
        let next = self
            .next_frame_id
            .checked_add(1)
            .ok_or(RuntimeError::Overflow)?;
        let id = FrameId(self.next_frame_id);
        self.next_frame_id = next;
        self.execution_frames.insert(id, None);
        Ok(id)
    }

    /// Transfer ownership of suspended locals without copying values or references.
    pub fn suspend_execution_frame(
        &mut self,
        id: FrameId,
        locals: &mut Vec<Value>,
    ) -> Result<(), RuntimeError> {
        let slot = self
            .execution_frames
            .get_mut(&id)
            .ok_or(RuntimeError::InvalidExecutionState)?;
        if slot.is_some() {
            return Err(RuntimeError::InvalidExecutionState);
        }
        *slot = Some(core::mem::take(locals));
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
        let slot = self
            .execution_frames
            .get_mut(&id)
            .ok_or(RuntimeError::InvalidExecutionState)?;
        *locals = slot.take().ok_or(RuntimeError::InvalidExecutionState)?;
        Ok(())
    }

    /// Borrow suspended locals for bounded snapshot accounting.
    pub fn suspended_execution_values(&self) -> impl Iterator<Item = &Value> {
        self.execution_frames
            .values()
            .flatten()
            .flat_map(|values| values.iter())
    }

    /// Invalidate all references to a completed activation.
    pub fn release_execution_frame(&mut self, id: FrameId) -> bool {
        self.execution_frames.remove(&id).is_some()
    }

    /// Whether an identity still belongs to an active or suspended invocation.
    pub fn execution_frame_is_live(&self, id: FrameId) -> bool {
        self.execution_frames.contains_key(&id)
    }

    pub(super) fn local_slot(&self, id: FrameId, offset: usize) -> Option<&Value> {
        if let Some(locals) = self.execution_frames.get(&id) {
            return locals.as_ref()?.get(offset);
        }
        self.frames
            .iter()
            .find(|frame| frame.id == id)
            .and_then(|frame| frame.variables.get_index(offset).map(|(_, value)| value))
    }

    pub(super) fn local_slot_mut(&mut self, id: FrameId, offset: usize) -> Option<&mut Value> {
        if self.execution_frames.contains_key(&id) {
            return self
                .execution_frames
                .get_mut(&id)?
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
