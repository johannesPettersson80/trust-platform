//! A JSON value admitted as a value of its destination's declared type, the value of a service's
//! `variables.write` (specification 11 §6.8.1, "Writing variables"): the JSON form in which
//! `variables.read` gives a value of that type, taken by the same rules as an IEC literal of
//! [`super::typed`]. A Boolean is `true` or `false`; an integer, a bit string and a subrange a JSON
//! integer; a `REAL` or an `LREAL` a JSON number, or `NaN`, `Infinity` or `-Infinity`; a `TIME` or an
//! `LTIME` its nanoseconds; a `DATE` or a `DT` its milliseconds since 1970-01-01 and a `TOD` its
//! milliseconds since midnight, an `LDATE`, an `LDT` or an `LTOD` the same in nanoseconds; a
//! string or a character a JSON string; an enumeration's value its enumerator's name. A value of
//! another form, outside the type's range, longer than the type holds or a date that is no whole
//! day is refused with a plain reason, and nothing is cut to fit.

use alloc::format;
use alloc::string::{String, ToString};

use serde_json::Value as Json;
use trust_runtime_core::datetime::NANOS_PER_DAY;
use trust_runtime_core::native::image::{type_kind as kind, Image};
use trust_runtime_core::native::memory::TypeShape;
use trust_runtime_core::value::Value;

use super::literal::Literal;
use super::typed::Target;

/// The milliseconds of a day.
const MILLIS_PER_DAY: i64 = 86_400_000;
/// The nanoseconds of a millisecond.
const NANOS_PER_MILLI: i64 = 1_000_000;

/// `json` as a value of the variable of type `type_index` of `image` (specification 11 §6.8.1,
/// "Writing variables").
///
/// # Errors
///
/// A plain reason for a value the type does not take: of another form, outside the type's range,
/// a text longer than the type holds, a name that is no value of the enumeration, a date that is
/// no whole day, and for a type that holds no single value.
pub fn json_value(image: &Image<'_>, type_index: u32, json: &Json) -> Result<Value, String> {
    let text = json.to_string();
    let target = Target::of(image, type_index, &text)?;
    let literal = match &target.shape {
        TypeShape::Elementary(kind) | TypeShape::Subrange(kind) => {
            if let Some(value) = non_finite(*kind, json) {
                return Ok(value);
            }
            elementary(*kind, json, &text).ok_or_else(|| target.mismatch())??
        }
        TypeShape::Text { wide, .. } => match json {
            Json::String(text) => Literal::Text {
                wide: *wide,
                text: text.clone(),
            },
            _ => return Err(target.mismatch()),
        },
        TypeShape::Enumeration { .. } => match json {
            Json::String(name) => Literal::Name(name.clone()),
            _ => return Err(target.mismatch()),
        },
        TypeShape::Array { .. } | TypeShape::Other => return Err(target.no_single_value()),
    };
    let value = target.admit(&literal)?;
    target.bytes(&value)?;
    Ok(value)
}

/// The value of a `REAL` or an `LREAL` that no JSON number holds, `NaN`, `Infinity` or
/// `-Infinity`, as `variables.read` writes it; `None` for every other value.
fn non_finite(kind: u32, json: &Json) -> Option<Value> {
    let number = match json.as_str()? {
        "NaN" => f64::NAN,
        "Infinity" => f64::INFINITY,
        "-Infinity" => f64::NEG_INFINITY,
        _ => return None,
    };
    match kind {
        // A NaN or an infinity converts to the same in single precision.
        #[allow(clippy::cast_possible_truncation)]
        kind::REAL => Some(Value::Real(number as f32)),
        kind::LREAL => Some(Value::LReal(number)),
        _ => None,
    }
}

/// The literal of the elementary type `kind` that `json` spells; `None` when the JSON form is not
/// one `kind` takes, and a reason when it is that form but no value of it, a date that is no whole
/// day.
fn elementary(kind: u32, json: &Json, text: &str) -> Option<Result<Literal, String>> {
    let integer = || {
        json.as_i64()
            .map(i128::from)
            .or_else(|| json.as_u64().map(i128::from))
    };
    let whole_day = |count: i64, per_day: i64| {
        if count.rem_euclid(per_day) == 0 {
            Ok(count.div_euclid(per_day))
        } else {
            Err(format!("'{text}' is no whole day"))
        }
    };
    Some(Ok(match kind {
        kind::BOOL => Literal::Bool(json.as_bool()?),
        kind::REAL | kind::LREAL => match integer() {
            Some(value) => Literal::Integer {
                value,
                based: false,
            },
            None if json.is_number() => Literal::Real(text.to_string()),
            None => return None,
        },
        kind::TIME | kind::LTIME => Literal::Duration {
            long: kind == kind::LTIME,
            nanos: json.as_i64()?,
        },
        kind::DATE => match whole_day(json.as_i64()?, MILLIS_PER_DAY) {
            Ok(days) => Literal::Date { long: false, days },
            Err(reason) => return Some(Err(reason)),
        },
        kind::LDATE => match whole_day(json.as_i64()?, NANOS_PER_DAY) {
            Ok(days) => Literal::Date { long: true, days },
            Err(reason) => return Some(Err(reason)),
        },
        kind::TOD => Literal::TimeOfDay {
            long: false,
            nanos: json.as_i64()?.checked_mul(NANOS_PER_MILLI)?,
        },
        kind::LTOD => Literal::TimeOfDay {
            long: true,
            nanos: json.as_i64()?,
        },
        kind::DT => {
            let millis = json.as_i64()?;
            Literal::DateAndTime {
                long: false,
                days: millis.div_euclid(MILLIS_PER_DAY),
                nanos: millis.rem_euclid(MILLIS_PER_DAY) * NANOS_PER_MILLI,
            }
        }
        kind::LDT => {
            let nanos = json.as_i64()?;
            Literal::DateAndTime {
                long: true,
                days: nanos.div_euclid(NANOS_PER_DAY),
                nanos: nanos.rem_euclid(NANOS_PER_DAY),
            }
        }
        kind::CHAR | kind::WCHAR => Literal::Text {
            wide: kind == kind::WCHAR,
            text: json.as_str()?.to_string(),
        },
        _ => Literal::Integer {
            value: integer()?,
            based: false,
        },
    }))
}

#[cfg(test)]
#[path = "json_tests.rs"]
mod tests;
