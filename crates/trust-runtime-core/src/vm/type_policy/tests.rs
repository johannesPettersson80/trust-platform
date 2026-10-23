use super::normalize_value_for_type_table;
use crate::bytecode::{TypeData, TypeEntry, TypeKind, TypeTable};
use crate::error::RuntimeError;
use crate::value::Value;
use alloc::{vec, vec::Vec};

fn primitive_table(prim_id: u16) -> TypeTable {
    primitive_table_with_max(prim_id, 0)
}

fn primitive_table_with_max(prim_id: u16, max_length: u16) -> TypeTable {
    TypeTable {
        offsets: Vec::new(),
        entries: vec![TypeEntry {
            kind: TypeKind::Primitive,
            name_idx: None,
            data: TypeData::Primitive {
                prim_id,
                max_length,
            },
        }],
    }
}

fn generic_counter_table() -> TypeTable {
    let mut table = primitive_table(0x0100);
    for target_type_id in [0, 1] {
        table.entries.push(TypeEntry {
            kind: TypeKind::Alias,
            name_idx: None,
            data: TypeData::Alias { target_type_id },
        });
    }
    table
}

#[test]
fn generic_counter_policy_preserves_unbound_null_and_every_integer_width() {
    let table = generic_counter_table();
    let values = [
        Value::Null,
        Value::SInt(i8::MIN),
        Value::SInt(i8::MAX),
        Value::Int(i16::MIN),
        Value::Int(i16::MAX),
        Value::DInt(i32::MIN),
        Value::DInt(i32::MAX),
        Value::LInt(i64::MIN),
        Value::LInt(i64::MAX),
        Value::USInt(u8::MIN),
        Value::USInt(u8::MAX),
        Value::UInt(u16::MIN),
        Value::UInt(u16::MAX),
        Value::UDInt(u32::MIN),
        Value::UDInt(u32::MAX),
        Value::ULInt(u64::MIN),
        Value::ULInt(u64::MAX),
    ];
    for ty in 0..table.entries.len() as u32 {
        for value in &values {
            let actual = normalize_value_for_type_table(&table, ty, value.clone(), 0)
                .expect("generic counter slot or alias accepts NULL and IEC integers");
            assert_eq!(
                core::mem::discriminant(&actual),
                core::mem::discriminant(value)
            );
            assert_eq!(
                &actual, value,
                "type {ty} must not narrow or rebind {value:?}"
            );
        }
    }
}

fn non_integer_counter_values() -> Vec<Value> {
    use crate::collections::OrderedMap;
    use crate::memory::{InstanceId, MemoryLocation};
    use crate::value::{
        ArrayValue, DateTimeValue, DateValue, Duration, EnumValue, LDateTimeValue, LDateValue,
        LTimeOfDayValue, StructValue, TimeOfDayValue, ValueRef,
    };
    use alloc::{boxed::Box, sync::Arc};

    vec![
        Value::Bool(false),
        Value::Bool(true),
        Value::Real(1.0),
        Value::LReal(1.0),
        Value::Real(f32::NAN),
        Value::LReal(f64::INFINITY),
        Value::Byte(1),
        Value::Word(1),
        Value::DWord(1),
        Value::LWord(1),
        Value::Time(Duration::ZERO),
        Value::LTime(Duration::ZERO),
        Value::Date(DateValue::new(0)),
        Value::LDate(LDateValue::new(0)),
        Value::Tod(TimeOfDayValue::new(0)),
        Value::LTod(LTimeOfDayValue::new(0)),
        Value::Dt(DateTimeValue::new(0)),
        Value::Ldt(LDateTimeValue::new(0)),
        Value::String("1".into()),
        Value::WString("1".into()),
        Value::Char(b'1'),
        Value::WChar(u16::from(b'1')),
        Value::Array(Box::new(ArrayValue::from_canonical_parts(
            vec![Value::Int(1)],
            vec![(1, 1)],
        ))),
        Value::Struct(Arc::new(StructValue::from_canonical_parts(
            "Record".into(),
            OrderedMap::default(),
        ))),
        Value::Enum(Box::new(EnumValue::from_canonical_parts(
            "Mode".into(),
            "One".into(),
            1,
        ))),
        Value::Reference(None),
        Value::Reference(Some(ValueRef {
            location: MemoryLocation::Global,
            offset: 0,
            path: vec![],
        })),
        Value::Instance(InstanceId(0)),
    ]
}

#[test]
fn generic_counter_policy_rejects_every_non_integer_value_class_through_aliases() {
    let table = generic_counter_table();
    for ty in 0..table.entries.len() as u32 {
        for value in non_integer_counter_values() {
            let error = normalize_value_for_type_table(&table, ty, value.clone(), 0)
                .expect_err("ANY_INT must not accept other value classes");
            assert_eq!(
                error,
                RuntimeError::TypeMismatch,
                "type {ty}, value {value:?}"
            );
        }
    }
}

#[test]
fn primitive_policy_rejects_incompatible_runtime_tags() {
    let real = primitive_table(14);
    let lreal = primitive_table(15);

    for error in [
        normalize_value_for_type_table(&real, 0, Value::DInt(16_777_217), 0),
        normalize_value_for_type_table(&lreal, 0, Value::LInt(9_007_199_254_740_993), 0),
        normalize_value_for_type_table(&real, 0, Value::Bool(true), 0),
    ] {
        let error = error.expect_err("incompatible primitive tag must reject");
        assert_eq!(error, RuntimeError::TypeMismatch);
        assert_eq!(error.stable_code().as_str(), "runtime_type_mismatch");
    }
}

#[test]
fn primitive_policy_materializes_only_accuracy_preserving_float_widening() {
    let real = primitive_table(14);
    let lreal = primitive_table(15);

    assert_eq!(
        normalize_value_for_type_table(&real, 0, Value::Int(i16::MIN), 0),
        Ok(Value::Real(f32::from(i16::MIN)))
    );
    assert_eq!(
        normalize_value_for_type_table(&lreal, 0, Value::DInt(16_777_217), 0),
        Ok(Value::LReal(16_777_217.0))
    );
}

#[test]
fn bounded_string_policy_truncates_by_scalar_and_rejects_wrong_family() {
    let string = primitive_table_with_max(24, 2);
    let wstring = primitive_table_with_max(25, 2);

    assert_eq!(
        normalize_value_for_type_table(&string, 0, Value::String("ÄB".into()), 0),
        Ok(Value::String("ÄB".into()))
    );
    assert_eq!(
        normalize_value_for_type_table(&wstring, 0, Value::WString("🙂Ω".into()), 0),
        Ok(Value::WString("🙂Ω".into()))
    );
    assert_eq!(
        normalize_value_for_type_table(&string, 0, Value::String("ÄBC".into()), 0),
        Ok(Value::String("ÄB".into()))
    );
    assert_eq!(
        normalize_value_for_type_table(&wstring, 0, Value::WString("🙂ΩX".into()), 0),
        Ok(Value::WString("🙂Ω".into()))
    );
    for error in [
        normalize_value_for_type_table(&string, 0, Value::WString("AB".into()), 0),
        normalize_value_for_type_table(&wstring, 0, Value::String("AB".into()), 0),
    ] {
        let error = error.expect_err("cross-family string tag must reject");
        assert_eq!(error, RuntimeError::TypeMismatch);
        assert_eq!(error.stable_code().as_str(), "runtime_type_mismatch");
    }
}

#[test]
fn subrange_policy_accepts_inclusive_bounds_and_rejects_other_values() {
    let table = TypeTable {
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
                    lower: -2,
                    upper: 2,
                },
            },
        ],
    };

    assert_eq!(
        normalize_value_for_type_table(&table, 1, Value::Int(-2), 0),
        Ok(Value::Int(-2))
    );
    assert_eq!(
        normalize_value_for_type_table(&table, 1, Value::Int(2), 0),
        Ok(Value::Int(2))
    );
    for error in [
        normalize_value_for_type_table(&table, 1, Value::Int(-3), 0),
        normalize_value_for_type_table(&table, 1, Value::Int(3), 0),
    ] {
        let error = error.expect_err("out-of-range subrange value must reject");
        assert!(matches!(error, RuntimeError::SubrangeViolation { .. }));
        assert_eq!(error.stable_code().as_str(), "runtime_subrange_violation");
    }

    let error = normalize_value_for_type_table(&table, 1, Value::Real(1.0), 0)
        .expect_err("wrong subrange base tag must reject");
    assert_eq!(error, RuntimeError::TypeMismatch);
    assert_eq!(error.stable_code().as_str(), "runtime_type_mismatch");
}

#[test]
fn indexed_string_reference_resolves_character_assignment_type() {
    let prepared = crate::vm::PreparedModule::from_bytes(
        include_bytes!(
            "../../../../trust-runtime/tests/fixtures/portability/stbc-2.0/program-v2.stbc"
        ),
        crate::vm::PreparationLimits::default(),
    )
    .unwrap();
    let mut module = prepared.vm;
    module.types = TypeTable {
        offsets: Vec::new(),
        entries: [24, 25, 26, 27]
            .into_iter()
            .map(|prim_id| TypeEntry {
                kind: TypeKind::Primitive,
                name_idx: None,
                data: TypeData::Primitive {
                    prim_id,
                    max_length: 0,
                },
            })
            .collect(),
    };
    let path = [crate::value::RefSegment::Index(vec![1])];
    assert_eq!(super::vm_type_for_path(&module, 0, &path), Some(2));
    assert_eq!(super::vm_type_for_path(&module, 1, &path), Some(3));
    assert_eq!(super::vm_type_for_path(&module, 2, &path), None);
    assert_eq!(
        super::vm_type_for_path(&module, 0, &[crate::value::RefSegment::Index(vec![1, 2])]),
        None
    );
}
