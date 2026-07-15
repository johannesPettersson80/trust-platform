use crate::bytecode::{TypeData, TypeEntry, TypeKind, TypeTable};
use crate::error::RuntimeError;
use crate::value::Value;

use super::normalize_value_for_type_table;

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
