//! Typed metadata comes from declaration bindings, never initialized values.

use super::*;
use crate::bytecode::{StorageOwner, VarMeta, VarMetaEntry};
use crate::memory::MemoryLocation;

impl BytecodeEncoder<'_> {
    pub(in crate::bytecode::encoder) fn build_source_var_meta(
        &mut self,
    ) -> Result<VarMeta, BytecodeError> {
        let mut bindings: Vec<_> = self
            .ref_map
            .iter()
            .map(|(reference, index)| (*index, reference.clone()))
            .collect();
        bindings.sort_by_key(|(index, _)| *index);
        let direct: HashMap<_, _> = self
            .construction
            .layout
            .entries
            .iter()
            .filter_map(|entry| entry.ref_idx.map(|id| (id, (entry.owner, entry.retain))))
            .collect();
        let instances: HashMap<_, _> = self
            .construction
            .roots
            .entries
            .iter()
            .filter_map(|root| Some((root.instance_owner_id?, root.template_pou_id?)))
            .collect();
        let mut member_retain = HashMap::new();
        for (template, members) in &self.construction.templates.template_fields {
            for id in members {
                let declaration = &self.construction.layout.entries[*id as usize];
                member_retain.insert((*template, declaration.slot as usize), declaration.retain);
            }
        }
        let local_names: HashMap<_, _> = self
            .local_var_meta
            .iter()
            .map(|local| (local.ref_idx, local.name.clone()))
            .collect();
        let mut entries = Vec::new();
        for (ref_idx, reference) in bindings {
            let Some(ty) = self.construction.bindings.types.get(&reference).copied() else {
                continue;
            };
            let retain = if let MemoryLocation::Instance(instance) = reference.location {
                instances
                    .get(&instance.0)
                    .and_then(|template| member_retain.get(&(*template, reference.offset)))
                    .copied()
                    .unwrap_or(0)
            } else {
                direct
                    .get(&ref_idx)
                    .filter(|(owner, _)| *owner != StorageOwner::Frame)
                    .map_or(0, |(_, retain)| *retain)
            };
            let name = if self.ref_entries[ref_idx as usize].location
                == crate::bytecode::RefLocation::Local
            {
                local_names.get(&ref_idx).cloned().ok_or_else(|| {
                    BytecodeError::InvalidSection("declared local metadata name missing".into())
                })?
            } else {
                SmolStr::new(format!("@binding/{ref_idx}"))
            };
            let name_idx = self.strings.intern(name);
            let type_id = self.type_index(ty)?;
            entries.push(VarMetaEntry {
                name_idx,
                type_id,
                ref_idx,
                retain,
                init_const_idx: None,
            });
        }
        Ok(VarMeta { entries })
    }
}
