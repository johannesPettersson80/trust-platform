//! The raw form of Identify's held-step settings (`identificationFormDraft.ts`; specification 35, A8.2 and A8.6; the
//! workflow specification, "Save, reopen and restart"): the text of every field exactly as the engineer typed it,
//! blanks and invalid entries included, kept with the study so that reopening shows the same draft. The draft is
//! decoded, never interpreted: its numbers are read when the engineer checks a recording.

#![allow(clippy::module_name_repetitions)] // `IdentificationFormDraft` is the name the `TypeScript` record has.

use std::collections::BTreeMap;

use schemars::JsonSchema;
use serde::{Deserialize, Serialize};
use serde_json::Value;

use crate::failure::{Cause, Failure, NextAction};
use crate::identification::data::ExcludedInterval;
use crate::identification::review::IdentificationModelSelection;
use crate::identification::sensor_lag_comparison::ModelAdequacy;
use crate::identification::step_response::{
    step_response_settings_problem, SearchGrid, StepResponseSettings,
};

/// The draft format this module reads and writes.
const FORM_DRAFT_SCHEMA_VERSION: u32 = 1;

/// The longest text one field of the draft may hold, in UTF-16 code units as the `TypeScript` counted a string's
/// length.
const MAXIMUM_FIELD_LENGTH: usize = 256;

/// The fields of the held-step settings (numerical annex N2.2 to N2.6), in the order of the form
/// (`IDENTIFICATION_FORM_KEYS`). Every draft holds a text for each of them.
pub const IDENTIFICATION_FORM_KEYS: [&str; 16] = [
    "baselineDurationSeconds",
    "finalDurationSeconds",
    "maximumWindowRange",
    "maximumWindowSlopePerSecond",
    "measurementBound",
    "onsetThreshold",
    "onsetSamples",
    "baselineStep",
    "gainStep",
    "timestampShiftSeconds",
    "delay.minimum",
    "delay.maximum",
    "delay.step",
    "timeConstant.minimum",
    "timeConstant.maximum",
    "timeConstant.step",
];

/// The fields of the sensor-lag comparison (`IDENTIFICATION_COMPARISON_FORM_KEYS`; A8.2). A draft may hold them and
/// keeps them as typed; the sopdt comparison reads them (`held_step_settings_of`).
pub const IDENTIFICATION_COMPARISON_FORM_KEYS: [&str; 5] = [
    "sensorTimeConstant.minimum",
    "sensorTimeConstant.maximum",
    "sensorTimeConstant.step",
    "modelAdequacy.maximumAbsoluteResidual",
    "modelAdequacy.maximumFinalError",
];

/// The raw form of Identify (`IdentificationFormDraft`; A8.2): the requested model, whether the held-step analysis is
/// on, the text of each settings field and the typed bounds of the excluded interval.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct IdentificationFormDraft {
    /// Always 1.
    pub schema_version: u32,
    /// The model the engineer asks for.
    pub model: IdentificationModelSelection,
    /// Whether the held-step analysis is switched on.
    pub held_step: bool,
    /// The text of each field by its key: every key of [`IDENTIFICATION_FORM_KEYS`], and any key of
    /// [`IDENTIFICATION_COMPARISON_FORM_KEYS`].
    pub values: BTreeMap<String, String>,
    /// The start of the excluded interval, as typed.
    pub excluded_start: String,
    /// The end of the excluded interval, as typed.
    pub excluded_end: String,
}

/// Decodes a saved form draft (`decodeIdentificationFormDraft`; A8.6), or `None` when it breaks a rule: schema version
/// 1, a known model, no unknown key, a text for every field of [`IDENTIFICATION_FORM_KEYS`], no field outside the two
/// key lists, and no text longer than 256 UTF-16 code units, the bounds of the excluded interval included.
#[must_use]
pub fn decode_identification_form_draft(value: &Value) -> Option<IdentificationFormDraft> {
    let draft = IdentificationFormDraft::deserialize(value).ok()?;
    valid_form_draft(&draft).then_some(draft)
}

/// Whether a typed draft keeps the rules of [`decode_identification_form_draft`] that its type does not already hold:
/// the schema version, the key lists and the length of every text.
#[must_use]
pub fn valid_form_draft(draft: &IdentificationFormDraft) -> bool {
    draft.schema_version == FORM_DRAFT_SCHEMA_VERSION
        && draft
            .values
            .iter()
            .all(|(key, text)| known_key(key) && fits(text))
        && IDENTIFICATION_FORM_KEYS
            .iter()
            .all(|key| draft.values.get(*key).map(String::as_str).is_some_and(fits))
        && fits(&draft.excluded_start)
        && fits(&draft.excluded_end)
}

/// Whether `key` names a field of the held-step settings or of the sensor-lag comparison.
fn known_key(key: &str) -> bool {
    IDENTIFICATION_FORM_KEYS.contains(&key) || IDENTIFICATION_COMPARISON_FORM_KEYS.contains(&key)
}

/// Whether a text fits one field: at most 256 UTF-16 code units, as JavaScript counts a string's length.
fn fits(text: &str) -> bool {
    text.encode_utf16().count() <= MAXIMUM_FIELD_LENGTH
}

/// The refusal of an exclusion interval that is blank, not a number, starts before zero or does not end after its start.
const EXCLUSION_REFUSAL: &str =
    "Enter an exclusion interval with a nonnegative start and an end strictly after it.";

/// A refusal of an exclusion interval, naming the form field it refuses (`excludedStart` or `excludedEnd`).
fn exclusion_failure(field: &str) -> Failure {
    Failure::new(EXCLUSION_REFUSAL, Cause::Settings, NextAction::None).with_field(field)
}

/// A form with nothing typed: the automatic model, every field blank, and the held-step analysis off. [`default_form`] is the form
/// a new identification starts with.
#[must_use]
pub fn blank_form() -> IdentificationFormDraft {
    IdentificationFormDraft {
        schema_version: 1,
        model: IdentificationModelSelection::Automatic,
        held_step: false,
        values: IDENTIFICATION_FORM_KEYS
            .iter()
            .map(|key| ((*key).to_string(), String::new()))
            .collect(),
        excluded_start: String::new(),
        excluded_end: String::new(),
    }
}

/// The numbers a new identification's form starts from, by field (B3.3): the text of every key of [`IDENTIFICATION_FORM_KEYS`] and
/// [`IDENTIFICATION_COMPARISON_FORM_KEYS`], in process units. The fixed windows are 5 s, the window limits and the lattice steps
/// are those of the held-step analysis (N2.2 to N2.6), the delay grid runs from 0 to 2 s and the time-constant grid from 0.1 to
/// 20 s in steps of 0.1 s, the additional lag from 0.1 to 10 s, and the model error limits are 0.2 and 0.1.
const FORM_DEFAULTS: [(&str, &str); 21] = [
    ("baselineDurationSeconds", "5"),
    ("finalDurationSeconds", "5"),
    ("maximumWindowRange", "0.1"),
    ("maximumWindowSlopePerSecond", "0.01"),
    ("measurementBound", "0.05"),
    ("onsetThreshold", "0.15"),
    ("onsetSamples", "3"),
    ("baselineStep", "0.01"),
    ("gainStep", "0.001"),
    ("timestampShiftSeconds", "0"),
    ("delay.minimum", "0"),
    ("delay.maximum", "2"),
    ("delay.step", "0.1"),
    ("timeConstant.minimum", "0.1"),
    ("timeConstant.maximum", "20"),
    ("timeConstant.step", "0.1"),
    ("sensorTimeConstant.minimum", "0.1"),
    ("sensorTimeConstant.maximum", "10"),
    ("sensorTimeConstant.step", "0.1"),
    ("modelAdequacy.maximumAbsoluteResidual", "0.2"),
    ("modelAdequacy.maximumFinalError", "0.1"),
];

/// The form a new identification starts with (B3.3): the automatic model, the held-step analysis off, no excluded interval, and
/// every field at its default number. The answer to the Recorded-data fit carries it until the engineer changes a field, and the
/// page shows exactly these numbers; it keeps none of its own.
#[must_use]
pub fn default_form() -> IdentificationFormDraft {
    let mut form = blank_form();
    for (key, text) in FORM_DEFAULTS {
        form.values.insert(key.to_string(), text.to_string());
    }
    form
}

/// The held-step settings the engineer typed, read as the Identify page read them (`readStepResponseSettings`): `None` while
/// the held-step analysis is off. The sensor-lag comparison's fields (`sensorTimeConstant`, `modelAdequacy`) are read and checked
/// only for the sopdt form, as the page reads them; the other forms carry none. The first problem of the settings is the refusal,
/// written as the page wrote it: the field, a colon, then the message.
///
/// # Errors
/// The settings break the admission rules of `step_response_settings_problem`.
pub fn held_step_settings_of(
    draft: &IdentificationFormDraft,
) -> Result<Option<StepResponseSettings>, Failure> {
    if !draft.held_step {
        return Ok(None);
    }
    let number = |key: &str| page_number(draft.values.get(key).map_or("", String::as_str));
    let grid = |group: &str| SearchGrid {
        minimum: number(format!("{group}.minimum").as_str()),
        maximum: number(format!("{group}.maximum").as_str()),
        step: number(format!("{group}.step").as_str()),
    };
    let comparison = draft.model == IdentificationModelSelection::Sopdt;
    let settings = StepResponseSettings {
        baseline_duration_seconds: number("baselineDurationSeconds"),
        final_duration_seconds: number("finalDurationSeconds"),
        maximum_window_range: number("maximumWindowRange"),
        maximum_window_slope_per_second: number("maximumWindowSlopePerSecond"),
        measurement_bound: number("measurementBound"),
        onset_threshold: number("onsetThreshold"),
        onset_samples: onset_samples_of(number("onsetSamples")),
        delay: grid("delay"),
        time_constant: grid("timeConstant"),
        baseline_step: number("baselineStep"),
        gain_step: number("gainStep"),
        timestamp_shift_seconds: number("timestampShiftSeconds"),
        sensor_time_constant: comparison.then(|| grid("sensorTimeConstant")),
        model_adequacy: comparison.then(|| ModelAdequacy {
            maximum_absolute_residual: number("modelAdequacy.maximumAbsoluteResidual"),
            maximum_final_error: number("modelAdequacy.maximumFinalError"),
        }),
    };
    match step_response_settings_problem(&settings) {
        Some(problem) => Err(Failure::new(
            &format!("{}: {}", problem.field, problem.message),
            Cause::Settings,
            NextAction::None,
        )
        .with_field(&problem.field)),
        None => Ok(Some(settings)),
    }
}

/// The refusal of a held step whose model the held step does not allow; the form's `model` field names it (N2.3, N2.3a).
const HELD_STEP_MODEL_REFUSAL: &str =
    "A held step fits a first-order-with-delay, an integrating or a two-lag model.";

/// Whether a held step allows `model`: the first-order-with-delay and two-lag forms, which the bounded step analysis fits (N2.4,
/// N2.10), and the integrating form, which has no fraction and is checked by the integrator rule at its fit (N2.3, N2.3a).
#[must_use]
pub fn held_step_allows(model: IdentificationModelSelection) -> bool {
    matches!(
        model,
        IdentificationModelSelection::Fopdt
            | IdentificationModelSelection::Integrating
            | IdentificationModelSelection::Sopdt
    )
}

/// Refuses a held step whose model it does not allow, naming the `model` field, before anything is stored. A form without a held
/// step allows every model.
///
/// # Errors
/// The form is a held step, and its model is not one that a held step allows.
pub fn check_held_step_model(draft: &IdentificationFormDraft) -> Result<(), Failure> {
    if draft.held_step && !held_step_allows(draft.model) {
        return Err(
            Failure::new(HELD_STEP_MODEL_REFUSAL, Cause::Settings, NextAction::None)
                .with_field("model"),
        );
    }
    Ok(())
}

/// The excluded interval the engineer typed, read as the Identify page read it (`readExcludedIntervals`): none while the
/// held-step analysis is on or both fields are blank, and otherwise one interval that starts at zero or later and ends after
/// its start.
///
/// # Errors
/// A field is blank or not a number, the start is negative, or the end is not after the start.
pub fn excluded_intervals_of(
    draft: &IdentificationFormDraft,
) -> Result<Vec<ExcludedInterval>, Failure> {
    if draft.held_step {
        return Ok(Vec::new());
    }
    let start_text = draft.excluded_start.trim();
    let end_text = draft.excluded_end.trim();
    if start_text.is_empty() && end_text.is_empty() {
        return Ok(Vec::new());
    }
    let start = page_number(start_text);
    let end = page_number(end_text);
    if start_text.is_empty() || !start.is_finite() || start < 0.0 {
        return Err(exclusion_failure("excludedStart"));
    }
    if end_text.is_empty() || !end.is_finite() || end <= start {
        return Err(exclusion_failure("excludedEnd"));
    }
    Ok(vec![ExcludedInterval {
        start_seconds: start,
        end_seconds: end,
    }])
}

/// A number as the Identify page read it (`Number` of the trimmed text): a blank reads as zero, and text that is not a number
/// reads as NaN, which the settings rules refuse.
fn page_number(text: &str) -> f64 {
    let text = text.trim();
    if text.is_empty() {
        0.0
    } else {
        text.parse::<f64>().unwrap_or(f64::NAN)
    }
}

/// The onset count a number names: a whole number from 1 to 100. Any other number becomes 0, which the settings rules refuse
/// at the field of the onset count (`onsetSamples`), in the order the rules check the fields.
#[allow(clippy::float_cmp)] // An exact whole number is what the rule admits.
fn onset_samples_of(value: f64) -> usize {
    (1..=100_usize)
        .find(|count| count_as_f64(*count) == value)
        .unwrap_or(0)
}

/// A count of at most 100 as a number, which is exact.
#[allow(clippy::cast_precision_loss)]
fn count_as_f64(count: usize) -> f64 {
    count as f64
}

#[cfg(test)]
#[path = "form_draft_tests.rs"]
mod tests;
