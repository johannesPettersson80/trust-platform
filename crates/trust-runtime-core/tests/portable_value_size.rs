use trust_runtime_core::error::RuntimeError;
use trust_runtime_core::value::{size_of_value_with, ArrayValue, EnumValue, SizeOfError, Value};

fn size(value: &Value) -> Result<u64, SizeOfError> {
    size_of_value_with(
        value,
        &mut |_| Err(SizeOfError::UnsupportedType),
        &mut |_| Ok(()),
    )
}
#[test]
fn runtime_size_counts_capacity_units_and_keeps_the_first_array_element_rule() {
    // A STRING value occupies its UTF-8 bytes and a WSTRING value two bytes per UTF-16 code
    // unit (docs/IEC_DECISIONS.md 2026-10-17): "å" and "β" are two bytes and one unit each.
    assert_eq!(size(&Value::String("åβ".into())), Ok(4));
    assert_eq!(size(&Value::WString("åβ".into())), Ok(4));
    assert_eq!(size(&Value::WString("🙂".into())), Ok(4));
    let array = Value::Array(Box::new(ArrayValue::from_canonical_parts(
        vec![Value::String("a".into()), Value::String("longer".into())],
        vec![(5, 6)],
    )));
    assert_eq!(size(&array), Ok(2));
    assert_eq!(
        size(&Value::Reference(None)),
        Ok(core::mem::size_of::<usize>() as u64)
    );
    assert_eq!(size(&Value::Null), Err(SizeOfError::UnsupportedType));
}
#[test]
fn enum_size_uses_metadata_callback_and_preserves_callback_fault() {
    let value = Value::Enum(Box::new(EnumValue::from_canonical_parts(
        "Mode".into(),
        "Run".into(),
        2,
    )));
    let mut names = Vec::new();
    let result: Result<u64, SizeOfError> = size_of_value_with(
        &value,
        &mut |name| {
            names.push(name.to_owned());
            Ok(2)
        },
        &mut |_| Ok(()),
    );
    assert_eq!(result, Ok(2));
    assert_eq!(names, vec!["Mode"]);
    let error = RuntimeError::ExecutionTimeout;
    let result: Result<u64, RuntimeError> =
        size_of_value_with(&value, &mut |_| Ok(2), &mut |_| Err(error.clone()));
    assert_eq!(result, Err(error));
}
#[test]
fn runtime_value_size_accepts_128_nested_nodes_and_rejects_the_first_excess() {
    let mut value = Value::Int(7);
    for _ in 0..127 {
        value = Value::Array(Box::new(ArrayValue::from_canonical_parts(
            vec![value],
            vec![(0, 0)],
        )));
    }
    assert_eq!(size(&value), Ok(2));
    value = Value::Array(Box::new(ArrayValue::from_canonical_parts(
        vec![value],
        vec![(0, 0)],
    )));
    assert_eq!(size(&value), Err(SizeOfError::UnsupportedType));
}
