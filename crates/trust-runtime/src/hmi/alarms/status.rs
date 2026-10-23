//! An alarm's state, read from its outputs (specification 40, §2.2).

use serde::Serialize;

use super::discover::{alarm_base, alarm_instances, Found, ALARM_BASE};
use crate::hmi::member_paths::find_ignoring_case;
use crate::memory::{InstanceId, VariableStorage};
use crate::runtime::Runtime;
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

/// The state of every alarm of the running program, read on the runtime thread. A program
/// without the library is not searched (specification 40, §5, item 5).
pub fn read_alarms(runtime: &Runtime) -> Vec<AlarmStatus> {
    if find_ignoring_case(runtime.function_blocks(), ALARM_BASE).is_none() {
        return Vec::new();
    }
    let storage = runtime.storage();
    let programs = runtime.program_templates();
    let is_program = |type_name: &str| find_ignoring_case(programs, type_name).is_some();
    let holdings = super::discover::holdings_of(runtime);
    alarm_instances(storage, &holdings, &is_program)
        .into_iter()
        .filter_map(|found| read_alarm(storage, found))
        .collect()
}

/// One alarm's outputs, read from its base part.
fn read_alarm(storage: &VariableStorage, found: Found) -> Option<AlarmStatus> {
    let Found {
        path,
        id,
        equipment,
    } = found;
    let base = alarm_base(storage, id)?;
    let member = |name: &str| find_ignoring_case(&base.variables, name);
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
        Value::UDInt(count) => *count,
        _ => 0,
    };
    let mut reading = reading(storage, id);
    if let (Some(reading), Some(equipment)) = (reading.as_mut(), equipment.as_deref())
        && let Some((source, unit)) =
            crate::hmi::presentation::alarm_value_format(storage, equipment, &path)
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

/// A limit alarm's value against its limit: from the first part of the instance, from its own
/// type down to the alarm base, with a number `Value` and `Limit` (a `LimitAlarm`, or a block that
/// extends one); its `Direction` says whether the limit is a low one.
fn reading(storage: &VariableStorage, id: InstanceId) -> Option<Raised> {
    let mut current = storage.get_instance(id);
    for _ in 0..8 {
        let part = current?;
        let number = |name: &str| match find_ignoring_case(&part.variables, name) {
            Some(Value::LReal(value)) => Some(*value),
            Some(Value::Real(value)) => Some(f64::from(*value)),
            _ => None,
        };
        if let (Some(value), Some(limit)) = (number("Value"), number("Limit")) {
            let low = matches!(
                find_ignoring_case(&part.variables, "Direction"),
                Some(Value::Enum(direction)) if direction.variant_name().eq_ignore_ascii_case("Low")
            );
            return Some(Raised {
                value,
                limit,
                high: !low,
                source: None,
                unit: None,
            });
        }
        current = part.parent.and_then(|parent| storage.get_instance(parent));
    }
    None
}
