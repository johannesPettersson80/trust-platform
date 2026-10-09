//! Frame declarations follow the references emitted by LocalScope, including scratch tails.

use super::*;
use crate::bytecode::{StorageDeclaration, StorageOwner, StorageRole};
use crate::program_model::{Param, VarDef};
use trust_hir::symbols::ParamDirection;

impl BytecodeEncoder<'_> {
    pub(in crate::bytecode::encoder) fn collect_frame_declarations(
        &mut self,
        owner: u32,
        scope: &super::super::LocalScope,
        return_slot: Option<(&SmolStr, TypeId)>,
        params: &[Param],
        locals: &[VarDef],
    ) -> Result<(), BytecodeError> {
        let mut declared_end = 0;
        if let Some((name, ty)) = return_slot {
            declared_end = self.add_frame_declaration(
                owner,
                scope,
                name,
                ty,
                StorageRole::Return,
                0,
                0,
                None,
            )?;
        }
        for param in params {
            let flags = match param.direction {
                ParamDirection::In => 2,
                ParamDirection::Out => 4,
                ParamDirection::InOut => 8,
            };
            declared_end = self.add_frame_declaration(
                owner,
                scope,
                &param.name,
                param.type_id,
                StorageRole::Parameter,
                0,
                flags,
                super::declarations::parameter_initializer(param),
            )?;
        }
        for local in locals.iter().filter(|local| !local.external) {
            declared_end = self.add_frame_declaration(
                owner,
                scope,
                &local.name,
                local.type_id,
                StorageRole::Variable,
                super::declarations::retention(local.retain),
                u8::from(local.constant),
                local.initializer.clone(),
            )?;
        }
        if declared_end < scope.local_ref_count {
            let name_idx = self.strings.intern(format!("@scratch/{owner}"));
            self.add_declaration(
                StorageDeclaration {
                    owner: StorageOwner::Frame,
                    role: StorageRole::Scratch,
                    retain: 0,
                    flags: 0,
                    owner_pou_id: Some(owner),
                    name_idx,
                    type_id: None,
                    slot: declared_end,
                    ref_idx: Some(scope.local_ref_start + declared_end),
                    default_const_idx: None,
                    construction_nodes: scope.local_ref_count - declared_end,
                    related_declaration_idx: None,
                    source_name_idx: None,
                },
                None,
                None,
            )?;
        }
        Ok(())
    }

    #[allow(clippy::too_many_arguments)]
    fn add_frame_declaration(
        &mut self,
        owner: u32,
        scope: &super::super::LocalScope,
        name: &SmolStr,
        ty: TypeId,
        role: StorageRole,
        retain: u8,
        flags: u8,
        expression: Option<Expr>,
    ) -> Result<u32, BytecodeError> {
        let reference = scope.locals.get(&normalize_name(name)).ok_or_else(|| {
            BytecodeError::InvalidSection("frame declaration binding missing".into())
        })?;
        let slot = u32::try_from(reference.offset)
            .map_err(|_| BytecodeError::InvalidSection("frame slot overflow".into()))?;
        let ref_idx = self.ref_index_for(reference)?;
        let name_idx = self.strings.intern(name.clone());
        self.add_declaration(
            StorageDeclaration {
                owner: StorageOwner::Frame,
                role,
                retain,
                flags,
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
            Some(ty),
            expression,
        )?;
        slot.checked_add(1)
            .ok_or_else(|| BytecodeError::InvalidSection("frame slot overflow".into()))
    }
}
