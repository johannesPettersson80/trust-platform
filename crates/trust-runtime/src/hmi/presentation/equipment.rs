//! Live presentation data from the owning equipment, sampled with its value (§5.9): for a value
//! that is the physical value of a known equipment contract, its unit, its range, its enabled alarm
//! limits and its alarms' priorities. The contracts are the Plant library's analog instrument
//! (`Plant.FB_AnalogInput` and every block derived from it) and its tank (`Plant.FB_Tank`); a
//! customer block that extends one of them takes part as well.
use std::collections::BTreeMap;

use serde::Serialize;

use crate::hmi::member_paths::{locate_member, MemberPath, PathSegment};
use crate::hmi::page_files::HmiLimits;
use crate::runtime::program_access::{Located, ProgramRead};
use crate::value::Value;

/// Presentation facts, not alarm states or command constraints.
#[derive(Debug, Clone, PartialEq, Serialize)]
pub struct EquipmentPresentation {
    /// The physical value's engineering unit, when the block declares a nonempty one.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub unit: Option<String>,
    /// The engineering display range, when finite and increasing.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub range: Option<[f64; 2]>,
    /// The enabled alarm limits, omitted when there are none or they are inconsistent.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub limits: Option<HmiLimits>,
    /// Each limit alarm's actual priority, including inactive alarms.
    pub priorities: BTreeMap<&'static str, String>,
}

/// A known equipment contract.
struct Contract {
    /// The block, or the base the equipment's block extends.
    block: &'static str,
    /// The member that holds the physical value.
    value: &'static str,
    /// The members of the range's ends; a low end of `None` is 0.
    range: (Option<&'static str>, &'static str),
    /// Whether it has the four limit alarms: per limit `<Name>Limit`, `<Name>Enabled` and the
    /// alarm instance `<Name>Alarm`.
    limits: bool,
}

static CONTRACTS: [Contract; 2] = [
    Contract {
        block: "Plant.FB_AnalogInput",
        value: "Value",
        range: (Some("RangeLow"), "RangeHigh"),
        limits: true,
    },
    // The tank owns no alarms: its level instrument does (Plant §7.5).
    Contract {
        block: "Plant.FB_Tank",
        value: "Level",
        range: (None, "Height"),
        limits: false,
    },
];

/// The four limits, low to high: the name the display uses and the members' common part.
const LIMITS: [(&str, &str); 4] = [
    ("low-low", "LowLow"),
    ("low", "Low"),
    ("high", "High"),
    ("high-high", "HighHigh"),
];

/// The presentation of a value that is a known equipment's physical value, read in the running
/// program.
pub(crate) fn equipment_presentation(
    program: &dyn ProgramRead,
    path: &MemberPath,
) -> Option<EquipmentPresentation> {
    let mut parent = path.clone();
    let PathSegment::Field(field) = parent.segments.pop()? else {
        return None;
    };
    let instance = locate_member(&parent, program)?;
    let contract = contract_of(program, &instance)?;
    if !field.eq_ignore_ascii_case(contract.value) {
        return None;
    }
    let member = |name: &str| member_of(program, &instance, name);
    let number = |name: &str| match member(name) {
        Some(Value::LReal(value)) if value.is_finite() => Some(value),
        Some(Value::Real(value)) if value.is_finite() => Some(f64::from(value)),
        _ => None,
    };
    let unit = match member("Unit") {
        Some(Value::String(unit)) if !unit.trim().is_empty() => Some(unit.to_string()),
        _ => None,
    };
    let low = match contract.range.0 {
        Some(name) => number(name),
        None => Some(0.0),
    };
    let range = low
        .zip(number(contract.range.1))
        .filter(|(low, high)| high > low)
        .map(|(low, high)| [low, high]);
    if !contract.limits {
        return Some(EquipmentPresentation {
            unit,
            range,
            limits: None,
            priorities: BTreeMap::new(),
        });
    }
    // A disabled limit is no alarm limit; the enabled ones must rise from low-low to high-high.
    let enabled: Vec<Option<f64>> = LIMITS
        .iter()
        .map(|(_, name)| {
            matches!(member(&format!("{name}Enabled")), Some(Value::Bool(true)))
                .then(|| number(&format!("{name}Limit")))
                .flatten()
        })
        .collect();
    let present: Vec<f64> = enabled.iter().flatten().copied().collect();
    let limits =
        (!present.is_empty() && present.windows(2).all(|pair| pair[0] <= pair[1])).then(|| {
            HmiLimits {
                low_low: enabled[0],
                low: enabled[1],
                high: enabled[2],
                high_high: enabled[3],
            }
        });
    let priorities = LIMITS
        .into_iter()
        .filter_map(|(shown, name)| {
            let Some(Value::Enum(priority)) = member(&format!("{name}Alarm.Priority")) else {
                return None;
            };
            let priority = priority.variant_name().to_ascii_lowercase();
            matches!(priority.as_str(), "low" | "medium" | "high").then_some((shown, priority))
        })
        .collect();
    Some(EquipmentPresentation {
        unit,
        range,
        limits,
        priorities,
    })
}

/// The contract an instance's block or one of its bases has.
fn contract_of(program: &dyn ProgramRead, instance: &Located) -> Option<&'static Contract> {
    CONTRACTS
        .iter()
        .find(|contract| program.is_a(instance, contract.block))
}

/// The value of the member `name` of the instance at `instance`, a member of a member when `name`
/// is dotted (`HighAlarm.Priority`).
fn member_of(program: &dyn ProgramRead, instance: &Located, name: &str) -> Option<Value> {
    let mut at = *instance;
    for part in name.split('.') {
        at = program.member(&at, part)?;
    }
    program.read_at(&at).ok()
}

/// The source and unit of one of the equipment contract's own limit alarms, at this scan.
pub(crate) fn alarm_value_format(
    program: &dyn ProgramRead,
    equipment: &str,
    alarm: &str,
) -> Option<(String, Option<String>)> {
    let (owner, name) = alarm.rsplit_once('.')?;
    if !owner.eq_ignore_ascii_case(equipment)
        || !LIMITS
            .iter()
            .any(|(_, limit)| name.eq_ignore_ascii_case(&format!("{limit}Alarm")))
    {
        return None;
    }
    let instance = locate_member(&MemberPath::parse(equipment).ok()?, program)?;
    let contract = contract_of(program, &instance).filter(|contract| contract.limits)?;
    let unit = match member_of(program, &instance, "Unit") {
        Some(Value::String(unit)) if !unit.trim().is_empty() => Some(unit.to_string()),
        _ => None,
    };
    Some((format!("{equipment}.{}", contract.value), unit))
}
