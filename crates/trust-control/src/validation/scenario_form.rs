//! The verification scenarios as the requirements editor sends them (specification 35, B4.3 and B9.9). The page sends what the engineer
//! typed: each number and each name as text, and the initial values as rows of a signal and a value, so that a repeated signal is seen.
//! This module reads that form and refuses the first rule the list breaks, under the field `scenarios`. A number that no finite number
//! reads is refused, and a number given as a JSON number is read as before.
//!
//! A scenario whose name is blank takes the first identity `scenario-N` that no other scenario of the list holds, so the page adds a
//! scenario with no name and chooses none. The text the engineer typed into a scenario's advanced fields travels in its `advanced` key
//! and is read here: it is the JSON object of the scenario's other fields, which it replaces.

use std::collections::BTreeSet;

use serde_json::{Map, Value};

use crate::failure::Failure;
use crate::json::{decode_value, number_or_text, parse_text};
use crate::validation::engineer_scenario_effects::requirements_failure;
use crate::validation::scenario_contract::EngineerScenario;

/// The key of the text the engineer typed into a scenario's advanced fields.
const ADVANCED: &str = "advanced";
/// The fields of a scenario that the form edits itself: the advanced text holds the others.
const EDITED_FIELDS: [&str; 3] = ["id", "initialValues", "events"];
/// The refusal of a list that is not a list.
const NOT_A_LIST: &str = "The verification scenarios are not a list.";
/// The refusal of a scenario that is not an object.
const NOT_A_SCENARIO: &str = "A verification scenario is not an object.";
/// The refusal of advanced fields that are not a JSON object.
const ADVANCED_NOT_AN_OBJECT: &str = "Advanced fields must be a valid JSON object.";
/// The refusal of advanced fields that hold a field the form edits itself.
const ADVANCED_EDITED_ABOVE: &str = "Advanced fields hold the scenario's other fields; its name, initial values and timed changes are edited above them.";
/// The refusal of two scenarios with one name.
const NAMES_UNIQUE: &str = "Scenario names must be unique.";
/// The refusal of a signal without a name.
const NO_SIGNAL: &str = "Enter a signal name.";
/// The refusal of a number that no finite number reads.
const NOT_FINITE: &str = "Enter a finite number.";
/// The refusal of a timed change before its run starts.
const BEFORE_START: &str = "A scenario event starts at zero or later.";

/// The verification scenarios of a change, read from the form that the requirements editor sends (B4.3, B9.9).
///
/// # Errors
/// The first rule the list breaks, under the field `scenarios`: the list is not a list; a scenario is not an object or repeats a name;
/// its advanced fields are not a JSON object, or hold its name, initial values or timed changes; an initial value or a timed change
/// names no signal, or an initial value repeats one; a number is not finite; or a timed change starts before zero.
pub fn scenarios_of_form(value: &Value) -> Result<Vec<EngineerScenario>, Failure> {
    let Value::Array(items) = value else {
        return Err(refused(NOT_A_LIST));
    };
    let mut names = BTreeSet::new();
    let mut scenarios = items
        .iter()
        .map(|item| scenario_of_form(item, &mut names))
        .collect::<Result<Vec<EngineerScenario>, Failure>>()?;
    name_unnamed(&mut scenarios, &mut names);
    Ok(scenarios)
}

/// Gives each scenario whose name is blank the first identity `scenario-N` that no other scenario of the list holds, in the list's order
/// (B4.3). `names` holds the names the scenarios of the list were given.
fn name_unnamed(scenarios: &mut [EngineerScenario], names: &mut BTreeSet<String>) {
    let mut number = 1_usize;
    for scenario in scenarios
        .iter_mut()
        .filter(|scenario| scenario.id.is_empty())
    {
        while !names.insert(format!("scenario-{number}")) {
            number += 1;
        }
        scenario.id = format!("scenario-{number}");
        number += 1;
    }
}

/// The fields of a scenario with its advanced text applied (B4.3). The text is the JSON object of the scenario's other fields, its
/// requirements, effects and model, which it replaces; the name, the initial values and the timed changes stay as the form gave them.
fn with_advanced(sent: &Map<String, Value>) -> Result<Map<String, Value>, Failure> {
    let mut fields = sent.clone();
    let typed = match fields.remove(ADVANCED) {
        None | Some(Value::Null) => return Ok(fields),
        Some(Value::String(text)) => parse_text(&text).ok(),
        Some(_) => None,
    };
    let Some(Value::Object(typed)) = typed else {
        return Err(refused(ADVANCED_NOT_AN_OBJECT));
    };
    if EDITED_FIELDS.iter().any(|key| typed.contains_key(*key)) {
        return Err(refused(ADVANCED_EDITED_ABOVE));
    }
    fields.retain(|key, _| EDITED_FIELDS.contains(&key.as_str()));
    fields.extend(typed);
    Ok(fields)
}

/// One scenario of the form: its advanced text applied, its name trimmed and unique (a blank name is given by `name_unnamed`), its
/// initial values and timed changes read, and its bounds read; the rest of it is the study's own form, which the scenario's type reads.
fn scenario_of_form(
    item: &Value,
    names: &mut BTreeSet<String>,
) -> Result<EngineerScenario, Failure> {
    let Value::Object(sent) = item else {
        return Err(refused(NOT_A_SCENARIO));
    };
    let fields = with_advanced(sent)?;
    let name = match fields.get("id") {
        Some(Value::String(text)) => text.trim().to_owned(),
        _ => String::new(),
    };
    if !name.is_empty() && !names.insert(name.clone()) {
        return Err(refused(NAMES_UNIQUE));
    }
    let mut form = fields.clone();
    form.insert("id".to_owned(), Value::String(name));
    if let Some(initial) = fields.get("initialValues") {
        form.insert("initialValues".to_owned(), initial_values_of(initial)?);
    }
    if let Some(events) = fields.get("events") {
        form.insert("events".to_owned(), events_of(events)?);
    }
    if let Some(requirements) = fields.get("requirements") {
        form.insert("requirements".to_owned(), requirements_of(requirements)?);
    }
    decode_value::<EngineerScenario>(Value::Object(form)).map_err(|error| {
        refused(&format!(
            "The verification scenarios are not valid: {error}."
        ))
    })
}

/// The initial values of a scenario: rows of a signal and a value, as the editor sends them, or the object of signals and values that
/// the study keeps. A signal is named once, and its value is a finite number.
fn initial_values_of(value: &Value) -> Result<Value, Failure> {
    let rows: Vec<(String, Value)> = match value {
        Value::Object(entries) => entries
            .iter()
            .map(|(symbol, value)| (symbol.clone(), value.clone()))
            .collect(),
        Value::Array(rows) => rows
            .iter()
            .map(row_of)
            .collect::<Result<Vec<(String, Value)>, Failure>>()?,
        _ => return Err(refused(NOT_A_LIST)),
    };
    let mut seen = BTreeSet::new();
    let mut values = Map::new();
    for (symbol, value) in rows {
        let symbol = symbol.trim().to_owned();
        if symbol.is_empty() {
            return Err(refused(NO_SIGNAL));
        }
        if !seen.insert(symbol.clone()) {
            return Err(refused(&format!(
                "Signal {symbol} already has an initial value."
            )));
        }
        values.insert(symbol, number_value(&value)?);
    }
    Ok(Value::Object(values))
}

/// One initial value's row: its signal (text, possibly blank) and its value.
fn row_of(item: &Value) -> Result<(String, Value), Failure> {
    let Value::Object(fields) = item else {
        return Err(refused(NO_SIGNAL));
    };
    let symbol = match fields.get("symbol") {
        Some(Value::String(text)) => text.clone(),
        _ => String::new(),
    };
    Ok((symbol, field(fields, "value")))
}

/// The timed changes of a scenario: each names its signal, and each time and value is a finite number, the time from zero.
fn events_of(value: &Value) -> Result<Value, Failure> {
    let Value::Array(events) = value else {
        return Ok(value.clone());
    };
    events
        .iter()
        .map(event_of)
        .collect::<Result<Vec<Value>, Failure>>()
        .map(Value::Array)
}

/// One timed change, with its time and value read as numbers and its signal trimmed.
fn event_of(item: &Value) -> Result<Value, Failure> {
    let Value::Object(fields) = item else {
        return Ok(item.clone());
    };
    let at_ms = number_value(&field(fields, "atMs"))?;
    if at_ms.as_f64().is_some_and(|time| time < 0.0) {
        return Err(refused(BEFORE_START));
    }
    let symbol = name_of(fields)?;
    let mut form = fields.clone();
    form.insert("atMs".to_owned(), at_ms);
    form.insert("symbol".to_owned(), Value::String(symbol));
    form.insert("value".to_owned(), number_of(fields, "value")?);
    Ok(Value::Object(form))
}

/// The requirements of a scenario: a bounds check's limits are read as numbers; every other check is kept as it is.
fn requirements_of(value: &Value) -> Result<Value, Failure> {
    let Value::Array(requirements) = value else {
        return Ok(value.clone());
    };
    requirements
        .iter()
        .map(requirement_of)
        .collect::<Result<Vec<Value>, Failure>>()
        .map(Value::Array)
}

/// One requirement: a bounds check reads its minimum and its maximum as numbers.
fn requirement_of(item: &Value) -> Result<Value, Failure> {
    let Value::Object(fields) = item else {
        return Ok(item.clone());
    };
    if !matches!(fields.get("kind"), Some(Value::String(kind)) if kind == "bounds") {
        return Ok(item.clone());
    }
    let mut form = fields.clone();
    form.insert("minimum".to_owned(), number_of(fields, "minimum")?);
    form.insert("maximum".to_owned(), number_of(fields, "maximum")?);
    Ok(Value::Object(form))
}

/// The signal named by a timed change: its text, trimmed, which must not be blank.
fn name_of(fields: &Map<String, Value>) -> Result<String, Failure> {
    match fields.get("symbol") {
        Some(Value::String(text)) if !text.trim().is_empty() => Ok(text.trim().to_owned()),
        _ => Err(refused(NO_SIGNAL)),
    }
}

/// The field of an object, or null when the object does not hold it, so that the number it reads refuses.
fn field(fields: &Map<String, Value>, key: &str) -> Value {
    fields.get(key).cloned().unwrap_or(Value::Null)
}

/// The number that a field of an object holds, as text or as a number (`number_or_text`).
fn number_of(fields: &Map<String, Value>, key: &str) -> Result<Value, Failure> {
    number_value(&field(fields, key))
}

/// A number as a JSON number: the number that the value spells, or a refusal when no finite number reads.
fn number_value(value: &Value) -> Result<Value, Failure> {
    serde_json::Number::from_f64(number_or_text(value))
        .map(Value::Number)
        .ok_or_else(|| refused(NOT_FINITE))
}

/// The refusal of a scenario list, under the field `scenarios` that the page shows the list's refusals under.
fn refused(sentence: &str) -> Failure {
    requirements_failure(sentence).with_field("scenarios")
}

#[cfg(test)]
#[path = "scenario_form_tests.rs"]
mod tests;
