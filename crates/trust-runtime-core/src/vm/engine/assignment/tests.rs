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
    // TIME-08: field matching and normalization passes are bookkeeping, so no work is consumed.
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

    assert_eq!(
        state
            .normalize_assignment_value(string, Value::String("åβγ".into()))
            .unwrap(),
        Value::String("åβ".into())
    );
    assert_eq!(
        state.data.resources.construction_bytes.get(),
        4,
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
        4,
        "in-place WSTRING truncation adds no second text allocation"
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
