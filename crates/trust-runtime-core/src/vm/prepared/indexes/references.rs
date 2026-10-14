//! Candidate aliases grouped once; physical owner and path checks remain dynamic.
use super::*;
use crate::memory::MemoryLocation;
use crate::vm::module::VmRef;

impl PreparedIndexes {
    pub(super) fn prepare_references(
        &mut self,
        vm: &VmModule,
        budget: &mut PreparationBudget,
    ) -> Result<(), RuntimeError> {
        for (id, reference) in vm.refs.iter().enumerate() {
            budget.charge(0, 1)?;
            let key = match reference {
                VmRef::Global { offset, .. } => (0, *offset),
                VmRef::Instance { offset, .. } => (1, *offset),
                // Current-frame locals are resolved directly. Other local,
                // retain, I/O and initializer descriptors do not participate
                // in runtime_access_target's dynamic type discovery.
                _ => continue,
            };
            self.reference_candidates.push(
                key,
                u32::try_from(id).map_err(|_| RuntimeError::Overflow)?,
                budget,
            )?;
        }
        self.reference_candidates.finish(budget)
    }
}

impl PreparedModule {
    // Candidate aliases are indexed by location kind and offset; lookups consume no work (TIME-08).
    pub(crate) fn dynamic_reference_candidates(
        &self,
        location: MemoryLocation,
        offset: usize,
    ) -> &[u32] {
        let kind = match location {
            MemoryLocation::Global => 0,
            MemoryLocation::Instance(_) => 1,
            _ => return &[],
        };
        self.indexes
            .reference_candidates
            .get(&(kind, offset))
            .unwrap_or(&[])
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::value::{RefSegment, Value, ValueRef, ValueRefView};
    use crate::vm::{context::ReferenceContext, dispatch_refs::dynamic_ref_type, VmFrame, VmTrap};

    // Type discovery does not read/write values. This context supplies only the
    // prepared candidate index while retaining the legacy encoded-owner mapper.
    struct TypeDiscovery<'a> {
        prepared: &'a PreparedModule,
        storage: crate::memory::VariableStorage,
    }
    impl ReferenceContext for TypeDiscovery<'_> {
        fn dynamic_reference_candidates(
            &self,
            module: &VmModule,
            reference: ValueRefView<'_>,
        ) -> Result<Option<&[u32]>, RuntimeError> {
            Ok(core::ptr::eq(module, &self.prepared.vm).then(|| {
                self.prepared
                    .dynamic_reference_candidates(reference.location, reference.offset)
            }))
        }
        fn check_reference_write(
            &self,
            _: ValueRefView<'_>,
            _: &Value,
        ) -> Result<(), RuntimeError> {
            unreachable!("type discovery cannot write")
        }
        fn check_reference_read(&self, _: ValueRefView<'_>) -> Result<(), RuntimeError> {
            unreachable!("type discovery cannot read values")
        }
        fn initializer_reference<'a>(
            &self,
            _: u32,
            _: &'a [RefSegment],
        ) -> Result<ValueRefView<'a>, VmTrap> {
            unreachable!("type discovery cannot enter an initializer")
        }
        fn storage(&self) -> &crate::memory::VariableStorage {
            &self.storage
        }
        fn storage_mut(&mut self) -> &mut crate::memory::VariableStorage {
            &mut self.storage
        }
        fn write_typed_storage(
            &mut self,
            _: MemoryLocation,
            _: usize,
            _: &[RefSegment],
            _: Value,
        ) -> Result<bool, RuntimeError> {
            unreachable!("type discovery cannot mutate values")
        }
        fn write_typed_local(&self, _: &mut Value, _: &[RefSegment], _: Value) -> bool {
            unreachable!("type discovery cannot mutate locals")
        }
    }

    #[test]
    fn indexed_and_fallback_discovery_reject_the_same_conflicting_alias() {
        let mut prepared = PreparedModule::from_bytes(
            include_bytes!(
                "../../../../../trust-runtime/tests/fixtures/portability/stbc-2.0/program-v2.stbc"
            ),
            PreparationLimits::default(),
        )
        .unwrap();
        let (original, ty) = prepared
            .vm
            .refs
            .iter()
            .enumerate()
            .find_map(|(id, reference)| {
                if !matches!(reference, VmRef::Instance { path, .. } if path.is_empty()) {
                    return None;
                }
                let ty = prepared.vm.ref_type(id as u32)?;
                matches!(
                    prepared.vm.types.entries[ty as usize].data,
                    crate::bytecode::TypeData::Primitive { .. }
                )
                .then_some((id, ty))
            })
            .expect("fixture has scalar instance references");
        let different = prepared
            .vm
            .types
            .entries
            .iter()
            .enumerate()
            .find_map(|(id, ty_entry)| {
                (id != ty as usize
                    && matches!(ty_entry.data, crate::bytecode::TypeData::Primitive { .. }))
                .then_some(id as u32)
            })
            .unwrap();
        prepared.vm.refs.push(prepared.vm.refs[original].clone());
        prepared.vm.ref_types.push(None);
        *prepared
            .vm
            .ref_types
            .last_mut()
            .expect("appended reference has a dense type slot") = Some(different);
        prepared.indexes = PreparedIndexes::build(
            &prepared.vm,
            &prepared.layout,
            &prepared.roots,
            &prepared.initializers,
            &prepared.method_owners,
            &prepared.resources,
            &mut PreparationBudget::new(prepared.limits),
        )
        .unwrap();
        let VmRef::Instance {
            owner_instance_id,
            offset,
            path,
        } = &prepared.vm.refs[original]
        else {
            unreachable!()
        };
        let reference = ValueRef {
            location: MemoryLocation::Instance(crate::memory::InstanceId(*owner_instance_id)),
            offset: *offset,
            path: path.clone(),
        };
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
        let context = TypeDiscovery {
            prepared: &prepared,
            storage: Default::default(),
        };
        for module in [&prepared.vm, &copied] {
            assert_eq!(
                dynamic_ref_type(&context, module, &frame, &reference)
                    .unwrap_err()
                    .into_runtime_error(),
                crate::vm::module::invalid_bytecode(
                    "conflicting type metadata for runtime reference"
                )
            );
        }
    }

    #[test]
    fn candidate_groups_preserve_every_alias_and_wire_order() {
        let mut prepared = PreparedModule::from_bytes(
            include_bytes!(
                "../../../../../trust-runtime/tests/fixtures/portability/stbc-2.0/program-v2.stbc"
            ),
            PreparationLimits::default(),
        )
        .unwrap();
        let aliases: Vec<_> = prepared
            .vm
            .refs
            .iter()
            .enumerate()
            .filter(|(_, reference)| {
                matches!(reference, VmRef::Global { .. } | VmRef::Instance { .. })
            })
            .map(|(index, reference)| (reference.clone(), prepared.vm.ref_type(index as u32)))
            .collect();
        assert!(!aliases.is_empty());
        for (reference, declared_type) in aliases {
            prepared.vm.refs.push(reference);
            prepared.vm.ref_types.push(None);
            *prepared
                .vm
                .ref_types
                .last_mut()
                .expect("appended reference has a dense type slot") = declared_type;
        }
        // Rebuild privately to verify duplicate descriptors are retained too.
        // No public consumer can mutate a prepared module's reference table.
        prepared.indexes = PreparedIndexes::build(
            &prepared.vm,
            &prepared.layout,
            &prepared.roots,
            &prepared.initializers,
            &prepared.method_owners,
            &prepared.resources,
            &mut PreparationBudget::new(prepared.limits),
        )
        .unwrap();
        for reference in &prepared.vm.refs {
            let (location, offset) = match reference {
                VmRef::Global { offset, .. } => (MemoryLocation::Global, *offset),
                VmRef::Instance {
                    owner_instance_id,
                    offset,
                    ..
                } => (
                    MemoryLocation::Instance(crate::memory::InstanceId(*owner_instance_id)),
                    *offset,
                ),
                _ => continue,
            };
            let expected: Vec<u32> = prepared
                .vm
                .refs
                .iter()
                .enumerate()
                .filter_map(|(id, candidate)| {
                    let matches = match (location, candidate) {
                        (
                            MemoryLocation::Global,
                            VmRef::Global {
                                offset: candidate, ..
                            },
                        )
                        | (
                            MemoryLocation::Instance(_),
                            VmRef::Instance {
                                offset: candidate, ..
                            },
                        ) => *candidate == offset,
                        _ => false,
                    };
                    matches.then_some(id as u32)
                })
                .collect();
            assert_eq!(
                prepared.dynamic_reference_candidates(location, offset),
                expected.as_slice()
            );
        }
        assert!(prepared
            .dynamic_reference_candidates(MemoryLocation::Global, usize::MAX)
            .is_empty());
        assert!(prepared
            .dynamic_reference_candidates(MemoryLocation::Local(crate::memory::FrameId(0)), 0)
            .is_empty());
    }
}
