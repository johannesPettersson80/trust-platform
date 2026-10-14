//! The recovery of a proportional campaign (`proportionalCommissioning.ts`, the recovery rules; specification 35, B6.4, B6.7, A6.2,
//! C13.9): the checks that decide whether a Restore may write, the Manual transfer a recovery needs, the check of the parameter
//! rollback, the return to the mode the loop was found in, and the result that a Restore, a cancel or a close leaves.
//!
//! Left out, as transport: the rollback's and the review's clients (`reviewRollback`, `rollback`, `cancelOwner`), the transfer
//! client (`status`, `apply`, `cancel`), the journal saves, the connection, and the observed-state reads between the checks. The
//! caller performs each read and write, in the order `recover` and `reviewRecovery` make them, and passes each read here.

use std::collections::BTreeMap;

use crate::failure::{Cause, Failure, NextAction};
use crate::loop_definition::LoopDefinition;
use crate::validation::engineer_verification_plan::EngineerRequirements;

use super::commissioning_result::{
    CommissioningResult, CommissioningStatus, TransferConfirmation, TransferStatus,
    TransferTransaction,
};
use super::found_mode::{reenter_found_mode, AutoReentry, FoundMode};
use super::gain_recovery::{matches_gain_recovery, RecoveryOrigin};
use super::parameter_status::{same_parameter_values, ParameterStatus, RecordState};
use super::proportional_commissioning::{
    auto_entry_step, binding_failure, restore_again, TransferStep,
};
use super::texts;
use super::transfer::{rounded_transfer_value, TransferBinding, TransferValue};

/// `assertRecoveryNeeded` (`proportionalCommissioning.ts`; B6.4): a campaign that confirmed its recovery and whose last transfer
/// confirms the Manual mode has no rollback left.
///
/// # Errors
/// The report's recovery is confirmed and its last transaction is output-image confirmed in the Manual mode.
pub fn assert_recovery_needed(
    report: &CommissioningResult,
    mode_symbol: &str,
    manual_value: bool,
) -> Result<(), Failure> {
    let last = report
        .transfers
        .last()
        .and_then(|transfer| transfer.transaction.as_ref());
    let in_manual = last.is_some_and(|transaction| {
        transaction.confirmation == Some(TransferConfirmation::OutputImage)
            && transaction
                .observed_values
                .as_ref()
                .and_then(|values| values.get(mode_symbol))
                .copied()
                == Some(TransferValue::Boolean(manual_value))
    });
    if report.recovery_confirmed && in_manual {
        return Err(Failure::new(
            &texts::prop_commission_already_recovered(),
            Cause::Unclassified,
            NextAction::None,
        ));
    }
    Ok(())
}

/// Whether a stored campaign is a record that a recovery can use (`reviewRecovery`, `proportionalCommissioning.ts`; B6.4): it passed,
/// or it never confirmed its recovery; it was reviewed on the model it names; its parameter origin is recorded, by its gain record
/// or by the saved previous revision and values; and its first transaction holds the original reference, output-image confirmed.
///
/// # Errors
/// The record lacks one of those, so the original settings cannot be recovered from it.
pub fn check_historical_record(
    report: &CommissioningResult,
    model_revision: &str,
    first: Option<&TransferTransaction>,
) -> Result<(), Failure> {
    let saved_origin = report.parameters.as_ref().is_some_and(|saved| {
        saved
            .previous_revision
            .as_deref()
            .is_some_and(|revision| !revision.is_empty())
            && saved.previous_values.is_some()
    });
    let reference_recorded = first.is_some_and(|transaction| {
        transaction.initial_values.is_some()
            && transaction.confirmation == Some(TransferConfirmation::OutputImage)
    });
    let usable = !(report.status != CommissioningStatus::Passed && report.recovery_confirmed)
        && model_revision == report.model_revision
        && (report.gain_recovery.is_some() || saved_origin)
        && reference_recorded;
    if usable {
        return Ok(());
    }
    Err(Failure::new(
        &texts::gain_recovery_incomplete(),
        Cause::ModelOrData,
        NextAction::None,
    ))
}

/// Whether the running parameter record is the one the campaign left (`reviewRecovery`, `proportionalCommissioning.ts`; B6.4). A gain
/// record decides it by [`matches_gain_recovery`]. A record without one compares the saved revisions and values, and the review's
/// previous values with the saved ones.
///
/// # Errors
/// The running record is not the one the campaign left, so Restore would overwrite another change.
pub fn check_historical_identity(
    report: &CommissioningResult,
    current: &ParameterStatus,
    review_values: &BTreeMap<String, f64>,
) -> Result<(), Failure> {
    let held = match (&report.gain_recovery, &report.parameters) {
        (Some(context), _) => matches_gain_recovery(current, Some(context)),
        (None, Some(saved)) => {
            current.program_revision == saved.program_revision
                && current.binding_revision == saved.binding_revision
                && current.active_revision == saved.active_revision
                && current.previous_revision == saved.previous_revision
                && same_parameter_values(&current.values, &saved.values)
                && saved
                    .previous_values
                    .as_ref()
                    .is_some_and(|previous| same_parameter_values(review_values, previous))
        }
        (None, None) => false,
    };
    if held {
        return Ok(());
    }
    Err(Failure::new(
        &texts::gain_recovery_changed(),
        Cause::Settings,
        NextAction::None,
    ))
}

/// The original reference a recovery writes back (`reviewRecovery`, `proportionalCommissioning.ts`; C13.9): the first transaction's
/// initial value of the reference. It is the value as recorded; it must fit the reference's type and bounds.
///
/// # Errors
/// The first transaction holds no number for the reference, or the number does not fit the reference.
pub fn historical_reference(
    binding: &TransferBinding,
    first: &TransferTransaction,
) -> Result<f64, Failure> {
    let reference = binding.states.first().ok_or_else(binding_failure)?;
    let recorded = first
        .initial_values
        .as_ref()
        .and_then(|values| values.get(&reference.symbol))
        .copied()
        .and_then(TransferValue::as_number);
    let Some(previous) = recorded else {
        return Err(Failure::new(
            &texts::prop_commission_reference_unavailable(),
            Cause::ModelOrData,
            NextAction::None,
        ));
    };
    rounded_transfer_value(reference, previous)?;
    Ok(previous)
}

/// Whether the running program and transfer binding are the campaign's, and the transfer supports proportional tracking
/// (`reviewRecovery`, `proportionalCommissioning.ts`; B6.4).
///
/// # Errors
/// The program or the binding differs from the campaign's evidence, or the runtime does not track proportionally.
pub fn check_historical_transfer(
    current_program: &str,
    observed: &TransferStatus,
    first: &TransferTransaction,
) -> Result<(), Failure> {
    let proportional = observed
        .supported_tracking
        .as_ref()
        .is_some_and(|kinds| kinds.iter().any(|kind| kind.as_str() == "proportional"));
    let same = observed.program_revision == current_program
        && first.program_revision == current_program
        && observed.binding_revision == first.binding_revision
        && proportional;
    if same {
        return Ok(());
    }
    Err(Failure::new(
        &texts::prop_commission_binding_differs(),
        Cause::Implementation,
        NextAction::None,
    ))
}

/// The gain transaction a recovery reviewed still matches (`recover`, `proportionalCommissioning.ts`; B6.4): the same program and
/// binding, not pending, and either the original set is in force or the candidate is applied over the original revision.
///
/// # Errors
/// The transaction changed since the recovery was reviewed, so Restore must be reviewed again.
pub fn check_gain_transaction(
    original: &RecoveryOrigin,
    candidate: &BTreeMap<String, f64>,
    before: &ParameterStatus,
) -> Result<(), Failure> {
    let identity = before.program_revision == original.program_revision
        && before.binding_revision == original.binding_revision
        && before.status != RecordState::Pending;
    let restored = before.active_revision == original.active_revision
        && same_parameter_values(&before.values, &original.values);
    let applied = before.previous_revision.as_deref() == Some(original.active_revision.as_str())
        && same_parameter_values(&before.values, candidate);
    if identity && (restored || applied) {
        return Ok(());
    }
    Err(Failure::new(
        &texts::prop_commission_gain_transaction(),
        Cause::Runtime,
        NextAction::Restore,
    ))
}

/// The gain values the review read are the ones the runtime holds now (`recover`, `proportionalCommissioning.ts`; B6.4).
///
/// # Errors
/// A value the review read changed while the Manual transfer was reviewed.
#[allow(clippy::float_cmp)] // The gains are compared exactly: a changed gain is a changed gain.
pub fn check_gain_unchanged(
    before: &ParameterStatus,
    observed: &BTreeMap<String, TransferValue>,
) -> Result<(), Failure> {
    let changed = before.values.iter().any(|(symbol, value)| {
        observed
            .get(symbol)
            .copied()
            .and_then(TransferValue::as_number)
            != Some(*value)
    });
    if changed {
        return Err(Failure::new(
            &texts::prop_commission_gain_changed_review(),
            Cause::Runtime,
            NextAction::Restore,
        ));
    }
    Ok(())
}

/// The program a recovery writes to is the campaign's (`recover`, `proportionalCommissioning.ts`; A6.7): a reloaded program is a new
/// identity, and no write goes to it.
///
/// # Errors
/// The program revision differs from the campaign's.
pub fn check_program_identity(
    original_program: &str,
    current_program: &str,
) -> Result<(), Failure> {
    if original_program == current_program {
        return Ok(());
    }
    Err(Failure::new(
        &texts::prop_commission_program_identity(),
        Cause::Runtime,
        NextAction::None,
    ))
}

/// Whether a recovery must write the Manual state (`recover`, `proportionalCommissioning.ts`; B6.4): the loop is not in its Manual
/// mode, or one of the step's states differs from the value the runtime holds.
#[must_use]
#[allow(clippy::float_cmp)] // The states are compared exactly, as the runtime reports them.
pub fn manual_transfer_needed(
    binding: &TransferBinding,
    step: &TransferStep,
    current: &BTreeMap<String, TransferValue>,
) -> bool {
    current.get(&binding.mode.symbol) != Some(&TransferValue::Boolean(binding.mode.manual_value))
        || step.values.iter().any(|(symbol, value)| {
            current
                .get(symbol)
                .copied()
                .and_then(TransferValue::as_number)
                != Some(*value)
        })
}

/// The parameter application of a recovery, read back (`recover`, `proportionalCommissioning.ts`; A6.8): whether the original gain set
/// is already in force, so that no rollback needs writing.
///
/// # Errors
/// The record's program or binding changed, or a parameter transaction is still pending, so the state must be inspected first.
pub fn parameter_already_restored(
    original: &RecoveryOrigin,
    status: &ParameterStatus,
) -> Result<bool, Failure> {
    if status.program_revision != original.program_revision
        || status.binding_revision != original.binding_revision
        || status.status == RecordState::Pending
    {
        return Err(Failure::new(
            &texts::prop_commission_parameter_unresolved(),
            Cause::Runtime,
            NextAction::None,
        ));
    }
    Ok(status.active_revision == original.active_revision
        && same_parameter_values(&status.values, &original.values))
}

/// The rollback the runtime offers for a recovery matches the campaign (`recover`, `proportionalCommissioning.ts`; B6.4): it returns to
/// the original set from the candidate, on the same program and binding, and the review's values are the original ones.
///
/// # Errors
/// The rollback review does not match the campaign, so Restore must be reviewed again.
pub fn check_rollback_review(
    original: &RecoveryOrigin,
    candidate: &BTreeMap<String, f64>,
    observed: &ParameterStatus,
    review_values: &BTreeMap<String, f64>,
) -> Result<(), Failure> {
    let matches = observed.program_revision == original.program_revision
        && observed.binding_revision == original.binding_revision
        && observed.previous_revision.as_deref() == Some(original.active_revision.as_str())
        && same_parameter_values(review_values, &original.values)
        && same_parameter_values(&observed.values, candidate);
    if matches {
        return Ok(());
    }
    Err(Failure::new(
        &texts::prop_commission_rollback_mismatch(),
        Cause::Runtime,
        NextAction::Restore,
    ))
}

/// The Auto entry that returns a recovered loop to Auto (`returnToFoundMode`, `proportionalCommissioning.ts`; A6.2, C13.9): once the
/// previous gain is back, a loop found in Auto re-enters Auto, aligned to the restored gain and allowed the noise of its restored
/// set. A loop found in Manual, or in a mode the runtime does not report as Manual, stays where it is, and so does a loop whose Auto
/// entry cannot be built from the restored set.
///
/// # Errors
/// The Auto entry cannot be built from the restored gain, for the reasons of [`auto_entry_step`].
pub fn found_mode_return(
    loop_definition: &LoopDefinition,
    binding: &TransferBinding,
    task: &EngineerRequirements,
    found: Option<FoundMode>,
    current: &BTreeMap<String, TransferValue>,
    restored: &BTreeMap<String, f64>,
    process_noise: Option<f64>,
) -> Result<Option<AutoReentry>, Failure> {
    let mut failure = None;
    let entry = reenter_found_mode(
        found,
        &binding.mode,
        current,
        |observed| match auto_entry_step(
            loop_definition,
            binding,
            task,
            restored,
            process_noise,
            observed,
        ) {
            Ok(step) => Some(AutoReentry {
                values: step.values,
                tolerance: step.tolerance,
                tracking: step.tracking,
            }),
            Err(error) => {
                failure = Some(error);
                None
            }
        },
    );
    match failure {
        Some(error) => Err(error),
        None => Ok(entry),
    }
}

/// The result a Restore leaves once the runtime confirms the previous settings (`rollback`, `proportionalCommissioning.ts`; B6.4): the
/// recovery is confirmed, the failure is cleared, and the reason names the mode the loop is in. The caller sets the parameter record
/// and the transfers it read.
#[must_use]
pub fn restored_result(
    mut result: CommissioningResult,
    found_in_auto: bool,
) -> CommissioningResult {
    result.recovery_confirmed = true;
    result.failure = None;
    result.reason = texts::prop_commission_recovered(found_in_auto);
    result
}

/// The result a Restore leaves when the runtime does not confirm the previous settings (`rollback`, `proportionalCommissioning.ts`;
/// B6.7): the run failed, the recovery is unconfirmed, and Restore is offered again.
#[must_use]
pub fn restore_failed(mut result: CommissioningResult, error: &str) -> CommissioningResult {
    let reason = texts::prop_commission_recovery_unconfirmed(error);
    result.status = CommissioningStatus::Failed;
    result.recovery_confirmed = false;
    result.failure = Some(restore_again(result.failure.take(), &reason));
    result.reason = reason;
    result
}

/// Whether a cancel leaves a finished campaign as it is (`cancel`, `proportionalCommissioning.ts`; A10.4): a passed campaign whose
/// recovery is confirmed is a completed record, and cancelling it is not a rollback.
#[must_use]
pub fn cancel_keeps_result(result: &CommissioningResult) -> bool {
    result.status == CommissioningStatus::Passed && result.recovery_confirmed
}

/// The result of a cancelled campaign whose previous settings are back (`cancel`, `proportionalCommissioning.ts`; B6.4, A6.2).
#[must_use]
pub fn cancel_recovered(
    mut result: CommissioningResult,
    found_in_auto: bool,
) -> CommissioningResult {
    result.recovery_confirmed = true;
    result.reason = texts::prop_commission_cancelled_recovered(found_in_auto);
    result
}

/// The result of a cancelled campaign whose recovery is not confirmed (`cancel`, `proportionalCommissioning.ts`; B6.7).
#[must_use]
pub fn cancel_unconfirmed(mut result: CommissioningResult, error: &str) -> CommissioningResult {
    result.recovery_confirmed = false;
    result.reason = texts::prop_commission_recovery_unconfirmed(error);
    result
}

/// The result of a campaign whose panel closed while its recovery is not confirmed (`settleForClose`, `proportionalCommissioning.ts`;
/// B6.4): the reason keeps the campaign's own sentence and adds the unconfirmed close.
#[must_use]
pub fn close_unconfirmed(mut result: CommissioningResult, error: &str) -> CommissioningResult {
    result.recovery_confirmed = false;
    result.reason = texts::prop_commission_close_unconfirmed(&result.reason, error);
    result
}

#[cfg(test)]
#[path = "proportional_recovery_tests.rs"]
mod tests;
