//! Read-only labels derived from admitted persistent references, on every target.
use super::PreparedModule;
use crate::{bytecode::StorageOwner, error::RuntimeError, value::RefSegment, vm::module::VmRef};
use alloc::string::{String, ToString};
use alloc::vec::Vec;
use core::fmt::Write;
use smol_str::SmolStr;

impl PreparedModule {
    /// Render a persistent reference as its declared root/member/index path.
    /// This tooling operation runs outside the control path and never treats an
    /// artifact instance identity as a live storage identity. It needs `alloc`
    /// only, so a device without `std` names its bindings as trust-runtime does.
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

#[cfg(test)]
mod tests {
    use crate::bytecode::StorageOwner;
    use crate::io_address::IoAddress;
    use crate::vm::{PreparationLimits, PreparedModule};

    /// The board probe: the resource global `probe_depth` and the program `Plant`, whose member
    /// `led` binds `%QX0.0` (`crates/trust-runtime/tests/fixtures/portability/f401/gpio.st`).
    const GPIO: &[u8] =
        include_bytes!("../../../../trust-runtime/tests/fixtures/portability/f401/gpio.stbc");

    #[test]
    fn persistent_reference_names_need_no_std() {
        // Specification 34 §4.3.1 with specification 11, "Live Values": a device names each
        // process-image binding by its declared root/member path, exactly as trust-runtime does,
        // and the name resolver needs `alloc` only, never `std`. Catches a build that gates the
        // resolver on `std`, which leaves a chip's `io.list` without binding names, as the C6
        // bench's firmware did.
        let prepared = PreparedModule::from_bytes(GPIO, PreparationLimits::default())
            .expect("the board probe is admitted");
        let global = prepared
            .storage_declarations()
            .iter()
            .find(|declaration| {
                declaration.owner == StorageOwner::Global
                    && prepared
                        .strings()
                        .get(declaration.name_idx as usize)
                        .is_some_and(|name| name == "probe_depth")
            })
            .and_then(|declaration| declaration.ref_idx)
            .expect("the fixture binds its global probe_depth to a reference");
        let led = IoAddress::parse("%QX0.0").expect("a valid image address");
        let member = prepared
            .io_bindings()
            .iter()
            .find(|binding| binding.address == led)
            .map(|binding| binding.ref_idx)
            .expect("the fixture binds its instance member led to %QX0.0");

        assert_eq!(
            prepared
                .persistent_reference_name(global)
                .expect("the global reference is declared")
                .as_str(),
            "probe_depth"
        );
        assert_eq!(
            prepared
                .persistent_reference_name(member)
                .expect("the instance member reference is declared")
                .as_str(),
            "Plant.led"
        );
    }
}
