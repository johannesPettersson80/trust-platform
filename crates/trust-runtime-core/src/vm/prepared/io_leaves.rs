//! Where an output or marker binding's scalar variable lies (checklist P1-6; specification 34
//! MEM-12). A binding whose static reference names a scalar global, or a scalar member of a
//! persistent instance, without a path has a leaf at a byte offset preparation fixes: the
//! global's in the persistent block, the member's in the instance of its construction root. A scan
//! publishes such a binding by reading its leaf where it lies, without resolving the reference's
//! root and walking to its variable by name each time; any other binding is read as before.

use super::*;
use crate::bytecode::{StorageOwner, TypeData};
use crate::io_address::IoSize;
use crate::memory::IoArea;
use crate::vm::module::VmRef;

/// The leaf of one binding's variable: its byte offset in the persistent block (`root` `None`) or
/// in the instance of construction root `root`, its bytes and primitive, and whether the variable
/// may be written (it is not `CONSTANT`).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) struct IoLeaf {
    pub(crate) root: Option<u32>,
    pub(crate) offset: u32,
    pub(crate) prim: u16,
    pub(crate) bytes: u8,
    pub(crate) writable: bool,
}

crate::memory::figure::holds_no_heap!(IoLeaf);

/// The input and marker bindings, when a scan samples every one of them into its variable's leaf
/// (checklist P1-6): each names, without a path, a writable scalar variable of the primitive the
/// binding declares, through a flat address of a scalar, and no two name one variable. `count` is
/// how many there are.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) struct InputLeaves {
    pub(crate) count: u32,
}

impl PreparedModule {
    /// The leaf of each binding, in binding order, or `None` for a binding that is no scalar
    /// variable reached without a path.
    pub(crate) fn binding_leaves(&self) -> Vec<Option<IoLeaf>> {
        self.io_bindings
            .iter()
            .map(|binding| self.binding_leaf(binding.ref_idx))
            .collect()
    }

    /// The input and marker bindings sampled into their leaves, when every one of them can be and
    /// there is one at least.
    pub(crate) fn input_leaves(&self) -> Option<InputLeaves> {
        let mut count = 0u32;
        for (index, binding) in self.io_bindings.iter().enumerate() {
            if !matches!(binding.address.area, IoArea::Input | IoArea::Memory) {
                continue;
            }
            let leaf = (*self.io_leaves.get(index)?)?;
            let declared = binding.type_id.and_then(|ty| {
                let types = &self.vm.types;
                let resolved = crate::vm::type_policy::resolved_alias_type(types, ty, 0)?;
                match types.entries.get(resolved as usize)?.data {
                    TypeData::Primitive { prim_id, .. } => Some(prim_id),
                    _ => None,
                }
            });
            let eligible = leaf.writable
                && declared == Some(leaf.prim)
                && binding.address.path.len() <= 1
                && !matches!(binding.address.size, IoSize::Bytes(_))
                && binding.codec.enum_type.is_none()
                && binding.codec.string_capacity.is_none();
            // A variable that two bindings write is saved once by the general snapshot.
            let shared =
                self.io_bindings[..index]
                    .iter()
                    .zip(&self.io_leaves)
                    .any(|(earlier, other)| {
                        matches!(earlier.address.area, IoArea::Input | IoArea::Memory)
                            && other.is_some_and(|other| {
                                other.root == leaf.root && other.offset == leaf.offset
                            })
                    });
            if !eligible || shared {
                return None;
            }
            count = count.checked_add(1)?;
        }
        (count > 0).then_some(InputLeaves { count })
    }

    fn binding_leaf(&self, ref_idx: u32) -> Option<IoLeaf> {
        let (root, id) = match self.vm.refs.get(ref_idx as usize)? {
            VmRef::Global { offset, path } if path.is_empty() => {
                let id = *self.declarations(StorageOwner::Global, None).get(*offset)?;
                (None, id)
            }
            VmRef::Instance {
                owner_instance_id,
                offset,
                path,
            } if path.is_empty() => {
                let (index, root) = self.root_for_instance_owner(*owner_instance_id)?;
                let template = root.template_pou_id?;
                let id = *self
                    .declarations(StorageOwner::Instance, Some(template))
                    .get(*offset)?;
                (Some(u32::try_from(index).ok()?), id)
            }
            _ => return None,
        };
        let entry = self.layout.entries.get(id as usize)?;
        let layout = self.type_layout(entry.type_id?)?;
        if self.presence_offset(id, 0).is_some() {
            return None;
        }
        let offset = match root {
            None => self.global_range(id)?.start,
            Some(_) => self.declaration_offset(id)?,
        };
        Some(IoLeaf {
            root,
            offset,
            prim: layout.scalar?,
            bytes: u8::try_from(layout.shape.bytes).ok()?,
            writable: !entry.is_constant(),
        })
    }
}
