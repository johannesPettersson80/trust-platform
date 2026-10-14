//! The recovery of a Control.PID run (`pidCommissioning.ts`: `recover`, `reviewRecovery`, `cancel`, `rollback` and `returnToFoundMode`;
//! specification 35, B6.4, B6.7, A6.2, A6.3): the checks that decide whether a Restore may write, the Manual transfer a recovery needs,
//! the rollback of the parameter record, the return to the mode the loop was found in, and the result that a Restore, a cancel or a
//! close leaves.
//!
//! A run that failed in this session recovers as its own [`PidCampaign`]. A saved campaign recovers as a [`RecoveryStart`], accepted at
//! the transfer identity the runtime reports now; it becomes a [`PidCampaign`] whose `recovery_values` are set. The rules are methods of
//! `PidCampaign`, so one recovery serves both, as the `TypeScript` class does.
//!
//! The caller performs the runtime's work in this order (`recover`), passing each reading here:
//! 1. cancel the parameter owner and the transfer, when the run owns them ([`recovery_needed`] says whether there is anything to write);
//! 2. read the parameter record and check it with [`PidCampaign::check_ownership`];
//! 3. read the transfer and check it with [`PidCampaign::check_program`];
//! 4. write the Manual transfer of [`PidCampaign::recovery_manual_step`], when it is `Some`;
//! 5. when the run applied a record, read it and check it with [`PidCampaign::check_parameters_settled`]; when
//!    [`PidCampaign::rollback_needed`], review the rollback, check it with [`PidCampaign::check_rollback`], and apply it;
//! 6. read the transfer and write the Auto entry of [`PidCampaign::found_mode_return`], when it is `Some`;
//! 7. the recovery is confirmed ([`PidCampaign::restore_confirmed`] or [`PidCampaign::cancel_confirmed`]).
//!
//! Left out, as transport: the review's and the rollback's clients (`reviewRollback`, `rollback`, `cancelOwner`, `status`), the transfer
//! client (`apply`, `cancel`, `status`), the connection and its live-write precondition, the journal saves, the recording stop, and the
//! owner that a rollback review names (`ParameterOwner`), which the caller keeps. The rules that the proportional run shares unchanged
//! with this one are re-exported here: [`restore_failed`] and [`cancel_keeps_result`].

use std::collections::BTreeMap;

use crate::failure::{Cause, Failure, NextAction};
use crate::loop_definition::LoopDefinition;
use crate::model::GeneralModel;
use crate::recording::RecordedRun;
use crate::validation::engineer_verification_plan::EngineerRequirements;

use super::commissioning_result::{CommissioningResult, TransferState, TransferStatus};
use super::entry_allowance::{auto_entry_error, auto_entry_tolerance, transfer_tolerance};
use super::found_mode::{reenter_found_mode, reviewed_mode_of, AutoReentry, FoundMode};
use super::parameter_application::ParameterReview;
use super::parameter_status::{same_parameter_values, ParameterStatus, RecordState};
use super::pid_commissioning::{number_at, PidCampaign, PidRunProfile, PidStart};
use super::proportional_commissioning::{binding_failure, restore_again, TransferStep};
use super::proportional_recovery;
use super::texts;
use super::transfer::{TransferModeBinding, TransferValue};

pub use super::proportional_recovery::{cancel_keeps_result, restore_failed};

/// What a Restore of a saved Control.PID campaign is reviewed with (`reviewRecovery`'s arguments; B6.4): the loop and its model, the
/// requirements, the controller structure, the rollback review of the runtime's record, and the report the campaign stored.
pub struct RecoverySetup<'a> {
    /// The loop the campaign commissioned.
    pub loop_definition: &'a LoopDefinition,
    /// The model the campaign is restored on.
    pub model: &'a GeneralModel,
    /// The requirements of the campaign.
    pub task: &'a EngineerRequirements,
    /// The controller structure of the campaign.
    pub profile: PidRunProfile,
    /// The rollback review the Restore is made in.
    pub review: ParameterReview,
    /// The report the campaign stored.
    pub report: &'a CommissioningResult,
}

/// A saved campaign's recovery before the runtime's transfer identity is read (`reviewRecovery`, its first part; B6.4): the stored report
/// passes the checks that make its settings recoverable, and the original values of its first transfer are known.
pub struct RecoveryStart<'a> {
    start: PidStart<'a>,
    first: TransferStatus,
    initial: BTreeMap<String, TransferValue>,
    reference: Option<f64>,
    found_mode: Option<FoundMode>,
}

impl<'a> RecoveryStart<'a> {
    /// Begins the review of a saved campaign's recovery (`reviewRecovery`; B6.4): a campaign that already recovered in Manual has no
    /// rollback left, and the stored report must match the reviewed runtime, its model and the rollback.
    ///
    /// # Errors
    /// The campaign already recovered in Manual, the stored report does not match the reviewed runtime, the model or the rollback, or the
    /// transfer binding does not fit the loop.
    pub fn begin(setup: RecoverySetup<'a>) -> Result<Self, Failure> {
        let binding = setup.profile.binding(setup.loop_definition)?;
        assert_recovery_needed(setup.report, &binding.mode)?;
        check_saved_campaign(
            setup.report,
            &setup.model.identity().revision,
            &setup.review,
        )?;
        let first = setup
            .report
            .transfers
            .first()
            .ok_or_else(saved_mismatch)?
            .clone();
        let initial = first
            .transaction
            .as_ref()
            .and_then(|transaction| transaction.initial_values.clone())
            .unwrap_or_default();
        let start = PidStart::recovery(
            setup.loop_definition,
            setup.model,
            setup.task,
            setup.profile,
            setup.review,
        )?;
        let reference = start
            .binding
            .states
            .first()
            .and_then(|state| initial.get(&state.symbol))
            .copied()
            .and_then(TransferValue::as_number);
        Ok(Self {
            start,
            first,
            initial,
            reference,
            found_mode: setup.report.found_mode,
        })
    }

    /// Accepts the runtime's transfer identity (`reviewRecovery`, its second part; B6.4): the transfer is the one the report recorded and
    /// is not pending, the original reference is a finite number, and every reference the campaign keeps was recorded.
    ///
    /// # Errors
    /// The transfer identity changed or is pending, the original reference is missing, or a kept reference was not recorded.
    pub fn accept(self, current: &TransferStatus) -> Result<PidCampaign<'a>, Failure> {
        let identity_held = current.program_revision == self.first.program_revision
            && current.binding_revision == self.first.binding_revision
            && current.status != TransferState::Pending;
        let Some(previous_reference) = self
            .reference
            .filter(|value| value.is_finite() && identity_held)
        else {
            return Err(identity_failure());
        };
        let kept = self.start.profile.references(&self.start.binding);
        let references = original_references(&self.initial, &kept)?;
        let reviewed_mode = reviewed_mode_of(self.found_mode, self.start.binding.mode.manual_value);
        let recovery_values = self.start.review.values.clone();
        Ok(PidCampaign {
            start: self.start,
            references,
            previous_reference,
            reviewed_mode,
            recovery_values: Some(recovery_values),
        })
    }
}

/// Refuses a Restore of a saved campaign that already recovered in Manual (`assertRecoveryNeeded`; B6.4): the campaign confirmed its
/// recovery, and its last transfer is output-image confirmed in the Manual mode, so no rollback is left.
///
/// # Errors
/// The report's recovery is confirmed and its last transfer is output-image confirmed in the Manual mode.
pub fn assert_recovery_needed(
    report: &CommissioningResult,
    mode: &TransferModeBinding,
) -> Result<(), Failure> {
    // The rule is the proportional campaign's (`assert_recovery_needed`): the same check of the recorded transfers. This run words its
    // refusal in its own sentence.
    proportional_recovery::assert_recovery_needed(report, &mode.symbol, mode.manual_value).map_err(
        |_| {
            Failure::new(
                &texts::pid_commission_already_recovered(),
                Cause::Unclassified,
                NextAction::None,
            )
        },
    )
}

/// Whether a recovery has anything to write back (`cancel` and `recover`; B6.4): a run that applied no record and owns no transfer has
/// nothing to restore, so its settings are back at once.
#[must_use]
pub fn recovery_needed(parameter_applied: bool, transfer_owned: bool) -> bool {
    parameter_applied || transfer_owned
}

/// The result of a cancelled run whose recovery stays unresolved (`cancel`; B6.7): the reason says so, and the failure offers Restore
/// again.
#[must_use]
pub fn cancel_unresolved(mut result: CommissioningResult, error: &str) -> CommissioningResult {
    let reason = texts::pid_commission_cancel_unresolved(error);
    result.recovery_confirmed = false;
    result.failure = Some(restore_again(result.failure.take(), &reason));
    result.reason = reason;
    result
}

impl PidCampaign<'_> {
    /// The values a Restore writes back (`restoreValues`; B6.4): the values the campaign reviewed, or for a recovery of a saved campaign its
    /// rollback values.
    #[must_use]
    pub fn restore_values(&self) -> &BTreeMap<String, f64> {
        self.recovery_values
            .as_ref()
            .unwrap_or(&self.start.review.observed.values)
    }

    /// The values a Restore's candidate holds (`candidateValues`; B6.4): the values the run wrote, or for a recovery of a saved campaign the
    /// values the runtime holds, which the saved campaign wrote.
    fn candidate_values(&self) -> &BTreeMap<String, f64> {
        if self.recovery_values.is_some() {
            &self.start.review.observed.values
        } else {
            &self.start.review.values
        }
    }

    /// Checks the parameter record a recovery reads before it writes (`recover`; A6.8, B6.4): the record is not pending and belongs to the
    /// reviewed program and binding, and it holds either the values a Restore writes back under the revision they were kept under, or the
    /// candidate the run applied over that revision. `applied_revision` is the revision the run's own apply queued, when it applied one.
    ///
    /// # Errors
    /// The record is pending, its identity changed, or another operation owns its values.
    pub fn check_ownership(
        &self,
        record: &ParameterStatus,
        applied_revision: Option<&str>,
    ) -> Result<(), Failure> {
        let reviewed = &self.start.review.observed;
        let (original, applied) = if self.recovery_values.is_some() {
            (
                reviewed.previous_revision.as_deref(),
                applied_revision.or(Some(reviewed.active_revision.as_str())),
            )
        } else {
            (Some(reviewed.active_revision.as_str()), applied_revision)
        };
        let restored = original == Some(record.active_revision.as_str())
            && same_parameter_values(&record.values, self.restore_values());
        let applied_held = same_parameter_values(&record.values, self.candidate_values())
            && match applied {
                Some(revision) => record.active_revision == revision,
                None => record.previous_revision.as_deref() == original,
            };
        let identity_held = record.status != RecordState::Pending
            && record.program_revision == reviewed.program_revision
            && record.binding_revision == reviewed.binding_revision;
        if identity_held && (restored || applied_held) {
            return Ok(());
        }
        Err(Failure::new(
            &texts::pid_commission_ownership_changed(),
            Cause::Runtime,
            NextAction::Restore,
        ))
    }

    /// Checks the program a recovery writes to (`recover`; A6.7): a reloaded program is a new identity, and nothing is written to it.
    ///
    /// # Errors
    /// The transfer's program differs from the reviewed one.
    pub fn check_program(&self, current: &TransferStatus) -> Result<(), Failure> {
        if current.program_revision == self.start.review.observed.program_revision {
            return Ok(());
        }
        Err(Failure::new(
            &texts::pid_commission_program_identity(),
            Cause::Runtime,
            NextAction::None,
        ))
    }

    /// The Manual transfer a recovery writes (`recover`; A6.2, B6.4), or `None` when the loop is in Manual with every reference held. The
    /// transfer holds the structure's states as the runtime reports them, sets each reference back to the value the review read, and sets
    /// the manual command to the output's current value. It writes no other state.
    ///
    /// # Errors
    /// A reference or the output has no finite value, or the structure's states do not fit the loop.
    pub fn recovery_manual_step(
        &self,
        current: &TransferStatus,
    ) -> Result<Option<TransferStep>, Failure> {
        let binding = &self.start.binding;
        let mode_held = current.values.get(&binding.mode.symbol)
            == Some(&TransferValue::Boolean(binding.mode.manual_value));
        if mode_held && self.references_held(&current.values)? {
            return Ok(None);
        }
        let mut values = self.start.profile.states(binding, &current.values)?;
        values.extend(self.references.clone());
        let manual = binding.states.get(1).ok_or_else(binding_failure)?;
        values.insert(
            manual.symbol.clone(),
            number_at(&current.values, &binding.output.symbol)?,
        );
        Ok(Some(TransferStep {
            values,
            tolerance: transfer_tolerance(
                self.start.loop_definition,
                self.start.task.transfer_tolerance,
            )?,
            tracking: self
                .start
                .profile
                .tracking(self.start.loop_definition, binding, false)?,
        }))
    }

    /// Checks the parameter record after the Manual transfer, when the run applied a record (`recover`; A6.8, B6.4): it is not pending and
    /// belongs to the reviewed program and binding.
    ///
    /// # Errors
    /// The record is pending, or its program or binding changed.
    pub fn check_parameters_settled(&self, record: &ParameterStatus) -> Result<(), Failure> {
        let reviewed = &self.start.review.observed;
        if record.status != RecordState::Pending
            && record.program_revision == reviewed.program_revision
            && record.binding_revision == reviewed.binding_revision
        {
            return Ok(());
        }
        Err(Failure::new(
            &texts::pid_commission_parameter_unresolved(),
            Cause::Runtime,
            NextAction::None,
        ))
    }

    /// Whether the record must be rolled back to the values a Restore writes (`recover`; B6.4): the values in force differ from them.
    #[must_use]
    pub fn rollback_needed(&self, record: &ParameterStatus) -> bool {
        !same_parameter_values(&record.values, self.restore_values())
    }

    /// Checks the rollback the runtime offers before the run writes it (`recover`; B6.4): it holds the values a Restore writes back.
    ///
    /// # Errors
    /// The rollback's values differ from the values a Restore writes back.
    pub fn check_rollback(&self, rollback: &ParameterReview) -> Result<(), Failure> {
        if same_parameter_values(&rollback.values, self.restore_values()) {
            return Ok(());
        }
        Err(Failure::new(
            &texts::pid_commission_rollback_mismatch(),
            Cause::Runtime,
            NextAction::Restore,
        ))
    }

    /// The Auto entry that returns a recovered loop to Auto (`returnToFoundMode`; A6.2, A6.4, B6.4): a loop found in Auto that the runtime
    /// reads in Manual re-enters Auto, with the allowance of the restored record and the states the runtime holds. A loop found in Manual,
    /// or in a mode the runtime does not report as Manual, stays where it is.
    ///
    /// # Errors
    /// The re-entry cannot be built: a state is missing, or its allowance cannot be built from the restored record.
    pub fn found_mode_return(
        &self,
        current: &TransferStatus,
    ) -> Result<Option<AutoReentry>, Failure> {
        let mut failure = None;
        let entry = reenter_found_mode(
            self.found_mode(),
            &self.start.binding.mode,
            &current.values,
            |observed| match self.reentry(observed) {
                Ok(reentry) => Some(reentry),
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

    /// The Auto entry of a recovered loop at the states `observed` holds (`reenterFoundMode`'s entry; A6.3, A6.4).
    fn reentry(&self, observed: &BTreeMap<String, TransferValue>) -> Result<AutoReentry, Failure> {
        let loop_definition = self.start.loop_definition;
        Ok(AutoReentry {
            values: self.start.profile.states(&self.start.binding, observed)?,
            tolerance: auto_entry_tolerance(
                loop_definition,
                self.start.task,
                self.restore_values(),
                self.start.process_noise,
                auto_entry_error(loop_definition, observed),
            )?,
            tracking: self
                .start
                .profile
                .tracking(loop_definition, &self.start.binding, true)?,
        })
    }

    /// The result a Restore leaves once the runtime confirms the previous settings (`rollback`; B6.4): the record and the transfers it read,
    /// the recovery confirmed, the failure cleared, and the reason naming the mode the loop is confirmed in.
    #[must_use]
    pub fn restore_confirmed(
        &self,
        mut result: CommissioningResult,
        parameters: ParameterStatus,
        transfers: Vec<TransferStatus>,
    ) -> CommissioningResult {
        result.parameters = Some(parameters);
        result.transfers = transfers;
        result.recovery_confirmed = true;
        result.failure = None;
        result.reason = texts::pid_commission_rollback_confirmed(self.found_in_auto());
        result
    }

    /// The result of a cancelled run whose previous settings and recording stop are confirmed (`cancel`; B6.4): the recording the stop kept
    /// when there is one, the record and the transfers it read, the recovery confirmed, and the mode named. The failure stays.
    #[must_use]
    pub fn cancel_confirmed(
        &self,
        mut result: CommissioningResult,
        recording: Option<RecordedRun>,
        parameters: ParameterStatus,
        transfers: Vec<TransferStatus>,
    ) -> CommissioningResult {
        if let Some(recording) = recording {
            result.recording = Some(recording);
        }
        result.parameters = Some(parameters);
        result.transfers = transfers;
        result.recovery_confirmed = true;
        result.reason = texts::pid_commission_cancel_confirmed(self.found_in_auto());
        result
    }
}

/// The refusal of a saved campaign that the reviewed runtime does not match (`reviewRecovery`; B6.4).
fn saved_mismatch() -> Failure {
    Failure::new(
        &texts::pid_commission_saved_mismatch(),
        Cause::Settings,
        NextAction::None,
    )
}

/// The refusal of a transfer whose identity changed, or whose result is still pending (`reviewRecovery`; B6.4).
fn identity_failure() -> Failure {
    Failure::new(
        &texts::pid_commission_identity_changed(),
        Cause::Runtime,
        NextAction::None,
    )
}

/// Whether the stored campaign is the one the reviewed runtime holds (`reviewRecovery`; B6.4): the model it names is the one reviewed,
/// its first transfer recorded its original values, its parameter record is the one the review reads, and the rollback holds the values
/// the campaign kept.
///
/// # Errors
/// The campaign does not match the reviewed runtime, the model or the rollback values.
fn check_saved_campaign(
    report: &CommissioningResult,
    model_revision: &str,
    review: &ParameterReview,
) -> Result<(), Failure> {
    let recorded = report
        .transfers
        .first()
        .and_then(|transfer| transfer.transaction.as_ref())
        .is_some_and(|transaction| transaction.initial_values.is_some());
    let matches = recorded
        && model_revision == report.model_revision
        && report.parameters.as_ref().is_some_and(|saved| {
            review.observed.program_revision == saved.program_revision
                && review.observed.binding_revision == saved.binding_revision
                && review.observed.active_revision == saved.active_revision
                && same_parameter_values(&review.observed.values, &saved.values)
                && saved
                    .previous_values
                    .as_ref()
                    .is_some_and(|previous| same_parameter_values(&review.values, previous))
        });
    if matches {
        return Ok(());
    }
    Err(saved_mismatch())
}

/// The original references of a saved campaign, as its first transfer recorded them (`reviewRecovery`; B6.4): each kept reference holds a
/// finite number, which a Restore writes back.
///
/// # Errors
/// A kept reference is not a finite number in the recorded values.
fn original_references(
    initial: &BTreeMap<String, TransferValue>,
    references: &[String],
) -> Result<BTreeMap<String, f64>, Failure> {
    let mut originals = BTreeMap::new();
    for symbol in references {
        let original = initial
            .get(symbol)
            .copied()
            .and_then(TransferValue::as_number)
            .filter(|number| number.is_finite())
            .ok_or_else(|| {
                Failure::new(
                    &texts::pid_commission_reference_unavailable(symbol),
                    Cause::ModelOrData,
                    NextAction::None,
                )
            })?;
        originals.insert(symbol.clone(), original);
    }
    Ok(originals)
}

#[cfg(test)]
#[path = "pid_recovery_tests.rs"]
mod tests;
