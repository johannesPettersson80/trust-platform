//! The gain set a campaign can return to (`gainRecoveryContext.ts`; specification 35, B6.4 and C0): the parameter record the
//! campaign started from, and the candidate that Apply wrote. A stored result carries it, so that a Restore after reopening can
//! check that the running record is still the one the campaign wrote (`matchesGainRecovery`), and a stored copy is decoded only
//! when its shape is exactly the one the campaign writes.

use std::collections::BTreeMap;

use schemars::JsonSchema;
use serde::{Deserialize, Serialize};

use super::parameter_status::{same_parameter_values, ParameterStatus};

/// The parameter record a campaign started from (`RecoveryOrigin`, the `TypeScript` `Pick` of the record, annex E7.2): its program,
/// binding and active revisions, and the values in force before Apply.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct RecoveryOrigin {
    /// The program revision the record belongs to.
    pub program_revision: String,
    /// The revision of the parameter binding.
    pub binding_revision: String,
    /// The revision of the values in force before Apply.
    pub active_revision: String,
    /// The values in force before Apply, one per bound parameter.
    pub values: BTreeMap<String, f64>,
}

impl RecoveryOrigin {
    /// The origin a parameter record shows: its program, binding and active revisions, and its values in force.
    #[must_use]
    pub fn of(status: &ParameterStatus) -> Self {
        Self {
            program_revision: status.program_revision.clone(),
            binding_revision: status.binding_revision.clone(),
            active_revision: status.active_revision.clone(),
            values: status.values.clone(),
        }
    }
}

/// The original gain set and the candidate of a campaign (`GainRecoveryContext`, B6.4): what a later Restore returns to, and what
/// Apply wrote. A stored copy is decoded only through [`decode_gain_recovery_context`].
#[allow(clippy::module_name_repetitions)] // The name is the TypeScript record's, and the contract's; it names the module's subject.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct GainRecoveryContext {
    /// The parameter record the campaign started from.
    pub origin: RecoveryOrigin,
    /// The candidate gain set Apply wrote, one value per parameter of the origin.
    pub candidate: BTreeMap<String, f64>,
}

/// Decodes a stored gain-recovery context (`decodeGainRecoveryContext`, B6.4). The context holds exactly its two fields, and the
/// origin exactly its four. Each revision is a text of one to 256 characters, each gain set holds one to 16 parameters with names
/// of one to 256 characters, and the candidate names the origin's parameters and no others. Numbers arrive finite, because
/// [`crate::json`] refuses every other number.
#[must_use]
pub fn decode_gain_recovery_context(value: serde_json::Value) -> Option<GainRecoveryContext> {
    let context: GainRecoveryContext = crate::json::decode_value(value).ok()?;
    let origin = &context.origin;
    let revisions_ok = [
        origin.program_revision.as_str(),
        origin.binding_revision.as_str(),
        origin.active_revision.as_str(),
    ]
    .into_iter()
    .all(text_ok);
    let well_formed = revisions_ok
        && set_ok(&origin.values)
        && set_ok(&context.candidate)
        && origin.values.len() == context.candidate.len()
        && origin
            .values
            .keys()
            .all(|symbol| context.candidate.contains_key(symbol));
    well_formed.then_some(context)
}

/// Whether a text is one to 256 characters long, counted as `JavaScript` counts a string's length (UTF-16 units).
fn text_ok(text: &str) -> bool {
    let length = text.encode_utf16().count();
    (1..=256).contains(&length)
}

/// Whether a gain set holds one to 16 parameters, each named by a text of the allowed length.
fn set_ok(values: &BTreeMap<String, f64>) -> bool {
    (1..=16).contains(&values.len()) && values.keys().all(|symbol| text_ok(symbol))
}

/// Whether the running parameter record is the one a context describes (`matchesGainRecovery`, B6.4): the same program and binding,
/// the previous revision is the origin's active revision and holds the origin's values, and the values in force are the candidate.
/// No context matches nothing.
#[must_use]
pub fn matches_gain_recovery(
    current: &ParameterStatus,
    context: Option<&GainRecoveryContext>,
) -> bool {
    let Some(context) = context else {
        return false;
    };
    current.program_revision == context.origin.program_revision
        && current.binding_revision == context.origin.binding_revision
        && current.previous_revision.as_deref() == Some(context.origin.active_revision.as_str())
        && current
            .previous_values
            .as_ref()
            .is_some_and(|previous| same_parameter_values(previous, &context.origin.values))
        && same_parameter_values(&current.values, &context.candidate)
}

#[cfg(test)]
#[path = "gain_recovery_tests.rs"]
mod tests;
