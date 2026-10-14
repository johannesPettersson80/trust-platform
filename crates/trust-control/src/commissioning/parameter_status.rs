// From parameterApplication.ts; moves with the parameter layer.
//! The parameter record of a controller as the runtime reports it (`control.parameters.status`), with the two
//! equalities that Restore and gain recovery use (specification 35, B6.4; annex E7). Only the record and the
//! comparisons are here: the review and transaction client is transport.

use std::collections::BTreeMap;

use schemars::JsonSchema;
use serde::{Deserialize, Serialize};

/// The state of the last parameter transaction, as the runtime reports it (annex E7.2).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "lowercase")]
pub enum RecordState {
    /// No transaction is pending or retained.
    Idle,
    /// A transaction is waiting for the task boundary.
    Pending,
    /// The reviewed values are in force.
    Applied,
    /// The runtime refused the transaction.
    Rejected,
}

/// A controller's parameter record and its revisions (`ParameterStatus`, annex E7.2).
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, JsonSchema)]
pub struct ParameterStatus {
    /// Whether the runtime keeps request identities for parameter operations.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub supports_request_id: Option<bool>,
    /// The identifier of the last transaction, when the runtime keeps one.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub transaction_id: Option<String>,
    /// The revision of the program the record belongs to.
    pub program_revision: String,
    /// The revision of the parameter binding.
    pub binding_revision: String,
    /// The revision of the values in force.
    pub active_revision: String,
    /// The values in force, one per bound parameter.
    pub values: BTreeMap<String, f64>,
    /// The state of the last transaction.
    pub status: RecordState,
    /// The revision that was in force before the last transaction, when there is one.
    pub previous_revision: Option<String>,
    /// The values that were in force before the last transaction, when there are some.
    pub previous_values: Option<BTreeMap<String, f64>>,
    /// Why the last transaction was refused, when it was.
    pub reason: Option<String>,
}

/// Whether two parameter value sets hold the same parameters with the same values (`sameParameterValues`). The values
/// are compared exactly: the runtime reports the values it holds, and Restore must write back the same ones.
#[must_use]
pub fn same_parameter_values(left: &BTreeMap<String, f64>, right: &BTreeMap<String, f64>) -> bool {
    left.len() == right.len()
        && left
            .iter()
            .all(|(symbol, value)| right.get(symbol) == Some(value))
}

/// Whether the runtime's current parameter record is the one a saved status describes, down to its revisions and values
/// (`sameParameterHistory`). A historical result selects a recovery route only while the native value history still
/// matches (B6.4).
#[must_use]
pub fn same_parameter_history(current: &ParameterStatus, saved: Option<&ParameterStatus>) -> bool {
    let Some(saved) = saved else {
        return false;
    };
    current.program_revision == saved.program_revision
        && current.binding_revision == saved.binding_revision
        && current.active_revision == saved.active_revision
        && current.previous_revision == saved.previous_revision
        && same_parameter_values(&current.values, &saved.values)
        && match (&current.previous_values, &saved.previous_values) {
            (None, None) => true,
            (Some(current_previous), Some(saved_previous)) => {
                same_parameter_values(current_previous, saved_previous)
            }
            _ => false,
        }
}

#[cfg(test)]
#[path = "parameter_status_tests.rs"]
mod tests;
