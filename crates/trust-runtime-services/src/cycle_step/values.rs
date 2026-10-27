//! The value texts a step writes and the encoding of the values it reads: the conformance suite's
//! (specification 11 §6.8.1, "Runtime conformance case runner" and "Stepped scans"), so that a
//! device's trace compares byte for byte with the suite's expected artifacts.

use alloc::collections::BTreeMap;
use alloc::format;
use alloc::string::{String, ToString};
use alloc::vec::Vec;

use serde_json::json;
use trust_runtime_core::value::{Duration, Value};

/// The value of a typed value text `KIND:VALUE`: `BOOL`, the signed and unsigned integers, the bit
/// strings, `REAL` and `LREAL`, `TIME` and `LTIME`, and `STRING`, each within its type's range. A
/// number may hold `_` between its digits. A duration is a whole number of `ms`, `ns` or `s`, or of
/// milliseconds without a unit, within the signed nanosecond range.
///
/// # Errors
///
/// Why the text is no value of its kind, or names no kind this reads.
pub fn parse_typed_value(raw: &str) -> Result<Value, String> {
    let (kind, payload) = raw
        .split_once(':')
        .ok_or_else(|| format!("typed value must be KIND:VALUE, got '{raw}'"))?;
    let payload = payload.trim();
    let normalized = kind.trim().to_ascii_uppercase();
    let number = payload.replace('_', "");
    let refused = |kind: &str| format!("parse {kind}: '{payload}'");
    Ok(match normalized.as_str() {
        "BOOL" => Value::Bool(parse_bool(payload)?),
        "SINT" => Value::SInt(number.parse().map_err(|_| refused("SINT"))?),
        "INT" => Value::Int(number.parse().map_err(|_| refused("INT"))?),
        "DINT" => Value::DInt(number.parse().map_err(|_| refused("DINT"))?),
        "LINT" => Value::LInt(number.parse().map_err(|_| refused("LINT"))?),
        "USINT" => Value::USInt(number.parse().map_err(|_| refused("USINT"))?),
        "UINT" => Value::UInt(number.parse().map_err(|_| refused("UINT"))?),
        "UDINT" => Value::UDInt(number.parse().map_err(|_| refused("UDINT"))?),
        "ULINT" => Value::ULInt(number.parse().map_err(|_| refused("ULINT"))?),
        "BYTE" => Value::Byte(number.parse().map_err(|_| refused("BYTE"))?),
        "WORD" => Value::Word(number.parse().map_err(|_| refused("WORD"))?),
        "DWORD" => Value::DWord(number.parse().map_err(|_| refused("DWORD"))?),
        "LWORD" => Value::LWord(number.parse().map_err(|_| refused("LWORD"))?),
        "REAL" => Value::Real(number.parse().map_err(|_| refused("REAL"))?),
        "LREAL" => Value::LReal(number.parse().map_err(|_| refused("LREAL"))?),
        "TIME" => Value::Time(parse_duration(payload)?),
        "LTIME" => Value::LTime(parse_duration(payload)?),
        "STRING" => Value::String(payload.into()),
        _ => return Err(format!("unsupported typed value kind '{normalized}'")),
    })
}

fn parse_bool(raw: &str) -> Result<bool, String> {
    match raw.trim().to_ascii_lowercase().as_str() {
        "true" => Ok(true),
        "false" => Ok(false),
        _ => Err(format!("invalid BOOL literal '{raw}'")),
    }
}

fn parse_duration(raw: &str) -> Result<Duration, String> {
    let text = raw.trim().to_ascii_lowercase().replace('_', "");
    let out_of_range = || format!("TIME value '{raw}' is outside the signed nanosecond range");
    let whole = |digits: &str, unit: &str| {
        digits
            .parse::<i64>()
            .map_err(|_| format!("parse TIME {unit}: '{raw}'"))
    };
    if let Some(millis) = text.strip_suffix("ms") {
        return Duration::checked_from_millis(whole(millis, "milliseconds")?)
            .ok_or_else(out_of_range);
    }
    if let Some(nanos) = text.strip_suffix("ns") {
        return Ok(Duration::from_nanos(whole(nanos, "nanoseconds")?));
    }
    if let Some(seconds) = text.strip_suffix('s') {
        return Duration::checked_from_secs(whole(seconds, "seconds")?).ok_or_else(out_of_range);
    }
    Duration::checked_from_millis(whole(&text, "as milliseconds")?).ok_or_else(out_of_range)
}

/// `value` as a conformance artifact writes it: `{"type": "INT", "value": 7}`, a duration in
/// `nanos`, a date, a time of day and a date and time in `ticks` (the long ones in `nanos`), an
/// enumeration by its type, value name and number, a structure by its type and fields, and an array
/// by its dimensions and elements.
#[must_use]
pub fn encode_value(value: &Value) -> serde_json::Value {
    match value {
        Value::Bool(v) => json!({"type": "BOOL", "value": v}),
        Value::SInt(v) => json!({"type": "SINT", "value": v}),
        Value::Int(v) => json!({"type": "INT", "value": v}),
        Value::DInt(v) => json!({"type": "DINT", "value": v}),
        Value::LInt(v) => json!({"type": "LINT", "value": v}),
        Value::USInt(v) => json!({"type": "USINT", "value": v}),
        Value::UInt(v) => json!({"type": "UINT", "value": v}),
        Value::UDInt(v) => json!({"type": "UDINT", "value": v}),
        Value::ULInt(v) => json!({"type": "ULINT", "value": v}),
        Value::Real(v) => json!({"type": "REAL", "value": v}),
        Value::LReal(v) => json!({"type": "LREAL", "value": v}),
        Value::Byte(v) => json!({"type": "BYTE", "value": v}),
        Value::Word(v) => json!({"type": "WORD", "value": v}),
        Value::DWord(v) => json!({"type": "DWORD", "value": v}),
        Value::LWord(v) => json!({"type": "LWORD", "value": v}),
        Value::Time(v) => json!({"type": "TIME", "nanos": v.as_nanos()}),
        Value::LTime(v) => json!({"type": "LTIME", "nanos": v.as_nanos()}),
        Value::Date(v) => json!({"type": "DATE", "ticks": v.ticks()}),
        Value::LDate(v) => json!({"type": "LDATE", "nanos": v.nanos()}),
        Value::Tod(v) => json!({"type": "TOD", "ticks": v.ticks()}),
        Value::LTod(v) => json!({"type": "LTOD", "nanos": v.nanos()}),
        Value::Dt(v) => json!({"type": "DT", "ticks": v.ticks()}),
        Value::Ldt(v) => json!({"type": "LDT", "nanos": v.nanos()}),
        Value::String(v) => json!({"type": "STRING", "value": v.to_string()}),
        Value::WString(v) => json!({"type": "WSTRING", "value": v}),
        Value::Char(v) => json!({"type": "CHAR", "value": v}),
        Value::WChar(v) => json!({"type": "WCHAR", "value": v}),
        Value::Array(array) => json!({
            "type": "ARRAY",
            "dimensions": array.dimensions(),
            "elements": array.elements().iter().map(encode_value).collect::<Vec<_>>()
        }),
        Value::Struct(value) => {
            let mut fields = BTreeMap::new();
            for (name, field_value) in value.fields() {
                fields.insert(name.to_string(), encode_value(field_value));
            }
            json!({
                "type": "STRUCT",
                "type_name": value.type_name().to_string(),
                "fields": fields
            })
        }
        Value::Enum(value) => json!({
            "type": "ENUM",
            "type_name": value.type_name().to_string(),
            "variant": value.variant_name().to_string(),
            "numeric": value.numeric_value()
        }),
        Value::Reference(reference) => json!({
            "type": "REFERENCE",
            "value": reference.as_ref().map(|entry| format!("{entry:?}"))
        }),
        Value::Instance(id) => json!({"type": "INSTANCE", "value": id.0}),
        Value::Null => json!({"type": "NULL"}),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// The limits of the duration texts: the largest whole number of milliseconds and of nanoseconds
    /// in the signed nanosecond range are taken, and one more is refused. A parser that multiplied
    /// without a check would wrap the refused texts into negative durations.
    #[test]
    fn durations_end_at_the_signed_nanosecond_range() {
        assert_eq!(
            parse_duration("9223372036854ms"),
            Ok(Duration::from_nanos(9_223_372_036_854_000_000))
        );
        assert_eq!(
            parse_duration("9223372036854775807ns"),
            Ok(Duration::from_nanos(i64::MAX))
        );
        for raw in [
            "9223372036855ms",
            "9223372036854775807s",
            "18446744073709551615ms",
        ] {
            assert!(parse_duration(raw).is_err(), "{raw} must be refused");
        }
    }

    /// Every kind the manifests write reads as its value and encodes under its own type name, and a
    /// text outside its kind's range or without a kind is refused. A parser that read `SINT:128` as
    /// -128 or `UINT:-1` as 65535 would write a value the manifest does not name.
    #[test]
    fn every_kind_reads_within_its_range() {
        let cases = [
            ("BOOL:true", Value::Bool(true), "BOOL"),
            ("SINT:-8", Value::SInt(-8), "SINT"),
            ("INT:-4", Value::Int(-4), "INT"),
            ("DINT:1_000", Value::DInt(1_000), "DINT"),
            ("LINT:64", Value::LInt(64), "LINT"),
            ("USINT:8", Value::USInt(8), "USINT"),
            ("UINT:16", Value::UInt(16), "UINT"),
            ("UDINT:32", Value::UDInt(32), "UDINT"),
            ("ULINT:64", Value::ULInt(64), "ULINT"),
            ("BYTE:8", Value::Byte(8), "BYTE"),
            ("WORD:41", Value::Word(41), "WORD"),
            ("DWORD:32", Value::DWord(32), "DWORD"),
            ("LWORD:64", Value::LWord(64), "LWORD"),
            ("REAL:1.5", Value::Real(1.5), "REAL"),
            ("LREAL:2.5", Value::LReal(2.5), "LREAL"),
            ("TIME:10ms", Value::Time(Duration::from_millis(10)), "TIME"),
            ("TIME:3s", Value::Time(Duration::from_secs(3)), "TIME"),
            ("TIME:250", Value::Time(Duration::from_millis(250)), "TIME"),
            ("LTIME:4ns", Value::LTime(Duration::from_nanos(4)), "LTIME"),
            ("STRING:hello", Value::String("hello".into()), "STRING"),
        ];
        for (raw, expected, encoded_type) in cases {
            let parsed = parse_typed_value(raw).unwrap_or_else(|error| panic!("{raw}: {error}"));
            assert_eq!(parsed, expected, "{raw}");
            assert_eq!(encode_value(&parsed)["type"], encoded_type, "{raw}");
        }
        for raw in [
            "missing-kind-separator",
            "BOOL:maybe",
            "SINT:128",
            "UINT:-1",
            "TIME:1minute",
            "UNKNOWN:1",
        ] {
            assert!(parse_typed_value(raw).is_err(), "{raw} must be refused");
        }
    }

    /// The encoding of the expected artifacts: a duration in nanoseconds and a number as itself. An
    /// encoder that wrote a duration in milliseconds would make every timer case differ.
    #[test]
    fn values_encode_as_the_expected_artifacts_write_them() {
        assert_eq!(
            encode_value(&Value::Time(Duration::from_millis(10))),
            json!({"type": "TIME", "nanos": 10_000_000})
        );
        assert_eq!(
            encode_value(&Value::Int(32767)),
            json!({"type": "INT", "value": 32767})
        );
        assert_eq!(
            encode_value(&Value::Bool(false)),
            json!({"type": "BOOL", "value": false})
        );
    }
}
