//! Parameter transactions of Control Studio, the client's rules (specification 35, B6.1 and B6.4; annex E7).
//!
//! The decisions of a parameter binding's check, of a record's check, of the text a review is typed as, of a review, of the Apply and
//! rollback requests, of the confirmation that the runtime's record gives and the sentence of a confirmed write, and of a cancel. The
//! client is `parameterApplication.ts`; the binding check is from `parameterBinding.ts`. The engine's `controlStudio/parameters`
//! command (`trust-control-studio`, `analysis_parameters_action`) applies these decisions.
//!
//! Each decision takes the runtime's answer as data that has already been read, and returns the request or the verdict. The
//! runtime calls, the status polls and their timers, and the transaction identifiers are transport: the caller performs them in
//! the order that each decision states.

use std::collections::{BTreeMap, BTreeSet};

use schemars::JsonSchema;
use serde::{Deserialize, Serialize};

use crate::failure::{Cause, Failure, NextAction};
use crate::json;
use crate::loop_definition::RealType;

use super::parameter_status::{same_parameter_values, ParameterStatus, RecordState};
use super::texts;

/// The most parameters one binding holds (E7.1).
const MAXIMUM_PARAMETERS: usize = 64;
/// The longest label of a binding, in UTF-16 units.
const MAXIMUM_LABEL_UNITS: usize = 1_024;
/// The longest review text, in UTF-16 units (64 kB).
pub(super) const MAXIMUM_REVIEW_UNITS: usize = 65_536;

/// Where a parameter write goes: a simulated target, or a physical one whose writes need the engineer's approval (E7.3, E10.2).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "lowercase")]
pub enum ParameterSource {
    /// A simulated target.
    Simulation,
    /// A physical target: every write needs the engineer's approval.
    Physical,
}

/// The binding that a parameter request names (E7.1; `ParameterBinding` of `parameterBinding.ts`). The runtime verifies the
/// compiled ownership of the symbols.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct RuntimeBinding {
    /// The loop the parameters belong to; not blank.
    pub loop_id: String,
    /// The task that owns the programs; `$background` selects unscheduled programs.
    pub task_name: String,
    /// The parameters: one to 64, unique without regard to case.
    pub parameters: Vec<RuntimeParameter>,
    /// The mode: a BOOL variable and the value that means Manual.
    pub mode: RuntimeMode,
}

/// One declared parameter of a binding (E7.1).
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct RuntimeParameter {
    /// The symbol of the scalar member.
    pub symbol: String,
    /// The IEC type: REAL or LREAL.
    pub iec_type: RealType,
    /// The unit; not blank.
    pub unit: String,
    /// The convention, for example `proportional gain`; not blank.
    pub convention: String,
    /// The smallest accepted value; finite.
    pub minimum: f64,
    /// The largest accepted value; finite, and not below the minimum.
    pub maximum: f64,
}

/// The mode of a binding (E7.1).
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct RuntimeMode {
    /// The BOOL variable of the mode.
    pub symbol: String,
    /// The value of the variable that means Manual.
    pub manual_value: bool,
}

/// Checks a binding before any runtime call (E7.1, C0; `decodeParameterBinding`): every label is not blank, at most 1,024 units
/// and free of control characters; the binding names `loop_id`; it holds one to 64 parameters; the symbols are unique without
/// regard to case and differ from the mode symbol; and each unit and convention is set, each bound is finite, and the minimum is
/// not above the maximum.
///
/// # Errors
/// The first rule that the binding breaks, as an implementation failure whose next action opens the loop setup.
pub fn check_binding(binding: &RuntimeBinding, loop_id: &str) -> Result<(), Failure> {
    if !is_label(&binding.loop_id) {
        return Err(binding_failure(&texts::parameter_binding_label(
            "loop identifier",
        )));
    }
    if binding.loop_id != loop_id {
        return Err(binding_failure(&texts::parameter_binding_loop(
            &binding.loop_id,
            loop_id,
        )));
    }
    if !is_label(&binding.task_name) {
        return Err(binding_failure(&texts::parameter_binding_label(
            "task name",
        )));
    }
    if !is_label(&binding.mode.symbol) {
        return Err(binding_failure(&texts::parameter_binding_label(
            "mode symbol",
        )));
    }
    let count = binding.parameters.len();
    if !(1..=MAXIMUM_PARAMETERS).contains(&count) {
        return Err(binding_failure(&texts::parameter_binding_count(count)));
    }
    let mut symbols = BTreeSet::from([binding.mode.symbol.to_uppercase()]);
    for parameter in &binding.parameters {
        if !is_label(&parameter.symbol) {
            return Err(binding_failure(&texts::parameter_binding_label(
                "parameter symbol",
            )));
        }
        if !symbols.insert(parameter.symbol.to_uppercase()) {
            return Err(binding_failure(&texts::parameter_binding_symbol(
                &parameter.symbol,
            )));
        }
        if !is_label(&parameter.unit) {
            return Err(binding_failure(&texts::parameter_binding_member(
                "unit",
                &parameter.symbol,
            )));
        }
        if !is_label(&parameter.convention) {
            let text = texts::parameter_binding_member("convention", &parameter.symbol);
            return Err(binding_failure(&text));
        }
        let bounds_valid = parameter.minimum.is_finite()
            && parameter.maximum.is_finite()
            && parameter.minimum <= parameter.maximum;
        if !bounds_valid {
            let text = texts::parameter_binding_bounds(
                &parameter.symbol,
                parameter.minimum,
                parameter.maximum,
            );
            return Err(binding_failure(&text));
        }
    }
    Ok(())
}

/// A label as a binding accepts it (`label`): not blank, at most 1,024 UTF-16 units, and free of control characters.
fn is_label(value: &str) -> bool {
    !value.trim().is_empty()
        && value.encode_utf16().count() <= MAXIMUM_LABEL_UNITS
        && !value.chars().any(|character| character < ' ')
}

/// A binding refusal: the binding comes from the loop setup, so the next action opens it.
fn binding_failure(text: &str) -> Failure {
    Failure::new(text, Cause::Implementation, NextAction::OpenLoopSetup)
}

/// Checks a parameter record that the runtime reported before it is used (`ParameterApplicationClient.status`): the three
/// revisions are set, a transaction identifier that is present is not empty, and the values and any previous values hold
/// exactly the declared symbols, each finite.
///
/// # Errors
/// The answer does not have the form of annex E7.2. Such an answer is never evidence and confirms nothing (E11.8).
pub fn check_status(
    binding: &RuntimeBinding,
    status: ParameterStatus,
) -> Result<ParameterStatus, Failure> {
    let revisions_set = [
        &status.program_revision,
        &status.binding_revision,
        &status.active_revision,
    ]
    .iter()
    .all(|revision| !revision.is_empty());
    let identifier_set = status
        .transaction_id
        .as_ref()
        .is_none_or(|identifier| !identifier.is_empty());
    let previous_set = status
        .previous_values
        .as_ref()
        .is_none_or(|previous| holds_declared(binding, previous));
    if revisions_set && identifier_set && holds_declared(binding, &status.values) && previous_set {
        Ok(status)
    } else {
        Err(Failure::new(
            &texts::parameter_status_invalid(),
            Cause::Runtime,
            NextAction::None,
        ))
    }
}

/// Whether `values` holds exactly the declared symbols of `binding`, each finite (`validValues`).
fn holds_declared(binding: &RuntimeBinding, values: &BTreeMap<String, f64>) -> bool {
    values.len() == binding.parameters.len()
        && values.values().all(|value| value.is_finite())
        && binding
            .parameters
            .iter()
            .all(|parameter| values.contains_key(&parameter.symbol))
}

/// What the values of a review are for (`review` or `reviewRollback`). A rollback writes the retained previous values back to
/// the revision they were retained under (E7.5).
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ReviewKind {
    /// An Apply: the reviewed values are written.
    Apply,
    /// A rollback: the retained previous values are written back.
    Rollback {
        /// The previous revision that the rollback restores (E7.5).
        previous_revision: String,
    },
}

/// A review of the exact values that a write would set, made against the record that the runtime reported
/// (`ParameterReview`). The one request that uses it spends it.
#[derive(Debug, PartialEq)]
pub struct ParameterReview {
    /// The transaction identifier that the request carries (E7.3).
    pub transaction_id: String,
    /// The record that the review was made against.
    pub observed: ParameterStatus,
    /// The values the write sets, one per declared symbol; each REAL value is single precision.
    pub values: BTreeMap<String, f64>,
    /// Whether the review is for an Apply or for a rollback.
    pub kind: ReviewKind,
}

impl ParameterReview {
    /// Whether the review restores retained values, that is, whether it is a rollback review.
    #[must_use]
    pub fn is_rollback(&self) -> bool {
        matches!(self.kind, ReviewKind::Rollback { .. })
    }
}

/// The values that the text a review names holds (`review`): JSON of at most 64 kB (in UTF-16 units), read as an object of numbers
/// by symbol. The page sends the text as the engineer typed it, and this is where it is read and refused (specification 35, C12).
///
/// # Errors
/// The text is missing or longer than 64 kB, is not JSON, or is not an object of numbers.
pub fn values_of_text(review_json: Option<&str>) -> Result<BTreeMap<String, f64>, Failure> {
    let text = review_json
        .filter(|text| text.encode_utf16().count() <= MAXIMUM_REVIEW_UNITS)
        .ok_or_else(|| {
            Failure::new(
                &texts::parameter_json_too_long(),
                Cause::Settings,
                NextAction::None,
            )
        })?;
    let parsed = json::parse_text(text).map_err(|_| {
        Failure::new(
            &texts::parameter_json_invalid(),
            Cause::Settings,
            NextAction::None,
        )
    })?;
    json::decode_value(parsed).map_err(|_| {
        Failure::new(
            &texts::parameter_review_values_invalid(),
            Cause::Settings,
            NextAction::None,
        )
    })
}

/// The values of a review, checked and rounded before the runtime is read (`review`, first part): exactly the declared symbols,
/// each finite; each REAL value is rounded to single precision as `Math.fround` rounds it; and the rounded value lies within the
/// bounds. The rounding is the IEC representation of the value (C0: a REAL value is rounded before it is submitted).
///
/// # Errors
/// The values do not hold exactly the declared symbols, or one of them is outside its bounds after IEC rounding (B6.2, C0).
pub fn review_input(
    binding: &RuntimeBinding,
    input: &BTreeMap<String, f64>,
) -> Result<BTreeMap<String, f64>, Failure> {
    if !holds_declared(binding, input) {
        return Err(values_invalid());
    }
    let mut values = BTreeMap::new();
    for parameter in &binding.parameters {
        let Some(&raw) = input.get(&parameter.symbol) else {
            return Err(values_invalid());
        };
        let rounded = match parameter.iec_type {
            RealType::Real => round_to_single(raw),
            RealType::Lreal => raw,
        };
        if !rounded.is_finite() || rounded < parameter.minimum || rounded > parameter.maximum {
            let text = texts::parameter_outside_bounds(
                &parameter.symbol,
                rounded,
                parameter.minimum,
                parameter.maximum,
                &parameter.unit,
            );
            return Err(Failure::new(&text, Cause::Settings, NextAction::None));
        }
        values.insert(parameter.symbol.clone(), rounded);
    }
    Ok(values)
}

/// The refusal of values that are not exactly the declared finite parameters.
fn values_invalid() -> Failure {
    Failure::new(
        &texts::parameter_review_values_invalid(),
        Cause::Settings,
        NextAction::None,
    )
}

/// The nearest single-precision value, as an `f64` (`Math.fround`).
#[allow(clippy::cast_possible_truncation)] // Rounding to single precision is the purpose of this cast.
fn round_to_single(value: f64) -> f64 {
    f64::from(value as f32)
}

/// The review of checked values against the record that the runtime reported (`review`, second part). A pending record refuses
/// it: the pending transaction is waited for first, and nothing is written over it (B6.2).
///
/// `observed` must be the record that `check_status` accepted.
///
/// # Errors
/// The record holds a pending transaction.
pub fn review_of(
    values: BTreeMap<String, f64>,
    observed: ParameterStatus,
    transaction_id: String,
) -> Result<ParameterReview, Failure> {
    if observed.status == RecordState::Pending {
        return Err(Failure::new(
            &texts::parameter_review_pending(),
            Cause::Runtime,
            NextAction::Wait,
        ));
    }
    Ok(ParameterReview {
        transaction_id,
        observed,
        values,
        kind: ReviewKind::Apply,
    })
}

/// The review of a rollback (`reviewRollback`): the retained previous values, which a rollback writes back to the previous
/// revision (E7.5). A pending record, or one that retains no previous revision or no previous values, refuses it.
///
/// `observed` must be the record that `check_status` accepted.
///
/// # Errors
/// No confirmed previous parameter set is retained (E7.5, `rollback unavailable`).
pub fn rollback_review(
    observed: ParameterStatus,
    transaction_id: String,
) -> Result<ParameterReview, Failure> {
    let previous_revision = observed
        .previous_revision
        .clone()
        .filter(|revision| !revision.is_empty());
    let previous_values = observed.previous_values.clone();
    match (
        observed.status == RecordState::Pending,
        previous_revision,
        previous_values,
    ) {
        (false, Some(previous_revision), Some(values)) => Ok(ParameterReview {
            transaction_id,
            observed,
            values,
            kind: ReviewKind::Rollback { previous_revision },
        }),
        _ => Err(Failure::new(
            &texts::parameter_rollback_unavailable(),
            Cause::Runtime,
            NextAction::None,
        )),
    }
}

/// The Apply request that the runtime checks at the task boundary (E7.3; `ParameterApplicationClient.apply`).
#[derive(Debug, Clone, PartialEq, Serialize, JsonSchema)]
pub struct ApplyRequest {
    /// The transaction identifier of the review.
    pub transaction_id: String,
    /// The binding that the values belong to.
    pub binding: RuntimeBinding,
    /// The program revision that the review read.
    pub expected_program_revision: String,
    /// The binding revision that the review read.
    pub expected_binding_revision: String,
    /// The active revision that the review read.
    pub expected_active_revision: String,
    /// The values that the review read.
    pub expected_values: BTreeMap<String, f64>,
    /// The values to write.
    pub values: BTreeMap<String, f64>,
    /// The target that the write goes to.
    pub source: ParameterSource,
    /// Whether the engineer approved the physical write (E10.2).
    pub physical_approved: bool,
}

/// The rollback request that the runtime checks at the task boundary (E7.5; `ParameterApplicationClient.rollback`).
#[derive(Debug, Clone, PartialEq, Serialize, JsonSchema)]
pub struct RollbackRequest {
    /// The transaction identifier of the review.
    pub transaction_id: String,
    /// The binding that the rollback belongs to.
    pub binding: RuntimeBinding,
    /// The program revision that the review read.
    pub expected_program_revision: String,
    /// The binding revision that the review read.
    pub expected_binding_revision: String,
    /// The active revision that the review read.
    pub expected_active_revision: String,
    /// The values that the review read.
    pub expected_values: BTreeMap<String, f64>,
    /// The previous revision that the rollback restores.
    pub previous_revision: String,
    /// The target that the rollback goes to.
    pub source: ParameterSource,
    /// Whether the engineer approved the physical rollback (E10.2).
    pub physical_approved: bool,
}

/// The Apply request of a review (`apply`, in its order of checks): the review must be an Apply review, a physical target needs
/// the engineer's approval, and the runtime must keep request identities. The caller spends the review once the request is built.
///
/// # Errors
/// The review is not an Apply review, the physical approval is missing, or the runtime does not support request identities.
pub fn apply_request(
    binding: &RuntimeBinding,
    review: &ParameterReview,
    source: ParameterSource,
    physical_approved: bool,
) -> Result<ApplyRequest, Failure> {
    if review.is_rollback() {
        return Err(Failure::new(
            &texts::parameter_apply_review_first(),
            Cause::Runtime,
            NextAction::None,
        ));
    }
    require_physical_approval(source, physical_approved, "parameter application")?;
    if !identity_supported(review) {
        return Err(Failure::new(
            &texts::parameter_apply_identity(),
            Cause::Runtime,
            NextAction::None,
        ));
    }
    Ok(ApplyRequest {
        transaction_id: review.transaction_id.clone(),
        binding: binding.clone(),
        expected_program_revision: review.observed.program_revision.clone(),
        expected_binding_revision: review.observed.binding_revision.clone(),
        expected_active_revision: review.observed.active_revision.clone(),
        expected_values: review.observed.values.clone(),
        values: review.values.clone(),
        source,
        physical_approved,
    })
}

/// The rollback request of a review (`rollback`, in its order of checks): the review must be a rollback review, a physical target
/// needs the engineer's approval, and the runtime must keep request identities. The caller spends the review once the request is built.
///
/// # Errors
/// The review is not a rollback review, the physical approval is missing, or the runtime does not support request identities.
pub fn rollback_request(
    binding: &RuntimeBinding,
    review: &ParameterReview,
    source: ParameterSource,
    physical_approved: bool,
) -> Result<RollbackRequest, Failure> {
    let ReviewKind::Rollback { previous_revision } = &review.kind else {
        return Err(Failure::new(
            &texts::parameter_rollback_review_first(),
            Cause::Runtime,
            NextAction::None,
        ));
    };
    require_physical_approval(source, physical_approved, "parameter rollback")?;
    if !identity_supported(review) {
        return Err(Failure::new(
            &texts::parameter_rollback_identity(),
            Cause::Runtime,
            NextAction::None,
        ));
    }
    Ok(RollbackRequest {
        transaction_id: review.transaction_id.clone(),
        binding: binding.clone(),
        expected_program_revision: review.observed.program_revision.clone(),
        expected_binding_revision: review.observed.binding_revision.clone(),
        expected_active_revision: review.observed.active_revision.clone(),
        expected_values: review.observed.values.clone(),
        previous_revision: previous_revision.clone(),
        source,
        physical_approved,
    })
}

/// Refuses a physical write that the engineer has not approved (E10.2, A2.5).
fn require_physical_approval(
    source: ParameterSource,
    approved: bool,
    operation: &str,
) -> Result<(), Failure> {
    if source == ParameterSource::Physical && !approved {
        return Err(Failure::new(
            &texts::parameter_physical_approval(operation),
            Cause::Approval,
            NextAction::Approve,
        ));
    }
    Ok(())
}

/// Whether the review's record and transaction identifier support the request identity that a write needs (E7.2, E7.3).
fn identity_supported(review: &ParameterReview) -> bool {
    review.observed.supports_request_id == Some(true) && !review.transaction_id.is_empty()
}

/// The verdict of one status read after a write (`confirm`): the write is confirmed, or the transaction is still pending.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Confirmation {
    /// The runtime holds the reviewed values under this transaction, and the revision moved as the write says.
    Confirmed,
    /// The transaction is still pending: the status is read again.
    Waiting,
}

/// The verdict of one status read after an Apply or a rollback (`confirm`, E7.4 and E7.5). The program and binding revisions must
/// still be the reviewed ones; the record must be applied under the review's transaction, hold the reviewed values, and have moved
/// as the write says: an Apply's previous revision is the observed active one, and a rollback's active revision is the one it
/// restores.
///
/// # Errors
/// The program or the binding changed, or the transaction ended without the reviewed values, for the runtime's reason when it gives one.
pub fn confirmation_step(
    review: &ParameterReview,
    status: &ParameterStatus,
) -> Result<Confirmation, Failure> {
    let observed = &review.observed;
    if status.program_revision != observed.program_revision
        || status.binding_revision != observed.binding_revision
    {
        return Err(Failure::new(
            &texts::parameter_program_changed(),
            Cause::Runtime,
            changed_action(review),
        ));
    }
    if status.status == RecordState::Applied
        && status.transaction_id.as_deref() == Some(review.transaction_id.as_str())
        && same_parameter_values(&status.values, &review.values)
        && revision_moved(review, status)
    {
        return Ok(Confirmation::Confirmed);
    }
    if status.status != RecordState::Pending {
        let reason = status
            .reason
            .clone()
            .unwrap_or_else(texts::parameter_not_confirmed);
        return Err(Failure::new(
            &reason,
            Cause::Runtime,
            not_confirmed_action(review),
        ));
    }
    Ok(Confirmation::Waiting)
}

/// Whether the record moved as the write says (E7.4, E7.5).
fn revision_moved(review: &ParameterReview, status: &ParameterStatus) -> bool {
    match &review.kind {
        ReviewKind::Apply => {
            status.previous_revision.as_deref() == Some(review.observed.active_revision.as_str())
        }
        ReviewKind::Rollback { previous_revision } => status.active_revision == *previous_revision,
    }
}

/// The next action when the program or binding changed under a write: a changed Apply candidate needs validating (B6.7); a
/// rollback is the restore itself, which explains its own unavailability (B6.4).
fn changed_action(review: &ParameterReview) -> NextAction {
    match review.kind {
        ReviewKind::Apply => NextAction::Validate,
        ReviewKind::Rollback { .. } => NextAction::None,
    }
}

/// The next action when a write is not confirmed: a rollback is restored again (B6.7, "Restore not confirmed"), and an Apply gives
/// the runtime's reason, which the engineer reads.
fn not_confirmed_action(review: &ParameterReview) -> NextAction {
    match review.kind {
        ReviewKind::Apply => NextAction::None,
        ReviewKind::Rollback { .. } => NextAction::Restore,
    }
}

/// The failure of a write whose confirmation window ran out (`confirm`). The status is unknown, so it is read again on the
/// original endpoint (E11.8); a rollback is restored again (B6.7).
#[must_use]
pub fn confirmation_timeout(review: &ParameterReview) -> Failure {
    let next_action = match review.kind {
        ReviewKind::Apply => NextAction::ConnectRuntime,
        ReviewKind::Rollback { .. } => NextAction::Restore,
    };
    Failure::new(
        &texts::parameter_confirmation_timeout(),
        Cause::Runtime,
        next_action,
    )
}

/// The sentence of a confirmed write (specification 35, C12; E7.4, E7.5): a rollback reports the restored values, and an Apply reports
/// the reviewed values, each observed after the owning task executed.
#[must_use]
pub fn applied_reason(rollback: bool) -> String {
    if rollback {
        texts::parameter_applied_rollback()
    } else {
        texts::parameter_applied_apply()
    }
}

/// The owner of a parameter operation that a cancel names: its transaction identifier and the revisions it was queued under
/// (E7.6; `ParameterOwner`). It is journaled in the commissioning checkpoint (A8.5).
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct ParameterOwner {
    /// The transaction identifier.
    pub id: String,
    /// The program revision the operation was queued under.
    pub program: String,
    /// The binding revision the operation was queued under.
    pub binding: String,
}

/// The runtime's answer to a cancel (E7.6). Only an answer that confirms the owner's identity settles the cancel.
#[derive(Debug, Clone, PartialEq, Eq, Deserialize)]
pub struct CancelResponse {
    /// Whether the runtime confirmed the cancel.
    #[serde(default)]
    pub confirmed: bool,
    /// The transaction identifier that the answer is for.
    #[serde(default)]
    pub transaction_id: Option<String>,
    /// The program revision that the answer is for.
    #[serde(default)]
    pub program_revision: Option<String>,
    /// The binding revision that the answer is for.
    #[serde(default)]
    pub binding_revision: Option<String>,
    /// Whether the transaction was cancelled before its task boundary, so that nothing was written.
    #[serde(default)]
    pub before_dispatch: bool,
}

/// How a cancel ended (E7.6).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum CancelOutcome {
    /// The transaction was cancelled before its task boundary: nothing was written.
    BeforeDispatch,
    /// The transaction had already ended: its outcome is what the record reports.
    Terminal,
}

/// The outcome that a cancel answer confirms (`cancelOwner`, first part, E7.6). A confirmed answer for the owner's transaction and
/// revisions settles it, as `BeforeDispatch` when the runtime says so and as `Terminal` otherwise. Any other answer, or none,
/// settles nothing, and the caller reads the status instead.
#[must_use]
pub fn cancel_acknowledgement(
    owner: &ParameterOwner,
    response: Option<&CancelResponse>,
) -> Option<CancelOutcome> {
    let response = response?;
    let names_owner = response.transaction_id.as_deref() == Some(owner.id.as_str())
        && response.program_revision.as_deref() == Some(owner.program.as_str())
        && response.binding_revision.as_deref() == Some(owner.binding.as_str());
    let outcome = if response.before_dispatch {
        CancelOutcome::BeforeDispatch
    } else {
        CancelOutcome::Terminal
    };
    (response.confirmed && names_owner).then_some(outcome)
}

/// Whether a cancel that is still unconfirmed is waiting, or has ended (`cancelOwner`, polling part).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum CancelProgress {
    /// The transaction is still pending: the status is read again.
    Waiting,
    /// The transaction has ended.
    Terminal,
}

/// One status read while a cancel is unconfirmed (`cancelOwner`, polling part). The owner's identity must still hold, and a record
/// that is no longer pending has ended the transaction.
///
/// # Errors
/// The identity remains unresolved: the program, the binding or the transaction identifier no longer belongs to the owner.
pub fn cancel_step(
    owner: &ParameterOwner,
    status: &ParameterStatus,
) -> Result<CancelProgress, Failure> {
    let same_owner = status.program_revision == owner.program
        && status.binding_revision == owner.binding
        && status.transaction_id.as_deref() == Some(owner.id.as_str());
    if !same_owner {
        return Err(Failure::new(
            &texts::parameter_identity_unresolved(),
            Cause::Runtime,
            NextAction::ConnectRuntime,
        ));
    }
    Ok(if status.status == RecordState::Pending {
        CancelProgress::Waiting
    } else {
        CancelProgress::Terminal
    })
}

/// The failure of a cancel that stays unconfirmed for its window (`cancelOwner`).
#[must_use]
pub fn cancel_timeout() -> Failure {
    Failure::new(
        &texts::parameter_cancel_unconfirmed(),
        Cause::Runtime,
        NextAction::ConnectRuntime,
    )
}

#[cfg(test)]
#[path = "parameter_application_tests.rs"]
mod tests;
