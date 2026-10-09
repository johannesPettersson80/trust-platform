//! Values for the runtime display (specification 39, §11.1): each subscribed path's value in the
//! form the browser's drawing takes.

use std::fmt::Write as _;

use serde::ser::{Serialize, SerializeMap, SerializeSeq, Serializer};
use serde_json::{json, Value as Json};

use crate::hmi::member_paths::{resolve_member, MemberPath};
use crate::memory::VariableStorage;
use crate::runtime::Runtime;
use crate::stdlib::time::{split_date, split_dt, split_ldt, split_ltod, split_tod};
use crate::value::{ArrayValue, DateTimeProfile, LDateTimeValue, Value};

/// The most elements one value may have; a larger value is unknown.
pub const MAX_ELEMENTS: usize = 20_000;

/// The largest integer a browser's JSON number holds exactly (2^53 − 1).
const MAX_SAFE_INTEGER: u64 = 9_007_199_254_740_991;

/// A value as the display draws it: a single JSON value, an array level, or the members of a
/// structure or instance in their declared order.
#[derive(Debug, Clone, PartialEq)]
pub enum Shown {
    /// A number, boolean, text or null.
    Single(Json),
    /// One level of an array.
    List(Vec<Shown>),
    /// Members by name, in their declared order.
    Members(Vec<(String, Shown)>),
}

impl Serialize for Shown {
    fn serialize<S: Serializer>(&self, serializer: S) -> Result<S::Ok, S::Error> {
        match self {
            Self::Single(value) => value.serialize(serializer),
            Self::List(items) => {
                let mut list = serializer.serialize_seq(Some(items.len()))?;
                for item in items {
                    list.serialize_element(item)?;
                }
                list.end()
            }
            Self::Members(members) => {
                let mut object = serializer.serialize_map(Some(members.len()))?;
                for (name, member) in members {
                    object.serialize_entry(name, member)?;
                }
                object.end()
            }
        }
    }
}

/// A value as the display receives it: `{ "value": …, "lowerBounds": […] }`.
#[derive(Debug, Clone, PartialEq, serde::Serialize)]
pub struct WireValue {
    /// The value.
    pub value: Shown,
    /// For an array, the declared lower bound of each dimension.
    #[serde(rename = "lowerBounds", skip_serializing_if = "Option::is_none")]
    pub lower_bounds: Option<Vec<i64>>,
    /// For a value located at an input of a link that exchanges no data, the time since the
    /// link's last data, in ms (specification 42, §5 item 4).
    #[serde(rename = "staleMs", skip_serializing_if = "Option::is_none")]
    pub stale_ms: Option<u64>,
    /// Equipment units and alarm thresholds sampled at the same Hmi.Display boundary.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub presentation: Option<crate::hmi::presentation::EquipmentPresentation>,
}

/// What the runtime thread answers to a display's read: its cycle count and each path's value,
/// `None` where the path is unknown.
#[derive(Debug, Clone, PartialEq, serde::Serialize)]
pub struct HmiRead {
    /// The runtime's cycle count when the paths were read.
    pub cycle: u64,
    /// Each path's value in the order asked for, as [`wire_value`] gives it.
    pub values: Vec<Option<WireValue>>,
    /// For each path, whether the running program has it (specification 39, §16.3).
    pub present: Vec<bool>,
    /// For each path that names a function block instance, its type (§8.7, item 4).
    pub instance_types: Vec<Option<String>>,
}

/// Reads a display's paths on the runtime thread, without copying the program's storage.
pub fn read(runtime: &Runtime, paths: &[MemberPath]) -> HmiRead {
    let storage = runtime.storage();
    let profile = runtime.profile();
    let resolved: Vec<Option<&Value>> = paths
        .iter()
        .map(|path| resolve_member(path, storage))
        .collect();
    HmiRead {
        cycle: runtime.cycle_counter(),
        values: resolved
            .iter()
            .zip(paths)
            .map(|(value, path)| {
                value
                    .and_then(|value| wire_value(value, storage, profile))
                    .map(|mut value| {
                        value.presentation =
                            crate::hmi::presentation::equipment_presentation(storage, path);
                        value
                    })
            })
            .collect(),
        present: resolved.iter().map(Option::is_some).collect(),
        instance_types: resolved
            .iter()
            .map(|value| match value {
                Some(Value::Instance(id)) => storage
                    .get_instance(*id)
                    .map(|instance| instance.type_name.to_string()),
                _ => None,
            })
            .collect(),
    }
}

/// A value as the display receives it, or `None` when it cannot be shown and is unknown: null, a
/// reference, or more than [`MAX_ELEMENTS`] elements.
pub fn wire_value(
    value: &Value,
    storage: &VariableStorage,
    profile: DateTimeProfile,
) -> Option<WireValue> {
    if matches!(value, Value::Null | Value::Reference(_)) {
        return None;
    }
    let mut encoder = Encoder {
        storage,
        profile,
        budget: MAX_ELEMENTS,
    };
    let shown = encoder.encode(value).ok()?;
    let lower_bounds = match value {
        Value::Array(array) => Some(array.dimensions().iter().map(|(low, _)| *low).collect()),
        _ => None,
    };
    Some(WireValue {
        value: shown,
        lower_bounds,
        stale_ms: None,
        presentation: None,
    })
}

/// The value has more elements than the display takes.
struct TooLarge;

struct Encoder<'a> {
    storage: &'a VariableStorage,
    profile: DateTimeProfile,
    /// Elements still allowed in this value.
    budget: usize,
}

impl Encoder<'_> {
    fn encode(&mut self, value: &Value) -> Result<Shown, TooLarge> {
        self.budget = self.budget.checked_sub(1).ok_or(TooLarge)?;
        Ok(match value {
            Value::Array(array) => self.array(array)?,
            Value::Struct(value) => self.members(value.fields())?,
            Value::Instance(id) => match self.storage.get_instance(*id) {
                Some(instance) => self.members(&instance.variables)?,
                None => Shown::Single(Json::Null),
            },
            single => Shown::Single(self.single(single)),
        })
    }

    fn single(&self, value: &Value) -> Json {
        match value {
            Value::Bool(value) => Json::Bool(*value),
            Value::SInt(value) => json!(value),
            Value::Int(value) => json!(value),
            Value::DInt(value) => json!(value),
            Value::LInt(value) => signed(*value),
            Value::USInt(value) => json!(value),
            Value::UInt(value) => json!(value),
            Value::UDInt(value) => json!(value),
            Value::ULInt(value) => unsigned(*value),
            Value::Byte(value) => json!(value),
            Value::Word(value) => json!(value),
            Value::DWord(value) => json!(value),
            Value::LWord(value) => unsigned(*value),
            // A REAL keeps its shortest digits: 0.1, not 0.10000000149011612.
            Value::Real(value) => real(value.to_string().parse().unwrap_or(f64::from(*value))),
            Value::LReal(value) => real(*value),
            Value::Time(value) => json!(duration_literal("T", value.as_nanos())),
            Value::LTime(value) => json!(duration_literal("LT", value.as_nanos())),
            Value::Date(_) => date_literal("D", split_date(value, self.profile).ok()),
            Value::LDate(date) => date_literal(
                "LD",
                split_ldt(&Value::Ldt(LDateTimeValue::new(date.nanos())))
                    .ok()
                    .map(|(year, month, day, ..)| (year, month, day)),
            ),
            Value::Tod(_) => time_of_day_literal("TOD", split_tod(value, self.profile).ok()),
            Value::LTod(_) => time_of_day_literal("LTOD", split_ltod(value).ok()),
            Value::Dt(_) => date_time_literal("DT", split_dt(value, self.profile).ok()),
            Value::Ldt(_) => date_time_literal("LDT", split_ldt(value).ok()),
            Value::String(value) => json!(value.as_str()),
            Value::WString(value) => json!(value),
            Value::Char(value) => json!(char::from(*value).to_string()),
            Value::WChar(value) => json!(char::from_u32(u32::from(*value))
                .unwrap_or(char::REPLACEMENT_CHARACTER)
                .to_string()),
            Value::Enum(value) => json!(value.variant_name().as_str()),
            Value::Array(_)
            | Value::Struct(_)
            | Value::Instance(_)
            | Value::Reference(_)
            | Value::Null => Json::Null,
        }
    }

    /// An array as nested lists, one level per dimension.
    fn array(&mut self, array: &ArrayValue) -> Result<Shown, TooLarge> {
        if array.elements().len() > self.budget {
            return Err(TooLarge);
        }
        let mut elements = Vec::with_capacity(array.elements().len());
        for element in array.elements() {
            elements.push(self.encode(element)?);
        }
        Ok(nest(&mut elements.into_iter(), array.dimensions()))
    }

    fn members<'v>(
        &mut self,
        members: impl IntoIterator<Item = (&'v smol_str::SmolStr, &'v Value)>,
    ) -> Result<Shown, TooLarge> {
        let mut shown = Vec::new();
        for (name, member) in members {
            shown.push((name.to_string(), self.encode(member)?));
        }
        Ok(Shown::Members(shown))
    }
}

fn nest(elements: &mut impl Iterator<Item = Shown>, dimensions: &[(i64, i64)]) -> Shown {
    let Some(((low, high), inner)) = dimensions.split_first() else {
        return elements.next().unwrap_or(Shown::Single(Json::Null));
    };
    let count = usize::try_from(high - low + 1).unwrap_or(0);
    Shown::List((0..count).map(|_| nest(elements, inner)).collect())
}

fn signed(value: i64) -> Json {
    if value.unsigned_abs() <= MAX_SAFE_INTEGER {
        json!(value)
    } else {
        json!(value.to_string())
    }
}

fn unsigned(value: u64) -> Json {
    if value <= MAX_SAFE_INTEGER {
        json!(value)
    } else {
        json!(value.to_string())
    }
}

fn real(value: f64) -> Json {
    if value.is_nan() {
        json!("NaN")
    } else if value.is_infinite() {
        json!(if value > 0.0 { "+Inf" } else { "-Inf" })
    } else {
        json!(value)
    }
}

/// A duration as an IEC literal with its parts, for example `T#1h2m3s500ms`.
fn duration_literal(prefix: &str, nanos: i64) -> String {
    if nanos == 0 {
        return format!("{prefix}#0s");
    }
    let mut text = format!("{prefix}#{}", if nanos < 0 { "-" } else { "" });
    let mut rest = nanos.unsigned_abs();
    for (unit, size) in [
        ("d", 86_400_000_000_000_u64),
        ("h", 3_600_000_000_000),
        ("m", 60_000_000_000),
        ("s", 1_000_000_000),
        ("ms", 1_000_000),
        ("us", 1_000),
        ("ns", 1),
    ] {
        let count = rest / size;
        if count > 0 {
            let _ = write!(text, "{count}{unit}");
            rest %= size;
        }
    }
    text
}

fn date_literal(prefix: &str, parts: Option<(i64, i64, i64)>) -> Json {
    parts.map_or(Json::Null, |(year, month, day)| {
        json!(format!("{prefix}#{year:04}-{month:02}-{day:02}"))
    })
}

fn clock(hour: i64, minute: i64, second: i64, millis: i64) -> String {
    let mut text = format!("{hour:02}:{minute:02}:{second:02}");
    if millis > 0 {
        let _ = write!(text, ".{millis:03}");
    }
    text
}

fn time_of_day_literal(prefix: &str, parts: Option<(i64, i64, i64, i64)>) -> Json {
    parts.map_or(Json::Null, |(hour, minute, second, millis)| {
        json!(format!("{prefix}#{}", clock(hour, minute, second, millis)))
    })
}

fn date_time_literal(prefix: &str, parts: Option<(i64, i64, i64, i64, i64, i64, i64)>) -> Json {
    parts.map_or(
        Json::Null,
        |(year, month, day, hour, minute, second, millis)| {
            json!(format!(
                "{prefix}#{year:04}-{month:02}-{day:02}-{}",
                clock(hour, minute, second, millis)
            ))
        },
    )
}

#[cfg(test)]
mod tests;
