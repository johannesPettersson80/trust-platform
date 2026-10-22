//! The roots that the inputs of a program write (specification 34 MEM-09, MEM-12; checklist C1-16,
//! C1-21). At the start of a scan the engine saves, as their bytes, the variables that hold the
//! destinations of the input and marker bindings, so that a refused scan leaves them as they were.
//! Their bytes follow from the module: a binding's static reference names a global or a member of a
//! persistent instance, and each member of a function-block value its path enters lies in that
//! block's own instance, so construction reserves them and the memory figure counts them.

use super::*;
use crate::bytecode::{StorageOwner, TypeData, TypeKind, BYTECODE_MAX_TYPE_NESTING};
use crate::memory::IoArea;
use crate::value::RefSegment;
use crate::vm::module::VmRef;

/// The variable that holds a destination: a global by its slot, or a member of the instance of a
/// construction root by its slot.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
enum Root {
    Global(u32),
    Member(usize, u32),
}

impl PreparedModule {
    /// The bytes that the snapshot of the input destinations saves at a scan start: the bytes of
    /// every distinct variable that an input or marker binding writes into, when its type holds no
    /// reference and no instance (C1-16). A binding whose destination cannot be resolved here is
    /// left to the scan, which refuses it.
    pub(crate) fn input_snapshot_bytes(&self) -> Result<usize, RuntimeError> {
        let mut roots = Vec::new();
        for binding in &self.io_bindings {
            if !matches!(binding.address.area, IoArea::Input | IoArea::Memory) {
                continue;
            }
            if let Some(root) = self
                .vm
                .refs
                .get(binding.ref_idx as usize)
                .and_then(|reference| self.io_root(reference))
            {
                roots.push(root);
            }
        }
        roots.sort_unstable_by_key(|(root, _)| *root);
        roots.dedup_by_key(|(root, _)| *root);
        roots
            .iter()
            .filter_map(|&(_, type_id)| {
                self.type_layout(type_id)
                    .filter(|layout| {
                        layout.reference_leaves.is_empty() && layout.instance_leaves.is_empty()
                    })
                    .map(|layout| layout.shape.bytes as usize)
            })
            .try_fold(0usize, |sum, bytes| sum.checked_add(bytes))
            .ok_or(RuntimeError::Overflow)
    }

    /// The variable that holds the destination of `reference`, and its type: the variable the
    /// reference names, or, where its path enters a function-block value with a member name, that
    /// member of the block's instance, and so on.
    fn io_root(&self, reference: &VmRef) -> Option<(Root, u32)> {
        let (mut root, path) = match reference {
            VmRef::Global { offset, path } => (Root::Global(u32::try_from(*offset).ok()?), path),
            VmRef::Instance {
                owner_instance_id,
                offset,
                path,
            } => {
                let (index, _) = self.root_for_instance_owner(*owner_instance_id)?;
                (Root::Member(index, u32::try_from(*offset).ok()?), path)
            }
            _ => return None,
        };
        let mut root_type = self.root_type(root)?;
        let mut current = root_type;
        let mut start = 0usize;
        for (index, segment) in path.iter().enumerate() {
            let entry = self.resolved_type(current)?;
            match (segment, &entry.data) {
                (RefSegment::Field(name), TypeData::Pou { .. })
                    if matches!(entry.kind, TypeKind::FunctionBlock | TypeKind::Class) =>
                {
                    let instance = self.root_bound_at(root, &path[start..index])?;
                    let (member, member_type) = self.instance_member(instance, name, 0)?;
                    root = member;
                    root_type = member_type;
                    current = member_type;
                    start = index + 1;
                }
                (
                    RefSegment::Field(name),
                    TypeData::Struct { fields } | TypeData::Union { fields },
                ) => {
                    current = fields
                        .iter()
                        .find(|field| {
                            self.vm
                                .strings
                                .get(field.name_idx as usize)
                                .is_some_and(|field| field.eq_ignore_ascii_case(name))
                        })?
                        .type_id;
                }
                (RefSegment::Index(_), TypeData::Array { elem_type_id, .. }) => {
                    current = *elem_type_id;
                }
                _ => return None,
            }
        }
        Some((root, root_type))
    }

    /// The type of `root`'s variable.
    fn root_type(&self, root: Root) -> Option<u32> {
        match root {
            Root::Global(slot) => {
                self.declaration(StorageOwner::Global, None, slot as usize)?
                    .1
                    .type_id
            }
            Root::Member(index, slot) => {
                let template = self.roots.entries.get(index)?.template_pou_id?;
                self.declaration(StorageOwner::Instance, Some(template), slot as usize)?
                    .1
                    .type_id
            }
        }
    }

    /// The type entry `type_id` names, through its aliases.
    fn resolved_type(&self, type_id: u32) -> Option<&crate::bytecode::TypeEntry> {
        let mut current = type_id;
        for _ in 0..=BYTECODE_MAX_TYPE_NESTING {
            let entry = self.vm.types.entries.get(current as usize)?;
            match &entry.data {
                TypeData::Alias { target_type_id } => current = *target_type_id,
                _ => return Some(entry),
            }
        }
        None
    }

    /// The construction root whose instance the function-block value at `path` within `root`'s
    /// variable is: the root bound there.
    fn root_bound_at(&self, root: Root, path: &[RefSegment]) -> Option<usize> {
        self.roots.entries.iter().position(|candidate| {
            candidate.template_pou_id.is_some()
                && candidate
                    .binding_ref_idx
                    .and_then(|binding| self.vm.refs.get(binding as usize))
                    .is_some_and(|binding| match (binding, root) {
                        (
                            VmRef::Global {
                                offset,
                                path: bound,
                            },
                            Root::Global(slot),
                        ) => *offset == slot as usize && same_path(bound, path),
                        (
                            VmRef::Instance {
                                owner_instance_id,
                                offset,
                                path: bound,
                            },
                            Root::Member(index, slot),
                        ) => {
                            self.roots
                                .entries
                                .get(index)
                                .and_then(|owner| owner.instance_owner_id)
                                == Some(*owner_instance_id)
                                && *offset == slot as usize
                                && same_path(bound, path)
                        }
                        _ => false,
                    })
        })
    }

    /// The member `name` of the instance of construction root `index`: one of its template's own
    /// variables, else one of its inheritance parent's, and its type.
    fn instance_member(&self, index: usize, name: &str, depth: usize) -> Option<(Root, u32)> {
        if depth > BYTECODE_MAX_TYPE_NESTING {
            return None;
        }
        let template = self.roots.entries.get(index)?.template_pou_id?;
        let own = self
            .declarations(StorageOwner::Instance, Some(template))
            .iter()
            .filter_map(|&id| self.storage_declaration(id as usize))
            .find(|declaration| {
                declaration.owner_pou_id == Some(template)
                    && self
                        .vm
                        .strings
                        .get(declaration.name_idx as usize)
                        .is_some_and(|member| member.eq_ignore_ascii_case(name))
            });
        if let Some(declaration) = own {
            return Some((Root::Member(index, declaration.slot), declaration.type_id?));
        }
        let parent = self.roots.entries.iter().position(|root| {
            root.is_inheritance_parent() && root.parent_root_idx == u32::try_from(index).ok()
        })?;
        self.instance_member(parent, name, depth + 1)
    }
}

/// Whether two reference paths name the same place, member names compared without regard to ASCII
/// case.
fn same_path(left: &[RefSegment], right: &[RefSegment]) -> bool {
    left.len() == right.len()
        && left.iter().zip(right).all(|pair| match pair {
            (RefSegment::Field(left), RefSegment::Field(right)) => left.eq_ignore_ascii_case(right),
            (left, right) => left == right,
        })
}
