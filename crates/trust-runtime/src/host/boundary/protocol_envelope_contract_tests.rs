use super::*;
use crate::value::ArrayValue;

#[test]
fn boundary_entry_contract_ok_contains_exact_value_only() {
    let entry = BoundaryEntry::ok(Value::DInt(42));
    assert_eq!(entry.status, BoundaryEntryStatus::Ok);
    assert_eq!(entry.value, Some(Value::DInt(42)));
    assert_eq!(entry.error, None);
}

#[test]
fn boundary_entry_contract_error_contains_exact_error_only() {
    let error = BoundaryError::UnresolvedName {
        path: "missing".into(),
    };
    let entry = BoundaryEntry::error(error.clone());
    assert_eq!(entry.status, BoundaryEntryStatus::Error);
    assert_eq!(entry.value, None);
    assert_eq!(entry.error, Some(error));
}

#[test]
fn boundary_entry_contract_is_ok_tracks_status() {
    assert!(BoundaryEntry::ok(Value::Bool(true)).is_ok());
    assert!(!BoundaryEntry::error(BoundaryError::InternalFailure { context: "test" }).is_ok());
}

#[test]
fn boundary_entry_contract_clone_preserves_complete_payload() {
    let entry = BoundaryEntry::error(BoundaryError::AmbiguousName {
        path: "state".into(),
        candidates: vec!["A.state".into(), "B.state".into()],
    });
    assert_eq!(entry.clone(), entry);
}

#[test]
fn boundary_entry_contract_status_values_are_distinct() {
    assert_ne!(BoundaryEntryStatus::Ok, BoundaryEntryStatus::Error);
    assert_eq!(BoundaryEntryStatus::Ok, BoundaryEntryStatus::Ok);
    assert_eq!(BoundaryEntryStatus::Error, BoundaryEntryStatus::Error);
}

#[test]
fn boundary_entry_contract_ok_preserves_composite_runtime_value() {
    let value = Value::Array(Box::new(ArrayValue::from_canonical_parts(
        vec![Value::Int(1), Value::Int(2)],
        vec![(4, 5)],
    )));
    let entry = BoundaryEntry::ok(value.clone());
    assert_eq!(entry.value, Some(value));
}
