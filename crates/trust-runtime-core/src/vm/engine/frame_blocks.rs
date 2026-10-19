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

    /// The place the input `reference` names, when slot `slot` of `pou` lies in its frame's bytes
    /// and the input is a place of the parameter's type `parameter` (C1-19c4): read as the value
    /// path reads it, its read check and the charge of its copy, then the charge of its
    /// normalization to its own type.
    pub(super) fn input_place(
        &self,
        pou: u32,
        slot: usize,
        parameter: u32,
        reference: &crate::value::ValueRef,
    ) -> Option<Result<crate::vm::Place, RuntimeError>> {
        self.prepared.frame_slot(pou, slot)?;
        let address = reference.as_view();
        let view = StoreAccess::read_reference(self, address).ok()?;
        let place = self.place_of(&view)?;
        if !crate::vm::prepared::byte_types::same_type(self.prepared, place.type_id, parameter, 0) {
            return None;
        }
        Some(
            crate::vm::context::ReferenceContext::check_reference_read(self, address)
                .and_then(|()| crate::vm::context::ReferenceContext::before_view_clone(self, &view))
                .and_then(|()| self.charge_place_normalization(place))
                .map(|()| place),
        )
    }

    /// Copy the bytes of `place` into slot `slot` of `frame`, on the activation stack (C1-19c4).
    pub(super) fn copy_input_place(
        &mut self,
        frame: &VmFrame,
        slot: usize,
        place: crate::vm::Place,
    ) -> Result<(), RuntimeError> {
        let block = self
            .frame_block(frame)
            .ok_or(RuntimeError::InvalidExecutionState)?;
        let range = {
            let super::store_access::Routed::Cells(reader) = self.data.routed(self.prepared) else {
                return Err(RuntimeError::InvalidExecutionState);
            };
            match reader.frame_slot_view(block.range, block.pou, slot) {
                Some(crate::memory::view::ValueView::Cells { range, .. }) => range,
                _ => return Err(RuntimeError::InvalidExecutionState),
            }
        };
        let EngineStore::Cells(arm) = &mut self.data.store else {
            return Err(RuntimeError::InvalidExecutionState);
        };
        if arm.copy_place(place.range, range, None) {
            Ok(())
        } else {
            Err(RuntimeError::InvalidExecutionState)
        }
    }

    /// Charge and check the result slot 0 of `frame` keeps on the activation stack, when it is a
    /// place, as `finish_stack_result` charges a copy of its value and `check_frame_return` checks
    /// it: the value holds no reference and no instance, so the check only visits it (C1-19c4).
    pub(super) fn check_return_place(&self, frame: &VmFrame) -> Option<Result<(), RuntimeError>> {
        let id = self.frame_slot_activation(frame, 0)?;
        let view = StoreAccess::read_slot(self, MemoryLocation::Local(id), 0).ok()?;
        let place = self.place_of(&view)?;
        Some(
            crate::vm::context::ReferenceContext::before_view_clone(self, &view)
                .and_then(|()| self.check_place_lifetime(place.type_id)),
        )
    }

    /// Push the result slot 0 of the finished frame `block` keeps onto `stack` (C1-19c4): a text as
    /// a temporary text, any other place as a copy of its bytes.
    pub(super) fn push_return_copy(
        &self,
        block: FrameBlock,
        stack: &mut crate::vm::stack::OperandStack,
    ) -> Result<(), RuntimeError> {
        let super::store_access::Routed::Cells(reader) = self.data.routed(self.prepared) else {
            return Err(RuntimeError::InvalidExecutionState);
        };
        let view = reader
            .frame_slot_view(block.range, block.pou, 0)
            .ok_or(RuntimeError::InvalidExecutionState)?;
        let place = self
            .place_of(&view)
            .ok_or(RuntimeError::InvalidExecutionState)?;
        // A text is handed back as a temporary text, which a text function, a comparison and a
        // store read where it lies (C1-12).
        let pushed = match reader.view_cells().text(place.range, place.type_id) {
            Some(crate::memory::flat::TextBytes::Narrow(text)) => stack.push_text(text, false),
            Some(wide) => stack.push_wide_text(wide.units()),
            None => {
                let bytes = reader
                    .view_cells()
                    .bytes(place.range)
                    .ok_or(RuntimeError::InvalidExecutionState)?;
                stack.push_copy(bytes, place.type_id)
            }
        };
        pushed.map_err(crate::vm::VmTrap::into_runtime_error)
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
