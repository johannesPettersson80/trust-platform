//! Frame defaults applied without the general declaration machinery.
//!
//! A frame-owned default action without an executable body either copies its
//! literal constant or builds a primitive scalar's intrinsic default. It has no
//! staged identity, once marker or recipe, and its value holds no reference, so
//! the general path reduces to its charges, checks and one slot write. This path
//! performs those in the same order (TIME-08) and writes the same value. Every
//! other action, and any inconsistent metadata, takes the general path unchanged.
use super::super::super::construction::values::scalar;
use super::*;
use crate::bytecode::{InitializationOnce, InitializationStage};

impl EngineState<'_> {
    /// Applies initializer `id` to `frame` when it is a direct frame default.
    /// Returns `Ok(false)`, having charged nothing, when the general path must run it.
    pub(super) fn apply_direct_frame_default(
        &mut self,
        id: u32,
        frame: &mut VmFrame,
        depth: u32,
    ) -> Result<bool, RuntimeError> {
        let prepared = self.prepared;
        let Some(entry) = prepared.initializers.entries.get(id as usize) else {
            return Ok(false);
        };
        if entry.stage != InitializationStage::Default
            || entry.code_length != 0
            || entry.once != InitializationOnce::None
        {
            return Ok(false);
        }
        let Some(declaration) = entry
            .declaration_idx
            .and_then(|index| prepared.layout.entries.get(index as usize))
        else {
            return Ok(false);
        };
        let slot = declaration.slot as usize;
        let Some(result_type) = prepared.vm.ref_type(entry.result_ref_idx) else {
            return Ok(false);
        };
        if declaration.owner != StorageOwner::Frame
            || slot >= frame.locals.len()
            || prepared
                .vm
                .strings
                .get(declaration.name_idx as usize)
                .is_none()
        {
            return Ok(false);
        }
        let literal = declaration
            .default_const_idx
            .and_then(|index| prepared.vm.consts.get(index as usize));
        let primitive = match prepared
            .vm
            .types
            .entries
            .get(result_type as usize)
            .map(|entry| &entry.data)
        {
            Some(TypeData::Primitive { prim_id, .. })
                if declaration.default_const_idx.is_none()
                    && entry.body_kind == InitializerBodyKind::Action
                    && entry.context_initializer_idx.is_none()
                    && entry.recipe_type_id.is_none()
                    && entry.recipe_member_idx.is_none()
                    && direct_scalar_intrinsic_default(&prepared.vm, result_type) =>
            {
                Some(*prim_id)
            }
            _ => None,
        };
        // Lifetime checks admit these literal values without visiting anything.
        let literal = literal.filter(|value| Self::lifetime_check_is_trivial(value));
        if literal.is_none() && primitive.is_none() {
            return Ok(false);
        }
        if depth as usize >= prepared.limits.max_call_depth {
            return Err(crate::vm::VmTrap::CallStackOverflow.into_runtime_error());
        }
        #[cfg(all(feature = "std", feature = "test-support"))]
        let branch_metadata = initializer_branch_metadata(prepared, id, entry, result_type);
        let value = if let Some(constant) = literal {
            #[cfg(all(feature = "std", feature = "test-support"))]
            log_initializer_branch(
                branch_metadata,
                crate::vm::diagnostic_timing::InitializerBranch::LiteralDefaultFastPath,
            );
            self.before_value_clone(constant)?;
            constant.clone()
        } else {
            #[cfg(all(feature = "std", feature = "test-support"))]
            log_initializer_branch(
                branch_metadata,
                crate::vm::diagnostic_timing::InitializerBranch::DirectScalarIntrinsicFastPath,
            );
            // The constructor's work unit, structural nesting check, type-stack
            // entry and constructed-value unit, in its order. No construction of a
            // primitive runs code, so the type's own cycle marker cannot be active.
            self.charge_work_units(1)?;
            if self.data.construction.types.len() > crate::bytecode::BYTECODE_MAX_TYPE_NESTING {
                return Err(RuntimeError::TypeMismatch);
            }
            self.charge_allocation_bytes(core::mem::size_of::<(
                u32,
                crate::vm::construction::values::ValueOperation,
            )>())?;
            self.charge_constructed_value()?;
            let Some(primitive) = primitive else {
                return Err(RuntimeError::InvalidExecutionState);
            };
            scalar::default(primitive, self.data.profile)?
        };
        frame.locals[slot] = value;
        Ok(true)
    }
}

#[cfg(test)]
mod call_path_declined_tests;
#[cfg(test)]
mod tests;
