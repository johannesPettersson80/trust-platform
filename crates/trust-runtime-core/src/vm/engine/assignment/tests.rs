use super::*;
use crate::bytecode::{BytecodeModule, SectionData, SectionId, TypeEntry, TypeKind, TypeTable};
use crate::value::ArrayValue;
use crate::vm::{engine::services, PreparedModule};
use alloc::{boxed::Box, format, vec};

const FIELDS: usize = 128;

fn prepared_types() -> (PreparedModule, u32, u32) {
    let mut module = BytecodeModule::decode(include_bytes!(
        "../../../../../trust-runtime/tests/fixtures/portability/f401/gpio.stbc"
    ))
    .unwrap();
    let SectionData::StringTable(strings) = module.section_mut(SectionId::StringTable).unwrap()
    else {
        panic!("fixture strings")
    };
    let name = strings.entries.len() as u32;
    strings.entries.push("AssignmentRecord".into());
    let first_field = strings.entries.len() as u32;
    for index in 0..FIELDS {
        strings.entries.push(format!("Field{index:03}").into());
    }
    let SectionData::TypeTable(types) = module.section_mut(SectionId::TypeTable).unwrap() else {
        panic!("fixture types")
    };
    let integer = types
        .entries
        .iter()
        .position(|entry| matches!(entry.data, TypeData::Primitive { prim_id: 7, .. }))
        .unwrap() as u32;
    let array = types.entries.len() as u32;
    types.entries.push(TypeEntry {
        kind: TypeKind::Array,
        name_idx: None,
        data: TypeData::Array {
            elem_type_id: integer,
            dims: vec![(1, 4096)],
        },
    });
    let record = types.entries.len() as u32;
    types.entries.push(TypeEntry {
        kind: TypeKind::Struct,
        name_idx: Some(name),
        data: TypeData::Struct {
            fields: (0..FIELDS)
                .map(|index| Field {
                    name_idx: first_field + index as u32,
                    type_id: integer,
                    bit_offset: None,
                })
                .collect(),
        },
    });
    let prepared = PreparedModule::from_bytes(
        &module.encode().unwrap(),
        crate::vm::PreparationLimits::default(),
    )
    .unwrap();
    (prepared, array, record)
}

fn prepared_primitive_types() -> (PreparedModule, u32, u32, u32, u32, u32) {
    let mut module = BytecodeModule::decode(include_bytes!(
        "../../../../../trust-runtime/tests/fixtures/portability/f401/gpio.stbc"
    ))
    .unwrap();
    let SectionData::TypeTable(types) = module.section_mut(SectionId::TypeTable).unwrap() else {
        panic!("fixture types")
    };
    let real = append_primitive(types, 14, 0);
    let lreal = append_primitive(types, 15, 0);
    let dint = append_primitive(types, 8, 0);
    let string = append_primitive(types, 24, 2);
    let wstring = append_primitive(types, 25, 2);
    let prepared = PreparedModule::from_bytes(
        &module.encode().unwrap(),
        crate::vm::PreparationLimits::default(),
    )
    .unwrap();
    (prepared, real, lreal, dint, string, wstring)
}

fn append_primitive(types: &mut TypeTable, prim_id: u16, max_length: u16) -> u32 {
    let index = types.entries.len() as u32;
    types.entries.push(TypeEntry {
        kind: TypeKind::Primitive,
        name_idx: None,
        data: TypeData::Primitive {
            prim_id,
            max_length,
        },
    });
    index
}

#[test]
fn owned_array_assignment_normalizes_in_place_without_another_compound_copy() {
    let (prepared, ty, _) = prepared_types();
    let mut data = EngineState::construct_data(&prepared, 0, &services::LOGICAL_ONLY).unwrap();
    data.resources.construction_bytes.set(0);
    data.resources.work_budget.reset(prepared.limits.max_work);
    let state = EngineState::borrow(&mut data, &prepared, &services::LOGICAL_ONLY);
    let value = Box::new(ArrayValue::from_canonical_parts(
        vec![Value::SInt(7); 4096],
        vec![(1, 4096)],
    ));
    let pointer = value.elements().as_ptr();
    let Value::Array(result) = state
        .normalize_assignment_value(ty, Value::Array(value))
        .unwrap()
    else {
        panic!("array result")
    };
    assert_eq!(result.elements().as_ptr(), pointer);
    assert!(result
        .elements()
        .iter()
        .all(|value| *value == Value::Int(7)));
    assert_eq!(state.data.resources.construction_bytes.get(), 0);
}

#[test]
fn canonical_struct_assignment_reuses_immutable_storage_after_borrowed_validation() {
    let (prepared, _, ty) = prepared_types();
    let mut data = EngineState::construct_data(&prepared, 0, &services::LOGICAL_ONLY).unwrap();
    data.resources.construction_bytes.set(0);
    data.resources.work_budget.reset(prepared.limits.max_work);
    let state = EngineState::borrow(&mut data, &prepared, &services::LOGICAL_ONLY);
    let value = Arc::new(StructValue::from_canonical_parts(
        "AssignmentRecord".into(),
        (0..FIELDS)
            .map(|index| (format!("Field{index:03}").into(), Value::Int(7)))
            .collect(),
    ));
    let Value::Struct(result) = state
        .normalize_assignment_value(ty, Value::Struct(Arc::clone(&value)))
        .unwrap()
    else {
        panic!("struct result")
    };
    assert!(Arc::ptr_eq(&value, &result));
    assert_eq!(state.data.resources.construction_bytes.get(), 0);
}

#[test]
fn unordered_case_folded_fields_use_prepared_positions_without_work_charges() {
    let (prepared, _, ty) = prepared_types();
    let mut data = EngineState::construct_data(&prepared, 0, &services::LOGICAL_ONLY).unwrap();
    data.resources.work_budget.reset(64_000);
    let state = EngineState::borrow(&mut data, &prepared, &services::LOGICAL_ONLY);
    let value = Arc::new(StructValue::from_canonical_parts(
        "assignmentrecord".into(),
        (0..FIELDS)
            .rev()
            .map(|index| (format!("field{index:03}").into(), Value::SInt(7)))
            .collect(),
    ));
    let Value::Struct(result) = state
        .normalize_assignment_value(ty, Value::Struct(value))
        .unwrap()
    else {
        panic!("struct result")
    };
    assert_eq!(result.type_name().as_str(), "AssignmentRecord");
    for (index, (name, value)) in result.fields().iter().enumerate() {
        assert_eq!(name.as_str(), format!("Field{index:03}"));
        assert_eq!(*value, Value::Int(7));
    }
    // TIME-08: the rebuild of a non-canonical structure is normalization bookkeeping. Its copy is
    // charged at the load, and this call has none, so no work is consumed.
    assert_eq!(state.data.resources.work_budget.remaining(), 64_000);
}

#[test]
fn an_admitted_enumeration_keeps_its_derived_integer_base_constraints() {
    use crate::bytecode::EnumVariant;
    use crate::value::EnumValue;
    let mut module = BytecodeModule::decode(include_bytes!(
        "../../../../../trust-runtime/tests/fixtures/portability/f401/gpio.stbc"
    ))
    .unwrap();
    let SectionData::StringTable(strings) = module.section_mut(SectionId::StringTable).unwrap()
    else {
        panic!("strings")
    };
    let name = strings.entries.len() as u32;
    strings
        .entries
        .extend(["BoundedChoice".into(), "one".into()]);
    let SectionData::TypeTable(types) = module.section_mut(SectionId::TypeTable).unwrap() else {
        panic!("types")
    };
    let base = types.entries.len() as u32;
    types.entries.extend([
        TypeEntry {
            kind: TypeKind::Primitive,
            name_idx: None,
            data: TypeData::Primitive {
                prim_id: 10,
                max_length: 0,
            },
        },
        TypeEntry {
            kind: TypeKind::Subrange,
            name_idx: None,
            data: TypeData::Subrange {
                base_type_id: base,
                lower: 1,
                upper: 3,
            },
        },
        TypeEntry {
            kind: TypeKind::Enum,
            name_idx: Some(name),
            data: TypeData::Enum {
                base_type_id: base + 1,
                variants: vec![EnumVariant {
                    name_idx: name + 1,
                    value: 1,
                }],
            },
        },
    ]);
    let prepared = PreparedModule::from_bytes(
        &module.encode().unwrap(),
        crate::vm::PreparationLimits::default(),
    )
    .unwrap();
    let mut data = EngineState::construct_data(&prepared, 0, &services::LOGICAL_ONLY).unwrap();
    let state = EngineState::borrow(&mut data, &prepared, &services::LOGICAL_ONLY);
    let valid = Value::Enum(Box::new(EnumValue::from_canonical_parts(
        "BoundedChoice".into(),
        "one".into(),
        1,
    )));
    assert_eq!(
        state
            .normalize_assignment_value(base + 2, valid.clone())
            .unwrap(),
        valid
    );
    let invalid = Value::Enum(Box::new(EnumValue::from_canonical_parts(
        "BoundedChoice".into(),
        "one".into(),
        4,
    )));
    assert_eq!(
        state.normalize_assignment_value(base + 2, invalid),
        Err(RuntimeError::TypeMismatch)
    );
}

#[test]
fn primitive_assignment_reuses_exact_values_and_preserves_existing_coercions() {
    let (prepared, real, lreal, dint, _, _) = prepared_primitive_types();
    let mut data = EngineState::construct_data(&prepared, 0, &services::LOGICAL_ONLY).unwrap();
    data.resources.construction_bytes.set(0);
    data.resources.work_budget.reset(prepared.limits.max_work);
    let state = EngineState::borrow(&mut data, &prepared, &services::LOGICAL_ONLY);

    let negative_zero = Value::Real(-0.0);
    let Value::Real(normalized_negative_zero) = state
        .normalize_assignment_value(real, negative_zero)
        .unwrap()
    else {
        panic!("REAL assignment keeps the REAL family")
    };
    assert_eq!(normalized_negative_zero.to_bits(), (-0.0f32).to_bits());

    let nan_payload = f32::from_bits(0x7fc0_1234);
    let Value::Real(normalized_nan) = state
        .normalize_assignment_value(real, Value::Real(nan_payload))
        .unwrap()
    else {
        panic!("REAL NaN assignment keeps the REAL family")
    };
    assert_eq!(normalized_nan.to_bits(), nan_payload.to_bits());

    assert_eq!(
        state
            .normalize_assignment_value(dint, Value::Int(-91))
            .unwrap(),
        Value::DInt(-91),
        "cross-kind integer coercion remains available"
    );
    assert_eq!(
        state
            .normalize_assignment_value(lreal, Value::Real(1.25))
            .unwrap(),
        Value::LReal(1.25),
        "cross-kind float widening remains available"
    );
    let Value::LReal(widened_zero) = state
        .normalize_assignment_value(lreal, Value::Real(-0.0))
        .unwrap()
    else {
        panic!("LREAL coercion widens REAL")
    };
    assert_eq!(widened_zero.to_bits(), (-0.0f64).to_bits());
    let Value::LReal(widened_nan) = state
        .normalize_assignment_value(lreal, Value::Real(f32::NAN))
        .unwrap()
    else {
        panic!("LREAL coercion widens REAL NaN")
    };
    assert!(widened_nan.is_nan());
}

#[test]
fn bounded_primitive_strings_normalize_before_same_family_fast_return() {
    let (prepared, _, _, _, string, wstring) = prepared_primitive_types();
    let mut data = EngineState::construct_data(&prepared, 0, &services::LOGICAL_ONLY).unwrap();
    data.resources.construction_bytes.set(0);
    data.resources.work_budget.reset(prepared.limits.max_work);
    let state = EngineState::borrow(&mut data, &prepared, &services::LOGICAL_ONLY);

    // STRING[2] holds two bytes of UTF-8 and WSTRING[2] two UTF-16 code units: "å", "β" and
    // "γ" are two bytes but one code unit each (docs/IEC_DECISIONS.md 2026-10-17).
    assert_eq!(
        state
            .normalize_assignment_value(string, Value::String("åβγ".into()))
            .unwrap(),
        Value::String("å".into())
    );
    assert_eq!(
        state.data.resources.construction_bytes.get(),
        2,
        "STRING truncation retains its charged UTF-8 result allocation"
    );
    assert_eq!(
        state
            .normalize_assignment_value(wstring, Value::WString("åβγ".into()))
            .unwrap(),
        Value::WString("åβ".into())
    );
    assert_eq!(
        state.data.resources.construction_bytes.get(),
        6,
        "WSTRING truncation charges its kept prefix: 2 + 2 UTF-16 units x 2 bytes = 6"
    );
    assert_eq!(
        state.normalize_assignment_value(string, Value::WString("å".into())),
        Err(RuntimeError::TypeMismatch),
        "bounded STRING keeps the legacy cross-family rejection"
    );
}

#[test]
fn primitive_type_table_policy_preserves_any_int_values_and_null() {
    let types = TypeTable {
        entries: vec![TypeEntry {
            kind: TypeKind::Primitive,
            name_idx: None,
            data: TypeData::Primitive {
                prim_id: 0x0100,
                max_length: 0,
            },
        }],
        ..TypeTable::default()
    };
    assert_eq!(
        crate::vm::type_policy::normalize_value_for_type_table(&types, 0, Value::LInt(-123), 0,)
            .unwrap(),
        Value::LInt(-123),
        "ANY_INT preserves width and signedness"
    );
    assert_eq!(
        crate::vm::type_policy::normalize_value_for_type_table(&types, 0, Value::Null, 0).unwrap(),
        Value::Null,
        "unbound ANY_INT remains NULL"
    );
    assert_eq!(
        crate::vm::type_policy::normalize_value_for_type_table(
            &types,
            0,
            Value::String("wrong family".into()),
            0,
        ),
        Err(RuntimeError::TypeMismatch)
    );
}

/// Spec 34 TIME-08: a non-canonical structure assignment is the load's copy and then the rebuild.
/// The rebuild is normalization bookkeeping (no charge beyond the text rows), so the assignment
/// charges exactly what a plain copy of the same value charges. It catches a rebuild that charges
/// its copy again.
#[test]
fn a_non_canonical_structure_assignment_charges_what_a_plain_copy_charges() {
    use crate::vm::context::ReferenceContext;
    let (prepared, _, ty) = prepared_types();
    // Reversed, case-folded field names and SINT values make the value non-canonical, so the
    // assignment rebuilds it into the record type's order and INT values.
    let value = Arc::new(StructValue::from_canonical_parts(
        "assignmentrecord".into(),
        (0..FIELDS)
            .rev()
            .map(|index| (format!("field{index:03}").into(), Value::SInt(7)))
            .collect(),
    ));

    // A plain copy: 1 unit for the structure and 1 for each of its 128 fields (SINT leaves pay no
    // row charge), so 1 + 128 = 129 units, and one 16-byte cell per leaf: 128 x 16 = 2,048 bytes.
    let mut copy_data = EngineState::construct_data(&prepared, 0, &services::LOGICAL_ONLY).unwrap();
    copy_data.resources.work_budget.reset(1_000);
    copy_data.resources.construction_bytes.set(0);
    let copy = EngineState::borrow(&mut copy_data, &prepared, &services::LOGICAL_ONLY);
    copy.before_value_clone(&Value::Struct(Arc::clone(&value)))
        .unwrap();
    assert_eq!(copy.data.resources.work_budget.remaining(), 1_000 - 129);
    assert_eq!(copy.data.resources.construction_bytes.get(), 2_048);

    // The assignment: the same load, then the normalization that rebuilds the value.
    let mut assign_data =
        EngineState::construct_data(&prepared, 0, &services::LOGICAL_ONLY).unwrap();
    assign_data.resources.work_budget.reset(1_000);
    assign_data.resources.construction_bytes.set(0);
    let assign = EngineState::borrow(&mut assign_data, &prepared, &services::LOGICAL_ONLY);
    assign
        .before_value_clone(&Value::Struct(Arc::clone(&value)))
        .unwrap();
    let Value::Struct(rebuilt) = assign
        .normalize_assignment_value(ty, Value::Struct(value))
        .unwrap()
    else {
        panic!("struct result")
    };
    assert_eq!(rebuilt.type_name().as_str(), "AssignmentRecord");
    assert_eq!(
        assign.data.resources.work_budget.remaining(),
        copy.data.resources.work_budget.remaining()
    );
    assert_eq!(
        assign.data.resources.construction_bytes.get(),
        copy.data.resources.construction_bytes.get()
    );
}

/// Spec 34 TIME-08: a text cut at a store allocates its kept prefix by the copy rule, two bytes
/// per UTF-16 code unit for a WSTRING. It catches a cut that charges UTF-8 bytes (the STRING
/// rule) or no allocation at all.
#[test]
fn a_wstring_cut_at_a_store_allocates_its_kept_prefix_at_two_bytes_per_unit() {
    let (mut prepared, _, _) = prepared_types();
    // A WSTRING[3]: primitive id 25 with a declared length of 3.
    let wstring3 = prepared.vm.types.entries.len() as u32;
    prepared.vm.types.entries.push(TypeEntry {
        kind: TypeKind::Primitive,
        name_idx: None,
        data: TypeData::Primitive {
            prim_id: 25,
            max_length: 3,
        },
    });
    let mut data = EngineState::construct_data(&prepared, 0, &services::LOGICAL_ONLY).unwrap();
    data.resources.construction_bytes.set(0);
    data.resources.work_budget.reset(100);
    let state = EngineState::borrow(&mut data, &prepared, &services::LOGICAL_ONLY);
    let stored = state
        .normalize_assignment_value(wstring3, Value::WString("abcde".into()))
        .unwrap();
    assert_eq!(stored, Value::WString("abc".into()));
    // The text row charges 2 units per UTF-8 byte of the value: 2 x 5 = 10 units. The kept prefix
    // "abc" is three UTF-16 code units at 2 bytes each: 3 x 2 = 6 bytes.
    assert_eq!(state.data.resources.work_budget.remaining(), 90);
    assert_eq!(state.data.resources.construction_bytes.get(), 6);
}
