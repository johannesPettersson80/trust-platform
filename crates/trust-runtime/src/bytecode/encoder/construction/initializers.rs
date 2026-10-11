//! Initializer identities are allocated before bodies, permitting bounded recursive calls.

use super::*;
use crate::bytecode::{
    InitializationOnce, InitializationPhase, InitializationStage, InitializationTarget,
    InitializerEntry, BYTECODE_MAX_CONSTRUCTION_RECORDS,
};
use crate::memory::{FrameId, MemoryLocation};

impl BytecodeEncoder<'_> {
    pub(in crate::bytecode::encoder) fn intern_default_recipe(
        &mut self,
        type_id: TypeId,
    ) -> Result<u32, BytecodeError> {
        if let Some(id) = self.construction.recipes.default_recipes.get(&type_id) {
            return Ok(*id);
        }
        let id = self.reserve_initializer_result(
            type_id,
            InitializerEntry {
                declaration_idx: None,
                owner_pou_id: None,
                result_ref_idx: 0,
                code_offset: 0,
                code_length: 0,
                visible_local_count: 0,
                visible_static_count: 0,
                phase: InitializationPhase::ValueDefault,
                once: InitializationOnce::None,
                stage: InitializationStage::Default,
                trigger: crate::bytecode::InitializationTrigger::Ordinary,
                target_idx: None,
                partial_kind: 0,
                target_kind: InitializationTarget::Declaration,
                target_reserved: [0; 2],
                partial_index: 0,
                context_initializer_idx: None,
                recipe_type_id: None,
                recipe_member_idx: None,
                body_kind: crate::bytecode::InitializerBodyKind::Action,
                recipe_reserved: [0; 3],
            },
        )?;
        self.construction
            .recipes
            .default_recipes
            .insert(type_id, id);
        self.construction.recipes.recipe_types.push((id, type_id));
        Ok(id)
    }

    pub(in crate::bytecode::encoder) fn reserve_initializer_result(
        &mut self,
        type_id: TypeId,
        mut entry: InitializerEntry,
    ) -> Result<u32, BytecodeError> {
        if self.construction.initializers.entries.len() >= BYTECODE_MAX_CONSTRUCTION_RECORDS {
            return Err(BytecodeError::InvalidSection(
                "initializer count limit exceeded".into(),
            ));
        }
        let id = u32::try_from(self.construction.initializers.entries.len())
            .map_err(|_| BytecodeError::InvalidSection("initializer identity overflow".into()))?;
        let frame = FrameId(u32::MAX - id);
        if frame.0 <= self.next_local_frame_id {
            return Err(BytecodeError::InvalidSection(
                "initializer frame identity collision".into(),
            ));
        }
        self.construction.bodies.result_frames.insert(frame, id);
        let reference = initializer_result_reference(id);
        entry.result_ref_idx = self.ref_index_for(&reference)?;
        self.construction.bindings.types.insert(reference, type_id);
        self.construction.initializers.entries.push(entry);
        Ok(id)
    }
}

/// The reserved local-frame namespace is translated to InitializerResult by refs.rs.
pub(super) fn initializer_result_reference(id: u32) -> ValueRef {
    ValueRef {
        location: MemoryLocation::Local(FrameId(u32::MAX - id)),
        offset: 0,
        path: Vec::new(),
    }
}
