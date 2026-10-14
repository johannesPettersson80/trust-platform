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
                self.data.construction.instance_templates.get(&id).copied(),
            ),
            MemoryLocation::Local(id) => {
                if let Some(initializer) = self
                    .data
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
                    self.data
                        .storage
                        .execution_frame_owner(id)
                        .map(|owner| owner.pou_id),
                )
            }
            _ => return Ok(None),
        };
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
    fn indexed_dynamic_type_discovery_keeps_alias_conflicts_and_budget_failures() {
        use crate::vm::{
            context::ReferenceContext, dispatch_refs::dynamic_ref_type, module::VmRef,
        };
        let prepared = PreparedModule::from_bytes(
            include_bytes!(
                "../../../../trust-runtime/tests/fixtures/portability/stbc-2.0/program-v2.stbc"
            ),
            crate::vm::PreparationLimits::default(),
        )
        .unwrap();
        let mut data =
            EngineState::construct_data(&prepared, 0, &super::super::services::LOGICAL_ONLY)
                .unwrap();
        let (original, reference, ty) = prepared
            .vm
            .refs
            .iter()
            .enumerate()
            .find_map(|(id, entry)| {
                let VmRef::Instance {
                    owner_instance_id,
                    offset,
                    path,
                } = entry
                else {
                    return None;
                };
                let ty = prepared.vm.ref_type(id as u32)?;
                if !matches!(
                    prepared.vm.types.entries.get(ty as usize)?.data,
                    TypeData::Primitive { .. }
                ) {
                    return None;
                }
                let root = prepared.root_for_instance_owner(*owner_instance_id)?.0;
                let instance = data.construction.roots[root]?;
                Some((
                    id as u32,
                    crate::value::ValueRef {
                        location: MemoryLocation::Instance(instance),
                        offset: *offset,
                        path: path.clone(),
                    },
                    ty,
                ))
            })
            .expect("fixture has a typed persistent instance member");
        let frame = VmFrame {
            activation: None,
            pou_id: None,
            return_pc: 0,
            code_start: 0,
            code_end: 0,
            local_ref_start: 0,
            local_ref_count: 0,
            locals: Vec::new(),
            parameter_values_present: Vec::new(),
            runtime_instance: None,
            instance_owner: None,
        };
        let copied = prepared.vm.clone();
        let state =
            EngineState::borrow(&mut data, &prepared, &super::super::services::LOGICAL_ONLY);
        assert!(state
            .dynamic_reference_candidates(&copied, reference.as_view())
            .unwrap()
            .is_none());
        let indexed = dynamic_ref_type(&state, &prepared.vm, &frame, &reference).unwrap();
        let legacy = dynamic_ref_type(&state, &copied, &frame, &reference).unwrap();
        assert_eq!(indexed, Some(ty));
        assert_eq!(indexed, legacy);
        // TIME-08: candidate discovery and owner/type lookups are bookkeeping. An empty
        // allowance neither refuses the lookup nor changes the discovered type.
        state.data.resources.work_budget.reset(0);
        assert_eq!(
            dynamic_ref_type(&state, &prepared.vm, &frame, &reference)
                .map_err(crate::vm::VmTrap::into_runtime_error),
            Ok(Some(ty))
        );
        assert_eq!(state.data.resources.work_budget.remaining(), 0);

        // Deliberately corrupt private admitted metadata to prove the discovery
        // defense still inspects every alias rather than trusting one slot type.
        let different = copied
            .types
            .entries
            .iter()
            .enumerate()
            .find(|(id, _)| *id != ty as usize)
            .map(|(id, _)| id as u32)
            .unwrap();
        let mut copied = copied;
        copied.refs.push(copied.refs[original as usize].clone());
        copied.ref_types.push(None);
        *copied
            .ref_types
            .last_mut()
            .expect("appended reference has a dense type slot") = Some(different);
        data.resources.work_budget.reset(prepared.limits.max_work);
        let state =
            EngineState::borrow(&mut data, &prepared, &super::super::services::LOGICAL_ONLY);
        let error = dynamic_ref_type(&state, &copied, &frame, &reference)
            .unwrap_err()
            .into_runtime_error();
        assert_eq!(
            error,
            invalid_bytecode("conflicting type metadata for runtime reference")
        );
    }

    #[test]
    fn reference_type_lookup_consumes_no_allowance() {
        let prepared = PreparedModule::from_bytes(
            include_bytes!(
                "../../../../trust-runtime/tests/fixtures/portability/stbc-2.0/program-v2.stbc"
            ),
            crate::vm::PreparationLimits::default(),
        )
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
        reference.path.push(RefSegment::Field("activations".into()));
        let value = Value::DInt(1);
        assert!(state
            .normalize_reference_value(reference.as_view(), value.clone())
            .is_ok());
        // TIME-08: the declaration and path-segment lookups are bookkeeping, so an empty
        // allowance still resolves the reference type and admits the same value.
        state.data.resources.work_budget.reset(0);
        assert!(state
            .normalize_reference_value(reference.as_view(), value)
            .is_ok());
        assert_eq!(state.data.resources.work_budget.remaining(), 0);
    }
}
