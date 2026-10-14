//! The parameter client of Control Studio (runtime protocol annex E7; specification 35, B6.1 to B6.4, E11.4 and E11.7): the review of
//! exact values, the Apply and the rollback of a reviewed record, the confirmation of each by the runtime's record, the cancel of an
//! owned operation, and the reconciliation of a retained one. From `parameterApplication.ts` and `manualApplicationReconciliation.ts`;
//! the rules are in `trust_control::commissioning::parameter_application` and `parameter_status`.

use std::collections::BTreeMap;
use std::thread::sleep;
use std::time::{Duration, Instant};

use trust_control::commissioning::application::ApplySource;
use trust_control::commissioning::parameter_application::{
    apply_request, cancel_acknowledgement, cancel_step, cancel_timeout, check_status,
    confirmation_step, confirmation_timeout, review_input, review_of, rollback_request,
    rollback_review, CancelOutcome, CancelProgress, Confirmation, ParameterOwner, ParameterReview,
    ParameterSource, ReviewKind, RuntimeBinding, RuntimeMode, RuntimeParameter,
};
use trust_control::commissioning::parameter_status::{
    same_parameter_values, ParameterStatus, RecordState,
};
use trust_control::commissioning::texts;
use trust_control::failure::{Cause, Failure, NextAction};
use trust_control::loop_definition::ParameterBinding;

use super::{new_transaction_id, RuntimeClient};

/// The parameter transactions of one controller on one runtime binding (annex E7): the review of exact values, the Apply and the rollback
/// of a reviewed record, each confirmed by the runtime's record, and the cancel of an owned operation (`ParameterApplicationClient`).
pub struct ParameterTransactions<'c> {
    client: &'c mut RuntimeClient,
    binding: RuntimeBinding,
    source: ApplySource,
}

impl<'c> ParameterTransactions<'c> {
    /// Transactions on `binding`, written to the target `source`.
    #[must_use]
    pub fn new(
        client: &'c mut RuntimeClient,
        binding: RuntimeBinding,
        source: ApplySource,
    ) -> Self {
        Self {
            client,
            binding,
            source,
        }
    }

    /// The parameter record of the binding, checked (`status` of `parameterApplication.ts`, E7.2).
    ///
    /// # Errors
    /// The runtime cannot be reached, refuses, or answers out of protocol.
    pub fn status(&mut self) -> Result<ParameterStatus, Failure> {
        let status = self.client.parameter_status(&self.binding)?;
        check_status(&self.binding, status)
    }

    /// Reviews the exact values that an apply would write, against the record that the runtime reports (`review`, B6.2, C0). The
    /// values are rounded as their IEC types round them, and a pending record refuses the review.
    ///
    /// # Errors
    /// The values are not exactly the declared parameters, one is outside its bounds after rounding, the record is pending, or the
    /// runtime cannot be read.
    pub fn review(&mut self, values: &BTreeMap<String, f64>) -> Result<ParameterReview, Failure> {
        let values = review_input(&self.binding, values)?;
        let observed = self.status()?;
        review_of(values, observed, new_transaction_id())
    }

    /// Reviews the rollback to the retained previous values (`reviewRollback`, E7.5).
    ///
    /// # Errors
    /// The record is pending, or it retains no previous revision and no previous values.
    pub fn review_rollback(&mut self) -> Result<ParameterReview, Failure> {
        let observed = self.status()?;
        rollback_review(observed, new_transaction_id())
    }

    /// Applies a reviewed record and confirms it (`apply` of `parameterApplication.ts`, E7.3, E7.4). An apply that the runtime refused or
    /// did not answer is never sent again: the record decides what happened (E11.4).
    ///
    /// # Errors
    /// The review is not an apply review, the physical approval is missing, the runtime does not keep request identities, or the
    /// confirmation fails, times out, or finds the program or binding changed.
    pub fn apply(
        &mut self,
        review: &ParameterReview,
        physical_approved: bool,
    ) -> Result<ParameterStatus, Failure> {
        let request = apply_request(
            &self.binding,
            review,
            self.parameter_source(),
            physical_approved,
        )?;
        let _ = self.client.parameter_apply(&request);
        self.confirm(review)
    }

    /// Rolls the record back to its retained previous values and confirms it (`rollback` of `parameterApplication.ts`, E7.5).
    ///
    /// # Errors
    /// The review is not a rollback review, the physical approval is missing, the runtime does not keep request identities, or the
    /// confirmation fails, times out, or finds the program or binding changed.
    pub fn rollback(
        &mut self,
        review: &ParameterReview,
        physical_approved: bool,
    ) -> Result<ParameterStatus, Failure> {
        let request = rollback_request(
            &self.binding,
            review,
            self.parameter_source(),
            physical_approved,
        )?;
        let _ = self.client.parameter_rollback(&request);
        self.confirm(review)
    }

    /// Cancels an owned operation by its identity (`cancelOwner` of `parameterApplication.ts`, E7.6). A confirmed answer for the owner
    /// settles it; otherwise the record is read until the operation has ended, within the confirmation window.
    ///
    /// # Errors
    /// The owner's identity no longer belongs to the record, or the cancel stays unconfirmed for its window.
    pub fn cancel_owner(&mut self, owner: &ParameterOwner) -> Result<CancelOutcome, Failure> {
        let response = self
            .client
            .parameter_cancel(owner, &self.binding.loop_id)
            .ok();
        if let Some(outcome) = cancel_acknowledgement(owner, response.as_ref()) {
            return Ok(outcome);
        }
        let timing = self.client.confirmation_timing(&self.binding.task_name);
        let deadline = Instant::now() + duration_ms(timing.window_ms);
        while Instant::now() < deadline {
            let current = self.status()?;
            match cancel_step(owner, &current)? {
                CancelProgress::Terminal => return Ok(CancelOutcome::Terminal),
                CancelProgress::Waiting => sleep(duration_ms(timing.poll_ms)),
            }
        }
        Err(cancel_timeout())
    }

    /// Reads the record until the reviewed operation is confirmed, rejected or ends unconfirmed (`confirm` of `parameterApplication.ts`,
    /// E7.4, E7.5).
    fn confirm(&mut self, review: &ParameterReview) -> Result<ParameterStatus, Failure> {
        let timing = self.client.confirmation_timing(&self.binding.task_name);
        let deadline = Instant::now() + duration_ms(timing.window_ms);
        while Instant::now() < deadline {
            let status = self.status()?;
            match confirmation_step(review, &status)? {
                Confirmation::Confirmed => return Ok(status),
                Confirmation::Waiting => sleep(duration_ms(timing.poll_ms)),
            }
        }
        Err(confirmation_timeout(review))
    }

    /// The target of the writes of this session, in the parameter layer's form.
    fn parameter_source(&self) -> ParameterSource {
        match self.source {
            ApplySource::Simulation => ParameterSource::Simulation,
            ApplySource::Physical => ParameterSource::Physical,
        }
    }
}

/// What reconciling a retained parameter operation found (`reconcileManualApplication`, E11.4, E11.7). Each of these outcomes clears the
/// owner. An operation that cannot be resolved is an error instead, and it keeps its owner.
#[derive(Debug, Clone, PartialEq)]
pub enum ParameterReconciliation {
    /// The runtime's record confirms the retained operation.
    Confirmed {
        /// The record the confirmation was read from.
        observed: ParameterStatus,
        /// The sentence that says what was confirmed.
        reason: String,
    },
    /// The runtime rejected the retained operation; its reason is in the record.
    Rejected {
        /// The record that rejected the operation.
        observed: ParameterStatus,
        /// The sentence that says what was rejected, and why.
        reason: String,
    },
    /// The retained operation was cancelled before its dispatch, so nothing was written and nothing is replayed.
    CancelledBeforeDispatch {
        /// The sentence that says the operation was cancelled.
        reason: String,
    },
}

/// Reconciles a retained parameter operation by its identity, never by a second write (`reconcileManualApplication`, E11.4, E11.7). The
/// record must still belong to the program and binding of the review, and it must be the record that the review's write leaves. When a
/// closed panel retained the operation, `close_observed` is the record it was retained with, and that record must still hold.
///
/// # Errors
/// The record cannot be read, the program or binding changed, the owner cannot be cancelled or read to an end, the record is still
/// pending or has no terminal status, it changed since the closed panel, or it does not match the review. The owner stays unresolved and
/// the operation is not repeated.
pub fn reconcile_parameter_operation(
    transactions: &mut ParameterTransactions<'_>,
    review: &ParameterReview,
    close_observed: Option<&ParameterStatus>,
) -> Result<ParameterReconciliation, Failure> {
    let operation = operation_word(review);
    let owner = ParameterOwner {
        id: review.transaction_id.clone(),
        program: review.observed.program_revision.clone(),
        binding: review.observed.binding_revision.clone(),
    };
    let mut observed = transactions.status().map_err(|failure| {
        Failure::new(
            &texts::parameter_reconcile_inspect(operation, &whole_text(&failure)),
            Cause::Runtime,
            NextAction::ConnectRuntime,
        )
    })?;
    if observed.program_revision != review.observed.program_revision
        || observed.binding_revision != review.observed.binding_revision
    {
        return Err(Failure::new(
            &texts::parameter_reconcile_program_changed(operation),
            Cause::Runtime,
            NextAction::None,
        ));
    }
    if observed.status == RecordState::Pending
        || observed.transaction_id.as_deref() != Some(review.transaction_id.as_str())
    {
        match transactions.cancel_owner(&owner) {
            Ok(CancelOutcome::BeforeDispatch) => {
                return Ok(ParameterReconciliation::CancelledBeforeDispatch {
                    reason: texts::parameter_reconcile_cancelled(operation),
                });
            }
            Ok(CancelOutcome::Terminal) => {
                observed = transactions
                    .status()
                    .map_err(|failure| unreconciled(&failure))?;
            }
            Err(failure) => return Err(unreconciled(&failure)),
        }
    }
    if matches!(observed.status, RecordState::Pending | RecordState::Idle) {
        return Err(Failure::new(
            &texts::parameter_reconcile_no_terminal(operation),
            Cause::Runtime,
            NextAction::None,
        ));
    }
    let changed_since_close = close_observed.is_some_and(|close| {
        observed.status != close.status
            || observed.active_revision != close.active_revision
            || !same_parameter_values(&observed.values, &close.values)
    });
    if changed_since_close {
        return Err(Failure::new(
            &texts::parameter_reconcile_status_changed(operation),
            Cause::Runtime,
            NextAction::None,
        ));
    }
    if !matches_review(&observed, review) {
        return Err(Failure::new(
            &texts::parameter_reconcile_mismatch(),
            Cause::Runtime,
            NextAction::None,
        ));
    }
    if observed.status == RecordState::Applied {
        let reason = texts::parameter_reconcile_confirmed(operation);
        return Ok(ParameterReconciliation::Confirmed { observed, reason });
    }
    let reason = texts::parameter_reconcile_rejected(operation, observed.reason.as_deref());
    Ok(ParameterReconciliation::Rejected { observed, reason })
}

/// Whether the record is the one that the review's write leaves: the same transaction, and either the values and revisions that an
/// applied write or rollback sets, or the unchanged record of a rejected write.
fn matches_review(observed: &ParameterStatus, review: &ParameterReview) -> bool {
    if observed.transaction_id.as_deref() != Some(review.transaction_id.as_str()) {
        return false;
    }
    match observed.status {
        RecordState::Applied => {
            same_parameter_values(&observed.values, &review.values)
                && match &review.kind {
                    ReviewKind::Rollback { previous_revision } => {
                        observed.active_revision == *previous_revision
                    }
                    ReviewKind::Apply => {
                        observed.previous_revision.as_deref()
                            == Some(review.observed.active_revision.as_str())
                            && observed.previous_values.as_ref().is_some_and(|previous| {
                                same_parameter_values(previous, &review.observed.values)
                            })
                    }
                }
        }
        RecordState::Rejected => {
            observed.active_revision == review.observed.active_revision
                && same_parameter_values(&observed.values, &review.observed.values)
        }
        RecordState::Idle | RecordState::Pending => false,
    }
}

/// The name of the operation that a review is for, as the sentences of reconciliation name it.
fn operation_word(review: &ParameterReview) -> &'static str {
    if review.is_rollback() {
        "rollback"
    } else {
        "apply"
    }
}

/// A failure's whole text: its detail when it has one, its message otherwise.
fn whole_text(failure: &Failure) -> String {
    failure
        .detail
        .clone()
        .unwrap_or_else(|| failure.message.clone())
}

/// The failure of a retained operation that cannot be reconciled; its owner stays.
fn unreconciled(failure: &Failure) -> Failure {
    Failure::new(
        &texts::parameter_reconcile_unreconciled(&whole_text(failure)),
        Cause::Runtime,
        NextAction::ConnectRuntime,
    )
}

/// The binding of a loop's declared parameters, in the runtime's form (E7.1). The loop's binding names the same members and the same
/// mode, so every field is kept.
#[must_use]
pub fn runtime_binding(binding: &ParameterBinding) -> RuntimeBinding {
    RuntimeBinding {
        loop_id: binding.loop_id.clone(),
        task_name: binding.task_name.clone(),
        parameters: binding
            .parameters
            .iter()
            .map(|parameter| RuntimeParameter {
                symbol: parameter.symbol.clone(),
                iec_type: parameter.iec_type,
                unit: parameter.unit.clone(),
                convention: parameter.convention.clone(),
                minimum: parameter.minimum,
                maximum: parameter.maximum,
            })
            .collect(),
        mode: RuntimeMode {
            symbol: binding.mode.symbol.clone(),
            manual_value: binding.mode.manual_value,
        },
    }
}

/// A duration of `milliseconds`: the intervals of the confirmation timing, which are finite and positive.
fn duration_ms(milliseconds: f64) -> Duration {
    Duration::from_secs_f64(milliseconds / 1000.0)
}

#[cfg(test)]
#[path = "parameters_tests.rs"]
mod tests;
