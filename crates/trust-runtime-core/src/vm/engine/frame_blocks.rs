//! Frames on the activation stack (specification 34 MEM-12; C1-11).
//!
//! On the flat store a call's frame lies on the activation stack, on top of its caller's: each
//! typed variable at the offset its POU's frame layout gives it, at its declared size. A slot that
//! has no type (compiler scratch) or whose value holds a reference stays a value of the frame
//! until references are flat. The frame's activation identity finds its bytes, so the reads and
//! writes of the dispatcher, the calls and the initializers reach a slot through the store as
//! `MemoryLocation::Local(id)`, whatever frame they run in.

use super::*;
use crate::memory::layout::{ByteRange, LayoutRegion};
use crate::memory::regions::FrameBlock;
use crate::memory::view::{StoreAccess, StoreMutate};
use crate::memory::MemoryLocation;

impl EngineState<'_> {
    /// Place the frame of activation `id` on top of the activation stack, its bytes zero, and move
    /// into those bytes every value the frame holds for a slot that lies there: the inputs its call
    /// bound and a return value its caller supplied. The defaults that follow write the slots no
    /// value was given for. A frame of a POU without typed variables, and any frame on the value
    /// store, keeps its values.
    pub(super) fn place_frame(
        &mut self,
        frame: &mut VmFrame,
        id: FrameId,
    ) -> Result<(), RuntimeError> {
        let Some(pou) = frame.pou_id else {
            return Ok(());
        };
        let Some(locals) = self
            .prepared
            .local_range(pou)
            .filter(|range| range.bytes != 0)
        else {
            return Ok(());
        };
        let EngineStore::Cells(arm) = &mut self.data.store else {
            return Ok(());
        };
        let range = ByteRange {
            region: LayoutRegion::Activation,
            start: arm.activation_top(),
            bytes: locals.bytes,
        };
        if !arm.clear_activation(range) {
            return Err(crate::vm::VmTrap::CallStackOverflow.into_runtime_error());
        }
        arm.push_frame(FrameBlock { id, range, pou })?;
        for slot in 0..frame.locals.len() {
            if self.prepared.frame_slot(pou, slot).is_none()
                || matches!(frame.locals[slot], Value::Null)
            {
                continue;
            }
            let value = core::mem::replace(&mut frame.locals[slot], Value::Null);
            if !StoreMutate::write_slot(self, MemoryLocation::Local(id), slot, value) {
                return Err(RuntimeError::InvalidExecutionState);
            }
        }
        Ok(())
    }

    /// Take the frame of activation `id` off the activation stack. Its bytes stay as they are until
    /// the next frame takes them, so a finished call's outputs are read from them.
    pub(super) fn release_frame_block(&mut self, id: FrameId) {
        if let EngineStore::Cells(arm) = &mut self.data.store {
            arm.release_frame(id);
        }
    }

    /// The frame `frame` has on the activation stack, when it has one.
    pub(super) fn frame_block(&self, frame: &VmFrame) -> Option<FrameBlock> {
        let EngineStore::Cells(arm) = &self.data.store else {
            return None;
        };
        arm.frame(frame.activation?)
    }

    /// The activation whose frame holds slot `slot` of `frame` on the activation stack, or `None`
    /// when the frame keeps the slot as a value.
    pub(super) fn frame_slot_activation(&self, frame: &VmFrame, slot: usize) -> Option<FrameId> {
        let block = self.frame_block(frame)?;
        self.prepared.frame_slot(block.pou, slot)?;
        Some(block.id)
    }

    /// Give slot `slot` of `frame` the value `value`: in its bytes on the activation stack, or as
    /// the frame's value when the frame keeps the slot as one.
    pub(super) fn store_frame_slot(
        &mut self,
        frame: &mut VmFrame,
        slot: usize,
        value: Value,
    ) -> Result<(), RuntimeError> {
        if let Some(id) = self.frame_slot_activation(frame, slot) {
            return if StoreMutate::write_slot(self, MemoryLocation::Local(id), slot, value) {
                Ok(())
            } else {
                Err(RuntimeError::InvalidExecutionState)
            };
        }
        let target = frame
            .locals
            .get_mut(slot)
            .ok_or(RuntimeError::InvalidExecutionState)?;
        *target = value;
        Ok(())
    }

    /// The value of slot `slot` of `frame` read from its bytes on the activation stack, or `None`
    /// when the frame keeps the slot as a value.
    pub(super) fn read_frame_slot(
        &self,
        frame: &VmFrame,
        slot: usize,
    ) -> Option<Result<Value, RuntimeError>> {
        let id = self.frame_slot_activation(frame, slot)?;
        Some(
            StoreAccess::read_slot(self, MemoryLocation::Local(id), slot)
                .map_err(crate::vm::VmTrap::into_runtime_error)
                .and_then(|view| view.materialize()),
        )
    }

    /// The value of slot `slot` of the finished frame `block`, read from its bytes, or `None` when
    /// the frame kept the slot as a value.
    pub(super) fn read_finished_slot(
        &self,
        block: FrameBlock,
        slot: usize,
    ) -> Option<Result<Value, RuntimeError>> {
        let super::store_access::Routed::Cells(reader) = self.data.routed(self.prepared) else {
            return None;
        };
        let view = reader.frame_slot_view(block.range, block.pou, slot)?;
        Some(view.materialize())
    }
}
