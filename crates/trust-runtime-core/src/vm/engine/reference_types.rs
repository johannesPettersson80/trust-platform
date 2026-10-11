//! Resolve storage reference types through admitted declarations and compound paths.
use super::*;
use crate::bytecode::{StorageDeclaration, StorageOwner, StorageRole, TypeData};
use crate::memory::MemoryLocation;
use crate::value::{RefSegment, ValueRefView};
use crate::vm::type_policy::{resolved_alias_type, vm_type_for_path};

impl EngineState<'_> {
    pub(super) fn reference_type(
        &self,
        reference: ValueRefView<'_>,
    ) -> Result<Option<u32>, RuntimeError> {
        let (owner, pou) = match reference.location {
            MemoryLocation::Global => (StorageOwner::Global, None),
            MemoryLocation::Instance(id) => (
                StorageOwner::Instance,
                self.construction.instance_templates.get(&id).copied(),
            ),
            MemoryLocation::Local(id) => {
                if let Some(initializer) = self
                    .construction
                    .initializers
                    .iter()
                    .find(|initializer| initializer.result == id)
                {
                    let ty = self.prepared.vm.ref_type(initializer.entry.result_ref_idx);
                    return self.reference_path_type(ty, None, reference.path);
                }
                (
                    StorageOwner::Frame,
                    self.lifetimes.activation_pous.get(&id).copied(),
                )
            }
            _ => return Ok(None),
        };
        self.charge_work_units(self.prepared.lookup_work())?;
        let Some((_, declaration)) = self.prepared.declaration(owner, pou, reference.offset) else {
            return Ok(None);
        };
        let program = (declaration.role == StorageRole::ProgramRoot)
            .then_some(declaration.owner_pou_id)
            .flatten();
        self.reference_path_type(declaration.type_id, program, reference.path)
    }

    fn reference_path_type(
        &self,
        ty: Option<u32>,
        program: Option<u32>,
        path: &[RefSegment],
    ) -> Result<Option<u32>, RuntimeError> {
        let (mut ty, path) = match ty {
            Some(ty) => (ty, path),
            None => {
                let (Some(pou), Some((RefSegment::Field(name), rest))) =
                    (program, path.split_first())
                else {
                    return Ok(None);
                };
                let Some(ty) = self
                    .pou_member_declaration(pou, name)?
                    .and_then(|member| member.type_id)
                else {
                    return Ok(None);
                };
                (ty, rest)
            }
        };
        for segment in path {
            self.charge_work_units(1)?;
            let Some(resolved) = resolved_alias_type(&self.prepared.vm.types, ty, 0) else {
                return Ok(None);
            };
            let Some(entry) = self.prepared.vm.types.entries.get(resolved as usize) else {
                return Ok(None);
            };
            let selected = match (&entry.data, segment) {
                (TypeData::Pou { pou_id }, RefSegment::Field(name)) => self
                    .pou_member_declaration(*pou_id, name)?
                    .and_then(|member| member.type_id),
                (TypeData::Struct { .. } | TypeData::Union { .. }, RefSegment::Field(name)) => {
                    self.charge_work_units(self.prepared.name_lookup_work(name))?;
                    self.prepared.type_member(resolved, name)
                }
                _ => vm_type_for_path(&self.prepared.vm, resolved, core::slice::from_ref(segment)),
            };
            let Some(selected) = selected else {
                return Ok(None);
            };
            ty = selected;
        }
        Ok(resolved_alias_type(&self.prepared.vm.types, ty, 0))
    }

    /// Read/write type checks share the same physical, inherited member lookup.
    pub(super) fn pou_member_declaration(
        &self,
        mut current: u32,
        name: &str,
    ) -> Result<Option<&StorageDeclaration>, RuntimeError> {
        for _ in 0..=self.prepared.vm.pou_by_id.len() {
            self.charge_work_units(self.prepared.name_lookup_work(name))?;
            if let Some(member) = self.prepared.member(current, name) {
                return Ok(Some(member));
            }
            let Some(parent) = self.prepared.vm.parent_pou_ids.get(&current) else {
                break;
            };
            current = *parent;
        }
        Ok(None)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn reference_type_budget_failure_is_not_reclassified_as_a_type_mismatch() {
        let prepared = PreparedModule::from_bytes(
            include_bytes!(
                "../../../../trust-runtime/tests/fixtures/portability/stbc-2.0/program-v2.stbc"
            ),
            crate::vm::PreparationLimits::default(),
        )
        .unwrap();
        let state = EngineState::new(&prepared, 0, &super::super::services::LOGICAL_ONLY).unwrap();
        let mut reference = state.storage.ref_for_global("Plant").unwrap();
        reference.path.push(RefSegment::Field("activations".into()));
        let value = Value::DInt(1);
        assert!(state
            .normalize_reference_value(reference.as_view(), value.clone())
            .is_ok());
        state.resources.work_budget.reset(0);
        assert_eq!(
            state.normalize_reference_value(reference.as_view(), value),
            Err(crate::vm::VmTrap::BudgetExceeded.into_runtime_error())
        );
    }
}
