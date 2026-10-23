//! The merge of one page change into the study's inputs (specification 35, A8.1, A8.4; B4.2 to B4.4). `controlStudio/inputs` takes
//! the change as the page sent it, and this module decides what the change means for the inputs the study records. The keys apply in
//! the page's order: the acquisition settings, the controller family, the tuning preferences, the pinned settings, then the
//! requirements record and its scenarios. A key the study does not take is refused by name, and a key that names no input (the
//! channel mapping, a binding change) changes nothing.
//!
//! The requirements editor sends what the engineer typed (B4.3): the rows of the record, or, for a loop with several controlled
//! outputs, the rows of each output under `outputRequirements`. This module assembles the record from them: the first output's rows are
//! the record's own values, each output's rows are kept under its channel, and the shared values stay beside them. It reads every number
//! given as text as the number it names, a blank optional row declares nothing, and a row that no number reads is refused under its
//! field. The manual settings of the tuning preferences are read the same way, except that an entry that no number reads is left out:
//! it is a value still being typed, and Tune and the preview refuse it under its symbol.

use std::collections::BTreeMap;

use serde_json::{Map, Number, Value};

use crate::failure::{Cause, Failure, NextAction};
use crate::identification::acquisition_settings::AcquisitionSettings;
use crate::json::number_or_text;
use crate::messages::TuneMethod;
use crate::model::general::js_text;
use crate::study::inputs::InputsSection;
use crate::study::tune::state::StateDesignInput;
use crate::tuning::requirements::TuningRequirements;
use crate::tuning::settings_pins::ControllerSettingsPins;
use crate::tuning::state_controller::preferences::StateDesignDraft;
use crate::tuning::state_controller::StateControllerFamily;
use crate::tuning::structure::Structure;
use crate::tuning::texts;
use crate::validation::engineer_verification_plan::duration_as_seconds;
use crate::validation::scenario_form::scenarios_of_form;

/// The key of the acquisition settings of Identify (B3.2).
const ACQUISITION: &str = "acquisitionSettings";
/// The key of the controller family the page chose (B4.4); a state law's family names the state design (C4).
const FAMILY: &str = "family";
/// The key of the tuning method and its values or draft (B4.2).
const PREFERENCES: &str = "tuningPreferences";
/// The key of the pinned settings (B4.2); `null` clears them.
const PINNED: &str = "pinnedValues";
/// The key that marks the requirements record (B4.3): a change with it carries the whole record.
const MARK: &str = "referenceChange";
/// The key of the rows the requirements editor typed for each controlled output of a loop with several (B4.3, C7): a list in the
/// editor's order, each entry the output's `channel` and its rows. A change with it carries the whole record, as a change with
/// [`MARK`] does, and the engine assembles the record from it.
const OUTPUTS: &str = "outputRequirements";
/// The key of the verification scenarios the requirements editor declares (B4.3, B9.9); they belong to the requirements record.
const SCENARIOS: &str = "scenarioDefinitions";
/// The key of the requirements of each controlled output, by the output's model channel (B4.3, C7).
const CHANNEL_REQUIREMENTS: &str = "channelRequirements";
/// The keys that a change may carry without naming an input of the study: the channel mapping (the loop file's channels) and the
/// binding of the setup stage (V6 S9).
const NAMES_NO_INPUT: [&str; 4] = [
    "channelMapping",
    "bindingInvalid",
    "bindingInputs",
    "bindingOutputs",
];
/// The keys of the requirements record (B4.3), as the record's own fields name them, and the requirements of each controlled output.
const REQUIREMENT_KEYS: [&str; 14] = [
    "referenceChange",
    "maximumError",
    "settlingBand",
    "settlingTimeSeconds",
    "durationSeconds",
    "constantInputDisturbance",
    "gainUncertaintyFraction",
    "maximumOvershootPercent",
    "measurementNoiseBand",
    "transferTolerance",
    "tuningObjective",
    "tuningMethod",
    "automaticCriteria",
    CHANNEL_REQUIREMENTS,
];
/// The rows of one controlled output, by the names of the record (B4.3, C7): the keys of an entry of [`OUTPUTS`] besides its `channel`.
const OUTPUT_ROWS: [&str; 5] = [
    "referenceChange",
    "maximumError",
    "settlingBand",
    "settlingTimeSeconds",
    "maximumOvershootPercent",
];
/// The required requirement values that are plain numbers (B4.3): each may be given as the text the engineer typed. The times
/// (`settlingTimeSeconds`, `durationSeconds`) are read as durations by the record.
const REQUIRED_NUMBERS: [&str; 3] = ["referenceChange", "maximumError", "settlingBand"];
/// The optional requirement values that are plain numbers (B4.3): each may be given as the text the engineer typed, and a blank text or
/// `null` declares no value.
const OPTIONAL_NUMBERS: [&str; 5] = [
    "maximumOvershootPercent",
    "constantInputDisturbance",
    "measurementNoiseBand",
    "gainUncertaintyFraction",
    "transferTolerance",
];
/// The refusal of typed outputs that are not a list of distinct channels, each with its rows (B4.3).
const OUTPUTS_INVALID: &str =
    "The requirements of the controlled outputs must name each output's channel once, with its values.";
/// The refusal of a set of settings that are not numbers, one for each symbol (B4.2, B4.5).
const NOT_NUMBERS: &str = "The controller settings must be numbers, one for each symbol.";
/// The refusal of a state design draft that does not hold the text of each field of its form (C4).
const DRAFT_NOT_TEXT: &str = "The state design draft must hold the text of each field of its form.";

/// Applies one page change to the inputs the study records (A8.1): the inputs the change names take the values the page gave, and the
/// others stay as they are. The result is the inputs the study holds after the change. The caller stores them only when they differ
/// from the recorded ones. `loop_revision` is the loop's own revision, which the pinned settings belong to (B4.2).
///
/// # Errors
/// The change names a key that the study does not take, or a requirement without the `referenceChange` (or the `outputRequirements`)
/// that marks its record; a tuning method that the engine does not run; typed outputs that are not a list of distinct channels with
/// their rows; or a value that is not of the shape its input takes.
pub fn apply_change(
    recorded: &InputsSection,
    change: &Map<String, Value>,
    loop_revision: &str,
) -> Result<InputsSection, Failure> {
    check_keys(change)?;
    let mut next = recorded.clone();
    if let Some(value) = change.get(ACQUISITION) {
        next.acquisition = acquisition_of(value)?;
    }
    if let Some(value) = change.get(FAMILY) {
        apply_family(&mut next, value);
    }
    if let Some(value) = change.get(PREFERENCES) {
        apply_preferences(&mut next, value)?;
    }
    if let Some(value) = change.get(PINNED) {
        apply_pins(&mut next, value, loop_revision)?;
    }
    if names_requirements(change) {
        next.requirements = Some(requirements_of(change)?);
        if let Some(value) = change.get(SCENARIOS) {
            next.scenarios = Some(scenarios_of_form(value)?);
        }
    }
    Ok(next)
}

/// Whether a change carries the requirements record (B4.3): it names the record's `referenceChange`, or the rows typed for each output.
fn names_requirements(change: &Map<String, Value>) -> bool {
    change.contains_key(MARK) || change.contains_key(OUTPUTS)
}

/// Refuses a change that names a key the study does not take, and a requirement or a scenario list without the `referenceChange` that
/// marks the requirements record (A8.1). The rows typed for each output mark the record too (B4.3). The keys that name no input are
/// accepted and change nothing.
fn check_keys(change: &Map<String, Value>) -> Result<(), Failure> {
    for key in change.keys() {
        let name = key.as_str();
        let input = [ACQUISITION, FAMILY, PREFERENCES, PINNED].contains(&name)
            || NAMES_NO_INPUT.contains(&name);
        let requirement = REQUIREMENT_KEYS.contains(&name) || name == SCENARIOS || name == OUTPUTS;
        if !input && !requirement {
            return Err(refusal(&format!(
                "The change names an input that the study does not take: {name}."
            )));
        }
        if requirement && !names_requirements(change) {
            return Err(refusal(&format!(
                "{name} belongs to the requirements, which the change marks with {MARK}."
            )));
        }
    }
    Ok(())
}

/// The acquisition settings a change gives (B3.2): `null` clears them, and an object is the page's settings, checked when the section is
/// stored.
fn acquisition_of(value: &Value) -> Result<Option<AcquisitionSettings>, Failure> {
    if value.is_null() {
        return Ok(None);
    }
    serde_json::from_value(value.clone())
        .map(Some)
        .map_err(|error| refusal(&format!("The acquisition settings are not valid: {error}.")))
}

/// The controller family a change names (B4.4): a structure sets the study's structure, and any other word, `automatic` included,
/// clears it. A state law's family also sets the family of the state design (C4).
fn apply_family(next: &mut InputsSection, value: &Value) {
    next.structure = serde_json::from_value::<Structure>(value.clone()).ok();
    if let Ok(family) = serde_json::from_value::<StateControllerFamily>(value.clone()) {
        next.state_design = Some(StateDesignInput {
            family: Some(family),
            ..next.state_design.clone().unwrap_or_default()
        });
    }
}

/// The tuning preferences a change gives (B4.2, C4): the method (Automatic is none), the manual settings with the manual method alone,
/// and the state design's draft with the state-space method alone.
fn apply_preferences(next: &mut InputsSection, value: &Value) -> Result<(), Failure> {
    let preferences = value
        .as_object()
        .ok_or_else(|| refusal(texts::METHOD_NOT_RUN))?;
    let method = method_of(preferences.get("method"))?;
    next.method = method;
    next.settings = match (method, preferences.get("values")) {
        (Some(TuneMethod::Manual), Some(values)) if !values.is_null() => {
            Some(typed_settings_of(values)?)
        }
        _ => None,
    };
    if let (Some(TuneMethod::StateSpace), Some(draft)) =
        (method, preferences.get("stateDesignDraft"))
        && draft.is_object()
    {
        let draft = serde_json::from_value::<StateDesignDraft>(draft.clone())
            .map_err(|_| refusal(DRAFT_NOT_TEXT))?;
        next.state_design = Some(StateDesignInput {
            draft: Some(draft),
            ..next.state_design.clone().unwrap_or_default()
        });
    }
    Ok(())
}

/// The tuning method a page word names (B4.2): `automatic` is none, and each word the engine runs is its method. Any other word, or none,
/// is refused.
fn method_of(word: Option<&Value>) -> Result<Option<TuneMethod>, Failure> {
    match word.and_then(Value::as_str) {
        Some("automatic") => Ok(None),
        Some(word) => serde_json::from_value(Value::String(word.to_owned()))
            .map(Some)
            .map_err(|_| refusal(texts::METHOD_NOT_RUN)),
        None => Err(refusal(texts::METHOD_NOT_RUN)),
    }
}

/// The pinned settings a change gives (B4.2, C0, C4). `null` clears them. In the state-space method they belong to the state design
/// (C4). Otherwise an object takes the family the study holds after this change, and it records nothing when there is none, so the pins
/// it had stay as they are.
fn apply_pins(next: &mut InputsSection, value: &Value, loop_revision: &str) -> Result<(), Failure> {
    if next.method == Some(TuneMethod::StateSpace) {
        let pins = if value.is_null() {
            None
        } else {
            Some(numbers_of(value)?)
        };
        next.state_design = Some(StateDesignInput {
            pins,
            ..next.state_design.clone().unwrap_or_default()
        });
    } else if value.is_null() {
        next.pins = None;
    } else {
        let values = numbers_of(value)?;
        if let Some(family) = next.structure {
            next.pins = Some(ControllerSettingsPins {
                loop_revision: loop_revision.to_owned(),
                family,
                values,
            });
        }
    }
    Ok(())
}

/// The requirements record a change gives (B4.3): the requirement keys it names, decoded as the record the study holds. The rows typed
/// for each output are assembled into the record first ([`assemble_outputs`]), and every number given as text is read as the number it
/// names. The record's own rules are checked when the section is stored.
fn requirements_of(change: &Map<String, Value>) -> Result<TuningRequirements, Failure> {
    let mut record: Map<String, Value> = change
        .iter()
        .filter(|(key, _)| REQUIREMENT_KEYS.contains(&key.as_str()))
        .map(|(key, value)| (key.clone(), value.clone()))
        .collect();
    if let Some(outputs) = change.get(OUTPUTS) {
        assemble_outputs(&mut record, outputs)?;
    }
    read_numbers(&mut record);
    // A row that no number reads is refused under its own field, as the requirements page names it: `row`, or `row:channel` for one of
    // several outputs. The record could only refuse it with the decoder's words and no field.
    if let Some(Value::Object(records)) = record.get_mut(CHANNEL_REQUIREMENTS) {
        for (channel, rows) in records {
            if let Some(rows) = rows.as_object_mut() {
                read_numbers(rows);
                refuse_unreadable_rows(rows, true, Some(channel.as_str()))?;
            }
        }
    }
    refuse_unreadable_rows(&record, true, None)?;
    refuse_unreadable_rows(&record, false, None)?;
    // The page sends the validation duration as the time it typed; the record holds its number of seconds, and a time that no
    // duration reads is refused under the field.
    let seconds = record
        .get("durationSeconds")
        .map(duration_as_seconds)
        .transpose()?;
    if let Some(seconds) = seconds {
        record.insert("durationSeconds".to_owned(), seconds);
    }
    serde_json::from_value(Value::Object(record)).map_err(|error| {
        Failure::new(
            &format!("The requirements are not valid: {error}."),
            Cause::Settings,
            NextAction::OpenRequirements,
        )
    })
}

/// Assembles the requirements record from the rows typed for each controlled output (B4.3, C7). `record` holds the shared values the
/// change names (the validation duration, the load trial, the noise band, the gain uncertainty, the Auto-entry tolerance and the origin of
/// the generated values), and `outputs` is the list of `outputRequirements` in the editor's order. The first output's rows become the
/// record's own values, which are the first output's copy; a loop with several outputs also keeps each output's rows under its channel in
/// `channelRequirements`, and a loop with one keeps none. The rows stay as they were typed: the record reads them and refuses a value under
/// its field.
///
/// # Errors
/// The outputs are not a list of entries that each name a distinct channel and hold only the rows of an output, or the change also gives
/// the rows as the record's own values or `channelRequirements`, which would be a second source for the same rows.
fn assemble_outputs(record: &mut Map<String, Value>, outputs: &Value) -> Result<(), Failure> {
    let invalid = || {
        Failure::new(
            OUTPUTS_INVALID,
            Cause::Settings,
            NextAction::OpenRequirements,
        )
    };
    let entries = outputs
        .as_array()
        .filter(|entries| !entries.is_empty())
        .ok_or_else(invalid)?;
    if record.contains_key(CHANNEL_REQUIREMENTS)
        || OUTPUT_ROWS.iter().any(|row| record.contains_key(*row))
    {
        return Err(invalid());
    }
    let mut typed: Vec<(String, Map<String, Value>)> = Vec::with_capacity(entries.len());
    for entry in entries {
        let mut rows = entry.as_object().cloned().ok_or_else(invalid)?;
        let Some(Value::String(channel)) = rows.remove("channel") else {
            return Err(invalid());
        };
        let repeated = typed.iter().any(|(seen, _)| *seen == channel);
        let unknown = rows.keys().any(|row| !OUTPUT_ROWS.contains(&row.as_str()));
        if channel.is_empty() || repeated || unknown {
            return Err(invalid());
        }
        typed.push((channel, rows));
    }
    record.extend(typed[0].1.clone());
    if typed.len() > 1 {
        let records: Map<String, Value> = typed
            .into_iter()
            .map(|(channel, rows)| (channel, Value::Object(rows)))
            .collect();
        record.insert(CHANNEL_REQUIREMENTS.to_owned(), Value::Object(records));
    }
    Ok(())
}

/// Reads the plain numbers of requirement rows (B4.3): a number given as the text the engineer typed becomes the number it names
/// ([`number_or_text`]), and a blank optional row, `null` included, declares no value. A text that no number reads stays as it is, so the
/// record refuses it.
fn read_numbers(rows: &mut Map<String, Value>) {
    for key in REQUIRED_NUMBERS {
        if let Some(value) = rows.get_mut(key) {
            read_number(value);
        }
    }
    for key in OPTIONAL_NUMBERS {
        if rows.get(key).is_some_and(declares_nothing) {
            rows.remove(key);
        } else if let Some(value) = rows.get_mut(key) {
            read_number(value);
        }
    }
}

/// A text value as the finite number it names; any other value, and a text that no number reads, as it is.
fn read_number(value: &mut Value) {
    if value.is_string()
        && let Some(number) = Number::from_f64(number_or_text(value))
    {
        *value = Value::Number(number);
    }
}

/// Whether an optional row declares no value: `null`, or a text that is blank.
fn declares_nothing(value: &Value) -> bool {
    match value {
        Value::Null => true,
        Value::String(text) => !js_text(text),
        _ => false,
    }
}

/// Refuses a row of plain numbers that no number reads (B4.3), under the row's own field: `row`, or `row:channel` for one of several outputs,
/// the names that the requirements page gives its rows. [`read_numbers`] has turned each entry that a number reads into that number and
/// dropped each blank optional row, so a row that is still no number is a blank required row, or text that is no number. `of_output`
/// chooses the rows of one output's record (the setpoint change, the error limit, the band and the overshoot limit) or the optional rows
/// that the outputs share (the load trial, the noise band, the gain uncertainty and the Auto-entry tolerance).
fn refuse_unreadable_rows(
    rows: &Map<String, Value>,
    of_output: bool,
    channel: Option<&str>,
) -> Result<(), Failure> {
    let numbers = REQUIRED_NUMBERS.into_iter().chain(OPTIONAL_NUMBERS);
    for row in numbers.filter(|row| OUTPUT_ROWS.contains(row) == of_output) {
        if rows.get(row).is_some_and(|value| !value.is_number()) {
            let field =
                channel.map_or_else(|| row.to_owned(), |channel| format!("{row}:{channel}"));
            return Err(Failure::new(
                unreadable_row_sentence(row),
                Cause::Settings,
                NextAction::OpenRequirements,
            )
            .with_field(&field));
        }
    }
    Ok(())
}

/// The sentence that asks for a number in a row that no number reads (B4.3).
fn unreadable_row_sentence(row: &str) -> &'static str {
    match row {
        "referenceChange" => "Enter the setpoint change as a number.",
        "maximumError" => "Enter the error limit as a number.",
        "settlingBand" => "Enter the settling band as a number.",
        "maximumOvershootPercent" => {
            "Enter the overshoot limit as a number, or leave it blank for no limit."
        }
        "constantInputDisturbance" => {
            "Enter the input disturbance as a number, or leave it blank for none."
        }
        "measurementNoiseBand" => "Enter the measurement noise as a number, or leave it blank.",
        "gainUncertaintyFraction" => {
            "Enter the gain uncertainty as a number from 0 to 1, or leave it blank."
        }
        "transferTolerance" => "Enter the Auto-entry tolerance as a number, or leave it blank.",
        _ => "Enter a number.",
    }
}

/// The settings of a page value: each symbol with its number (B4.2, B4.5).
fn numbers_of(value: &Value) -> Result<BTreeMap<String, f64>, Failure> {
    serde_json::from_value(value.clone()).map_err(|_| refusal(NOT_NUMBERS))
}

/// The manual settings of a page value (B4.2, B4.5): each symbol with its entry as the engineer typed it, a number or the text of one. An
/// entry that a number reads is recorded as that number, as Tune and the preview read it ([`number_or_text`]). An entry that no number
/// reads, a blank row or a value still being typed, is no setting yet: it is left out of the inputs, and Tune and the preview refuse it
/// under its symbol.
///
/// # Errors
/// The value is not an object of settings.
fn typed_settings_of(value: &Value) -> Result<BTreeMap<String, f64>, Failure> {
    let entries = value.as_object().ok_or_else(|| refusal(NOT_NUMBERS))?;
    Ok(entries
        .iter()
        .filter_map(|(symbol, entry)| {
            let number = number_or_text(entry);
            number.is_finite().then(|| (symbol.clone(), number))
        })
        .collect())
}

/// A refusal of the change, with the settings cause and no next action.
fn refusal(text: &str) -> Failure {
    Failure::new(text, Cause::Settings, NextAction::None)
}

#[cfg(test)]
#[path = "inputs_change_tests.rs"]
mod tests;
