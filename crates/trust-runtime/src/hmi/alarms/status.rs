//! An alarm's state, read from its outputs (specification 40, §2.2).

use serde::Serialize;

use super::discover::{alarm_instances, Found};
use crate::runtime::program_access::{Located, ProgramRead};
use crate::value::Value;

/// An alarm's priority (ISA-18.2, EEMUA 191).
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Serialize, Default)]
#[serde(rename_all = "kebab-case")]
pub enum Priority {
    /// Most alarms.
    #[default]
    Low,
    /// Some alarms.
    Medium,
    /// Few alarms.
    High,
}

/// A limit alarm's value against its limit when it was raised (specification 39, §5.9 item 2).
#[derive(Debug, Clone, PartialEq, Serialize)]
pub struct Raised {
    /// The value that went beyond the limit.
    pub value: f64,
    /// The limit.
    pub limit: f64,
    /// A high limit, crossed upward; else a low one.
    pub high: bool,
    /// The known equipment physical-value path, for shared decimal formatting.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub source: Option<String>,
    /// The engineering unit when raised; later changes must not relabel history.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub unit: Option<String>,
}

/// A limit alarm's value against its limit at one read. It is equal to any other reading, so a
/// moving value is no change of the alarm's state; the board keeps the one that raised it.
#[derive(Debug, Clone, Default)]
pub struct Reading(pub Option<Raised>);

impl PartialEq for Reading {
    fn eq(&self, _other: &Self) -> bool {
        true
    }
}

/// One alarm as the runtime reads it.
#[derive(Debug, Clone, PartialEq, Default, Serialize)]
pub struct AlarmStatus {
    /// The instance path, its identity.
    pub path: String,
    /// What the operator reads.
    pub message: String,
    /// Its priority.
    pub priority: Priority,
    /// The ISA-18.2 state's name.
    pub state: String,
    /// Active: the condition holds or it is latched.
    pub active: bool,
    /// Waiting for acknowledgement.
    pub unacknowledged: bool,
    /// Shelved by an operator.
    pub shelved: bool,
    /// Out of service.
    pub out_of_service: bool,
    /// Suppressed by design.
    pub suppressed: bool,
    /// First out in its group.
    pub first_out: bool,
    /// Shown in the active list.
    pub annunciating: bool,
    /// How often it became active.
    pub raise_count: u32,
    /// It stays active until it is reset (its `Latching` input).
    pub latching: bool,
    /// The function block instance nearest above it that holds it; none when a program holds
    /// it directly.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub equipment: Option<String>,
    /// A limit alarm's value against its limit now (specification 39, §5.9 item 2).
    #[serde(skip)]
    pub reading: Reading,
}

/// The state of every alarm of the running program, read on the runtime thread at a call of
/// `Hmi.Display`. A program without the library is not searched (specification 40, §5, item 5).
pub fn read_alarms(program: &dyn ProgramRead) -> Vec<AlarmStatus> {
    alarm_instances(program)
        .into_iter()
        .filter_map(|found| read_alarm(program, found))
        .collect()
}

/// One alarm's outputs, which a derived alarm inherits from the alarm base.
fn read_alarm(program: &dyn ProgramRead, found: Found) -> Option<AlarmStatus> {
    let Found {
        path,
        at,
        equipment,
    } = found;
    let member = |name: &str| member_value(program, &at, name);
    let flag = |name: &str| matches!(member(name), Some(Value::Bool(true)));
    let text = |name: &str| match member(name) {
        Some(Value::String(text)) => text.to_string(),
        Some(Value::Enum(value)) => value.variant_name().to_string(),
        _ => String::new(),
    };
    let priority = match text("Priority").to_ascii_lowercase().as_str() {
        "high" => Priority::High,
        "medium" => Priority::Medium,
        _ => Priority::Low,
    };
    let raise_count = match member("RaiseCount")? {
        Value::UDInt(count) => count,
        _ => 0,
    };
    let mut reading = reading(program, &at);
    if let (Some(reading), Some(equipment)) = (reading.as_mut(), equipment.as_deref())
        && let Some((source, unit)) =
            crate::hmi::presentation::alarm_value_format(program, equipment, &path)
    {
        reading.source = Some(source);
        reading.unit = unit;
    }
    Some(AlarmStatus {
        message: text("Message"),
        priority,
        state: text("State"),
        active: flag("Active"),
        unacknowledged: flag("Unacknowledged"),
        shelved: flag("Shelved"),
        out_of_service: flag("OutOfService"),
        suppressed: flag("SuppressedByDesign"),
        first_out: flag("FirstOut"),
        annunciating: flag("Annunciating"),
        raise_count,
        latching: flag("Latching"),
        reading: Reading(reading),
        equipment,
        path,
    })
}

/// The value of the member `name` of the instance at `at`, its inherited members included.
fn member_value(program: &dyn ProgramRead, at: &Located, name: &str) -> Option<Value> {
    program.read_at(&program.member(at, name)?).ok()
}

/// A limit alarm's value against its limit: an instance with a number `Value` and `Limit` (a
/// `LimitAlarm`, or a block that extends one, whose members include its bases'); its `Direction`
/// says whether the limit is a low one.
fn reading(program: &dyn ProgramRead, at: &Located) -> Option<Raised> {
    let number = |name: &str| match member_value(program, at, name) {
        Some(Value::LReal(value)) => Some(value),
        Some(Value::Real(value)) => Some(f64::from(value)),
        _ => None,
    };
    let (value, limit) = (number("Value")?, number("Limit")?);
    let low = matches!(
        member_value(program, at, "Direction"),
        Some(Value::Enum(direction)) if direction.variant_name().eq_ignore_ascii_case("Low")
    );
    Some(Raised {
        value,
        limit,
        high: !low,
        source: None,
        unit: None,
    })
}
