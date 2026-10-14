//! The records that Apply and Restore answer (specification 35, §0.1b, B6.1 to B6.5; annex E11.8): the review that a
//! confirmation approves, its identity, and the outcome of each operation. A method without a review identity answers the
//! review and writes nothing; with one, the operation runs only while the identity recomputed from the live runtime is the one
//! the review gave (§0.1b Confirmation). The review replaces the `review` reply of engineerActions/application.ts (`rollback`,
//! `previous`, `next` and `reason`). The records are plain data: the engine reads the runtime, builds them and answers them.

use std::collections::BTreeMap;

use schemars::JsonSchema;
use serde::{Deserialize, Serialize};
use serde_json::{json, Value};
use trust_control_math::model::OperatingPoint;

use super::commissioning_result::CommissioningResult;
use super::found_mode::FoundMode;
use super::parameter_status::ParameterStatus;
use super::reviewed_application::ApplicationAction;
use crate::identification::independent_validation::digest;
use crate::loop_definition::LoopDefinition;
use crate::recording::RecordedRun;

/// What a review is for (§0.1b): an Apply (B6.1) or a Restore of the previous settings (B6.4). It is the answered form of
/// `ApplicationAction`, which has no serde form, and serializes to that action's word.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "lowercase")]
pub enum ReviewedAction {
    /// Apply the validated candidate (B6.1).
    Apply,
    /// Restore the previous settings (B6.4).
    Restore,
}

impl From<ApplicationAction> for ReviewedAction {
    fn from(action: ApplicationAction) -> Self {
        match action {
            ApplicationAction::Apply => Self::Apply,
            ApplicationAction::Restore => Self::Restore,
        }
    }
}

/// One parameter of a review (§0.1b `apply`: each parameter's running and candidate value): its symbol and unit as the
/// parameter binding declares them (E7.1), the value it holds in the running PLC, and the value the confirmation writes.
#[derive(Debug, Clone, PartialEq, Serialize, JsonSchema)]
#[serde(rename_all = "camelCase")]
pub struct ReviewedParameter {
    /// The parameter's symbol.
    pub symbol: String,
    /// The parameter's unit.
    pub unit: String,
    /// The value in force now, as the runtime reports it (E7.2).
    pub running: f64,
    /// The value the confirmation writes: the candidate's for an Apply, and for a Restore the value from before Apply that the
    /// checkpoint keeps (E7.5).
    pub next: f64,
}

/// The validated operating point that a review shows (§0.1b `apply`; B6.2): the plant inputs and measured outputs the
/// candidate was validated at, one value per model input and output. It is the study model's own operating point record
/// (`operatingPoint` of the model), so a review copies it as it is.
pub type ReviewedOperatingPoint = OperatingPoint;

/// The review of an Apply or a Restore (§0.1b; B6.1, B6.4): what the confirmation would do, read from the live runtime without
/// writing anything. The confirmation names `identity`, and it is refused when the identity recomputed then differs (E11.8).
#[derive(Debug, Clone, PartialEq, Serialize, JsonSchema)]
#[serde(rename_all = "camelCase")]
pub struct ApplicationReview {
    /// What the review is for.
    pub action: ReviewedAction,
    /// The review identity that the confirmation names (`review_identity`).
    pub identity: String,
    /// The mode the loop was found in: the mode it is in for an Apply, and the mode before Apply that a Restore returns it to
    /// (A6.2); `None` when it is unknown.
    pub found_mode: Option<FoundMode>,
    /// Each bound parameter with its running value and the value the confirmation writes.
    pub parameters: Vec<ReviewedParameter>,
    /// The sentence that describes the reviewed operation (A11.1).
    pub description: String,
    /// Whether the target is physical, so that the confirmation needs the engineer's approval (A2.4, A2.5).
    pub physical: bool,
    /// The operating point the candidate was validated at (B6.2); `None` when the review has none to show.
    pub operating_point: Option<ReviewedOperatingPoint>,
    /// What one physical approval covers (A2.5, B9.3), filled in Rust for the page to show; a simulated target shows it too.
    pub approval: ApprovalBlock,
}

/// One signal that a reviewed operation writes, as its approval shows it (A2.5, B9.3): the value in force, the value written, and the
/// limits the parameter's declared range gives (`minimum` and `maximum`, the range Tune may choose within).
#[derive(Debug, Clone, PartialEq, Serialize, JsonSchema)]
#[serde(rename_all = "camelCase")]
pub struct ApprovedSignal {
    /// The signal's symbol.
    pub symbol: String,
    /// The signal's unit.
    pub unit: String,
    /// The value in force now.
    pub running: f64,
    /// The value the confirmation writes.
    pub next: f64,
    /// The smallest value the signal may take; not a number when the loop declares no range for it.
    pub minimum: f64,
    /// The largest value the signal may take; not a number when the loop declares no range for it.
    pub maximum: f64,
}

/// What one physical approval covers (A2.5, B9.3): the loop, every signal the operation writes with its values and limits, the
/// duration of the observation, the stop conditions and the restoration. The engineer approves exactly this, and one approval covers
/// one reviewed operation shown in full (A2.5).
#[derive(Debug, Clone, PartialEq, Serialize, JsonSchema)]
#[serde(rename_all = "camelCase")]
pub struct ApprovalBlock {
    /// The loop the operation works on.
    pub loop_id: String,
    /// Every signal the operation writes.
    pub signals: Vec<ApprovedSignal>,
    /// The duration the observation records, in seconds; `None` for a Restore, which records nothing (B6.4).
    pub duration_seconds: Option<f64>,
    /// What ends the operation before its end, and what the runtime does then (A2.5, B6.3, B6.7).
    pub stop_conditions: Vec<String>,
    /// What the restoration returns, and when the engineer can ask for it (A2.5, B6.4).
    pub restoration: String,
}

/// The stop conditions of an Apply, in the words the approval shows (B6.3, B6.7).
const APPLY_STOP_CONDITIONS: [&str; 3] = [
    "A measured output leaves its permitted range: the previous settings are restored at once.",
    "A requirement fails: the response is reported, and Restore previous settings is offered.",
    "A Cancel, or a step the runtime refuses after the first write: the previous settings are restored at once.",
];

/// The stop conditions of a Restore, in the words the approval shows (B6.4, B6.7).
const RESTORE_STOP_CONDITIONS: [&str; 1] = [
    "A step the runtime does not confirm: Restore stops there, and Restore previous settings is offered again.",
];

/// The restoration an Apply's approval names (A2.5, B6.4).
const APPLY_RESTORATION: &str = "Restore previous settings returns the parameter record and the mode found before Apply. It stays offered \
    until the runtime confirms it.";

/// The restoration a Restore's approval names (A2.5, B6.4).
const RESTORE_RESTORATION: &str = "The parameter record and the mode found before Apply go back to the values Apply replaced. Restored is \
    shown only after the runtime confirms it.";

/// The approval of one reviewed operation (A2.5, B9.3). Its signals take their limits from the loop's parameter binding, and the
/// `parameters` are the signals the review writes; `duration_seconds` is the observation's duration, and `None` for a Restore.
#[must_use]
pub fn approval_block(
    loop_definition: &LoopDefinition,
    action: ReviewedAction,
    parameters: &[ReviewedParameter],
    duration_seconds: Option<f64>,
) -> ApprovalBlock {
    let ranges: BTreeMap<&str, (f64, f64)> = loop_definition
        .parameter_binding
        .iter()
        .flat_map(|binding| binding.parameters.iter())
        .map(|parameter| {
            (
                parameter.symbol.as_str(),
                (parameter.minimum, parameter.maximum),
            )
        })
        .collect();
    let signals: Vec<ApprovedSignal> = parameters
        .iter()
        .map(|parameter| {
            let (minimum, maximum) = ranges
                .get(parameter.symbol.as_str())
                .copied()
                .unwrap_or((f64::NAN, f64::NAN));
            ApprovedSignal {
                symbol: parameter.symbol.clone(),
                unit: parameter.unit.clone(),
                running: parameter.running,
                next: parameter.next,
                minimum,
                maximum,
            }
        })
        .collect();
    let (stop_conditions, restoration): (Vec<String>, String) = match action {
        ReviewedAction::Apply => (
            APPLY_STOP_CONDITIONS
                .iter()
                .map(|text| (*text).to_owned())
                .collect(),
            APPLY_RESTORATION.to_owned(),
        ),
        ReviewedAction::Restore => (
            RESTORE_STOP_CONDITIONS
                .iter()
                .map(|text| (*text).to_owned())
                .collect(),
            RESTORE_RESTORATION.to_owned(),
        ),
    };
    ApprovalBlock {
        loop_id: loop_definition.id.clone(),
        signals,
        duration_seconds,
        stop_conditions,
        restoration,
    }
}

/// The outcome of a confirmed Apply (B6.1 to B6.3, B6.7): the commissioning result, and what the engineer may do next.
#[derive(Debug, Clone, PartialEq, Serialize, JsonSchema)]
#[serde(rename_all = "camelCase")]
pub struct ApplyOutcome {
    /// The stored result of the run (B6.3).
    pub result: CommissioningResult,
    /// Whether the previous settings were restored at once: after a measured output left its permitted range, or after a
    /// failure that followed the first write (B6.3, B6.4).
    pub restored: bool,
    /// Whether Restore previous settings is offered (B6.3, B7).
    pub restore_available: bool,
    /// Whether Keep in project is offered: the settings stay applied and no Restore followed (B6.5).
    pub keep_available: bool,
    /// The identifier of the Apply's journaled commissioning checkpoint (`CommissioningCheckpoint::id`, A8.5), which a later
    /// Restore starts from (B6.4).
    pub checkpoint_id: String,
    /// The identity of the study's validation that the applied candidate passed, as the checkpoint keeps it (A8.1).
    pub validation_identity: String,
}

/// The outcome of a confirmed Restore (B6.4, B6.7).
#[derive(Debug, Clone, PartialEq, Serialize, JsonSchema)]
#[serde(rename_all = "camelCase")]
pub struct RestoreOutcome {
    /// Whether the runtime confirmed the restoration; Restored is shown only then (B6.4).
    pub restored: bool,
    /// The sentence the engineer reads: what was restored, or why the restoration is unavailable or unconfirmed (A11.1).
    pub reason: String,
    /// The mode the loop was returned to (A6.2), when it is known.
    pub mode: Option<FoundMode>,
    /// The parameter record as the runtime last reported it (E7.2).
    pub parameters: ParameterStatus,
    /// The recording the Restore stopped and kept as evidence (E11.6, A8.1); `None` when no recording was held.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub recording: Option<RecordedRun>,
    /// Whether Restore previous settings is still offered for the checkpoint: a Restore the runtime did not confirm leaves it
    /// offered (B6.7), a confirmed one closes the checkpoint (B6.4).
    pub restore_available: bool,
    /// Whether Keep in project is offered: never after a Restore (B6.5).
    pub keep_available: bool,
}

/// The review identity (§0.1b Confirmation): `sha256:` followed by the lowercase hexadecimal SHA-256 digest of the canonical
/// JSON of `parts` (`study::canonical_json`: object keys sorted at every level, arrays kept in order), so the identity depends
/// only on what `parts` holds, never on the order it was built in. It has the format of the study's other digests and is
/// computed by the same function (`independent_validation::digest`).
#[must_use]
pub fn review_identity(parts: &Value) -> String {
    digest(parts)
}

/// The parts of an Apply's review identity: everything its confirmation approves (§0.1b Confirmation; B6.1), as a JSON object
/// with exactly these keys:
///
/// - `programRevision`, `bindingRevision` and `activeRevision`: the revisions of the parameter record in `status` (E7.2); the
///   active revision is the runtime's hash of the values in force, so it covers the running values;
/// - `foundMode`: the mode the loop was found in, `"auto"` or `"manual"`, or `null` when it is unknown (A6.2);
/// - `values`: the values the confirmation writes, by symbol (E7.3);
/// - `transferBindingRevision`: the revision of the transfer binding that the mode changes go through (E6.2);
/// - `validationIdentity`: the identity of the study's validation that the candidate passed (B5).
#[must_use]
pub fn apply_identity_parts(
    status: &ParameterStatus,
    found_mode: Option<FoundMode>,
    values: &BTreeMap<String, f64>,
    transfer_binding_revision: &str,
    validation_identity: &str,
) -> Value {
    json!({
        "programRevision": status.program_revision,
        "bindingRevision": status.binding_revision,
        "activeRevision": status.active_revision,
        "foundMode": found_mode,
        "values": values,
        "transferBindingRevision": transfer_binding_revision,
        "validationIdentity": validation_identity,
    })
}

/// The parts of a Restore's review identity: everything its confirmation approves (§0.1b Confirmation; B6.4), as a JSON object
/// with exactly these keys:
///
/// - `programRevision`, `bindingRevision` and `activeRevision`: the revisions of the parameter record in `status` (E7.2); the
///   active revision is the runtime's hash of the values in force, so it covers the running values;
/// - `foundMode`: the mode before Apply that the Restore returns the loop to, `"auto"` or `"manual"`, or `null` when it is
///   unknown (A6.2);
/// - `values`: the values the confirmation writes back, by symbol: the checkpoint's origin values, which the rollback restores
///   (E7.5);
/// - `transferBindingRevision`: the revision of the transfer binding that the mode changes go through (E6.2);
/// - `checkpointId`: the journaled commissioning checkpoint the Restore starts from (A8.5, B6.4), which fixes the origin
///   values.
#[must_use]
pub fn restore_identity_parts(
    status: &ParameterStatus,
    found_mode: Option<FoundMode>,
    values: &BTreeMap<String, f64>,
    transfer_binding_revision: &str,
    checkpoint_id: &str,
) -> Value {
    json!({
        "programRevision": status.program_revision,
        "bindingRevision": status.binding_revision,
        "activeRevision": status.active_revision,
        "foundMode": found_mode,
        "values": values,
        "transferBindingRevision": transfer_binding_revision,
        "checkpointId": checkpoint_id,
    })
}

#[cfg(test)]
#[path = "application_records_tests.rs"]
mod tests;
