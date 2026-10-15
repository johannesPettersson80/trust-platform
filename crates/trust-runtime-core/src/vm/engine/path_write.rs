//! Charge copy-on-write backing storage before changing a field or element.
use super::*;
use crate::memory::MemoryLocation;
use crate::value::{array_offset_i64, RefSegment};
use alloc::sync::Arc;

impl EngineState<'_> {
    pub(super) fn charge_storage_path_write(
        &self,
        storage: &VariableStorage,
        location: MemoryLocation,
        offset: usize,
        path: &[RefSegment],
        input: &Value,
    ) -> Result<(), RuntimeError> {
        // Ordinary scalar root replacement retains the original constant-cost
        // path. Only structured physical writes need detached backing/COW work.
        if path.is_empty() && !matches!(input, Value::Struct(_)) {
            return Ok(());
        }
        let root = storage
            .read_direct_slot_by_location(location, offset)
            .ok_or(RuntimeError::NullReference)?;
        self.charge_path_write_inner(storage, root, path, input, false, 0)
    }

    pub(super) fn charge_path_write(
        &self,
        root: &Value,
        path: &[RefSegment],
        input: &Value,
    ) -> Result<(), RuntimeError> {
        self.charge_path_write_inner(&self.data.storage, root, path, input, false, 0)
    }

    fn charge_path_write_inner(
        &self,
        storage: &VariableStorage,
        root: &Value,
        path: &[RefSegment],
        input: &Value,
        ancestor_cloned: bool,
        depth: usize,
    ) -> Result<(), RuntimeError> {
        if let Value::Struct(structure) = root {
            let layout = structure.physical_layout();
            let path = layout
                .as_ref()
                .and_then(|layout| layout.canonical_path(path))
                .unwrap_or(path);
            if path.is_empty() && structure.physical_whole_assignment_is_admitted(input) {
                return Ok(());
            }
            if let Some(cost) = structure.physical_write_cost(path)? {
                self.charge_work_units(cost.work)?;
                self.charge_allocation_bytes(cost.allocation)?;
                let shared = self.charge_structure_copy(structure, ancestor_cloned)?;
                let input_cloned = path.is_empty()
                    && matches!(input, Value::Struct(value) if Arc::strong_count(value) > 1);
                if input_cloned && let Value::Struct(input) = input {
                    // commit moves fields from a unique input; a shared input clones
                    // only its field map, not its old immutable physical backing.
                    let mut bytes = input
                        .fields()
                        .len()
                        .checked_mul(4 * core::mem::size_of::<(smol_str::SmolStr, Value)>())
                        .ok_or(RuntimeError::Overflow)?;
                    for value in input.fields().values() {
                        bytes = bytes
                            .checked_add(self.value_clone_charge(value, 0)?)
                            .ok_or(RuntimeError::Overflow)?;
                    }
                    self.charge_allocation_bytes(bytes)?;
                }
                if let Some(children) = structure.physical_affected_children(path) {
                    for child in children {
                        if let Some(suffix) = child.strip_prefix(path) {
                            self.charge_physical_child_copy(input, suffix, input_cloned, depth)?;
                        } else {
                            self.charge_physical_child_copy(root, child, shared, depth)?;
                        }
                    }
                }
                return Ok(());
            }
        }
        let Some((segment, rest)) = path.split_first() else {
            return Ok(());
        };
        if depth >= crate::bytecode::BYTECODE_MAX_TYPE_NESTING {
            return Err(RuntimeError::TypeMismatch);
        }
        match (root, segment) {
            (Value::Struct(structure), RefSegment::Field(name)) => {
                // A cloned ancestor increments descendant Arc counts even when the
                // original descendant currently has one strong owner.
                let shared = self.charge_structure_copy(structure, ancestor_cloned)?;
                let field = structure.field(name).ok_or(RuntimeError::NullReference)?;
                self.charge_path_write_inner(storage, field, rest, input, shared, depth + 1)
            }
            (Value::Instance(instance), RefSegment::Field(name)) => {
                let reference = storage
                    .ref_for_instance_recursive(*instance, name)
                    .ok_or(RuntimeError::NullReference)?;
                let field = storage
                    .read_by_ref_ref(&reference)
                    .ok_or(RuntimeError::NullReference)?;
                self.charge_path_write_inner(storage, field, rest, input, false, depth + 1)
            }
            (Value::Array(array), RefSegment::Index(indices)) => {
                let offset = array_offset_i64(array.dimensions(), indices)
                    .ok_or(RuntimeError::NullReference)?;
                let element = array
                    .elements()
                    .get(offset)
                    .ok_or(RuntimeError::NullReference)?;
                self.charge_path_write_inner(
                    storage,
                    element,
                    rest,
                    input,
                    ancestor_cloned,
                    depth + 1,
                )
            }
            (Value::String(text), RefSegment::Index(_)) => self.charge_string_path_write(text),
            (Value::WString(text), RefSegment::Index(_)) => self.charge_string_path_write(text),
            _ => Err(RuntimeError::NullReference),
        }
    }

    fn charge_structure_copy(
        &self,
        structure: &Arc<crate::value::StructValue>,
        ancestor_cloned: bool,
    ) -> Result<bool, RuntimeError> {
        let shared = ancestor_cloned || Arc::strong_count(structure) > 1;
        if shared {
            let mut bytes = core::mem::size_of::<crate::value::StructValue>()
                .checked_add(2 * core::mem::size_of::<usize>())
                .and_then(|bytes| bytes.checked_add(structure.physical_byte_len()))
                .and_then(|bytes| {
                    structure
                        .fields()
                        .len()
                        .checked_mul(4 * core::mem::size_of::<(smol_str::SmolStr, Value)>())
                        .and_then(|fields| bytes.checked_add(fields))
                })
                .ok_or(RuntimeError::Overflow)?;
            for value in structure.fields().values() {
                bytes = bytes
                    .checked_add(self.value_clone_charge(value, 0)?)
                    .ok_or(RuntimeError::Overflow)?;
            }
            self.charge_allocation_bytes(bytes)?;
        } else if Arc::weak_count(structure) > 0 {
            // Dissociating a weak observer moves the value into a new Arc but
            // does not duplicate its existing owned fields/backing.
            self.charge_allocation_bytes(
                core::mem::size_of::<crate::value::StructValue>()
                    + 2 * core::mem::size_of::<usize>(),
            )?;
        }
        Ok(shared)
    }

    /// Charge each affected structure exactly once. Ancestor COW propagates
    /// sharing, but unrelated siblings/descendants never allocate for this write.
    fn charge_physical_child_copy(
        &self,
        mut value: &Value,
        path: &[RefSegment],
        mut copied: bool,
        depth: usize,
    ) -> Result<(), RuntimeError> {
        for (index, segment) in path.iter().enumerate() {
            if depth + index >= crate::bytecode::BYTECODE_MAX_TYPE_NESTING {
                return Err(RuntimeError::TypeMismatch);
            }
            value = match (value, segment) {
                (Value::Struct(structure), RefSegment::Field(name)) => {
                    copied |= Arc::strong_count(structure) > 1;
                    structure.field(name).ok_or(RuntimeError::TypeMismatch)?
                }
                (Value::Array(array), RefSegment::Index(indices)) => {
                    let offset = array_offset_i64(array.dimensions(), indices)
                        .ok_or(RuntimeError::TypeMismatch)?;
                    array
                        .elements()
                        .get(offset)
                        .ok_or(RuntimeError::TypeMismatch)?
                }
                _ => return Err(RuntimeError::TypeMismatch),
            };
        }
        let Value::Struct(structure) = value else {
            return Err(RuntimeError::TypeMismatch);
        };
        self.charge_structure_copy(structure, copied)?;
        Ok(())
    }

    fn charge_string_path_write(&self, text: &str) -> Result<(), RuntimeError> {
        // write_string_element collects Unicode scalars, then builds new UTF-8;
        // STRING also copies that result into SmolStr. Include vector growth and
        // the replacement scalar's possible UTF-8 expansion before either allocation.
        let units = text.len().checked_add(4).ok_or(RuntimeError::Overflow)?;
        self.charge_work_units(units.checked_mul(3).ok_or(RuntimeError::Overflow)?)?;
        self.charge_allocation_bytes(units.checked_mul(16).ok_or(RuntimeError::Overflow)?)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::collections::OrderedMap;
    use crate::value::{ArrayValue, StructValue};
    use crate::vm::{
        call::{bindings::VmWriteTarget, context::CallContext},
        VmTrap,
    };

    #[test]
    fn output_copyback_charges_shared_struct_backing_before_any_partial_commit() {
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
        let mut state = EngineState::borrow(
            &mut state_data,
            &prepared,
            &super::super::services::LOGICAL_ONLY,
        );
        let mut fields = OrderedMap::default();
        fields.insert("small".into(), Value::DInt(9));
        fields.insert(
            "large".into(),
            Value::Array(alloc::boxed::Box::new(ArrayValue::from_canonical_parts(
                vec![Value::DInt(0); 2048],
                vec![(0, 2047)],
            ))),
        );
        state.data.storage.set_global(
            "aggregate_probe",
            Value::Struct(Arc::new(StructValue::from_canonical_parts(
                "Probe".into(),
                fields,
            ))),
        );
        state.data.storage.set_global("first_probe", Value::DInt(7));
        let first = state.data.storage.ref_for_global("first_probe").unwrap();
        let mut second = state
            .data
            .storage
            .ref_for_global("aggregate_probe")
            .unwrap();
        second.path.push(RefSegment::Field("small".into()));
        let mut frame = VmFrame {
            activation: None,
            pou_id: Some(0),
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
        let error = state
            .with_output_transaction(&mut frame, |state, frame| {
                // The snapshot shares the aggregate Arc; only the member write needs
                // its large owned array copied. Reserve too little for that operation.
                state
                    .data
                    .resources
                    .construction_bytes
                    .set(prepared.limits.max_construction_bytes - 1024);
                VmWriteTarget::from_reference(&first).write(state, frame, Value::DInt(70))?;
                VmWriteTarget::from_reference(&second).write(state, frame, Value::DInt(90))
            })
            .unwrap_err();
        assert!(matches!(
            error,
            VmTrap::Runtime(RuntimeError::ExecutionLimit {
                kind: crate::error::ExecutionLimitKind::AllocationBytes,
                ..
            })
        ));
        assert_eq!(
            state.data.storage.get_global("first_probe"),
            Some(&Value::DInt(7))
        );
        assert_eq!(
            state.data.storage.read_by_ref_ref(&second),
            Some(&Value::DInt(9))
        );
    }

    #[test]
    fn string_element_charge_fails_before_replacing_backing_text() {
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
        let mut state = EngineState::borrow(
            &mut state_data,
            &prepared,
            &super::super::services::LOGICAL_ONLY,
        );
        state
            .data
            .storage
            .set_global("text_probe", Value::String("abc".into()));
        let mut reference = state.data.storage.ref_for_global("text_probe").unwrap();
        reference.path.push(RefSegment::Index(vec![1]));
        state
            .data
            .resources
            .construction_bytes
            .set(prepared.limits.max_construction_bytes);
        let error = crate::vm::context::ReferenceContext::write_typed_storage(
            &mut state,
            reference.location,
            reference.offset,
            &reference.path,
            Value::Char(b'Z'),
        )
        .unwrap_err();
        assert!(matches!(
            error,
            RuntimeError::ExecutionLimit {
                kind: crate::error::ExecutionLimitKind::AllocationBytes,
                ..
            }
        ));
        assert_eq!(
            state.data.storage.get_global("text_probe"),
            Some(&Value::String("abc".into()))
        );
    }
    #[test]
    fn hundred_field_physical_write_fits_small_work_allowance_and_precharges_scratch() {
        use crate::bytecode::{Field, TypeData, TypeEntry, TypeKind, TypeTable};
        use crate::value::StorageAbi;
        let prepared = PreparedModule::from_bytes(
            include_bytes!(
                "../../../../trust-runtime/tests/fixtures/portability/stbc-2.0/program-v2.stbc"
            ),
            crate::vm::PreparationLimits::default(),
        )
        .unwrap();
        let names: Vec<smol_str::SmolStr> =
            (0..100).map(|i| alloc::format!("f{i:03}").into()).collect();
        let types = TypeTable {
            offsets: Vec::new(),
            entries: vec![
                TypeEntry {
                    kind: TypeKind::Primitive,
                    name_idx: None,
                    data: TypeData::Primitive {
                        prim_id: 4,
                        max_length: 0,
                    },
                },
                TypeEntry {
                    kind: TypeKind::OverlappingStruct,
                    name_idx: None,
                    data: TypeData::Struct {
                        fields: (0..100)
                            .map(|i| Field {
                                name_idx: i,
                                type_id: 0,
                                bit_offset: Some(i * 32),
                            })
                            .collect(),
                    },
                },
            ],
        };
        let layouts = crate::value::overlap::prepare_layouts(
            &types,
            &names,
            StorageAbi::UniformTemporal64,
            &mut |_, _| Ok(()),
        )
        .unwrap();
        let fields = names
            .iter()
            .map(|name| (name.clone(), Value::DWord(0)))
            .collect();
        let probe = StructValue::from_canonical_parts("Hundred".into(), fields)
            .initialize_physical_layout(layouts[1].clone())
            .unwrap();
        let mut data =
            EngineState::construct_data(&prepared, 0, &super::super::services::LOGICAL_ONLY)
                .unwrap();
        let mut state =
            EngineState::borrow(&mut data, &prepared, &super::super::services::LOGICAL_ONLY);
        state
            .data
            .storage
            .set_global("probe", Value::Struct(Arc::new(probe)));
        let mut reference = state.data.storage.ref_for_global("probe").unwrap();
        reference.path.push(RefSegment::Field("f050".into()));
        state.data.resources.work_budget.reset(4096);
        state.data.resources.construction_bytes.set(0);
        assert!(crate::vm::context::ReferenceContext::write_typed_storage(
            &mut state,
            reference.location,
            reference.offset,
            &reference.path,
            Value::DWord(7),
        )
        .unwrap());
        assert_eq!(
            state.data.storage.read_by_ref_ref(&reference),
            Some(&Value::DWord(7))
        );
        assert!(state.data.resources.work_budget.remaining() > 0);
        // Reserving zero scratch refuses the next write before either physical or logical publication.
        state.data.resources.work_budget.reset(4096);
        state
            .data
            .resources
            .construction_bytes
            .set(prepared.limits.max_construction_bytes);
        let error = crate::vm::context::ReferenceContext::write_typed_storage(
            &mut state,
            reference.location,
            reference.offset,
            &reference.path,
            Value::DWord(9),
        )
        .unwrap_err();
        assert!(matches!(
            error,
            RuntimeError::ExecutionLimit {
                kind: crate::error::ExecutionLimitKind::AllocationBytes,
                ..
            }
        ));
        assert_eq!(
            state.data.storage.read_by_ref_ref(&reference),
            Some(&Value::DWord(7))
        );
    }

    #[test]
    fn physical_patch_completion_samples_deadline_before_publication_but_scalar_stores_keep_stride()
    {
        use crate::bytecode::{Field, TypeData, TypeEntry, TypeKind, TypeTable};
        use crate::value::StorageAbi;
        use core::cell::Cell;
        struct DeadlineProbe {
            calls: Cell<usize>,
        }
        impl ExecutionServices for DeadlineProbe {
            fn deadline_exceeded(&self) -> bool {
                let calls = self.calls.get() + 1;
                self.calls.set(calls);
                // First sample is the aggregate work charge; the second is the
                // completion boundary after the detached patch has been prepared.
                calls >= 2
            }
            fn has_wall_clock(&self) -> bool {
                false
            }
            fn current_dt(&self) -> Result<crate::value::DateTimeValue, RuntimeError> {
                Err(RuntimeError::TypeMismatch)
            }
        }
        let prepared = PreparedModule::from_bytes(
            include_bytes!(
                "../../../../trust-runtime/tests/fixtures/portability/stbc-2.0/program-v2.stbc"
            ),
            crate::vm::PreparationLimits::default(),
        )
        .unwrap();
        let types = TypeTable {
            offsets: Vec::new(),
            entries: vec![
                TypeEntry {
                    kind: TypeKind::Primitive,
                    name_idx: None,
                    data: TypeData::Primitive {
                        prim_id: 4,
                        max_length: 0,
                    },
                },
                TypeEntry {
                    kind: TypeKind::OverlappingStruct,
                    name_idx: None,
                    data: TypeData::Struct {
                        fields: vec![Field {
                            name_idx: 0,
                            type_id: 0,
                            bit_offset: Some(0),
                        }],
                    },
                },
            ],
        };
        let layouts = crate::value::overlap::prepare_layouts(
            &types,
            &["word".into()],
            StorageAbi::UniformTemporal64,
            &mut |_, _| Ok(()),
        )
        .unwrap();
        let mut fields = OrderedMap::default();
        fields.insert("word".into(), Value::DWord(7));
        let physical = StructValue::from_canonical_parts("Probe".into(), fields)
            .initialize_physical_layout(layouts[1].clone())
            .unwrap();
        let mut data =
            EngineState::construct_data(&prepared, 0, &super::super::services::LOGICAL_ONLY)
                .unwrap();
        data.storage
            .set_global("physical_probe", Value::Struct(Arc::new(physical)));
        data.storage.set_global("scalar_probe", Value::DWord(0));
        let mut reference = data.storage.ref_for_global("physical_probe").unwrap();
        reference.path.push(RefSegment::Field("word".into()));
        let scalar = data.storage.ref_for_global("scalar_probe").unwrap();
        let probe = DeadlineProbe {
            calls: Cell::new(0),
        };
        let mut state = EngineState::borrow(&mut data, &prepared, &probe);
        state.data.resources.work_budget.reset(4096);
        state.data.resources.construction_bytes.set(0);
        for value in 1..=8 {
            assert!(crate::vm::context::ReferenceContext::write_typed_storage(
                &mut state,
                scalar.location,
                scalar.offset,
                &[],
                Value::DWord(value),
            )
            .unwrap());
        }
        assert_eq!(
            probe.calls.get(),
            0,
            "ordinary stores do not add deadline samples outside dispatcher stride"
        );
        let error = crate::vm::context::ReferenceContext::write_typed_storage(
            &mut state,
            reference.location,
            reference.offset,
            &reference.path,
            Value::DWord(9),
        )
        .unwrap_err();
        assert_eq!(error, VmTrap::DeadlineExceeded.into_runtime_error());
        assert_eq!(
            probe.calls.get(),
            2,
            "completion samples after the non-expired aggregate work sample"
        );
        assert_eq!(
            state.data.storage.read_by_ref_ref(&reference),
            Some(&Value::DWord(7))
        );
        assert_eq!(
            state.data.storage.read_by_ref_ref(&scalar),
            Some(&Value::DWord(8))
        );
    }

    #[test]
    fn function_block_reference_binding_keeps_physical_subrange_fault_before_publication() {
        use crate::bytecode::{Field, TypeData, TypeEntry, TypeKind, TypeTable};
        use crate::value::StorageAbi;
        let prepared = PreparedModule::from_bytes(
            include_bytes!(
                "../../../../trust-runtime/tests/fixtures/portability/stbc-2.0/program-v2.stbc"
            ),
            crate::vm::PreparationLimits::default(),
        )
        .unwrap();
        let types = TypeTable {
            offsets: Vec::new(),
            entries: vec![
                TypeEntry {
                    kind: TypeKind::Primitive,
                    name_idx: None,
                    data: TypeData::Primitive {
                        prim_id: 7,
                        max_length: 0,
                    },
                },
                TypeEntry {
                    kind: TypeKind::Subrange,
                    name_idx: None,
                    data: TypeData::Subrange {
                        base_type_id: 0,
                        lower: 0,
                        upper: 10,
                    },
                },
                TypeEntry {
                    kind: TypeKind::OverlappingStruct,
                    name_idx: None,
                    data: TypeData::Struct {
                        fields: vec![
                            Field {
                                name_idx: 0,
                                type_id: 0,
                                bit_offset: Some(0),
                            },
                            Field {
                                name_idx: 1,
                                type_id: 1,
                                bit_offset: Some(0),
                            },
                        ],
                    },
                },
            ],
        };
        let layouts = crate::value::overlap::prepare_layouts(
            &types,
            &["raw".into(), "bounded".into()],
            StorageAbi::UniformTemporal64,
            &mut |_, _| Ok(()),
        )
        .unwrap();
        let fields = [
            ("raw".into(), Value::Int(7)),
            ("bounded".into(), Value::Int(7)),
        ]
        .into_iter()
        .collect();
        let physical = StructValue::from_canonical_parts("Range".into(), fields)
            .initialize_physical_layout(layouts[2].clone())
            .unwrap();
        let mut data =
            EngineState::construct_data(&prepared, 0, &super::super::services::LOGICAL_ONLY)
                .unwrap();
        data.storage
            .set_global("probe", Value::Struct(Arc::new(physical)));
        let mut reference = data.storage.ref_for_global("probe").unwrap();
        reference.path.push(RefSegment::Field("raw".into()));
        let binding = crate::vm::call::bindings::VmFbFieldBinding::Reference(reference.clone());
        let mut state =
            EngineState::borrow(&mut data, &prepared, &super::super::services::LOGICAL_ONLY);
        state.data.resources.work_budget.reset(4096);
        state.data.resources.construction_bytes.set(0);
        let error = binding
            .write_checked(&mut state, Value::Int(11))
            .unwrap_err();
        assert!(matches!(
            error,
            VmTrap::Runtime(RuntimeError::SubrangeViolation {
                value: 11,
                lower: 0,
                upper: 10
            })
        ));
        assert_eq!(
            state.data.storage.read_by_ref_ref(&reference),
            Some(&Value::Int(7))
        );
        reference.path[0] = RefSegment::Field("bounded".into());
        assert_eq!(
            state.data.storage.read_by_ref_ref(&reference),
            Some(&Value::Int(7))
        );
    }

    #[test]
    fn reference_path_depth_admits_64_segments_and_refuses_the_65th_with_type_mismatch() {
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
        let state =
            EngineState::borrow(&mut data, &prepared, &super::super::services::LOGICAL_ONLY);
        // Sixty-five nested single-element arrays. The structural bound is 64 edges
        // (spec 12 §4.6; spec 34 §6.1 MEM-06: exceeding it reports TypeMismatch). Segment depth 64
        // is refused at path_write.rs:90, before the segment is matched against its value.
        let mut root = Value::DInt(7);
        for _ in 0..65 {
            root = Value::Array(alloc::boxed::Box::new(ArrayValue::from_canonical_parts(
                vec![root],
                vec![(0, 0)],
            )));
        }
        let admitted = vec![RefSegment::Index(vec![0]); 64];
        assert_eq!(
            state.charge_path_write(&root, &admitted, &Value::DInt(1)),
            Ok(())
        );
        let refused = vec![RefSegment::Index(vec![0]); 65];
        assert_eq!(
            state.charge_path_write(&root, &refused, &Value::DInt(1)),
            Err(RuntimeError::TypeMismatch)
        );
    }
}

#[cfg(test)]
mod text_work_element_tests;
#[cfg(test)]
mod text_work_overlap_charge_tests;
#[cfg(test)]
#[path = "path_write/work_allowance_tests.rs"]
mod work_allowance_tests;
