use trust_runtime_core::collections::OrderedMap;
use trust_runtime_core::error::RuntimeError;
use trust_runtime_core::program_model::{apply_binary, BinaryOp};
use trust_runtime_core::retain::RetainSnapshot;
use trust_runtime_core::value::{DateTimeProfile, Duration, StructValue, Value};

#[test]
fn ordered_values_and_retain_preserve_replacement_order_and_independent_copies() {
    let mut fields = OrderedMap::default();
    fields.insert("SECOND".into(), Value::Int(2));
    fields.insert("FIRST".into(), Value::Int(1));
    fields.insert("SECOND".into(), Value::Int(3));
    let value = StructValue::from_canonical_parts("Pair".into(), fields.clone());
    let snapshot = RetainSnapshot::from_values(fields);
    let mut restored = snapshot.clone().into_values();
    restored.insert("FIRST".into(), Value::Int(9));
    assert_eq!(
        snapshot
            .values()
            .keys()
            .map(|key| key.as_str())
            .collect::<Vec<_>>(),
        ["SECOND", "FIRST"]
    );
    assert_eq!(value.fields().get("SECOND"), Some(&Value::Int(3)));
    assert_eq!(snapshot.values().get("FIRST"), Some(&Value::Int(1)));
    assert_eq!(restored.get("FIRST"), Some(&Value::Int(9)));
}

#[cfg(feature = "std")]
#[test]
fn hosted_map_api_keeps_the_indexmap_default_builder() {
    let mut fields = indexmap::IndexMap::new();
    fields.insert("VALUE".into(), Value::Int(7));
    let value = StructValue::from_canonical_parts("Record".into(), fields.clone());
    let _: &indexmap::IndexMap<smol_str::SmolStr, Value> = value.fields();
    let fields: indexmap::IndexMap<smol_str::SmolStr, Value> =
        RetainSnapshot::from_values(fields).into_values();
    assert_eq!(fields.get("VALUE"), Some(&Value::Int(7)));
}

fn binary(op: BinaryOp, left: Value, right: Value) -> Result<Value, RuntimeError> {
    apply_binary(op, left, right, &DateTimeProfile::default())
}

#[test]
fn portable_exponentiation_preserves_width_and_faults() {
    assert_eq!(
        binary(BinaryOp::Pow, Value::Real(2.0), Value::Real(3.0)),
        Ok(Value::Real(8.0))
    );
    let result = binary(BinaryOp::Pow, Value::LReal(2.0), Value::LReal(0.5)).unwrap();
    let Value::LReal(value) = result else {
        panic!("expected LREAL")
    };
    assert!((value - core::f64::consts::SQRT_2).abs() <= 8.0 * f64::EPSILON);
    assert_eq!(
        binary(BinaryOp::Pow, Value::LReal(-1.0), Value::LReal(0.5)),
        Err(RuntimeError::Overflow)
    );
    assert_eq!(
        binary(BinaryOp::Pow, Value::LReal(2.0), Value::LReal(1024.0)),
        Err(RuntimeError::Overflow)
    );
    assert_eq!(
        binary(BinaryOp::Pow, Value::Real(10.0), Value::Real(100.0)),
        Err(RuntimeError::Overflow)
    );
}

#[test]
fn real_arithmetic_retains_rounding_subnormals_and_checked_faults() {
    assert_eq!(
        binary(BinaryOp::Add, Value::Real(16_777_216.0), Value::Real(1.0)),
        Ok(Value::Real(16_777_216.0))
    );
    assert_eq!(
        binary(
            BinaryOp::Mul,
            Value::Real(f32::MIN_POSITIVE),
            Value::Real(0.5)
        ),
        Ok(Value::Real(f32::from_bits(0x0040_0000)))
    );
    let Value::Real(zero) = binary(BinaryOp::Mul, Value::Real(-0.0), Value::Real(1.0)).unwrap()
    else {
        panic!("expected REAL")
    };
    assert_eq!(zero.to_bits(), (-0.0_f32).to_bits());
    assert_eq!(
        binary(BinaryOp::Div, Value::Real(1.0), Value::Real(-0.0)),
        Err(RuntimeError::DivisionByZero)
    );
    assert_eq!(
        binary(BinaryOp::Mul, Value::Real(f32::MAX), Value::Real(2.0)),
        Err(RuntimeError::Overflow)
    );
}

#[test]
fn duration_scaling_truncates_toward_zero_and_keeps_faults() {
    for (factor, expected) in [(1.5, 4), (-1.5, -4)] {
        assert_eq!(
            binary(
                BinaryOp::Mul,
                Value::Time(Duration::from_nanos(3)),
                Value::LReal(factor)
            ),
            Ok(Value::Time(Duration::from_nanos(expected)))
        );
    }
    assert_eq!(
        binary(
            BinaryOp::Div,
            Value::LTime(Duration::from_nanos(-9)),
            Value::Real(2.0)
        ),
        Ok(Value::LTime(Duration::from_nanos(-4)))
    );
    assert_eq!(
        binary(
            BinaryOp::Div,
            Value::Time(Duration::from_nanos(3)),
            Value::LReal(0.0)
        ),
        Err(RuntimeError::DivisionByZero)
    );
    assert_eq!(
        binary(
            BinaryOp::Mul,
            Value::Time(Duration::from_nanos(i64::MAX)),
            Value::LReal(2.0)
        ),
        Err(RuntimeError::Overflow)
    );
}

#[test]
fn record_value_slot_layout() {
    use trust_runtime_core::value::layout::{VALUE_ALIGN_BYTES, VALUE_SIZE_BYTES};
    println!("value_slot_bytes={VALUE_SIZE_BYTES} value_slot_alignment={VALUE_ALIGN_BYTES}");
    assert!(VALUE_SIZE_BYTES >= core::mem::size_of::<f64>());
    assert!(VALUE_ALIGN_BYTES.is_power_of_two());
}
