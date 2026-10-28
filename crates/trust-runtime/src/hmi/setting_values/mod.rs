//! The settings an Engineer changes in the runtime's Settings dialog (specification 39, §21.4;
//! Plant §10): each described by its component definition's `[[settings]]`, shown in its unit and
//! checked against that description and its variable's type before anything is written. The
//! builder's inspector shows settings in the same form: member, title, unit, limits, choices,
//! help, `key`, the kind of value and the value in the setting's unit.

use serde::Serialize;
use serde_json::{json, Value as Json};
use smol_str::SmolStr;

use crate::hmi::commands::plan::matches;
use crate::hmi::commands::write::WriteAction;
use crate::hmi::page_files::{HmiChoice, HmiScalar, HmiSetting};
use crate::hmi::write_value::{json_of, value_for_target_with};
use crate::value::Value;

#[cfg(test)]
mod tests;

/// The kind of value a setting holds, as the dialog edits it.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "kebab-case")]
pub enum SettingKind {
    /// TRUE or FALSE.
    Bool,
    /// A whole number.
    Integer,
    /// A real number.
    Real,
    /// A duration, edited as a number in its unit.
    Time,
    /// Text.
    Text,
    /// Anything else, such as an enumeration, edited by its value's name.
    Other,
}

impl SettingKind {
    /// The kind of a variable's value.
    pub fn of(value: &Value) -> Self {
        match value {
            Value::Bool(_) => Self::Bool,
            Value::SInt(_)
            | Value::Int(_)
            | Value::DInt(_)
            | Value::LInt(_)
            | Value::USInt(_)
            | Value::UInt(_)
            | Value::UDInt(_)
            | Value::ULInt(_)
            | Value::Byte(_)
            | Value::Word(_)
            | Value::DWord(_)
            | Value::LWord(_) => Self::Integer,
            Value::Real(_) | Value::LReal(_) => Self::Real,
            Value::Time(_) | Value::LTime(_) => Self::Time,
            Value::String(_) | Value::WString(_) => Self::Text,
            _ => Self::Other,
        }
    }
}

/// A setting as the dialog shows it: its description with a `$Member` unit resolved, and its
/// variable's value in its unit; a value one of the choices holds is that choice's value.
#[derive(Debug, Clone, PartialEq, Serialize)]
pub struct ShownSetting {
    /// The member, relative to the instance.
    pub member: String,
    /// Its title in plain words.
    pub title: String,
    /// Its unit: a duration's editing unit, or the description's with `$Member` resolved.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub unit: Option<String>,
    /// The smallest value, in its unit.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub min: Option<f64>,
    /// The largest value, in its unit.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub max: Option<f64>,
    /// The step of a value control, in its unit.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub step: Option<f64>,
    /// The allowed values, when there is a short list.
    pub choices: Vec<HmiChoice>,
    /// One line of help.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub help: Option<String>,
    /// Shown in the builder's inspector.
    pub key: bool,
    /// The kind of value it holds.
    pub kind: SettingKind,
    /// The value in its unit; `null` when the variable cannot be read.
    pub value: Json,
}

/// The units a duration is edited in, with their length in nanoseconds; seconds unless the
/// setting names another.
const TIME_UNITS: [(&str, f64); 4] = [("ms", 1.0e6), ("s", 1.0e9), ("min", 6.0e10), ("h", 3.6e12)];

/// The unit a duration setting is edited in, and its length in nanoseconds.
pub fn time_unit(setting: &HmiSetting) -> (&'static str, f64) {
    setting
        .unit
        .as_deref()
        .and_then(|unit| TIME_UNITS.iter().find(|(name, _)| *name == unit))
        .copied()
        .unwrap_or(("s", 1.0e9))
}

/// A setting as the dialog shows it. `unit` is the description's unit with a `$Member` unit
/// resolved to that member's text; `value` the variable's value, `None` when it cannot be read.
pub fn shown_setting(
    setting: &HmiSetting,
    unit: Option<String>,
    value: Option<&Value>,
) -> ShownSetting {
    let kind = value.map_or(SettingKind::Other, SettingKind::of);
    let unit = match kind {
        // A duration with choices is chosen by the choices' titles.
        SettingKind::Time if setting.choices.is_empty() => Some(time_unit(setting).0.to_string()),
        SettingKind::Time => None,
        _ => unit.filter(|unit| !unit.trim().is_empty()),
    };
    ShownSetting {
        member: setting.member.clone(),
        title: setting.title.clone(),
        unit,
        min: setting.min,
        max: setting.max,
        step: setting.step,
        choices: setting.choices.clone(),
        help: setting.help.clone(),
        key: setting.key,
        kind,
        value: value.map_or(Json::Null, |value| shown_value(setting, value)),
    }
}

/// A variable's value as the dialog shows it: the value of the choice that holds it, else a
/// boolean, a number in the setting's unit (a duration in `ms`, `s`, `min` or `h`), or a text (an
/// enumeration's value by its name).
pub fn shown_value(setting: &HmiSetting, value: &Value) -> Json {
    if let Some(choice) = setting
        .choices
        .iter()
        .find(|choice| choice_holds(&choice.value, value))
    {
        return scalar_json(&choice.value);
    }
    match value {
        Value::Time(duration) | Value::LTime(duration) => {
            json!(duration.as_nanos() as f64 / time_unit(setting).1)
        }
        // A REAL keeps its shortest digits: 0.1, not 0.10000000149011612.
        Value::Real(real) => json!(real.to_string().parse::<f64>().unwrap_or(f64::from(*real))),
        other => json_of(other).unwrap_or(Json::Null),
    }
}

/// What an Engineer asks to write to a setting, checked against its description and its
/// variable's type (`current` gives the type, and `values_of` an enumeration's values by its name,
/// as the running program's type table declares them): one of its choices' values when it has
/// choices, else a value of its kind within its limits, a duration in its unit. The answer is the
/// write in the form the variable takes; a refusal names the setting by its title.
///
/// `requested` is the entry as the Settings dialog's field holds it (specification 39, §21.4
/// item 4): a number may come as text, a flag as `true` or `false`, a choice as its value's text.
///
/// # Errors
///
/// The refusal, which begins with the setting's title.
pub fn requested_write(
    setting: &HmiSetting,
    unit: Option<&str>,
    current: &Value,
    requested: &Json,
    values_of: impl FnOnce(&str) -> Option<Vec<(SmolStr, i64)>>,
) -> Result<WriteAction, String> {
    let title = &setting.title;
    let chosen = if setting.choices.is_empty() {
        free_value(setting, unit, current, requested)
    } else {
        setting
            .choices
            .iter()
            .find(|choice| chooses(requested, &choice.value))
            .ok_or_else(|| "choose one of its values".to_string())
            .and_then(|choice| native(&choice.value, current))
    };
    let written = chosen.map_err(|reason| format!("{title}: {reason}"))?;
    value_for_target_with(current, &written, values_of)
        .map_err(|reason| format!("{title}: {reason}"))?;
    Ok(WriteAction::Set(written))
}

/// A value without choices: of the setting's kind, a finite number within its limits in its unit
/// for a number or a duration, whole for an integer; a duration becomes seconds. A refusal states a
/// limit in `unit`, the unit the dialog shows (a `$Member` unit resolved).
fn free_value(
    setting: &HmiSetting,
    unit: Option<&str>,
    current: &Value,
    requested: &Json,
) -> Result<Json, String> {
    let kind = SettingKind::of(current);
    match kind {
        SettingKind::Integer | SettingKind::Real | SettingKind::Time => {}
        SettingKind::Bool => return Ok(entered_flag(requested)),
        SettingKind::Text | SettingKind::Other => return Ok(requested.clone()),
    }
    let (entered, number) =
        entered_number(requested).ok_or_else(|| format!("enter a number, not {requested}"))?;
    if kind == SettingKind::Integer && number.fract() != 0.0 {
        return Err(format!("enter a whole number, not {number}"));
    }
    let unit = match kind {
        SettingKind::Time => Some(time_unit(setting).0.to_string()),
        _ => unit
            .map(str::trim)
            .filter(|unit| !unit.is_empty() && !unit.starts_with('$'))
            .map(str::to_owned),
    };
    let limit = |value: f64| match &unit {
        Some(unit) => format!("{value} {unit}"),
        None => value.to_string(),
    };
    if let Some(min) = setting.min.filter(|min| number < *min) {
        return Err(format!("{number} is below the minimum {}", limit(min)));
    }
    if let Some(max) = setting.max.filter(|max| number > *max) {
        return Err(format!("{number} is above the maximum {}", limit(max)));
    }
    Ok(match kind {
        SettingKind::Time => json!(number * time_unit(setting).1 / 1.0e9),
        _ => entered,
    })
}

/// A number as a field holds it, as JSON and as its value: a finite JSON number, or a text,
/// trimmed, with `,` read as the decimal separator (`"1,5"` is 1.5); a whole number in text keeps
/// all its digits. `None` when it is no finite number.
pub(crate) fn entered_number(requested: &Json) -> Option<(Json, f64)> {
    let entered = match requested {
        Json::Number(_) => requested.clone(),
        Json::String(text) => {
            let text = text.trim().replace(',', ".");
            if let Ok(whole) = text.parse::<i64>() {
                Json::from(whole)
            } else if let Ok(whole) = text.parse::<u64>() {
                Json::from(whole)
            } else {
                Json::Number(serde_json::Number::from_f64(text.parse::<f64>().ok()?)?)
            }
        }
        _ => return None,
    };
    let value = entered.as_f64().filter(|value| value.is_finite())?;
    Some((entered, value))
}

/// A flag as a field holds it: a JSON boolean, or the text `true` or `false` in any case.
/// Anything else stays as it is, for the variable's type to refuse.
fn entered_flag(requested: &Json) -> Json {
    match requested.as_str().map(str::trim) {
        Some(text) if text.eq_ignore_ascii_case("true") => Json::Bool(true),
        Some(text) if text.eq_ignore_ascii_case("false") => Json::Bool(false),
        _ => requested.clone(),
    }
}

/// Whether an entry names a choice: its value, or the text of its value as a list of choices
/// sends it (`"2"` or `"2,0"` for the number 2, `"true"` for TRUE, `"T#1h"` for that text).
fn chooses(requested: &Json, choice: &HmiScalar) -> bool {
    let value = scalar_json(choice);
    if matches(requested, &value) {
        return true;
    }
    match (choice, requested) {
        (HmiScalar::Number(_), Json::String(_)) => {
            entered_number(requested).is_some_and(|(number, _)| matches(&number, &value))
        }
        (HmiScalar::Bool(_), Json::String(_)) => entered_flag(requested) == value,
        _ => false,
    }
}

/// Whether a choice holds a variable's value: the same boolean, number, duration (a choice's text
/// such as `T#1h`), enumeration value or text.
fn choice_holds(choice: &HmiScalar, value: &Value) -> bool {
    match (choice, value) {
        (HmiScalar::Bool(flag), Value::Bool(current)) => flag == current,
        (HmiScalar::Text(text), Value::Time(duration) | Value::LTime(duration)) => {
            duration_nanos(text) == Some(duration.as_nanos())
        }
        (HmiScalar::Text(text), Value::Enum(current)) => {
            current.variant_name().eq_ignore_ascii_case(text)
        }
        (HmiScalar::Text(text), Value::String(current)) => current.as_str() == text.as_str(),
        (HmiScalar::Number(number), _) => json_of(value)
            .filter(Json::is_number)
            .is_some_and(|current| matches(&current, &json!(number))),
        _ => false,
    }
}

/// A choice's value in the form the variable takes: a duration's text (`T#1h`) as seconds.
fn native(choice: &HmiScalar, current: &Value) -> Result<Json, String> {
    match (choice, current) {
        (HmiScalar::Text(text), Value::Time(_) | Value::LTime(_)) => duration_nanos(text)
            .map(|nanos| json!(nanos as f64 / 1.0e9))
            .ok_or_else(|| format!("{text} is not a duration")),
        _ => Ok(scalar_json(choice)),
    }
}

fn scalar_json(value: &HmiScalar) -> Json {
    match value {
        HmiScalar::Bool(flag) => json!(flag),
        HmiScalar::Number(number) => json!(number),
        HmiScalar::Text(text) => json!(text),
    }
}

/// The nanoseconds of an IEC 61131-3 duration literal such as `T#1h30m` or `TIME#500ms`.
pub fn duration_nanos(text: &str) -> Option<i64> {
    let upper = text.trim().to_ascii_uppercase();
    let (prefix, body) = upper.split_once('#')?;
    if !matches!(prefix, "T" | "TIME" | "LT" | "LTIME") {
        return None;
    }
    let (sign, mut rest) = match body.strip_prefix('-') {
        Some(rest) => (-1.0, rest),
        None => (1.0, body.strip_prefix('+').unwrap_or(body)),
    };
    let mut total = 0.0;
    let mut parts = 0;
    while !rest.is_empty() {
        let digits = rest
            .find(|c: char| !(c.is_ascii_digit() || c == '.' || c == '_'))
            .unwrap_or(rest.len());
        let number: f64 = rest[..digits].replace('_', "").parse().ok()?;
        rest = &rest[digits..];
        let letters = rest
            .find(|c: char| !c.is_ascii_alphabetic())
            .unwrap_or(rest.len());
        let nanos = match &rest[..letters] {
            "D" => 8.64e13,
            "H" => 3.6e12,
            "M" => 6.0e10,
            "S" => 1.0e9,
            "MS" => 1.0e6,
            "US" => 1.0e3,
            "NS" => 1.0,
            _ => return None,
        };
        total += number * nanos;
        parts += 1;
        rest = rest[letters..].trim_start_matches('_');
    }
    (parts > 0).then(|| (sign * total).round() as i64)
}
