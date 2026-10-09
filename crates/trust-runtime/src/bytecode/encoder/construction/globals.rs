//! Reserve global and module-static bindings before lowering any initializer.

use super::declarations::retention;
use super::*;
use crate::bytecode::{StorageDeclaration, StorageOwner, StorageRole};

impl BytecodeEncoder<'_> {
    pub(in crate::bytecode::encoder) fn reserve_source_globals(
        &mut self,
    ) -> Result<(), BytecodeError> {
        let input = self.authoring.ok_or_else(|| {
            BytecodeError::InvalidSection("source authoring input missing".into())
        })?;
        for global in &input.globals {
            let reference = self
                .construction
                .bindings
                .allocate_global(&global.name, Some(global.type_id))?;
            let ref_idx = self.ref_index_for(&reference)?;
            let name_idx = self.strings.intern(global.name.clone());
            let slot = u32::try_from(reference.offset)
                .map_err(|_| BytecodeError::InvalidSection("global slot overflow".into()))?;
            self.add_declaration(
                StorageDeclaration {
                    owner: StorageOwner::Global,
                    role: StorageRole::Variable,
                    retain: retention(global.retain),
                    flags: u8::from(global.constant),
                    owner_pou_id: None,
                    name_idx,
                    type_id: None,
                    slot,
                    ref_idx: Some(ref_idx),
                    default_const_idx: None,
                    construction_nodes: 0,
                    related_declaration_idx: None,
                    source_name_idx: None,
                },
                Some(global.type_id),
                global.initializer.clone(),
            )?;
        }
        let config = input.configuration.as_ref().ok_or_else(|| {
            BytecodeError::InvalidSection("configuration has not been resolved".into())
        })?;
        for program in &config.programs {
            let owner = self
                .pou_ids
                .program_id(&program.type_name)
                .ok_or_else(|| BytecodeError::InvalidSection("program template missing".into()))?;
            let reference = self
                .construction
                .bindings
                .allocate_global(&program.name, None)?;
            let ref_idx = self.ref_index_for(&reference)?;
            let name_idx = self.strings.intern(program.name.clone());
            let slot = u32::try_from(reference.offset)
                .map_err(|_| BytecodeError::InvalidSection("global slot overflow".into()))?;
            self.add_declaration(
                StorageDeclaration {
                    owner: StorageOwner::Global,
                    role: StorageRole::ProgramRoot,
                    retain: 0,
                    flags: 0,
                    owner_pou_id: Some(owner),
                    name_idx,
                    type_id: None,
                    slot,
                    ref_idx: Some(ref_idx),
                    default_const_idx: None,
                    construction_nodes: 0,
                    related_declaration_idx: None,
                    source_name_idx: None,
                },
                None,
                None,
            )?;
        }
        for function in input.runtime.functions().values() {
            let owner = self
                .pou_ids
                .function_id(&function.name)
                .ok_or_else(|| BytecodeError::InvalidSection("function owner missing".into()))?;
            for local in &function.static_locals {
                if local.initializer.is_some()
                    && (crate::harness::function_block_type_name(
                        local.type_id,
                        self.runtime.registry(),
                    )
                    .is_some()
                        || crate::harness::class_type_name(local.type_id, self.runtime.registry())
                            .is_some())
                {
                    return Err(BytecodeError::InvalidSection(
                        "function VAR_STAT instances cannot have initializers".into(),
                    ));
                }
                let name = crate::program_model::static_storage_name(&function.name, &local.name);
                let reference = self
                    .construction
                    .bindings
                    .allocate_global(&name, Some(local.type_id))?;
                let ref_idx = self.ref_index_for(&reference)?;
                let name_idx = self.strings.intern(name);
                let slot = u32::try_from(reference.offset)
                    .map_err(|_| BytecodeError::InvalidSection("global slot overflow".into()))?;
                let source_name_idx = Some(self.strings.intern(local.name.clone()));
                self.add_declaration(
                    StorageDeclaration {
                        owner: StorageOwner::Global,
                        role: StorageRole::Static,
                        retain: retention(local.retain),
                        flags: u8::from(local.constant),
                        owner_pou_id: Some(owner),
                        name_idx,
                        type_id: None,
                        slot,
                        ref_idx: Some(ref_idx),
                        default_const_idx: None,
                        construction_nodes: 0,
                        related_declaration_idx: None,
                        source_name_idx,
                    },
                    Some(local.type_id),
                    local.initializer.clone(),
                )?;
            }
        }
        Ok(())
    }
}
