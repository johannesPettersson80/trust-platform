//! The number entries of an analysis request, read as the engineer typed them (`controlStudio/analyze`; specification 35, B9.9 and
//! §0.1a). The page sends every entry of an analysis input as the text of its field. A blank entry is absent, so the engine's default
//! applies, but a frequency grid is read whole, so a blank entry of a grid is refused. An entry that is no number, whole number or list
//! of numbers is refused under its page field. The typed request decodes after these entries are read.

use serde_json::{Map, Value};

use crate::failure::{Cause, Failure, NextAction};

/// The refusal of an entry that is not a number.
const NOT_A_NUMBER: &str = "Enter a number.";
/// The refusal of a count that is not a whole number.
const NOT_A_WHOLE_NUMBER: &str = "Enter a whole number.";
/// The refusal of a list with an entry that is not a number.
const NOT_A_LIST: &str = "Enter numbers separated by commas.";

/// The page field of the value of a lead/lag design in lag mode: the low-frequency gain ratio.
const LAG_VALUE_FIELD: &str = "lead-lag-ratio";
/// The page field of the value of a lead/lag design in lead mode: the phase contribution.
const LEAD_VALUE_FIELD: &str = "lead-lag-phase";

/// The entries of an analysis request, by their key in `inputs`, their page field and how they are read.
const ENTRIES: [(&str, &str, Kind); 8] = [
    ("selectedValue", "parameter-value", Kind::Number),
    ("frequencies", "periodic-frequencies", Kind::List),
    ("segmentCount", "periodic-segments", Kind::Count),
    ("minimumCoherence", "periodic-coherence", Kind::Number),
    ("startupSamples", "periodic-startup", Kind::Count),
    ("crossoverRadPerSecond", "lead-lag-crossover", Kind::Number),
    ("initialState", "response-initial", Kind::List),
    ("input", "response-input", Kind::List),
];

/// The entries of a frequency grid, by their key in `grid`, their page field and how they are read.
const GRID_ENTRIES: [(&str, &str, Kind); 3] = [
    ("minimumFrequency", "minimum-frequency", Kind::Number),
    ("maximumFrequency", "maximum-frequency", Kind::Number),
    ("points", "frequency-points", Kind::Count),
];

/// How an entry reads its text.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Kind {
    /// A finite number.
    Number,
    /// A whole number from 0 to the largest `u32`.
    Count,
    /// Finite numbers separated by commas.
    List,
}

/// Reads the number entries of an analysis request (`controlStudio/analyze`): its `inputs`, in place. Text becomes its number, list or
/// count, a blank entry is removed, and a number or a list already in the request is kept. The durations, the connection and the
/// channels are read by their own decoders.
///
/// # Errors
/// An entry the engine cannot read is refused under its page field, with the sentence of what the field takes.
pub fn read_entries(request: &mut Value) -> Result<(), Failure> {
    match request.get_mut("inputs") {
        Some(inputs) => read_inputs(inputs),
        None => Ok(()),
    }
}

/// Reads the number entries of the page's analysis (`controlStudio/page`): the `inputs` of its `analysis`, in place, as
/// [`read_entries`] reads a request's. The page records the entries it can read, so that reopening shows them again (A8.1).
///
/// # Errors
/// An entry the engine cannot read is refused under its page field, with the sentence of what the field takes.
pub fn read_page_entries(request: &mut Value) -> Result<(), Failure> {
    match request
        .get_mut("analysis")
        .and_then(|analysis| analysis.get_mut("inputs"))
    {
        Some(inputs) => read_inputs(inputs),
        None => Ok(()),
    }
}

/// Reads one `inputs` object in place (see [`read_entries`]).
fn read_inputs(inputs: &mut Value) -> Result<(), Failure> {
    let Some(inputs) = inputs.as_object_mut() else {
        return Ok(());
    };
    for (key, field, kind) in ENTRIES {
        read_entry(inputs, key, field, kind, true)?;
    }
    // The value of a lead/lag design is named by its mode, as the page labels it.
    let lag = inputs.get("mode").and_then(Value::as_str) == Some("lag");
    let value_field = if lag {
        LAG_VALUE_FIELD
    } else {
        LEAD_VALUE_FIELD
    };
    read_entry(inputs, "value", value_field, Kind::Number, true)?;
    read_grid(inputs)
}

/// Reads a frequency grid whole, or not at all: when every entry is blank the engine's grid applies, and otherwise each entry is read,
/// so that a blank one is refused under its field.
fn read_grid(inputs: &mut Map<String, Value>) -> Result<(), Failure> {
    let blank = match inputs.get("grid").and_then(Value::as_object) {
        None => return Ok(()),
        Some(grid) => GRID_ENTRIES
            .iter()
            .all(|(key, _, _)| is_blank(grid.get(*key))),
    };
    if blank {
        inputs.remove("grid");
        return Ok(());
    }
    if let Some(grid) = inputs.get_mut("grid").and_then(Value::as_object_mut) {
        for (key, field, kind) in GRID_ENTRIES {
            read_entry(grid, key, field, kind, false)?;
        }
    }
    Ok(())
}

/// Reads the entry `key` of `map` as `kind` reads it. A blank text is removed when `blank_is_absent`, and refused otherwise. A value that
/// is not text, such as a number already in the request, is kept as it is.
fn read_entry(
    map: &mut Map<String, Value>,
    key: &str,
    field: &str,
    kind: Kind,
    blank_is_absent: bool,
) -> Result<(), Failure> {
    let Some(text) = map.get(key).and_then(Value::as_str).map(str::to_owned) else {
        return Ok(());
    };
    let text = text.trim();
    if text.is_empty() {
        if blank_is_absent {
            map.remove(key);
            return Ok(());
        }
        return Err(refused(sentence_of(kind), field));
    }
    let value = value_of(text, kind).ok_or_else(|| refused(sentence_of(kind), field))?;
    map.insert(key.to_owned(), value);
    Ok(())
}

/// The value `text` spells as `kind` reads it, or none when it spells no such value.
fn value_of(text: &str, kind: Kind) -> Option<Value> {
    match kind {
        Kind::Number => finite(text).map(Value::from),
        Kind::Count => text.parse::<u32>().ok().map(Value::from),
        Kind::List => text
            .split(',')
            .map(|item| finite(item.trim()))
            .collect::<Option<Vec<f64>>>()
            .map(Value::from),
    }
}

/// A finite number spelled by `text`.
fn finite(text: &str) -> Option<f64> {
    text.parse::<f64>().ok().filter(|value| value.is_finite())
}

/// The sentence that refuses an entry of `kind`.
fn sentence_of(kind: Kind) -> &'static str {
    match kind {
        Kind::Number => NOT_A_NUMBER,
        Kind::Count => NOT_A_WHOLE_NUMBER,
        Kind::List => NOT_A_LIST,
    }
}

/// Whether an entry is missing, or is text that spells nothing. A number already in the request is not blank.
fn is_blank(entry: Option<&Value>) -> bool {
    match entry {
        None => true,
        Some(value) => value.as_str().is_some_and(|text| text.trim().is_empty()),
    }
}

/// The refusal of the entry of `field`, with the sentence that says what it takes.
fn refused(sentence: &str, field: &str) -> Failure {
    Failure::new(sentence, Cause::Settings, NextAction::None).with_field(field)
}

#[cfg(test)]
#[path = "entries_tests.rs"]
mod tests;
