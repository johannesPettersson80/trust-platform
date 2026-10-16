//! Read-only hosted labels derived from admitted persistent references.
use super::PreparedModule;
use crate::{bytecode::StorageOwner, error::RuntimeError, value::RefSegment, vm::module::VmRef};
use alloc::{string::String, vec::Vec};
use core::fmt::Write;
use smol_str::SmolStr;

impl PreparedModule {
    /// Render a persistent reference as its declared root/member/index path.
    /// This tooling operation runs outside the control path and never treats an
    /// artifact instance identity as a live storage identity.
    #[cfg(feature = "std")]
    pub fn persistent_reference_name(&self, index: u32) -> Result<SmolStr, RuntimeError> {
        let mut current = index;
        let mut parts = Vec::new();
        // Each instance step follows an admitted parent/root binding. Bound the
        // walk even if an internal fixture has bypassed admission invariants.
        for _ in 0..=self.vm.refs.len() {
            let reference = self
                .vm
                .refs
                .get(current as usize)
                .ok_or(RuntimeError::InvalidExecutionState)?;
            let (name, path, parent) = match reference {
                VmRef::Global { offset, path } => {
                    let (_, declaration) = self
                        .declaration(StorageOwner::Global, None, *offset)
                        .ok_or(RuntimeError::InvalidExecutionState)?;
                    (declaration.name_idx, path, None)
                }
                VmRef::Instance {
                    owner_instance_id,
                    offset,
                    path,
                } => {
                    let (_, root) = self
                        .root_for_instance_owner(*owner_instance_id)
                        .ok_or(RuntimeError::InvalidExecutionState)?;
                    let (_, declaration) = self
                        .declaration(StorageOwner::Instance, root.template_pou_id, *offset)
                        .ok_or(RuntimeError::InvalidExecutionState)?;
                    let mut root = root;
                    // Inheritance parents occupy the same named destination.
                    for _ in 0..=self.roots.entries.len() {
                        if root.binding_ref_idx.is_some() {
                            break;
                        }
                        let parent = root
                            .parent_root_idx
                            .ok_or(RuntimeError::InvalidExecutionState)?;
                        root = self
                            .roots
                            .entries
                            .get(parent as usize)
                            .ok_or(RuntimeError::InvalidExecutionState)?;
                    }
                    (
                        declaration.name_idx,
                        path,
                        Some(
                            root.binding_ref_idx
                                .ok_or(RuntimeError::InvalidExecutionState)?,
                        ),
                    )
                }
                _ => return Err(RuntimeError::InvalidExecutionState),
            };
            let mut part = self
                .vm
                .strings
                .get(name as usize)
                .ok_or(RuntimeError::InvalidExecutionState)?
                .to_string();
            for segment in path {
                match segment {
                    RefSegment::Field(field) => {
                        part.push('.');
                        part.push_str(field);
                    }
                    RefSegment::Index(indices) => {
                        part.push('[');
                        for (at, index) in indices.iter().enumerate() {
                            if at != 0 {
                                part.push(',');
                            }
                            write!(&mut part, "{index}")
                                .map_err(|_| RuntimeError::InvalidExecutionState)?;
                        }
                        part.push(']');
                    }
                }
            }
            parts
                .try_reserve(1)
                .map_err(|_| RuntimeError::PreparationLimit)?;
            parts.push(part);
            if let Some(parent) = parent {
                current = parent;
            } else {
                let length = parts
                    .iter()
                    .try_fold(0usize, |sum, part| {
                        sum.checked_add(part.len())
                            .and_then(|sum| sum.checked_add(1))
                    })
                    .ok_or(RuntimeError::Overflow)?;
                let mut name = String::new();
                name.try_reserve_exact(length)
                    .map_err(|_| RuntimeError::PreparationLimit)?;
                for (at, part) in parts.iter().rev().enumerate() {
                    if at != 0 {
                        name.push('.');
                    }
                    name.push_str(part);
                }
                return Ok(name.into());
            }
        }
        Err(RuntimeError::InvalidExecutionState)
    }
}
