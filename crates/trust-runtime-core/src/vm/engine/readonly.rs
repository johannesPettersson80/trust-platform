//! Constant declaration protection independent of compiler-produced code.
use super::*;
use crate::bytecode::{StorageOwner, StorageRole, TypeData};
use crate::memory::MemoryLocation;
use crate::value::{RefSegment, ValueRefView};

impl EngineState<'_> {
    pub(super) fn check_writable_declaration(
        &self,
        reference: ValueRefView<'_>,
    ) -> Result<(), RuntimeError> {
        let (owner, pou) = match reference.location {
            MemoryLocation::Global => (StorageOwner::Global, None),
            MemoryLocation::Instance(id) => (
                StorageOwner::Instance,
                self.data.construction.instance_templates.get(&id).copied(),
            ),
            MemoryLocation::Local(id) => {
                if self
                    .data
                    .construction
                    .initializers
                    .iter()
                    .any(|initializer| initializer.result == id)
                {
                    return Ok(());
                }
                (
                    StorageOwner::Frame,
                    self.data
                        .storage
                        .execution_frame_owner(id)
                        .map(|owner| owner.pou_id),
                )
            }
            _ => return Ok(()),
        };
        #[cfg(all(feature = "std", feature = "test-support"))]
        let resolved = crate::vm::diagnostic_timing::observe_index_lookup_result(
            crate::vm::diagnostic_timing::IndexLookupOperation::WritableDeclaration,
            || Ok::<_, RuntimeError>(self.prepared.declaration(owner, pou, reference.offset)),
        )?;
        #[cfg(not(all(feature = "std", feature = "test-support")))]
        let resolved = self.prepared.declaration(owner, pou, reference.offset);
        let Some((_, declaration)) = resolved else {
            return Ok(()); // Compiler scratch has no individual record.
        };
        if declaration.is_constant() {
            return Err(RuntimeError::ConstantWrite);
        }
        let (mut ty, path) = if declaration.role == StorageRole::ProgramRoot {
            let Some((RefSegment::Field(name), rest)) = reference.path.split_first() else {
                return Err(RuntimeError::ProgramRootReplacement);
            };
            let pou = declaration
                .owner_pou_id
                .ok_or_else(|| RuntimeError::TypeMismatch)?;
            (self.writable_member_type(pou, name)?, rest)
        } else {
            let Some(ty) = declaration.type_id else {
                return Ok(());
            };
            (ty, reference.path)
        };
        for segment in path {
            ty = super::super::type_policy::resolved_alias_type(&self.prepared.vm.types, ty, 0)
                .ok_or_else(|| RuntimeError::TypeMismatch)?;
            let entry = self
                .prepared
                .vm
                .types
                .entries
                .get(ty as usize)
                .ok_or_else(|| RuntimeError::TypeMismatch)?;
            if let (TypeData::Pou { pou_id }, RefSegment::Field(name)) = (&entry.data, segment) {
                ty = self.writable_member_type(*pou_id, name)?;
            } else if let (
                TypeData::Struct { .. } | TypeData::Union { .. },
                RefSegment::Field(name),
            ) = (&entry.data, segment)
            {
                ty = self
                    .prepared
                    .type_member(ty, name)
                    .ok_or_else(|| RuntimeError::TypeMismatch)?;
            } else {
                ty = super::super::type_policy::vm_type_for_path(
                    &self.prepared.vm,
                    ty,
                    core::slice::from_ref(segment),
                )
                .ok_or_else(|| RuntimeError::TypeMismatch)?;
            }
        }
        Ok(())
    }

    fn writable_member_type(&self, current: u32, name: &str) -> Result<u32, RuntimeError> {
        let member = self
            .pou_member_declaration(current, name)?
            .ok_or_else(|| RuntimeError::NullReference)?;
        if member.is_constant() {
            return Err(RuntimeError::ConstantWrite);
        }
        member.type_id.ok_or_else(|| RuntimeError::TypeMismatch)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::bytecode::{BytecodeModule, SectionData, SectionId};

    #[test]
    fn program_root_field_selection_preserves_member_write_permissions() {
        let bytes = include_bytes!(
            "../../../../trust-runtime/tests/fixtures/portability/stbc-2.0/program-v2.stbc"
        );
        for constant in [false, true] {
            let mut module = BytecodeModule::decode(bytes).unwrap();
            let Some(SectionData::StringTable(strings)) = module.section(SectionId::StringTable)
            else {
                panic!("strings")
            };
            let name = strings
                .entries
                .iter()
                .position(|name| name == "activations")
                .unwrap() as u32;
            if constant {
                let Some(SectionData::StorageLayout(layout)) =
                    module.section_mut(SectionId::StorageLayout)
                else {
                    panic!("layout")
                };
                layout
                    .entries
                    .iter_mut()
                    .find(|entry| entry.owner == StorageOwner::Instance && entry.name_idx == name)
                    .unwrap()
                    .flags |= 1;
            }
            let prepared =
                PreparedModule::from_decoded(&module, crate::vm::PreparationLimits::default())
                    .unwrap();
            let mut state_data =
                EngineState::construct_data(&prepared, 0, &super::super::services::LOGICAL_ONLY)
                    .unwrap();
            let state = EngineState::borrow(
                &mut state_data,
                &prepared,
                &super::super::services::LOGICAL_ONLY,
            );
            let mut reference = state.data.storage.ref_for_global("Plant").unwrap();
            assert_eq!(
                state.check_write(reference.as_view(), &Value::DInt(9)),
                Err(RuntimeError::ProgramRootReplacement)
            );
            reference.path.push(RefSegment::Field("activations".into()));
            let expected = if constant {
                Err(RuntimeError::ConstantWrite)
            } else {
                Ok(())
            };
            assert_eq!(
                state.check_write(reference.as_view(), &Value::DInt(9)),
                expected
            );
        }
    }
}
