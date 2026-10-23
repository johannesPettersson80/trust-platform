//! Checked direct/protocol binding updates using the resource-only transaction.
use super::installed::Runtime;
use crate::{
    bytecode::{IoBinding, IoMap, SectionData, SectionId},
    error::RuntimeError,
    io::IoAddress,
    value::ValueRef,
};
use smol_str::SmolStr;
use std::sync::Arc;

impl Runtime {
    /// Bind a declared live destination. Image growth is admitted together with
    /// the binding; no PLC values or pending management requests are reset.
    pub(crate) fn bind_checked_io(
        &mut self,
        reference: ValueRef,
        address: IoAddress,
        name: SmolStr,
    ) -> Result<(), RuntimeError> {
        let mut sizes = [
            self.io().inputs().len(),
            self.io().outputs().len(),
            self.io().memory().len(),
        ];
        if let Some(end) = address.flat_byte_end()? {
            let area = match address.area {
                crate::memory::IoArea::Input => 0,
                crate::memory::IoArea::Output => 1,
                crate::memory::IoArea::Memory => 2,
            };
            sizes[area] = sizes[area].max(end as usize);
        }
        self.configure_io_bindings(&[(reference, address, name)], sizes)
    }

    /// One atomic binding/image transaction for an entire protocol mapping set.
    /// Existing exact reference/address records keep their order and receive the
    /// admitted codec, including enum labels and bounded-string capacity.
    pub(crate) fn configure_io_bindings(
        &mut self,
        changes: &[(ValueRef, IoAddress, SmolStr)],
        sizes: [usize; 3],
    ) -> Result<(), RuntimeError> {
        let mut module = self
            .construction_artifact()
            .map_err(RuntimeError::from)?
            .clone();
        let count = match module.section(SectionId::RefTable) {
            Some(SectionData::RefTable(table)) => table.entries.len(),
            _ => return Err(RuntimeError::InvalidExecutionState),
        };
        let mut metadata = self.metadata_snapshot();
        for (reference, address, name) in changes {
            let mut matched = None;
            for index in 0..count {
                let id = u32::try_from(index).map_err(|_| RuntimeError::Overflow)?;
                match self.state.resolve_persistent_reference(id) {
                    Ok(found) if found == *reference => {
                        matched = Some(id);
                        break;
                    }
                    Ok(_) | Err(RuntimeError::TypeMismatch | RuntimeError::NullReference) => {}
                    Err(error) => return Err(error),
                }
            }
            let ref_idx = matched.ok_or_else(|| {
                RuntimeError::InvalidConfig(
                    "I/O destination is not an admitted persistent reference".into(),
                )
            })?;
            let type_id = self
                .prepared()
                .reference_type(ref_idx)
                .ok_or(RuntimeError::TypeMismatch)?;
            // Explicit hosted binding APIs historically expose canonical enum
            // identity, including aliases. Preserve the declared reference type;
            // only the equivalent I/O codec record selects its enum descriptor.
            let mut leaf = type_id;
            let mut codec_type = type_id;
            for _ in 0..self.prepared().type_table().entries.len() {
                match &self.prepared().type_table().entries[leaf as usize].data {
                    crate::bytecode::TypeData::Alias { target_type_id } => leaf = *target_type_id,
                    crate::bytecode::TypeData::Subrange { base_type_id, .. } => {
                        leaf = *base_type_id
                    }
                    crate::bytecode::TypeData::Enum { .. } => {
                        codec_type = leaf;
                        break;
                    }
                    _ => break,
                }
            }

            let text = crate::bytecode::format_authoring_io_address(address);
            let address_str_idx = match module.section_mut(SectionId::StringTable) {
                Some(SectionData::StringTable(strings)) => {
                    match strings.entries.iter().position(|old| *old == text) {
                        Some(index) => u32::try_from(index).map_err(|_| RuntimeError::Overflow)?,
                        None => {
                            let index = u32::try_from(strings.entries.len())
                                .map_err(|_| RuntimeError::Overflow)?;
                            strings.entries.push(text);
                            index
                        }
                    }
                }
                _ => return Err(RuntimeError::InvalidExecutionState),
            };
            if module.section(SectionId::IoMap).is_none() {
                module.sections.push(crate::bytecode::Section {
                    id: SectionId::IoMap.as_raw(),
                    flags: 0,
                    data: SectionData::IoMap(IoMap::default()),
                });
            }
            let Some(SectionData::IoMap(io)) = module.section_mut(SectionId::IoMap) else {
                return Err(RuntimeError::InvalidExecutionState);
            };
            let binding = IoBinding {
                address_str_idx,
                ref_idx,
                type_id: Some(codec_type),
            };
            let index =
                if let Some(index) = io.bindings.iter().position(|old| {
                    old.ref_idx == ref_idx && old.address_str_idx == address_str_idx
                }) {
                    io.bindings[index] = binding;
                    index
                } else {
                    let index = io.bindings.len();
                    io.bindings.push(binding);
                    index
                };
            metadata.io_display_names.resize(io.bindings.len(), None);
            metadata.io_display_names[index] = Some(name.clone());
        }
        let Some(SectionData::ResourceMeta(resources)) =
            module.section_mut(SectionId::ResourceMeta)
        else {
            return Err(RuntimeError::InvalidExecutionState);
        };
        if resources.resources.len() != 1 {
            return Err(RuntimeError::InvalidExecutionState);
        }
        let resource = &mut resources.resources[0];
        resource.inputs_size = u32::try_from(sizes[0]).map_err(|_| RuntimeError::Overflow)?;
        resource.outputs_size = u32::try_from(sizes[1]).map_err(|_| RuntimeError::Overflow)?;
        resource.memory_size = u32::try_from(sizes[2]).map_err(|_| RuntimeError::Overflow)?;
        self.reconfigure_resource_module(module, Arc::new(metadata), false)
    }
}
