//! The raw form of Identify's held-step settings (`identificationFormDraft.ts`; specification 35, A8.2 and A8.6; the
//! workflow specification, "Save, reopen and restart"): the text of every field exactly as the engineer typed it,
//! blanks and invalid entries included, kept with the study so that reopening shows the same draft. The draft is
//! decoded, never interpreted: its numbers are read when the engineer checks a recording.

#![allow(clippy::module_name_repetitions)] // `IdentificationFormDraft` is the name the `TypeScript` record has.

use std::collections::BTreeMap;

use serde::{Deserialize, Serialize};
use serde_json::Value;

use crate::identification::review::IdentificationModelSelection;

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
/// keeps them as typed; the comparison that reads them is parked with stage V7.
pub const IDENTIFICATION_COMPARISON_FORM_KEYS: [&str; 5] = [
    "sensorTimeConstant.minimum",
    "sensorTimeConstant.maximum",
    "sensorTimeConstant.step",
    "modelAdequacy.maximumAbsoluteResidual",
    "modelAdequacy.maximumFinalError",
];

/// The raw form of Identify (`IdentificationFormDraft`; A8.2): the requested model, whether the held-step analysis is
/// on, the text of each settings field and the typed bounds of the excluded interval.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
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
pub(crate) fn valid_form_draft(draft: &IdentificationFormDraft) -> bool {
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

#[cfg(test)]
#[path = "form_draft_tests.rs"]
mod tests;
