//! The bytes of the root a reference operand lies in (specification 34 MEM-09; C1-19c4). A call's
//! output copy-back saves the root of each of its targets before it writes it, as the root's bytes
//! when the root holds no reference and no instance, so construction reserves the most bytes the
//! targets of one call save.

use crate::bytecode::StorageOwner;
use crate::vm::module::VmRef;
use crate::vm::PreparedModule;

/// The bytes the output journal saves of a root of type `type_id`: all of them when it holds no
/// reference and no instance, and none otherwise, since such a root is saved as its value.
pub(super) fn saved_bytes(prepared: &PreparedModule, type_id: u32) -> u32 {
    prepared.type_layout(type_id).map_or(0, |layout| {
        if layout.reference_leaves.is_empty() && layout.instance_leaves.is_empty() {
            layout.shape.bytes
        } else {
            0
        }
    })
}

/// The bytes the output journal saves of the root static reference `ref_idx` starts at, in a body
/// of `pou`: the variable declared at its slot, a global or one of the frame's or the instance's,
/// or `largest` when the walk cannot tell which variable that is.
pub(super) fn of_ref(
    prepared: &PreparedModule,
    pou: Option<u32>,
    ref_idx: u32,
    largest: u32,
) -> u32 {
    let (owner, owner_pou, slot) = match prepared.vm.refs.get(ref_idx as usize) {
        Some(VmRef::Global { offset, .. }) => (StorageOwner::Global, None, *offset),
        Some(VmRef::Local { offset, .. }) => (StorageOwner::Frame, pou, *offset),
        Some(VmRef::Instance { offset, .. }) => (StorageOwner::Instance, pou, *offset),
        _ => return largest,
    };
    prepared
        .declarations(owner, owner_pou)
        .iter()
        .filter_map(|&id| prepared.storage_declaration(id as usize))
        .find(|declaration| declaration.slot as usize == slot)
        .and_then(|declaration| declaration.type_id)
        .map_or(largest, |type_id| saved_bytes(prepared, type_id))
}
