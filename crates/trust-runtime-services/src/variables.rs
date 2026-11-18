//! The `variables.read` operation (specification 11 §6.8.1, "Reading variables"; specification 48
//! RU-62): the values a set of the program's variables had when a scan completed, by their paths,
//! the read of a Rust service. A computer and a chip answer it alike: each reads the set between two
//! scans with [`read_set`] on its program's memory, and this operation owns the parameters, the
//! stable error texts and the reply's JSON.

use alloc::format;
use alloc::string::{String, ToString};
use alloc::vec::Vec;

use serde_json::{json, Number, Value as Json};
use smol_str::SmolStr;
use trust_runtime_core::native::memory::type_name;
use trust_runtime_core::native::reflect::resolve_path;
use trust_runtime_core::native::variables::CompiledVariables;
use trust_runtime_core::value::Value;

use crate::ports::ResourcePort;
use crate::protocol::ControlResponse;
use trust_runtime_core::scheduler::ResourceState;

/// The most paths one request reads.
pub const MAX_PATHS: usize = 256;

/// The values a completed scan left of a set of variables.
#[derive(Debug, Clone, PartialEq)]
pub struct ReadSet {
    /// The number of the completed scan, as `io.read` numbers its snapshot.
    pub scan: u64,
    /// Its logical time in nanoseconds (`TIME()`).
    pub time_ns: i64,
    /// Each path's declared type and value, in the request's order.
    pub values: Vec<(SmolStr, Value)>,
}

/// Why a set is not read; a refused request reads nothing.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ReadRefusal {
    /// No scan has completed since the program was loaded.
    NoCompletedScan,
    /// The path names no variable.
    Unknown(String),
    /// The path names a structure, an array, an instance or a reference, which hold no single
    /// value.
    NotElementary(String),
    /// The platform could not read between its scans, with the reason.
    Unavailable(String),
}

/// `variables.read`: the values of the `paths` of `params` that the last completed scan left, with
/// the scan's number and logical time, the resource's state and the values' quality; or the
/// refusal of parameters that are not 1 to [`MAX_PATHS`] paths, or of the read.
#[must_use]
pub fn read<H: ResourcePort>(id: u64, params: Option<&Json>, host: &H) -> ControlResponse {
    let paths = match paths(params) {
        Ok(paths) => paths,
        Err(reason) => return ControlResponse::error(id, format!("invalid params: {reason}")),
    };
    let set = match host.read_variables(&paths) {
        Ok(set) => set,
        Err(refusal) => return ControlResponse::error(id, refusal_text(&refusal)),
    };
    let state = host.resource_state();
    let values: Vec<Json> = paths
        .iter()
        .zip(&set.values)
        .map(|(path, (ty, value))| json!({ "path": path, "type": ty.as_str(), "value": json_value(value) }))
        .collect();
    ControlResponse::ok(
        id,
        json!({
            "scan": set.scan,
            "time_ns": set.time_ns,
            "state": format!("{state:?}").to_ascii_lowercase(),
            "quality": if state == ResourceState::Running { "good" } else { "stale" },
            "values": values,
        }),
    )
}

/// The declared type and value of each of `paths` in `memory`, the program's memory between two
/// scans, in the order of `paths`; or the refusal of the first path that names no variable or one
/// that holds no single value.
///
/// # Errors
///
/// [`ReadRefusal::Unknown`] and [`ReadRefusal::NotElementary`], naming the path.
pub fn read_set<M: CompiledVariables + ?Sized>(
    memory: &M,
    paths: &[String],
) -> Result<Vec<(SmolStr, Value)>, ReadRefusal> {
    let image = memory.program_image();
    let mut values = Vec::with_capacity(paths.len());
    for path in paths {
        let at = resolve_path(&image, path).map_err(|_| ReadRefusal::Unknown(path.clone()))?;
        let value = memory
            .read_located(&at)
            .map_err(|_| ReadRefusal::NotElementary(path.clone()))?;
        if matches!(
            value,
            Value::Array(_)
                | Value::Struct(_)
                | Value::Reference(_)
                | Value::Instance(_)
                | Value::Null
        ) {
            return Err(ReadRefusal::NotElementary(path.clone()));
        }
        values.push((type_name(&image, at.type_index), value));
    }
    Ok(values)
}

/// The paths `params` name: exactly `paths`, 1 to [`MAX_PATHS`] strings.
fn paths(params: Option<&Json>) -> Result<Vec<String>, &'static str> {
    let Some(Json::Object(params)) = params else {
        return Err("paths is required");
    };
    if params.keys().any(|key| key != "paths") {
        return Err("only paths is taken");
    }
    let Some(Json::Array(paths)) = params.get("paths") else {
        return Err("paths must be an array of variable paths");
    };
    if paths.is_empty() || paths.len() > MAX_PATHS {
        return Err("paths must name 1 to 256 variables");
    }
    paths
        .iter()
        .map(|path| {
            path.as_str()
                .map(str::to_string)
                .ok_or("paths must be an array of variable paths")
        })
        .collect()
}

/// The stable error text of `refusal`.
fn refusal_text(refusal: &ReadRefusal) -> String {
    match refusal {
        ReadRefusal::NoCompletedScan => "no completed scan".to_string(),
        ReadRefusal::Unknown(path) => format!("unknown variable: {path}"),
        ReadRefusal::NotElementary(path) => format!("not an elementary variable: {path}"),
        ReadRefusal::Unavailable(reason) => format!("variables unavailable: {reason}"),
    }
}

/// The JSON form of an elementary value (specification 11 §6.8.1): a Boolean, an integer, a real
/// number as its shortest text that reads back as the same value of its type, a duration in
/// nanoseconds, a date or time of day in the units specification 10 counts it in, a text, or an
/// enumerator's name.
fn json_value(value: &Value) -> Json {
    match value {
        Value::Bool(value) => json!(value),
        Value::SInt(value) => json!(value),
        Value::Int(value) => json!(value),
        Value::DInt(value) => json!(value),
        Value::LInt(value) => json!(value),
        Value::USInt(value) | Value::Byte(value) => json!(value),
        Value::UInt(value) | Value::Word(value) => json!(value),
        Value::UDInt(value) | Value::DWord(value) => json!(value),
        Value::ULInt(value) | Value::LWord(value) => json!(value),
        // The shortest text of the single-precision value, read as a double, prints as that text.
        Value::Real(value) => real(f64::from(*value), value.to_string().parse().ok()),
        Value::LReal(value) => real(*value, Some(*value)),
        Value::Time(value) | Value::LTime(value) => json!(value.as_nanos()),
        Value::Date(value) => json!(value.ticks()),
        Value::Tod(value) => json!(value.ticks()),
        Value::Dt(value) => json!(value.ticks()),
        Value::LDate(value) => json!(value.nanos()),
        Value::LTod(value) => json!(value.nanos()),
        Value::Ldt(value) => json!(value.nanos()),
        Value::String(text) => json!(text.as_str()),
        Value::WString(text) => json!(text),
        Value::Char(byte) => json!(char::from(*byte).to_string()),
        Value::WChar(unit) => json!(char::from_u32(u32::from(*unit))
            .unwrap_or(char::REPLACEMENT_CHARACTER)
            .to_string()),
        Value::Enum(value) => json!(value.variant_name().as_str()),
        Value::Array(_)
        | Value::Struct(_)
        | Value::Reference(_)
        | Value::Instance(_)
        | Value::Null => Json::Null,
    }
}

/// A real number `value` as a JSON number, written from `shortest` (the shortest text of the value
/// in its own type, read back as a double); the strings `NaN`, `Infinity` and `-Infinity` for a
/// value no JSON number holds.
fn real(value: f64, shortest: Option<f64>) -> Json {
    if value.is_nan() {
        return json!("NaN");
    }
    if value.is_infinite() {
        return json!(if value > 0.0 { "Infinity" } else { "-Infinity" });
    }
    shortest
        .and_then(Number::from_f64)
        .map_or(Json::Null, Json::Number)
}

#[cfg(test)]
#[path = "variables_tests.rs"]
mod tests;
