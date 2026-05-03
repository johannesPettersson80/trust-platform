//! Stable automation helpers for harness-driven external protocols.

#![allow(missing_docs)]

use std::collections::BTreeMap;
use std::fmt;

use serde_json::{json, Value as JsonValue};

use crate::boundary::{BoundaryEntry, BoundaryError};
use crate::error::RuntimeError;
use crate::value::{
    ArrayValue, DateTimeValue, DateValue, Duration, EnumValue, LDateTimeValue, LDateValue,
    LTimeOfDayValue, StructValue, TimeOfDayValue, Value,
};
use crate::RestartMode;

use super::TestHarness;

/// Business-logic surface for deterministic harness automation.
#[derive(Default)]
pub struct HarnessAutomation {
    harness: Option<TestHarness>,
}

/// Basic harness state summary.
#[derive(Debug, Clone, PartialEq)]
pub struct HarnessStateSummary {
    pub cycle_count: u64,
    pub elapsed_ms: i64,
}

/// Result of loading or reloading source text into the harness.
#[derive(Debug, Clone, PartialEq)]
pub struct HarnessLoadSummary {
    pub source_count: usize,
    pub cycle_count: u64,
    pub elapsed_ms: i64,
}

/// Snapshot of watched values after a harness operation.
#[derive(Debug, Clone, PartialEq)]
pub struct HarnessWatchSnapshot {
    pub cycle_count: u64,
    pub elapsed_ms: i64,
    pub values: BTreeMap<String, BoundaryEntry>,
}

/// Output lookup result for a named variable.
#[derive(Debug, Clone, PartialEq)]
pub struct HarnessValueSnapshot {
    pub name: String,
    pub value: Value,
}

/// Result of a bounded `run_until` loop.
#[derive(Debug, Clone, PartialEq)]
pub struct HarnessRunUntilSummary {
    pub name: String,
    pub cycles_ran: u64,
    pub cycle_count: u64,
    pub elapsed_ms: i64,
    pub matched_value: Value,
    pub values: BTreeMap<String, BoundaryEntry>,
}

/// Stable error model for harness automation surfaces.
#[derive(Debug, Clone, PartialEq)]
pub enum HarnessAutomationError {
    NotLoaded,
    InvalidArgument(String),
    Compile(String),
    Runtime(String),
    RuntimeCycle {
        message: String,
        errors: Vec<String>,
    },
    Boundary(BoundaryError),
    RunUntilTimeout {
        name: String,
        max_cycles: u64,
        expected: Value,
    },
}

impl fmt::Display for HarnessAutomationError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::NotLoaded => write!(f, "Harness is not loaded. Call load first."),
            Self::InvalidArgument(message) => write!(f, "{message}"),
            Self::Compile(message) => write!(f, "{message}"),
            Self::Runtime(message) => write!(f, "{message}"),
            Self::RuntimeCycle { message, errors } => {
                if errors.is_empty() {
                    write!(f, "{message}")
                } else {
                    write!(f, "{message}: {}", errors.join("; "))
                }
            }
            Self::Boundary(error) => write!(f, "{error}"),
            Self::RunUntilTimeout {
                name, max_cycles, ..
            } => write!(
                f,
                "run_until exceeded {max_cycles} cycles before '{name}' matched the expected value"
            ),
        }
    }
}

impl std::error::Error for HarnessAutomationError {}

impl HarnessAutomation {
    /// Create an empty automation session.
    #[must_use]
    pub fn new() -> Self {
        Self::default()
    }

    /// Return whether a harness is currently loaded.
    #[must_use]
    pub fn is_loaded(&self) -> bool {
        self.harness.is_some()
    }

    /// Load source texts into a fresh harness instance.
    pub fn load_sources(
        &mut self,
        source_texts: &[String],
    ) -> Result<HarnessLoadSummary, HarnessAutomationError> {
        let source_refs = validate_sources(source_texts)?;
        let mut harness = TestHarness::from_sources(&source_refs)
            .map_err(|error| HarnessAutomationError::Compile(error.to_string()))?;
        let cycle = harness.cycle();
        if !cycle.errors.is_empty() {
            return Err(HarnessAutomationError::RuntimeCycle {
                message: "initial cycle failed".to_string(),
                errors: render_runtime_errors(&cycle.errors),
            });
        }
        let summary = HarnessLoadSummary {
            source_count: source_texts.len(),
            cycle_count: harness.cycle_count(),
            elapsed_ms: harness.current_time().as_millis(),
        };
        self.harness = Some(harness);
        Ok(summary)
    }

    /// Reload source texts while preserving the harness state semantics already supported by `TestHarness`.
    pub fn reload_sources(
        &mut self,
        source_texts: &[String],
    ) -> Result<HarnessLoadSummary, HarnessAutomationError> {
        let source_refs = validate_sources(source_texts)?;
        let harness = self.harness_mut()?;
        harness
            .reload_sources(&source_refs)
            .map_err(|error| HarnessAutomationError::Compile(error.to_string()))?;
        Ok(HarnessLoadSummary {
            source_count: source_texts.len(),
            cycle_count: harness.cycle_count(),
            elapsed_ms: harness.current_time().as_millis(),
        })
    }

    /// Advance execution for one or more cycles, optionally advancing virtual time before each cycle.
    pub fn cycle(
        &mut self,
        count: u32,
        dt_ms: i64,
        watch: &[String],
    ) -> Result<HarnessWatchSnapshot, HarnessAutomationError> {
        let dt_ms = validate_non_negative(dt_ms, "dt_ms")?;
        let harness = self.harness_mut()?;
        for _ in 0..count {
            if dt_ms > 0 {
                harness.advance_time(Duration::from_millis(dt_ms));
            }
            let cycle = harness.cycle();
            if !cycle.errors.is_empty() {
                return Err(HarnessAutomationError::RuntimeCycle {
                    message: "cycle failed".to_string(),
                    errors: render_runtime_errors(&cycle.errors),
                });
            }
        }
        Ok(snapshot_for_watch(harness, watch))
    }

    /// Set an input variable.
    pub fn set_input(&mut self, name: &str, value: Value) -> Result<(), HarnessAutomationError> {
        let harness = self.harness_mut()?;
        harness
            .try_set_input(name, value)
            .map_err(HarnessAutomationError::Boundary)?;
        Ok(())
    }

    /// Read an output or global variable.
    pub fn get_output(
        &mut self,
        name: &str,
    ) -> Result<HarnessValueSnapshot, HarnessAutomationError> {
        let harness = self.harness_mut()?;
        let value = harness
            .try_get_output(name)
            .map_err(HarnessAutomationError::Boundary)?;
        Ok(HarnessValueSnapshot {
            name: name.to_string(),
            value,
        })
    }

    /// Set a `VAR_ACCESS` value.
    pub fn set_access(&mut self, name: &str, value: Value) -> Result<(), HarnessAutomationError> {
        let harness = self.harness_mut()?;
        harness.set_access(name, value).map_err(runtime_to_error)?;
        Ok(())
    }

    /// Read a `VAR_ACCESS` value.
    pub fn get_access(
        &mut self,
        name: &str,
    ) -> Result<HarnessValueSnapshot, HarnessAutomationError> {
        let harness = self.harness_mut()?;
        let value = harness
            .get_access(name)
            .ok_or_else(|| BoundaryError::UnresolvedName { path: name.into() })
            .map_err(HarnessAutomationError::Boundary)?;
        Ok(HarnessValueSnapshot {
            name: name.to_string(),
            value,
        })
    }

    /// Bind a variable to a direct I/O address.
    pub fn bind_direct(&mut self, name: &str, address: &str) -> Result<(), HarnessAutomationError> {
        let harness = self.harness_mut()?;
        harness
            .bind_direct(name, address)
            .map_err(HarnessAutomationError::Boundary)?;
        Ok(())
    }

    /// Write a value to a direct input address.
    pub fn set_direct_input(
        &mut self,
        address: &str,
        value: Value,
    ) -> Result<(), HarnessAutomationError> {
        let harness = self.harness_mut()?;
        harness
            .set_direct_input(address, value)
            .map_err(runtime_to_error)?;
        Ok(())
    }

    /// Read a value from a direct output address.
    pub fn get_direct_output(
        &mut self,
        address: &str,
    ) -> Result<HarnessValueSnapshot, HarnessAutomationError> {
        let harness = self.harness_mut()?;
        let value = harness
            .get_direct_output(address)
            .map_err(runtime_to_error)?;
        Ok(HarnessValueSnapshot {
            name: address.to_string(),
            value,
        })
    }

    /// Advance virtual time without executing a cycle.
    pub fn advance_time(
        &mut self,
        duration_ms: i64,
    ) -> Result<HarnessStateSummary, HarnessAutomationError> {
        let duration_ms = validate_non_negative(duration_ms, "duration_ms")?;
        let harness = self.harness_mut()?;
        harness.advance_time(Duration::from_millis(duration_ms));
        Ok(HarnessStateSummary {
            cycle_count: harness.cycle_count(),
            elapsed_ms: harness.current_time().as_millis(),
        })
    }

    /// Restart the harness runtime.
    pub fn restart(
        &mut self,
        mode: RestartMode,
    ) -> Result<HarnessStateSummary, HarnessAutomationError> {
        let harness = self.harness_mut()?;
        harness.restart(mode).map_err(runtime_to_error)?;
        Ok(HarnessStateSummary {
            cycle_count: harness.cycle_count(),
            elapsed_ms: harness.current_time().as_millis(),
        })
    }

    /// Return a snapshot of watched values without executing additional work.
    pub fn snapshot(
        &mut self,
        watch: &[String],
    ) -> Result<HarnessWatchSnapshot, HarnessAutomationError> {
        let harness = self.harness_mut()?;
        Ok(snapshot_for_watch(harness, watch))
    }

    /// Run until a named output matches the expected value or the cycle budget is exhausted.
    pub fn run_until(
        &mut self,
        name: &str,
        expected: Value,
        dt_ms: i64,
        max_cycles: u64,
        watch: &[String],
    ) -> Result<HarnessRunUntilSummary, HarnessAutomationError> {
        let dt_ms = validate_non_negative(dt_ms, "dt_ms")?;
        let harness = self.harness_mut()?;

        let mut cycles_ran = 0_u64;
        loop {
            if harness
                .try_get_output(name)
                .map_err(HarnessAutomationError::Boundary)?
                == expected
            {
                break;
            }
            if cycles_ran >= max_cycles {
                return Err(HarnessAutomationError::RunUntilTimeout {
                    name: name.to_string(),
                    max_cycles,
                    expected,
                });
            }
            if dt_ms > 0 {
                harness.advance_time(Duration::from_millis(dt_ms));
            }
            let cycle = harness.cycle();
            if !cycle.errors.is_empty() {
                return Err(HarnessAutomationError::RuntimeCycle {
                    message: "run_until cycle failed".to_string(),
                    errors: render_runtime_errors(&cycle.errors),
                });
            }
            cycles_ran += 1;
        }

        let snapshot = snapshot_for_watch(harness, watch);
        let matched_value = harness
            .try_get_output(name)
            .map_err(HarnessAutomationError::Boundary)?;
        Ok(HarnessRunUntilSummary {
            name: name.to_string(),
            cycles_ran,
            cycle_count: snapshot.cycle_count,
            elapsed_ms: snapshot.elapsed_ms,
            matched_value,
            values: snapshot.values,
        })
    }

    fn harness_mut(&mut self) -> Result<&mut TestHarness, HarnessAutomationError> {
        self.harness
            .as_mut()
            .ok_or(HarnessAutomationError::NotLoaded)
    }
}

/// Encode a runtime value into a stable JSON shape suitable for external protocols.
#[must_use]
pub fn encode_json_value(value: &Value) -> JsonValue {
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
        Value::Char(v) => json!({"type": "CHAR", "value": char::from(*v).to_string()}),
        Value::WChar(v) => json!({
            "type": "WCHAR",
            "value": char::from_u32(u32::from(*v)).unwrap_or('\u{FFFD}').to_string()
        }),
        Value::Array(array) => json!({
            "type": "ARRAY",
            "dimensions": array.dimensions(),
            "elements": array.elements().iter().map(encode_json_value).collect::<Vec<_>>(),
        }),
        Value::Struct(value) => {
            let fields = value
                .fields()
                .iter()
                .map(|(name, field)| (name.to_string(), encode_json_value(field)))
                .collect::<serde_json::Map<String, JsonValue>>();
            json!({
                "type": "STRUCT",
                "type_name": value.type_name().to_string(),
                "fields": fields,
            })
        }
        Value::Enum(value) => json!({
            "type": "ENUM",
            "type_name": value.type_name().to_string(),
            "variant": value.variant_name().to_string(),
            "numeric": value.numeric_value(),
        }),
        Value::Reference(reference) => json!({
            "type": "REFERENCE",
            "value": reference.as_ref().map(|entry| format!("{entry:?}")),
        }),
        Value::Instance(id) => json!({"type": "INSTANCE", "value": id.0}),
        Value::Null => json!({"type": "NULL"}),
    }
}

/// Decode a JSON value from an external harness protocol into a runtime value.
pub fn decode_json_value(value: &JsonValue) -> Result<Value, HarnessAutomationError> {
    match value {
        JsonValue::Bool(flag) => Ok(Value::Bool(*flag)),
        JsonValue::Number(number) => decode_untyped_number(number),
        JsonValue::String(text) => Ok(Value::String(text.clone().into())),
        JsonValue::Object(object) => decode_typed_object(object),
        JsonValue::Null => Ok(Value::Null),
        JsonValue::Array(values) => {
            let elements = values
                .iter()
                .map(decode_json_value)
                .collect::<Result<Vec<_>, _>>()?;
            Ok(Value::Array(Box::new(
                ArrayValue::from_untyped_parts(
                    elements,
                    vec![(0, values.len().saturating_sub(1) as i64)],
                )
                .map_err(|error| {
                    HarnessAutomationError::InvalidArgument(format!(
                        "Invalid JSON array value: {error}"
                    ))
                })?,
            )))
        }
    }
}

fn decode_untyped_number(number: &serde_json::Number) -> Result<Value, HarnessAutomationError> {
    if let Some(integer) = number.as_i64() {
        if let Ok(value) = i8::try_from(integer) {
            Ok(Value::SInt(value))
        } else if let Ok(value) = i16::try_from(integer) {
            Ok(Value::Int(value))
        } else if let Ok(value) = i32::try_from(integer) {
            Ok(Value::DInt(value))
        } else {
            Ok(Value::LInt(integer))
        }
    } else if let Some(integer) = number.as_u64() {
        if let Ok(value) = u8::try_from(integer) {
            Ok(Value::USInt(value))
        } else if let Ok(value) = u16::try_from(integer) {
            Ok(Value::UInt(value))
        } else if let Ok(value) = u32::try_from(integer) {
            Ok(Value::UDInt(value))
        } else {
            Ok(Value::ULInt(integer))
        }
    } else if let Some(float) = number.as_f64() {
        Ok(Value::LReal(float))
    } else {
        Err(HarnessAutomationError::InvalidArgument(
            "Unsupported numeric value in JSON payload.".to_string(),
        ))
    }
}

fn decode_typed_object(
    object: &serde_json::Map<String, JsonValue>,
) -> Result<Value, HarnessAutomationError> {
    let kind = object
        .get("type")
        .and_then(JsonValue::as_str)
        .ok_or_else(|| {
            HarnessAutomationError::InvalidArgument(
                "Typed value objects must include a string 'type' field.".to_string(),
            )
        })?;

    match kind.to_ascii_uppercase().as_str() {
        "BOOL" => Ok(Value::Bool(parse_bool(object, "value", "BOOL")?)),
        "SINT" => Ok(Value::SInt(parse_signed(object, "value", "SINT")?)),
        "INT" => Ok(Value::Int(parse_signed(object, "value", "INT")?)),
        "DINT" => Ok(Value::DInt(parse_signed(object, "value", "DINT")?)),
        "LINT" => Ok(Value::LInt(parse_signed(object, "value", "LINT")?)),
        "USINT" => Ok(Value::USInt(parse_unsigned(object, "value", "USINT")?)),
        "UINT" => Ok(Value::UInt(parse_unsigned(object, "value", "UINT")?)),
        "UDINT" => Ok(Value::UDInt(parse_unsigned(object, "value", "UDINT")?)),
        "ULINT" => Ok(Value::ULInt(parse_unsigned(object, "value", "ULINT")?)),
        "REAL" => Ok(Value::Real(parse_f32(object, "value", "REAL")?)),
        "LREAL" => Ok(Value::LReal(parse_f64(object, "value", "LREAL")?)),
        "BYTE" => Ok(Value::Byte(parse_unsigned(object, "value", "BYTE")?)),
        "WORD" => Ok(Value::Word(parse_unsigned(object, "value", "WORD")?)),
        "DWORD" => Ok(Value::DWord(parse_unsigned(object, "value", "DWORD")?)),
        "LWORD" => Ok(Value::LWord(parse_unsigned(object, "value", "LWORD")?)),
        "TIME" => Ok(Value::Time(Duration::from_nanos(parse_signed(
            object, "nanos", "TIME",
        )?))),
        "LTIME" => Ok(Value::LTime(Duration::from_nanos(parse_signed(
            object, "nanos", "LTIME",
        )?))),
        "DATE" => Ok(Value::Date(DateValue::new(parse_signed(
            object, "ticks", "DATE",
        )?))),
        "LDATE" => Ok(Value::LDate(LDateValue::new(parse_signed(
            object, "nanos", "LDATE",
        )?))),
        "TOD" => Ok(Value::Tod(TimeOfDayValue::new(parse_signed(
            object, "ticks", "TOD",
        )?))),
        "LTOD" => Ok(Value::LTod(LTimeOfDayValue::new(parse_signed(
            object, "nanos", "LTOD",
        )?))),
        "DT" => Ok(Value::Dt(DateTimeValue::new(parse_signed(
            object, "ticks", "DT",
        )?))),
        "LDT" => Ok(Value::Ldt(LDateTimeValue::new(parse_signed(
            object, "nanos", "LDT",
        )?))),
        "STRING" => Ok(Value::String(
            parse_string(object, "value", "STRING")?.into(),
        )),
        "WSTRING" => Ok(Value::WString(parse_string(object, "value", "WSTRING")?)),
        "CHAR" => Ok(Value::Char(parse_char(object, "value", "CHAR")?)),
        "WCHAR" => Ok(Value::WChar(parse_wchar(object, "value", "WCHAR")?)),
        "ARRAY" => decode_array_value(object),
        "STRUCT" => decode_struct_value(object),
        "ENUM" => decode_enum_value(object),
        "NULL" => Ok(Value::Null),
        other => Err(HarnessAutomationError::InvalidArgument(format!(
            "Unsupported typed value kind '{other}'."
        ))),
    }
}

fn decode_array_value(
    object: &serde_json::Map<String, JsonValue>,
) -> Result<Value, HarnessAutomationError> {
    let dimensions = object
        .get("dimensions")
        .and_then(JsonValue::as_array)
        .ok_or_else(|| {
            HarnessAutomationError::InvalidArgument(
                "ARRAY values require an array 'dimensions' field.".to_string(),
            )
        })?
        .iter()
        .map(|entry| {
            let pair = entry.as_array().ok_or_else(|| {
                HarnessAutomationError::InvalidArgument(
                    "ARRAY dimensions must be [lower, upper] pairs.".to_string(),
                )
            })?;
            if pair.len() != 2 {
                return Err(HarnessAutomationError::InvalidArgument(
                    "ARRAY dimensions must be [lower, upper] pairs.".to_string(),
                ));
            }
            let lower = pair[0].as_i64().ok_or_else(|| {
                HarnessAutomationError::InvalidArgument(
                    "ARRAY dimension lower bound must be an integer.".to_string(),
                )
            })?;
            let upper = pair[1].as_i64().ok_or_else(|| {
                HarnessAutomationError::InvalidArgument(
                    "ARRAY dimension upper bound must be an integer.".to_string(),
                )
            })?;
            Ok((lower, upper))
        })
        .collect::<Result<Vec<_>, _>>()?;
    let elements = object
        .get("elements")
        .and_then(JsonValue::as_array)
        .ok_or_else(|| {
            HarnessAutomationError::InvalidArgument(
                "ARRAY values require an array 'elements' field.".to_string(),
            )
        })?
        .iter()
        .map(decode_json_value)
        .collect::<Result<Vec<_>, _>>()?;
    Ok(Value::Array(Box::new(
        ArrayValue::from_untyped_parts(elements, dimensions).map_err(|error| {
            HarnessAutomationError::InvalidArgument(format!("Invalid ARRAY value: {error}"))
        })?,
    )))
}

fn decode_struct_value(
    object: &serde_json::Map<String, JsonValue>,
) -> Result<Value, HarnessAutomationError> {
    let type_name = parse_string(object, "type_name", "STRUCT")?;
    let fields = object
        .get("fields")
        .and_then(JsonValue::as_object)
        .ok_or_else(|| {
            HarnessAutomationError::InvalidArgument(
                "STRUCT values require an object 'fields' field.".to_string(),
            )
        })?
        .iter()
        .map(|(name, field)| decode_json_value(field).map(|value| (name.clone().into(), value)))
        .collect::<Result<indexmap::IndexMap<_, _>, _>>()?;
    Ok(Value::Struct(std::sync::Arc::new(
        StructValue::from_untyped_parts(type_name.into(), fields),
    )))
}

fn decode_enum_value(
    object: &serde_json::Map<String, JsonValue>,
) -> Result<Value, HarnessAutomationError> {
    Ok(Value::Enum(Box::new(EnumValue::from_canonical_parts(
        parse_string(object, "type_name", "ENUM")?.into(),
        parse_string(object, "variant", "ENUM")?.into(),
        parse_signed(object, "numeric", "ENUM")?,
    ))))
}

fn parse_bool(
    object: &serde_json::Map<String, JsonValue>,
    key: &str,
    kind: &str,
) -> Result<bool, HarnessAutomationError> {
    object.get(key).and_then(JsonValue::as_bool).ok_or_else(|| {
        HarnessAutomationError::InvalidArgument(format!("{kind} values require boolean '{key}'."))
    })
}

fn parse_string(
    object: &serde_json::Map<String, JsonValue>,
    key: &str,
    kind: &str,
) -> Result<String, HarnessAutomationError> {
    object
        .get(key)
        .and_then(JsonValue::as_str)
        .map(ToOwned::to_owned)
        .ok_or_else(|| {
            HarnessAutomationError::InvalidArgument(format!(
                "{kind} values require string '{key}'."
            ))
        })
}

fn parse_char(
    object: &serde_json::Map<String, JsonValue>,
    key: &str,
    kind: &str,
) -> Result<u8, HarnessAutomationError> {
    let text = parse_string(object, key, kind)?;
    let mut chars = text.chars();
    let ch = chars.next().ok_or_else(|| {
        HarnessAutomationError::InvalidArgument(format!(
            "{kind} values require a one-character string '{key}'."
        ))
    })?;
    if chars.next().is_some() || !ch.is_ascii() {
        return Err(HarnessAutomationError::InvalidArgument(format!(
            "{kind} values require a one-character ASCII string '{key}'."
        )));
    }
    Ok(ch as u8)
}

fn parse_wchar(
    object: &serde_json::Map<String, JsonValue>,
    key: &str,
    kind: &str,
) -> Result<u16, HarnessAutomationError> {
    let text = parse_string(object, key, kind)?;
    let mut chars = text.chars();
    let ch = chars.next().ok_or_else(|| {
        HarnessAutomationError::InvalidArgument(format!(
            "{kind} values require a one-character string '{key}'."
        ))
    })?;
    if chars.next().is_some() {
        return Err(HarnessAutomationError::InvalidArgument(format!(
            "{kind} values require a one-character string '{key}'."
        )));
    }
    u16::try_from(ch as u32).map_err(|_| {
        HarnessAutomationError::InvalidArgument(format!("{kind} value '{text}' is out of range."))
    })
}

fn parse_signed<T>(
    object: &serde_json::Map<String, JsonValue>,
    key: &str,
    kind: &str,
) -> Result<T, HarnessAutomationError>
where
    T: TryFrom<i64>,
{
    let value = object.get(key).and_then(JsonValue::as_i64).ok_or_else(|| {
        HarnessAutomationError::InvalidArgument(format!("{kind} values require integer '{key}'."))
    })?;
    T::try_from(value).map_err(|_| {
        HarnessAutomationError::InvalidArgument(format!("{kind} value '{value}' is out of range."))
    })
}

fn parse_unsigned<T>(
    object: &serde_json::Map<String, JsonValue>,
    key: &str,
    kind: &str,
) -> Result<T, HarnessAutomationError>
where
    T: TryFrom<u64>,
{
    let value = object.get(key).and_then(JsonValue::as_u64).ok_or_else(|| {
        HarnessAutomationError::InvalidArgument(format!(
            "{kind} values require unsigned integer '{key}'."
        ))
    })?;
    T::try_from(value).map_err(|_| {
        HarnessAutomationError::InvalidArgument(format!("{kind} value '{value}' is out of range."))
    })
}

fn parse_f32(
    object: &serde_json::Map<String, JsonValue>,
    key: &str,
    kind: &str,
) -> Result<f32, HarnessAutomationError> {
    let value = object.get(key).and_then(JsonValue::as_f64).ok_or_else(|| {
        HarnessAutomationError::InvalidArgument(format!("{kind} values require numeric '{key}'."))
    })?;
    if !value.is_finite() || value < f32::MIN as f64 || value > f32::MAX as f64 {
        return Err(HarnessAutomationError::InvalidArgument(format!(
            "{kind} value '{value}' is out of range."
        )));
    }
    Ok(value as f32)
}

fn parse_f64(
    object: &serde_json::Map<String, JsonValue>,
    key: &str,
    kind: &str,
) -> Result<f64, HarnessAutomationError> {
    let value = object.get(key).and_then(JsonValue::as_f64).ok_or_else(|| {
        HarnessAutomationError::InvalidArgument(format!("{kind} values require numeric '{key}'."))
    })?;
    if !value.is_finite() {
        return Err(HarnessAutomationError::InvalidArgument(format!(
            "{kind} value '{value}' is not finite."
        )));
    }
    Ok(value)
}

fn validate_sources(source_texts: &[String]) -> Result<Vec<&str>, HarnessAutomationError> {
    if source_texts.is_empty() {
        return Err(HarnessAutomationError::InvalidArgument(
            "source list must not be empty".to_string(),
        ));
    }
    Ok(source_texts.iter().map(String::as_str).collect())
}

fn validate_non_negative(value: i64, field: &str) -> Result<i64, HarnessAutomationError> {
    if value < 0 {
        return Err(HarnessAutomationError::InvalidArgument(format!(
            "{field} must be non-negative."
        )));
    }
    Ok(value)
}

fn render_runtime_errors(errors: &[RuntimeError]) -> Vec<String> {
    errors
        .iter()
        .map(std::string::ToString::to_string)
        .collect()
}

fn runtime_to_error(error: RuntimeError) -> HarnessAutomationError {
    HarnessAutomationError::Runtime(error.to_string())
}

fn snapshot_for_watch(harness: &TestHarness, watch: &[String]) -> HarnessWatchSnapshot {
    let values = watch
        .iter()
        .map(|name| {
            (
                name.clone(),
                harness
                    .try_get_output(name)
                    .map(BoundaryEntry::ok)
                    .unwrap_or_else(BoundaryEntry::error),
            )
        })
        .collect::<BTreeMap<_, _>>();
    HarnessWatchSnapshot {
        cycle_count: harness.cycle_count(),
        elapsed_ms: harness.current_time().as_millis(),
        values,
    }
}
