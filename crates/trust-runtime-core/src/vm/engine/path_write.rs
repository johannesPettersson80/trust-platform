//! Charge copy-on-write backing storage before changing a field or element.
//!
//! A write is charged for what the program writes. A field or element write copies no structure,
//! so the engine's detach of shared storage is bookkeeping and is not charged (TIME-08).
use super::*;
use crate::memory::view::{StoreAccess, ValueView};
use crate::memory::MemoryLocation;
use crate::value::{array_offset_i64, RefSegment};

impl EngineState<'_> {
    pub(super) fn charge_storage_path_write<S: StoreAccess>(
        &self,
        storage: &S,
        location: MemoryLocation,
        offset: usize,
        path: &[RefSegment],
        input: &Value,
    ) -> Result<(), RuntimeError> {
        // Ordinary scalar root replacement retains the original constant-cost
        // path. Only structured physical writes need layout work.
        if path.is_empty() && !matches!(input, Value::Struct(_)) {
            return Ok(());
        }
        let root = storage
            .read_slot(location, offset)
            .map_err(|_| RuntimeError::NullReference)?;
        self.charge_path_write_inner(storage, root, path, input, 0)
    }

    pub(super) fn charge_path_write(
        &self,
        root: &Value,
        path: &[RefSegment],
        input: &Value,
    ) -> Result<(), RuntimeError> {
        self.charge_path_write_inner(self, ValueView::Value(root), path, input, 0)
    }

    /// The view form of [`Self::charge_path_write`] (C1-9d2a, item (a)): the charge of a write inside
    /// a stored view, whether it holds a `Value` or cells.
    pub(super) fn charge_path_write_view(
        &self,
        root: &ValueView<'_>,
        path: &[RefSegment],
        input: &Value,
    ) -> Result<(), RuntimeError> {
        self.charge_path_write_inner(self, *root, path, input, 0)
    }

    /// The charge of a write to `path` inside the value `root` (C1-9d1). A `Value` root walks as it
    /// always has, including the physical layout of a structure; a cell root walks its members and
    /// elements through the view. Both charge the same numbers for the same value.
    fn charge_path_write_inner<S: StoreAccess>(
        &self,
        storage: &S,
        root: ValueView<'_>,
        path: &[RefSegment],
        input: &Value,
        depth: usize,
    ) -> Result<(), RuntimeError> {
        let value = match root {
            ValueView::Value(value) => value,
            // A structure with a physical layout, or a node of its backing, is charged as its value
            // is, through its physical layout (C1-15).
            view if view.lies_as_backing() => {
                return self.charge_backing_path_write(storage, view, path, input, depth);
            }
            cells => return self.charge_cell_path_write(storage, cells, path, input, depth),
        };
        if let Value::Struct(structure) = value {
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
                if let Some(children) = structure.physical_affected_children(path) {
                    for child in children {
                        if let Some(suffix) = child.strip_prefix(path) {
                            validate_physical_child(input, suffix, depth)?;
                        } else {
                            validate_physical_child(value, child, depth)?;
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
        match (value, segment) {
            (Value::Struct(structure), RefSegment::Field(name)) => {
                let field = structure.field(name).ok_or(RuntimeError::NullReference)?;
                self.charge_path_write_inner(
                    storage,
                    ValueView::Value(field),
                    rest,
                    input,
                    depth + 1,
                )
            }
            (Value::Instance(instance), RefSegment::Field(name)) => {
                let view = storage
                    .read_instance_field(*instance, name)
                    .map_err(|_| RuntimeError::NullReference)?;
                self.charge_path_write_inner(storage, view, rest, input, depth + 1)
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
                    ValueView::Value(element),
                    rest,
                    input,
                    depth + 1,
                )
            }
            (Value::String(text), RefSegment::Index(_)) => {
                self.charge_string_path_write(text.len())
            }
            (Value::WString(text), RefSegment::Index(_)) => {
                self.charge_string_path_write(text.len())
            }
            _ => Err(RuntimeError::NullReference),
        }
    }

    /// The cell form of [`Self::charge_path_write_inner`] (C1-9d1): the same steps over the members,
    /// elements and text that the view reads. A structure with a physical layout is charged by
    /// [`Self::charge_path_write_inner`] as its value (C1-15).
    fn charge_cell_path_write<S: StoreAccess>(
        &self,
        storage: &S,
        root: ValueView<'_>,
        path: &[RefSegment],
        input: &Value,
        depth: usize,
    ) -> Result<(), RuntimeError> {
        let Some((segment, rest)) = path.split_first() else {
            return Ok(());
        };
        if depth >= crate::bytecode::BYTECODE_MAX_TYPE_NESTING {
            return Err(RuntimeError::TypeMismatch);
        }
        match segment {
            RefSegment::Field(_) if root.is_struct() => {
                let field = root
                    .walk(core::slice::from_ref(segment))
                    .ok_or(RuntimeError::NullReference)?;
                self.charge_path_write_inner(storage, field, rest, input, depth + 1)
            }
            RefSegment::Field(name) => {
                let instance = root.instance()?.ok_or(RuntimeError::NullReference)?;
                let view = storage
                    .read_instance_field(instance, name)
                    .map_err(|_| RuntimeError::NullReference)?;
                self.charge_path_write_inner(storage, view, rest, input, depth + 1)
            }
            RefSegment::Index(_) if root.array_dimensions().is_some() => {
                let element = root
                    .walk(core::slice::from_ref(segment))
                    .ok_or(RuntimeError::NullReference)?;
                self.charge_path_write_inner(storage, element, rest, input, depth + 1)
            }
            RefSegment::Index(_) => match root.text_utf8_len()? {
                Some(length) => self.charge_string_path_write(length),
                None => Err(RuntimeError::NullReference),
            },
        }
    }

    /// The charge of one element write into a text of `length` UTF-8 bytes.
    fn charge_string_path_write(&self, length: usize) -> Result<(), RuntimeError> {
        // write_string_element collects Unicode scalars, then builds new UTF-8;
        // STRING also copies that result into SmolStr. Include vector growth and
        // the replacement scalar's possible UTF-8 expansion before either allocation.
        let units = length.checked_add(4).ok_or(RuntimeError::Overflow)?;
        self.charge_work_units(units.checked_mul(3).ok_or(RuntimeError::Overflow)?)?;
        self.charge_allocation_bytes(units.checked_mul(16).ok_or(RuntimeError::Overflow)?)
    }
}

/// Walks the path of a physical child to the structure it names. The walk charges nothing: a
/// child's copy-on-write detach is bookkeeping (TIME-08), so this only checks the path.
fn validate_physical_child(
    mut value: &Value,
    path: &[RefSegment],
    depth: usize,
) -> Result<(), RuntimeError> {
    for (index, segment) in path.iter().enumerate() {
        if depth + index >= crate::bytecode::BYTECODE_MAX_TYPE_NESTING {
            return Err(RuntimeError::TypeMismatch);
        }
        value = match (value, segment) {
            (Value::Struct(structure), RefSegment::Field(name)) => {
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
    match value {
        Value::Struct(_) => Ok(()),
        _ => Err(RuntimeError::TypeMismatch),
    }
}

/// The charge of a write into a structure that lies as its backing (C1-15b).
#[path = "path_write/backing.rs"]
mod backing;

#[cfg(test)]
mod tests {
    use super::*;
    use crate::value::ArrayValue;
    use crate::vm::{
        call::{bindings::VmWriteTarget, context::CallContext},
        VmTrap,
    };

    #[test]
    fn output_copyback_into_a_shared_aggregate_charges_no_detach() {
        // `aggregate_probe : Probe`, a structure of `small : DINT` and a 2,048-element array
        // `large`, and `first_probe : DINT`, declared beside the cell fixture's globals.
        let prepared = {
            let mut builder = crate::memory::cell_access::fixture::Builder::new();
            let small = builder.name("small");
            let large = builder.name("large");
            let probe = builder.name("Probe");
            let array = builder.ty(crate::memory::cell_access::fixture::anonymous(
                crate::bytecode::TypeKind::Array,
                crate::bytecode::TypeData::Array {
                    elem_type_id: crate::memory::cell_access::fixture::DINT_TYPE,
                    dims: vec![(0, 2047)],
                },
            ));
            let record = builder.ty(crate::memory::cell_access::fixture::named(
                crate::bytecode::TypeKind::Struct,
                probe,
                crate::bytecode::TypeData::Struct {
                    fields: vec![
                        crate::memory::cell_access::fixture::field(
                            small,
                            crate::memory::cell_access::fixture::DINT_TYPE,
                        ),
                        crate::memory::cell_access::fixture::field(large, array),
                    ],
                },
            ));
            builder.global("aggregate_probe", record);
            builder.global(
                "first_probe",
                crate::memory::cell_access::fixture::DINT_TYPE,
            );
            builder.prepare()
        };
        let mut state_data =
            EngineState::construct_data(&prepared, 0, &super::super::services::LOGICAL_ONLY)
                .unwrap();
        let mut state = EngineState::borrow(
            &mut state_data,
            &prepared,
            &super::super::services::LOGICAL_ONLY,
        );
        state
            .write_named_path(
                "aggregate_probe",
                &[RefSegment::Field("small".into())],
                Value::DInt(9),
            )
            .unwrap();
        crate::vm::engine::test_store::set_global(&mut state, "first_probe", Value::DInt(7));
        let first =
            crate::memory::view::StoreAccess::ref_for_global(&state, "first_probe").unwrap();
        let mut second =
            crate::memory::view::StoreAccess::ref_for_global(&state, "aggregate_probe").unwrap();
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
        // A member write into the aggregate copies nothing of it (TIME-08): on the flat store it
        // writes the member's bytes where they lie, so the 1024 spare bytes admit both writes.
        state
            .with_output_transaction(&mut frame, |state, frame| {
                state
                    .data
                    .resources
                    .construction_bytes
                    .set(prepared.limits.max_construction_bytes - 1024);
                VmWriteTarget::from_reference(&first).write(state, frame, Value::DInt(70))?;
                VmWriteTarget::from_reference(&second).write(state, frame, Value::DInt(90))
            })
            .unwrap();
        assert_eq!(
            crate::vm::engine::test_store::global(&state, "first_probe"),
            Some(Value::DInt(70))
        );
        assert_eq!(
            crate::vm::engine::test_store::read(&state, &second),
            Some(Value::DInt(90))
        );
    }

    #[test]
    fn string_element_charge_fails_before_replacing_backing_text() {
        // `text_probe : STRING[10]`, declared beside the cell fixture's globals.
        let prepared = crate::memory::cell_access::fixture::prepared_with_globals(&[(
            "text_probe",
            crate::memory::cell_access::fixture::primitive(24, 10),
        )]);
        let mut state_data =
            EngineState::construct_data(&prepared, 0, &super::super::services::LOGICAL_ONLY)
                .unwrap();
        let mut state = EngineState::borrow(
            &mut state_data,
            &prepared,
            &super::super::services::LOGICAL_ONLY,
        );
        crate::vm::engine::test_store::set_global(
            &mut state,
            "text_probe",
            Value::String("abc".into()),
        );
        let mut reference =
            crate::memory::view::StoreAccess::ref_for_global(&state, "text_probe").unwrap();
        reference.path.push(RefSegment::Index(vec![1]));
        state
            .data
            .resources
            .construction_bytes
            .set(prepared.limits.max_construction_bytes);
        let error = crate::vm::context::ReferenceContext::write_typed_storage(
            &mut state,
            reference.location,
            reference.offset as usize,
            &reference.path,
            Value::Char(b'Z'),
            crate::value::MAX_STRING_CAPACITY,
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
            crate::vm::engine::test_store::global(&state, "text_probe"),
            Some(Value::String("abc".into()))
        );
    }
    #[test]
    fn hundred_field_physical_write_fits_small_work_allowance_and_precharges_scratch() {
        // `probe : Hundred`, a `STRUCT OVERLAP` of the hundred `DWORD` members `f000` to `f099`, each at
        // its own 32 bits, declared beside the cell fixture's globals (specification 02 §3.6).
        let prepared = {
            let mut builder = crate::memory::cell_access::fixture::Builder::new();
            let dword = builder.ty(crate::memory::cell_access::fixture::primitive(4, 0));
            let names: Vec<u32> = (0..100)
                .map(|i| builder.name(&alloc::format!("f{i:03}")))
                .collect();
            // ST declares a `STRUCT OVERLAP` as a named type only, and the store constructs one so.
            let hundred_name = builder.name("Hundred");
            let hundred = builder.ty(crate::memory::cell_access::fixture::named(
                crate::bytecode::TypeKind::OverlappingStruct,
                hundred_name,
                crate::bytecode::TypeData::Struct {
                    fields: names
                        .iter()
                        .zip(0u32..)
                        .map(|(&name_idx, i)| crate::bytecode::Field {
                            name_idx,
                            type_id: dword,
                            bit_offset: Some(i * 32),
                        })
                        .collect(),
                },
            ));
            builder.global("probe", hundred);
            builder.prepare()
        };
        let mut data =
            EngineState::construct_data(&prepared, 0, &super::super::services::LOGICAL_ONLY)
                .unwrap();
        let mut state =
            EngineState::borrow(&mut data, &prepared, &super::super::services::LOGICAL_ONLY);
        let mut reference =
            crate::memory::view::StoreAccess::ref_for_global(&state, "probe").unwrap();
        reference.path.push(RefSegment::Field("f050".into()));
        state.data.resources.work_budget.reset(4096);
        state.data.resources.construction_bytes.set(0);
        assert!(crate::vm::context::ReferenceContext::write_typed_storage(
            &mut state,
            reference.location,
            reference.offset as usize,
            &reference.path,
            Value::DWord(7),
            crate::value::MAX_STRING_CAPACITY,
        )
        .unwrap());
        assert_eq!(
            crate::vm::engine::test_store::read(&state, &reference),
            Some(Value::DWord(7))
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
            reference.offset as usize,
            &reference.path,
            Value::DWord(9),
            crate::value::MAX_STRING_CAPACITY,
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
            crate::vm::engine::test_store::read(&state, &reference),
            Some(Value::DWord(7))
        );
    }

    #[test]
    fn physical_patch_completion_samples_deadline_before_publication_but_scalar_stores_keep_stride()
    {
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
        // `physical_probe : Probe`, a `STRUCT OVERLAP` of `word : DWORD`, and `scalar_probe : DWORD`,
        // declared beside the cell fixture's globals.
        let prepared = {
            let mut builder = crate::memory::cell_access::fixture::Builder::new();
            let dword = builder.ty(crate::memory::cell_access::fixture::primitive(4, 0));
            let word = builder.name("word");
            let probe_name = builder.name("Probe");
            let overlap = builder.ty(crate::memory::cell_access::fixture::named(
                crate::bytecode::TypeKind::OverlappingStruct,
                probe_name,
                crate::bytecode::TypeData::Struct {
                    fields: vec![crate::bytecode::Field {
                        name_idx: word,
                        type_id: dword,
                        bit_offset: Some(0),
                    }],
                },
            ));
            builder.global("physical_probe", overlap);
            builder.global("scalar_probe", dword);
            builder.prepare()
        };
        let mut data =
            EngineState::construct_data(&prepared, 0, &super::super::services::LOGICAL_ONLY)
                .unwrap();
        let (reference, scalar) = {
            let mut state =
                EngineState::borrow(&mut data, &prepared, &super::super::services::LOGICAL_ONLY);
            let word = [RefSegment::Field("word".into())];
            state
                .write_named_path("physical_probe", &word, Value::DWord(7))
                .unwrap();
            let mut reference =
                crate::memory::view::StoreAccess::ref_for_global(&state, "physical_probe").unwrap();
            reference.path.extend(word);
            (
                reference,
                crate::memory::view::StoreAccess::ref_for_global(&state, "scalar_probe").unwrap(),
            )
        };
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
                scalar.offset as usize,
                &[],
                Value::DWord(value),
                crate::value::MAX_STRING_CAPACITY,
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
            reference.offset as usize,
            &reference.path,
            Value::DWord(9),
            crate::value::MAX_STRING_CAPACITY,
        )
        .unwrap_err();
        assert_eq!(error, VmTrap::DeadlineExceeded.into_runtime_error());
        assert_eq!(
            probe.calls.get(),
            2,
            "completion samples after the non-expired aggregate work sample"
        );
        assert_eq!(
            crate::vm::engine::test_store::read(&state, &reference),
            Some(Value::DWord(7))
        );
        assert_eq!(
            crate::vm::engine::test_store::read(&state, &scalar),
            Some(Value::DWord(8))
        );
    }

    #[test]
    fn function_block_reference_binding_keeps_physical_subrange_fault_before_publication() {
        // `probe : Range`, a `STRUCT OVERLAP` of `raw : INT` and `bounded : INT(0..10)` over the same
        // bits, declared beside the cell fixture's globals (specification 02 §3.6).
        let prepared = {
            let mut builder = crate::memory::cell_access::fixture::Builder::new();
            let int = builder.ty(crate::memory::cell_access::fixture::primitive(7, 0));
            let bounded_type = builder.ty(crate::memory::cell_access::fixture::anonymous(
                crate::bytecode::TypeKind::Subrange,
                crate::bytecode::TypeData::Subrange {
                    base_type_id: int,
                    lower: 0,
                    upper: 10,
                },
            ));
            let raw = builder.name("raw");
            let bounded = builder.name("bounded");
            let range_name = builder.name("Range");
            let range = builder.ty(crate::memory::cell_access::fixture::named(
                crate::bytecode::TypeKind::OverlappingStruct,
                range_name,
                crate::bytecode::TypeData::Struct {
                    fields: vec![
                        crate::bytecode::Field {
                            name_idx: raw,
                            type_id: int,
                            bit_offset: Some(0),
                        },
                        crate::bytecode::Field {
                            name_idx: bounded,
                            type_id: bounded_type,
                            bit_offset: Some(0),
                        },
                    ],
                },
            ));
            builder.global("probe", range);
            builder.prepare()
        };
        let mut data =
            EngineState::construct_data(&prepared, 0, &super::super::services::LOGICAL_ONLY)
                .unwrap();
        let mut reference = {
            let mut state =
                EngineState::borrow(&mut data, &prepared, &super::super::services::LOGICAL_ONLY);
            let raw = [RefSegment::Field("raw".into())];
            state
                .write_named_path("probe", &raw, Value::Int(7))
                .unwrap();
            let mut reference =
                crate::memory::view::StoreAccess::ref_for_global(&state, "probe").unwrap();
            reference.path.extend(raw);
            reference
        };
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
            crate::vm::engine::test_store::read(&state, &reference),
            Some(Value::Int(7))
        );
        reference.path[0] = RefSegment::Field("bounded".into());
        assert_eq!(
            crate::vm::engine::test_store::read(&state, &reference),
            Some(Value::Int(7))
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
