//! Typed experiment-value and trajectory decoding, independent of lifecycle writes.
use super::super::parse_value;
use super::{RelayFeedbackParams, RelayFeedbackRecord, TrajectoryPoint};
use crate::io::{validate_process_image_address, IoAddress, IoSnapshot, IoSnapshotValue};
use crate::value::Value;
use serde::Deserialize;
use serde_json::Value as JsonValue;
use std::time::Duration;
use trust_hir::TypeId;

/// `coerce` converts an output value to the type the experiment's target holds it in.
pub(super) fn decode_relay_feedback(
    params: &RelayFeedbackParams,
    snapshot: &IoSnapshot,
    minimum: f64,
    maximum: f64,
    output_value_type: Option<&str>,
    coerce: &dyn Fn(Value) -> Result<Value, String>,
    requested: f64,
) -> Result<RelayFeedbackRecord, (String, &'static str)> {
    if !params.center.is_finite() || !params.hysteresis.is_finite() || params.hysteresis <= 0.0 {
        return Err((
            "relay center must be finite and hysteresis must be positive".to_string(),
            "invalid_experiment",
        ));
    }
    let address = IoAddress::parse(&params.address)
        .map_err(|error| (error.to_string(), "invalid_address"))?;
    validate_process_image_address(&address)
        .map_err(|error| (error.to_string(), "invalid_address"))?;
    if snapshot.forced.iter().any(|forced| forced == &address) {
        return Err((
            "relay feedback signal is forced".to_string(),
            "forced_signal",
        ));
    }
    let value_type =
        experiment_type_id(&params.value_type).map_err(|message| (message, "invalid_value"))?;
    if snapshot_type(snapshot, &address) != Some(value_type) {
        return Err((
            "declared relay feedback type does not match the runtime binding".to_string(),
            "incompatible_binding",
        ));
    }
    snapshot_value(snapshot, &address)
        .and_then(numeric_value)
        .filter(|value| value.is_finite())
        .ok_or_else(|| {
            (
                "relay feedback must be finite and numeric".to_string(),
                "bad_runtime_data",
            )
        })?;

    let decode_output = |value: &JsonValue| -> Result<(Value, f64), (String, &'static str)> {
        let raw = experiment_value(value, output_value_type)
            .map_err(|message| (message, "invalid_value"))?;
        let numeric = numeric_value(&raw)
            .filter(|value| value.is_finite())
            .ok_or_else(|| {
                (
                    "relay output must be finite and numeric".to_string(),
                    "invalid_value",
                )
            })?;
        if numeric < minimum || numeric > maximum {
            return Err((
                "relay output is outside declared limits".to_string(),
                "outside_limits",
            ));
        }
        let write_value = coerce(raw).map_err(|error| (error, "incompatible_binding"))?;
        Ok((write_value, numeric))
    };
    let (low_value, low) = decode_output(&params.low_value)?;
    let (high_value, high) = decode_output(&params.high_value)?;
    if low >= high || (requested - high).abs() > f64::EPSILON {
        return Err((
            "relay outputs must be ordered and the requested value must equal the high output"
                .to_string(),
            "invalid_experiment",
        ));
    }
    Ok(RelayFeedbackRecord {
        address,
        address_text: params.address.clone(),
        value_type,
        center: params.center,
        hysteresis: params.hysteresis,
        low_value,
        high_value,
        low_numeric: low,
        high_numeric: high,
        high_active: true,
    })
}

/// `coerce` converts a value to the type the experiment's target holds it in.
pub(super) fn decode_trajectory(
    points: Vec<TrajectoryPoint>,
    duration_ms: u64,
    minimum: f64,
    maximum: f64,
    value_type: Option<&str>,
    coerce: &dyn Fn(Value) -> Result<Value, String>,
) -> Result<Vec<(Duration, Value)>, String> {
    if points.len() > 10_000 {
        return Err("experiment trajectory exceeds 10000 points".to_string());
    }
    let mut previous = None;
    let mut values = Vec::with_capacity(points.len());
    for point in points {
        if point.at_ms >= duration_ms || previous.is_some_and(|last| point.at_ms <= last) {
            return Err(
                "experiment trajectory times must be strictly increasing and before duration"
                    .to_string(),
            );
        }
        if previous.is_none() && point.at_ms != 0 {
            return Err("experiment trajectory must begin at 0 ms".to_string());
        }
        let raw = experiment_value(&point.value, value_type)?;
        let numeric = numeric_value(&raw)
            .filter(|value| value.is_finite())
            .ok_or_else(|| "trajectory value must be finite and numeric".to_string())?;
        if numeric < minimum || numeric > maximum {
            return Err("trajectory value is outside declared limits".to_string());
        }
        values.push((Duration::from_millis(point.at_ms), coerce(raw)?));
        previous = Some(point.at_ms);
    }
    Ok(values)
}

/// Refuses a trajectory whose consecutive points are closer than the task's interval: the executor queues one point per
/// execution, so a closer point could never reach the program (Specification 35, A3.5).
pub(super) fn check_trajectory_spacing(
    trajectory: &[(Duration, Value)],
    interval: Duration,
) -> Result<(), String> {
    for pair in trajectory.windows(2) {
        let (first, second) = (pair[0].0, pair[1].0);
        if second.saturating_sub(first) < interval {
            return Err(format!(
                "trajectory points at {} ms and {} ms are closer than the task interval {} ms",
                first.as_millis(),
                second.as_millis(),
                interval.as_millis()
            ));
        }
    }
    Ok(())
}

pub(super) fn snapshot_value<'a>(
    snapshot: &'a IoSnapshot,
    address: &IoAddress,
) -> Option<&'a Value> {
    snapshot
        .inputs
        .iter()
        .chain(&snapshot.outputs)
        .chain(&snapshot.memory)
        .find(|entry| &entry.address == address)
        .and_then(|entry| match &entry.value {
            IoSnapshotValue::Value(value) => Some(value),
            IoSnapshotValue::Error(_) | IoSnapshotValue::Unresolved => None,
        })
}

pub(super) fn snapshot_type(snapshot: &IoSnapshot, address: &IoAddress) -> Option<TypeId> {
    snapshot
        .inputs
        .iter()
        .chain(&snapshot.outputs)
        .chain(&snapshot.memory)
        .find(|entry| &entry.address == address)
        .and_then(|entry| entry.value_type)
}

pub(super) fn experiment_value(
    value: &JsonValue,
    value_type: Option<&str>,
) -> Result<Value, String> {
    if let Some(value_type) = value_type {
        return typed_numeric_value(value, value_type);
    }
    match value {
        JsonValue::Number(number) if number.is_i64() => {
            Ok(Value::LInt(number.as_i64().expect("checked i64")))
        }
        JsonValue::Number(number) => number
            .as_f64()
            .filter(|value| value.is_finite())
            .map(Value::LReal)
            .ok_or_else(|| "experiment value must be finite".to_string()),
        JsonValue::Bool(value) => Ok(Value::Bool(*value)),
        JsonValue::String(value) => {
            parse_value(value).map_err(|error_value| error_value.to_string())
        }
        _ => Err("experiment value must be numeric or boolean".to_string()),
    }
}

pub(super) fn typed_numeric_value(value: &JsonValue, value_type: &str) -> Result<Value, String> {
    let invalid = || {
        format!(
            "experiment value is not a valid {}",
            value_type.to_ascii_uppercase()
        )
    };
    let signed = || value.as_i64().ok_or_else(invalid);
    let unsigned = || value.as_u64().ok_or_else(invalid);
    let real = || {
        value
            .as_f64()
            .filter(|number| number.is_finite())
            .ok_or_else(invalid)
    };
    match value_type.trim().to_ascii_uppercase().as_str() {
        "BOOL" => value.as_bool().map(Value::Bool).ok_or_else(invalid),
        "SINT" => i8::try_from(signed()?)
            .map(Value::SInt)
            .map_err(|_| invalid()),
        "INT" => i16::try_from(signed()?)
            .map(Value::Int)
            .map_err(|_| invalid()),
        "DINT" => i32::try_from(signed()?)
            .map(Value::DInt)
            .map_err(|_| invalid()),
        "LINT" => signed().map(Value::LInt),
        "USINT" => u8::try_from(unsigned()?)
            .map(Value::USInt)
            .map_err(|_| invalid()),
        "UINT" => u16::try_from(unsigned()?)
            .map(Value::UInt)
            .map_err(|_| invalid()),
        "UDINT" => u32::try_from(unsigned()?)
            .map(Value::UDInt)
            .map_err(|_| invalid()),
        "ULINT" => unsigned().map(Value::ULInt),
        "REAL" => {
            let number = real()?;
            let narrowed = number as f32;
            narrowed
                .is_finite()
                .then_some(Value::Real(narrowed))
                .ok_or_else(invalid)
        }
        "LREAL" => real().map(Value::LReal),
        _ => Err(format!("unsupported experiment value type '{value_type}'")),
    }
}

pub(super) fn experiment_type_id(value_type: &str) -> Result<TypeId, String> {
    match value_type.trim().to_ascii_uppercase().as_str() {
        "BOOL" => Ok(TypeId::BOOL),
        "SINT" => Ok(TypeId::SINT),
        "INT" => Ok(TypeId::INT),
        "DINT" => Ok(TypeId::DINT),
        "LINT" => Ok(TypeId::LINT),
        "USINT" => Ok(TypeId::USINT),
        "UINT" => Ok(TypeId::UINT),
        "UDINT" => Ok(TypeId::UDINT),
        "ULINT" => Ok(TypeId::ULINT),
        "REAL" => Ok(TypeId::REAL),
        "LREAL" => Ok(TypeId::LREAL),
        _ => Err(format!("unsupported experiment value type '{value_type}'")),
    }
}

pub(super) fn numeric_value(value: &Value) -> Option<f64> {
    match value {
        Value::Bool(value) => Some(if *value { 1.0 } else { 0.0 }),
        Value::SInt(value) => Some(f64::from(*value)),
        Value::Int(value) => Some(f64::from(*value)),
        Value::DInt(value) => Some(f64::from(*value)),
        Value::LInt(value) => Some(*value as f64),
        Value::USInt(value) => Some(f64::from(*value)),
        Value::UInt(value) => Some(f64::from(*value)),
        Value::UDInt(value) => Some(f64::from(*value)),
        Value::ULInt(value) => Some(*value as f64),
        Value::Real(value) => Some(f64::from(*value)),
        Value::LReal(value) => Some(*value),
        _ => None,
    }
}

pub(super) fn decode_params<T: for<'de> Deserialize<'de>>(
    params: Option<JsonValue>,
) -> Result<T, String> {
    serde_json::from_value(params.ok_or_else(|| "missing params".to_string())?)
        .map_err(|error_value| format!("invalid params: {error_value}"))
}
