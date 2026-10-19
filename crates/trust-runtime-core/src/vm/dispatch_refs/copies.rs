//! Stores of the copies a call leaves on the operand stack (specification 34 MEM-12, ARCH-05;
//! C1-19b).
//!
//! A copy holds the bytes of a place, taken before a call that could change them. A store writes
//! them where it would copy the place: into a destination of the place's type in the persistent
//! block, checked and charged as that copy is, its texts' capacity checks read from the copy
//! (TIME-08). A node of a backing, whose copy is checked against the place where it lies, and any
//! other destination are given the value the copy stands for.

use super::{
    copy_place_to, dynamic_ref_type, dynamic_store_ref, load_ref_view, store_ref, StoreOrder,
};
use crate::value::ValueRef;
use crate::vm::context::ReferenceContext;
use crate::vm::errors::VmTrap;
use crate::vm::frames::FrameStack;
use crate::vm::module::{VmModule, VmRef};
use crate::vm::Place;

/// Store `bytes`, the copy of the place `place` stands for, through a static reference, as
/// [`super::store_ref_place`] stores the place.
pub(crate) fn store_ref_copy(
    runtime: &mut impl ReferenceContext,
    module: &VmModule,
    frames: &mut FrameStack,
    ref_idx: u32,
    bytes: &[u8],
    place: Place,
) -> Result<(), VmTrap> {
    let reference = module
        .refs
        .get(ref_idx as usize)
        .ok_or_else(|| VmTrap::InvalidRefIndex(ref_idx))?;
    // Locals are stored as values, as places are (C1-11).
    if !matches!(reference, VmRef::Local { .. }) {
        let address = load_ref_view(runtime, module, frames, ref_idx)?;
        let declared = module.ref_type(ref_idx);
        if let Some(destination) = runtime
            .place_destination(address, place, declared)
            .filter(|destination| destination.node.is_none())
        {
            let (source, order) = ((place, Some(bytes)), StoreOrder::NormalizeFirst);
            return copy_place_to(runtime, address, source, destination, declared, order);
        }
    }
    let value = runtime
        .copy_value(bytes, place.type_id)
        .map_err(VmTrap::Runtime)?;
    store_ref(runtime, module, frames, ref_idx, value)
}

/// Store `bytes`, the copy of the place `place` stands for, through a dynamic reference, as
/// [`super::dynamic_store_place`] stores the place.
pub(crate) fn dynamic_store_copy(
    runtime: &mut impl ReferenceContext,
    module: &VmModule,
    frames: &mut FrameStack,
    reference: &ValueRef,
    bytes: &[u8],
    place: Place,
) -> Result<(), VmTrap> {
    let frame = frames.current().ok_or_else(|| VmTrap::CallStackUnderflow)?;
    if !frame.owns_local_reference(reference) {
        let declared = dynamic_ref_type(runtime, module, frame, reference)?;
        let address = reference.as_view();
        if let Some(destination) = runtime
            .place_destination(address, place, declared)
            .filter(|destination| destination.node.is_none())
        {
            let (source, order) = ((place, Some(bytes)), StoreOrder::CheckFirst);
            return copy_place_to(runtime, address, source, destination, declared, order);
        }
    }
    let value = runtime
        .copy_value(bytes, place.type_id)
        .map_err(VmTrap::Runtime)?;
    dynamic_store_ref(runtime, module, frames, reference, value)
}
