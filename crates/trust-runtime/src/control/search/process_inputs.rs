//! The live runtime's process-image inputs carried into an isolated run (Specification 35, the
//! isolated run): the copy starts its first scan with every bound input as the plant had it when
//! the run was requested, so sensor blocks see real signals; the run's plant then writes only its
//! own feedback targets and every other input keeps its value. Outputs and memory are not copied.

use crate::io::{IoAddress, IoSnapshot, IoSnapshotValue};
use crate::memory::IoArea;
use crate::value::Value;
use crate::Runtime;
use serde::{Deserialize, Serialize};
use serde_json::{json, Value as JsonValue};

/// The most inputs one run carries.
pub(super) const MAX_PROCESS_INPUTS: usize = 4096;

/// One bound input with its elementary type and value.
#[derive(Clone, Debug, PartialEq, Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
pub(super) struct ProcessInput {
    pub address: String,
    pub value_type: String,
    pub value: JsonValue,
}

/// The bound inputs of the live runtime's latest scan; none before a scan was captured.
pub(super) fn capture(snapshot: Option<&IoSnapshot>) -> Vec<ProcessInput> {
    let Some(snapshot) = snapshot else {
        return Vec::new();
    };
    snapshot
        .inputs
        .iter()
        .filter(|entry| entry.address.area == IoArea::Input && !entry.address.wildcard)
        .filter_map(|entry| {
            let IoSnapshotValue::Value(value) = &entry.value else {
                return None;
            };
            let (value_type, value) = encode(value)?;
            Some(ProcessInput {
                address: super::super::types::format_address(&entry.address),
                value_type: value_type.to_string(),
                value,
            })
        })
        .take(MAX_PROCESS_INPUTS)
        .collect()
}

/// Writes the carried inputs into the isolated runtime's process image before its first scan.
pub(super) fn apply(runtime: &mut Runtime, inputs: &[ProcessInput]) -> Result<(), String> {
    if inputs.len() > MAX_PROCESS_INPUTS {
        return Err("live process inputs exceed their bound".into());
    }
    for input in inputs {
        let (address, value) =
            decode(input).map_err(|reason| format!("live input {}: {reason}", input.address))?;
        // The process image holds each address in its own storage (an `%IL` input a 64-bit
        // pattern), as the worker writes an addressed binding (`assign`).
        let stored = match type_of(&input.value_type) {
            Some(kind) => crate::io::coerce_to_io(value, kind, address.size)
                .map_err(|error| format!("live input {}: {error}", input.address))?,
            None => value,
        };
        runtime
            .io_mut()
            .write(&address, stored)
            .map_err(|error| format!("live input {}: {error}", input.address))?;
    }
    Ok(())
}

/// The elementary type a carried input names.
fn type_of(name: &str) -> Option<trust_hir::TypeId> {
    use trust_hir::TypeId;
    Some(match name {
        "BOOL" => TypeId::BOOL,
        "SINT" => TypeId::SINT,
        "INT" => TypeId::INT,
        "DINT" => TypeId::DINT,
        "LINT" => TypeId::LINT,
        "USINT" => TypeId::USINT,
        "UINT" => TypeId::UINT,
        "UDINT" => TypeId::UDINT,
        "ULINT" => TypeId::ULINT,
        "BYTE" => TypeId::BYTE,
        "WORD" => TypeId::WORD,
        "DWORD" => TypeId::DWORD,
        "LWORD" => TypeId::LWORD,
        "REAL" => TypeId::REAL,
        "LREAL" => TypeId::LREAL,
        _ => return None,
    })
}

fn encode(value: &Value) -> Option<(&'static str, JsonValue)> {
    let finite = |number: f64| number.is_finite().then(|| json!(number));
    Some(match value {
        Value::Bool(value) => ("BOOL", json!(value)),
        Value::SInt(value) => ("SINT", json!(value)),
        Value::Int(value) => ("INT", json!(value)),
        Value::DInt(value) => ("DINT", json!(value)),
        Value::LInt(value) => ("LINT", json!(value)),
        Value::USInt(value) => ("USINT", json!(value)),
        Value::UInt(value) => ("UINT", json!(value)),
        Value::UDInt(value) => ("UDINT", json!(value)),
        Value::ULInt(value) => ("ULINT", json!(value)),
        Value::Byte(value) => ("BYTE", json!(value)),
        Value::Word(value) => ("WORD", json!(value)),
        Value::DWord(value) => ("DWORD", json!(value)),
        Value::LWord(value) => ("LWORD", json!(value)),
        Value::Real(value) => ("REAL", finite(f64::from(*value))?),
        Value::LReal(value) => ("LREAL", finite(*value)?),
        _ => return None,
    })
}

fn decode(input: &ProcessInput) -> Result<(IoAddress, Value), String> {
    let address = IoAddress::parse(&input.address).map_err(|error| error.to_string())?;
    if address.area != IoArea::Input || address.wildcard {
        return Err("only explicit input addresses are carried".into());
    }
    let wrong = || format!("not a valid {}", input.value_type);
    let signed = || input.value.as_i64().ok_or_else(wrong);
    let unsigned = || input.value.as_u64().ok_or_else(wrong);
    let value = match input.value_type.as_str() {
        "BOOL" => Value::Bool(input.value.as_bool().ok_or_else(wrong)?),
        "SINT" => Value::SInt(i8::try_from(signed()?).map_err(|_| wrong())?),
        "INT" => Value::Int(i16::try_from(signed()?).map_err(|_| wrong())?),
        "DINT" => Value::DInt(i32::try_from(signed()?).map_err(|_| wrong())?),
        "LINT" => Value::LInt(signed()?),
        "USINT" => Value::USInt(u8::try_from(unsigned()?).map_err(|_| wrong())?),
        "UINT" => Value::UInt(u16::try_from(unsigned()?).map_err(|_| wrong())?),
        "UDINT" => Value::UDInt(u32::try_from(unsigned()?).map_err(|_| wrong())?),
        "ULINT" => Value::ULInt(unsigned()?),
        "BYTE" => Value::Byte(u8::try_from(unsigned()?).map_err(|_| wrong())?),
        "WORD" => Value::Word(u16::try_from(unsigned()?).map_err(|_| wrong())?),
        "DWORD" => Value::DWord(u32::try_from(unsigned()?).map_err(|_| wrong())?),
        "LWORD" => Value::LWord(unsigned()?),
        "REAL" => Value::Real(
            input
                .value
                .as_f64()
                .filter(|number| number.is_finite())
                .ok_or_else(wrong)? as f32,
        ),
        "LREAL" => Value::LReal(
            input
                .value
                .as_f64()
                .filter(|number| number.is_finite())
                .ok_or_else(wrong)?,
        ),
        other => return Err(format!("unsupported input type {other}")),
    };
    Ok((address, value))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn an_input_survives_the_trip_into_the_isolated_run() {
        for value in [
            Value::Bool(true),
            Value::Int(-7),
            Value::Word(513),
            Value::Real(11.25),
            Value::LReal(12.8),
        ] {
            let (value_type, encoded) = encode(&value).expect("an elementary input encodes");
            let input = ProcessInput {
                address: "%IW4".into(),
                value_type: value_type.into(),
                value: encoded,
            };
            assert_eq!(decode(&input).expect("it decodes").1, value);
        }
    }

    #[test]
    fn only_explicit_inputs_are_carried() {
        let output = ProcessInput {
            address: "%QL0".into(),
            value_type: "LREAL".into(),
            value: json!(1.0),
        };
        assert!(decode(&output).is_err());
        assert!(encode(&Value::LReal(f64::NAN)).is_none());
    }
}
