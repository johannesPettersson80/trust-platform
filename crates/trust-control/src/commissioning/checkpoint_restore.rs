//! The rules of Restore previous settings from a journaled commissioning checkpoint (specification 35, B6.4, A6.2, A6.4, A8.5, A8.6,
//! B6.7; annex E6.10, E7.5, E7.6, E11.4, E11.7, E11.9), from `interruptedCommissioningRecovery.ts` (`review`, `rollback`,
//! `recoveryStates`, `reenterable`, `autoReentry`, `entryTolerance`, `assertParameterOwner`) and `applyObservation.ts`
//! (`stopRecovered`, `incompleteRecording`): whether the journal holds settings to restore, whether the runtime and the checkpoint are
//! still the ones Apply wrote, whether the parameter record and the transfer are the checkpoint's own, the Manual transfer a Restore
//! writes and when it needs one, when the record is rolled back, the return to the mode the loop was found in, and the evidence of a
//! recording the runtime no longer holds.
//!
//! A Restore never replays a write (E11.4): it starts from the checkpoint and the runtime's current answers, and a step the runtime
//! already confirmed is not repeated (B6.4). Each rule is pure and takes the runtime's answers as data. The engine reads and writes the
//! runtime in the order of `rollback` (`workflow/restore.rs` of trust-control-studio):
//! 1. cancel the checkpoint's parameter operation and transfer by their identities;
//! 2. read the parameter record and check it with [`check_owned_parameters`], and the transfer with [`check_transfer_settled`];
//! 3. write the Manual transfer of [`restore_states`] when [`restore_transfer_needed`] says so;
//! 4. read the record again, check it, and roll it back when [`rollback_needed`], once [`check_rollback_values`] holds;
//! 5. stop a recording the checkpoint holds without a result; one the runtime no longer holds is kept as [`incomplete_recording`];
//! 6. write the Auto entry of [`restore_auto_entry`], when it is `Some`.
//!
//! The review checks, in order, [`restorable`], [`check_runtime_identity`], [`check_checkpoint_identity`],
//! [`check_restore_available`], [`check_reviewed_parameters`] and [`check_transfer_owner`]. The sentences are in [`texts`].

use std::collections::BTreeMap;

use super::actuator_feedback::transfer_target_allowance;
use super::adapters::controller_adapter;
use super::application_records::ReviewedParameter;
use super::application_texts::physical_approval_needed;
use super::checkpoint::{CheckpointStage, CommissioningCheckpoint};
use super::commissioning_result::{TransferState, TransferStatus};
use super::entry_allowance::{auto_entry_error, auto_entry_tolerance};
use super::found_mode::{found_mode_of, reenter_found_mode, AutoReentry, FoundMode};
use super::parameter_application::ParameterReview;
use super::parameter_status::{same_parameter_values, ParameterStatus, RecordState};
use super::profile::CommissioningProfile;
use super::proportional_commissioning::{auto_entry_step, binding_failure};
use super::restore_availability::restore_unavailable_reason;
use super::texts::pid_commission_ownership_changed;
use super::transfer::{TransferBinding, TransferValue};
use super::transfer_tracking::{parallel_pid_tracking, proportional_tracking, TransferTracking};
use crate::experiment::RecordingRecoveryOwner;
use crate::failure::{Cause, Failure, NextAction};
use crate::limits::{RECORDING_DURATION_LIMIT_MS, RECORDING_SAMPLE_LIMIT};
use crate::loop_definition::LoopDefinition;
use crate::recording::{Acquisition, Completeness, RecordedRun, RecordingRequest, StopReason};
use crate::signals::SignalBinding;

pub mod texts;

/// The checkpoint a Restore starts from (B6.4, A8.5): the one the loop's journal holds while an Apply's settings may be restored. A
/// journal without one, or with one whose previous settings are back already or that is kept as history, has nothing to restore.
///
/// # Errors
/// The journal holds no checkpoint, or its checkpoint is recovered or retired.
pub fn restorable(
    checkpoint: Option<CommissioningCheckpoint>,
    loop_id: &str,
) -> Result<CommissioningCheckpoint, Failure> {
    let Some(checkpoint) = checkpoint else {
        return Err(Failure::new(
            &texts::no_applied_settings(loop_id),
            Cause::Settings,
            NextAction::None,
        ));
    };
    let stage = checkpoint.stage;
    if matches!(stage, CheckpointStage::Recovered | CheckpointStage::Retired) {
        return Err(Failure::new(
            &texts::checkpoint_closed(&checkpoint.id, stage == CheckpointStage::Retired),
            Cause::Settings,
            NextAction::None,
        ));
    }
    Ok(checkpoint)
}

/// Refuses a Restore on a runtime that was restarted, or whose program was reloaded, since Apply (B6.4, A6.7; the restarted runtime of
/// `reviewInterruptedRollback`, engineerActions/recovery.ts): nothing of the checkpoint can be restored on a new program, so the
/// checkpoint stays as history and the loop continues from a new validation. A record of another program that is still pending is
/// left to the ownership check, as the `TypeScript` leaves it.
///
/// # Errors
/// The record's program differs from the one Apply found, and no transaction of it is pending.
pub fn check_runtime_identity(
    checkpoint: &CommissioningCheckpoint,
    observed: &ParameterStatus,
) -> Result<(), Failure> {
    if observed.program_revision == checkpoint.origin.program_revision
        || observed.status == RecordState::Pending
    {
        return Ok(());
    }
    Err(Failure::new(
        &texts::runtime_replaced(
            &checkpoint.origin.program_revision,
            &observed.program_revision,
        ),
        Cause::Runtime,
        NextAction::Validate,
    ))
}

/// Checks that the checkpoint belongs to the current controller (`review`, interruptedCommissioningRecovery.ts; B6.4): it was written for
/// the loop's revision, and it names the transfer binding and the recovery tracking that the loop's adapter gives now
/// (`ControllerAdapter::transfer_binding`, and `recovery_tracking` of the checkpoint's model).
///
/// # Errors
/// The loop revision, the binding or the tracking differs from the checkpoint's.
pub fn check_checkpoint_identity(
    loop_revision: &str,
    binding: &TransferBinding,
    recovery_tracking: &TransferTracking,
    checkpoint: &CommissioningCheckpoint,
) -> Result<(), Failure> {
    if checkpoint.loop_revision == loop_revision
        && checkpoint.binding == *binding
        && checkpoint.recovery_tracking == *recovery_tracking
    {
        return Ok(());
    }
    Err(Failure::new(
        &texts::checkpoint_mismatch(&checkpoint.loop_revision, loop_revision),
        Cause::Settings,
        NextAction::None,
    ))
}

/// Whether Restore may proceed on the record the runtime reports (B6.4; `restoreUnavailableReason`): while the record is still the one
/// Apply left (`applied`, the record of the Apply's receipt, when it has one), or is already back at the record Apply found, since a
/// rollback the runtime confirmed is not repeated. Any other change since Apply makes Restore unavailable, so another change is never
/// overwritten.
///
/// # Errors
/// The program, the active revision or the values changed since Apply, and the record is not back at the one Apply found.
pub fn check_restore_available(
    checkpoint: &CommissioningCheckpoint,
    observed: &ParameterStatus,
    applied: Option<&ParameterStatus>,
) -> Result<(), Failure> {
    if origin_in_force(checkpoint, observed) {
        return Ok(());
    }
    match restore_unavailable_reason(observed, applied) {
        None => Ok(()),
        Some(reason) => Err(Failure::new(&reason, Cause::Settings, NextAction::None)),
    }
}

/// Checks the parameter record that a Restore's review reads (`assertParameterOwner` with the owned operation allowed,
/// interruptedCommissioningRecovery.ts; B6.4, E11.7): as [`check_owned_parameters`], except that a pending transaction is allowed when it
/// is the checkpoint's own parameter operation, which the Restore cancels by its identity before it writes (E7.6, E11.4).
///
/// # Errors
/// The record is pending for another operation, its program or binding changed, or it holds neither the record Apply found nor the
/// candidate Apply wrote over it.
pub fn check_reviewed_parameters(
    checkpoint: &CommissioningCheckpoint,
    observed: &ParameterStatus,
) -> Result<(), Failure> {
    owner_check(checkpoint, observed, true)
}

/// Checks the parameter record before a Restore writes (`assertParameterOwner`, interruptedCommissioningRecovery.ts; A6.8, B6.4): it
/// belongs to the program and binding Apply found, no transaction is pending, and it holds either the record Apply found, under its
/// revision, or the candidate Apply wrote over it, with the record Apply found retained as its previous values. Another operation's
/// values are never overwritten.
///
/// # Errors
/// The record is pending, its program or binding changed, or another operation owns its values.
pub fn check_owned_parameters(
    checkpoint: &CommissioningCheckpoint,
    observed: &ParameterStatus,
) -> Result<(), Failure> {
    owner_check(checkpoint, observed, false)
}

/// The ownership check of both reads (`assertParameterOwner`): `allow_owned_pending` admits a pending transaction of the checkpoint's
/// own parameter operation.
fn owner_check(
    checkpoint: &CommissioningCheckpoint,
    current: &ParameterStatus,
    allow_owned_pending: bool,
) -> Result<(), Failure> {
    let origin = &checkpoint.origin;
    let adopted = current.previous_revision.as_deref() == Some(origin.active_revision.as_str())
        && current
            .previous_values
            .as_ref()
            .is_some_and(|previous| same_parameter_values(previous, &origin.values))
        && same_parameter_values(&current.values, &checkpoint.candidate);
    let pending_elsewhere = current.status == RecordState::Pending
        && (!allow_owned_pending
            || current.transaction_id.as_deref()
                != Some(checkpoint.parameter_operation.id.as_str()));
    let held = !pending_elsewhere
        && current.program_revision == origin.program_revision
        && current.binding_revision == origin.binding_revision
        && (origin_in_force(checkpoint, current) || adopted);
    if held {
        return Ok(());
    }
    Err(Failure::new(
        &pid_commission_ownership_changed(),
        Cause::Runtime,
        NextAction::None,
    ))
}

/// Whether `record` is the record Apply found: its revision and its values.
fn origin_in_force(checkpoint: &CommissioningCheckpoint, record: &ParameterStatus) -> bool {
    record.active_revision == checkpoint.origin.active_revision
        && same_parameter_values(&record.values, &checkpoint.origin.values)
}

/// Checks the transfer that a Restore's review reads (`review`, interruptedCommissioningRecovery.ts; B6.4, E6.3): it runs on the program
/// Apply found and, when the checkpoint owns a transfer, on that transfer's binding, and no transaction of another owner is pending.
///
/// # Errors
/// Another program or another transfer owns the runtime's transfer.
pub fn check_transfer_owner(
    checkpoint: &CommissioningCheckpoint,
    observed: &TransferStatus,
) -> Result<(), Failure> {
    let owner = checkpoint.transfer.as_ref();
    let binding_owned = owner.is_none_or(|owner| observed.binding_revision == owner.binding);
    let last = observed
        .transaction
        .as_ref()
        .map(|transaction| transaction.id.as_str());
    let pending_elsewhere =
        observed.status == TransferState::Pending && last != owner.map(|owner| owner.id.as_str());
    if observed.program_revision == checkpoint.origin.program_revision
        && binding_owned
        && !pending_elsewhere
    {
        return Ok(());
    }
    Err(Failure::new(
        &texts::transfer_owned_elsewhere(
            &observed.program_revision,
            &observed.binding_revision,
            last,
        ),
        Cause::Runtime,
        NextAction::None,
    ))
}

/// Checks the transfer once the checkpoint's owned operations are cancelled (`rollback`, interruptedCommissioningRecovery.ts; E6.10,
/// E11.7): it runs on the program Apply found and no transaction is pending, so the Restore's own transfer can be written.
///
/// # Errors
/// The program changed, or a transaction is still pending: the Restore is tried again once the runtime has ended it (B6.7).
pub fn check_transfer_settled(
    checkpoint: &CommissioningCheckpoint,
    current: &TransferStatus,
) -> Result<(), Failure> {
    let origin = &checkpoint.origin.program_revision;
    if current.program_revision != *origin {
        return Err(Failure::new(
            &texts::transfer_program_changed(origin, &current.program_revision),
            Cause::Runtime,
            NextAction::None,
        ));
    }
    if current.status == TransferState::Pending {
        let pending = current
            .transaction
            .as_ref()
            .map(|transaction| transaction.id.as_str());
        return Err(Failure::new(
            &texts::transfer_pending(pending),
            Cause::Runtime,
            NextAction::Restore,
        ));
    }
    Ok(())
}

/// Whether a Restore writes its Manual transfer (`rollback`, interruptedCommissioningRecovery.ts; A6.2, B6.4): the loop is not in its
/// Manual mode, or a reference differs from the value the checkpoint kept. A loop already in Manual with every reference held needs no
/// transfer, so the step is not repeated.
#[must_use]
#[allow(clippy::float_cmp)] // The references are compared exactly, as the runtime reports them (the TypeScript's `!==`).
pub fn restore_transfer_needed(
    checkpoint: &CommissioningCheckpoint,
    current: &BTreeMap<String, TransferValue>,
) -> bool {
    let mode = &checkpoint.binding.mode;
    current.get(&mode.symbol) != Some(&TransferValue::Boolean(mode.manual_value))
        || checkpoint.references.iter().any(|(symbol, value)| {
            current
                .get(symbol)
                .copied()
                .and_then(TransferValue::as_number)
                != Some(*value)
        })
}

/// The states a Restore's transfers write (`recoveryStates`, interruptedCommissioningRecovery.ts; B6.4, C0 "Recovery restores the
/// original reference vector"): each declared state as the runtime reports it, with the references the checkpoint kept written over
/// them. Live state comes from current observations and is never replayed from old history.
///
/// # Errors
/// A declared state is not reported as a finite number.
pub fn restore_states(
    checkpoint: &CommissioningCheckpoint,
    observed: &BTreeMap<String, TransferValue>,
) -> Result<BTreeMap<String, f64>, Failure> {
    let mut states = BTreeMap::new();
    for member in &checkpoint.binding.states {
        let value = observed
            .get(&member.symbol)
            .copied()
            .and_then(TransferValue::as_number)
            .filter(|value| value.is_finite())
            .ok_or_else(|| {
                Failure::new(
                    &texts::recovery_state_unavailable(&member.symbol),
                    Cause::Runtime,
                    NextAction::None,
                )
            })?;
        states.insert(member.symbol.clone(), value);
    }
    states.extend(checkpoint.references.clone());
    Ok(states)
}

/// Whether the record must be rolled back (`rollback`, interruptedCommissioningRecovery.ts; B6.4, E7.5): its values differ from the
/// record Apply found. A record already back is not rolled back again.
#[must_use]
pub fn rollback_needed(checkpoint: &CommissioningCheckpoint, record: &ParameterStatus) -> bool {
    !same_parameter_values(&record.values, &checkpoint.origin.values)
}

/// Checks the rollback the runtime offers before a Restore writes it (`rollback`, interruptedCommissioningRecovery.ts; E7.5): it returns
/// the record to the values Apply found.
///
/// # Errors
/// The retained previous values differ from the record Apply found.
pub fn check_rollback_values(
    checkpoint: &CommissioningCheckpoint,
    rollback: &ParameterReview,
) -> Result<(), Failure> {
    if same_parameter_values(&rollback.values, &checkpoint.origin.values) {
        return Ok(());
    }
    Err(Failure::new(
        &texts::rollback_record_changed(&rollback.values, &checkpoint.origin.values),
        Cause::Runtime,
        NextAction::None,
    ))
}

/// Whether the checkpoint's controller has an Auto entry (`reenterable`, interruptedCommissioningRecovery.ts; A6.2): the parallel PID and
/// the proportional law have one, and a declared controller such as `Control.PID` has one when its adapter declares a commissioning
/// profile for the restored record.
///
/// # Errors
/// The loop has no adapter, or its declared profile cannot be built from the checkpoint's model and the restored record.
pub fn reenterable(
    loop_definition: &LoopDefinition,
    checkpoint: &CommissioningCheckpoint,
) -> Result<bool, Failure> {
    match &checkpoint.recovery_tracking {
        TransferTracking::Proportional(_) | TransferTracking::ParallelPid(_) => Ok(true),
        TransferTracking::DeclaredState(_) => {
            Ok(declared_profile(loop_definition, checkpoint)?.is_some())
        }
    }
}

/// The Auto entry that returns a restored loop to the mode it was found in (`reenterFoundMode` with `autoReentry`,
/// interruptedCommissioningRecovery.ts; A6.2, A6.4, B6.4, E11.9). Once the previous settings are back, a loop found in Auto that the
/// runtime reads in Manual re-enters Auto, its state initialized from current observations, with the Auto-entry allowance of A6.4 for the
/// restored record, the noise Apply measured and the error the loop enters with (`entryTolerance`). The entry follows the checkpoint's
/// controller: a parallel PID writes its states as observed with the references the checkpoint kept; a proportional law aligns its
/// reference to the current output at the restored gain, as Apply's entry does; a declared controller enters through its declared
/// initialization with the restored record. Each aligns the output. A loop found in Manual or of unknown mode, a loop the runtime does
/// not read in Manual, and a declared controller without a commissioning profile stay where they are.
///
/// # Errors
/// The entry cannot be built: a state or the output is not reported, the restored gain cannot align the reference, the allowance cannot
/// be built, or the declared profile does not fit the loop.
pub fn restore_auto_entry(
    loop_definition: &LoopDefinition,
    checkpoint: &CommissioningCheckpoint,
    current: &BTreeMap<String, TransferValue>,
) -> Result<Option<AutoReentry>, Failure> {
    let mut failure = None;
    let entry = reenter_found_mode(
        checkpoint.found_mode,
        &checkpoint.binding.mode,
        current,
        |observed| match auto_entry(loop_definition, checkpoint, observed) {
            Ok(entry) => entry,
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

/// The Auto entry of the checkpoint's controller at the states `observed` holds (`autoReentry`); `None` for a declared controller
/// without a commissioning profile.
fn auto_entry(
    loop_definition: &LoopDefinition,
    checkpoint: &CommissioningCheckpoint,
    observed: &BTreeMap<String, TransferValue>,
) -> Result<Option<AutoReentry>, Failure> {
    let binding = &checkpoint.binding;
    let entry = match &checkpoint.recovery_tracking {
        TransferTracking::Proportional(_) => {
            // `proportionalEntryValues`: the reference aligned to the held output at the restored gain, and the Manual command at the held
            // output. The allowance is the entry's own (`entryTolerance`), with the error the loop enters with.
            let step = auto_entry_step(
                loop_definition,
                binding,
                &checkpoint.task,
                &checkpoint.origin.values,
                checkpoint.process_noise,
                observed,
            )?;
            AutoReentry {
                values: step.values,
                tolerance: entry_tolerance(loop_definition, checkpoint, observed)?,
                tracking: step.tracking,
            }
        }
        TransferTracking::ParallelPid(_) => AutoReentry {
            values: restore_states(checkpoint, observed)?,
            tolerance: entry_tolerance(loop_definition, checkpoint, observed)?,
            tracking: TransferTracking::ParallelPid(parallel_pid_tracking(
                loop_definition,
                binding,
                true,
            )?),
        },
        TransferTracking::DeclaredState(_) => {
            let Some(profile) = declared_profile(loop_definition, checkpoint)? else {
                return Ok(None);
            };
            AutoReentry {
                values: profile.states(binding, observed)?,
                tolerance: entry_tolerance(loop_definition, checkpoint, observed)?,
                tracking: profile.tracking(binding, true)?,
            }
        }
    };
    Ok(Some(entry))
}

/// The reference step that returns the references Apply found once the Restore has re-entered Auto (B6.4; C13 step 8), or `None`
/// when the loop holds them already. The Auto entry aligns the references to the measurement, so that its first output is bumpless
/// (A6.4, annex E6.4); this step is the deliberate change back, as Apply's target change is (B6.3): the states as the runtime reports
/// them with the kept references, the target allowance, and the tracking of a target change, which keeps the states.
///
/// # Errors
/// A declared state is not reported, the binding has no output to bound the step, or the tracking does not fit the loop.
pub fn restore_reference_step(
    loop_definition: &LoopDefinition,
    checkpoint: &CommissioningCheckpoint,
    observed: &BTreeMap<String, TransferValue>,
) -> Result<Option<AutoReentry>, Failure> {
    let holds_references = checkpoint.references.iter().all(|(symbol, value)| {
        observed
            .get(symbol)
            .copied()
            .and_then(TransferValue::as_number)
            == Some(*value)
    });
    if holds_references {
        return Ok(None);
    }
    let binding = &checkpoint.binding;
    let tracking = match &checkpoint.recovery_tracking {
        TransferTracking::Proportional(_) => {
            TransferTracking::Proportional(proportional_tracking(loop_definition, binding, false)?)
        }
        TransferTracking::ParallelPid(_) => {
            TransferTracking::ParallelPid(parallel_pid_tracking(loop_definition, binding, false)?)
        }
        TransferTracking::DeclaredState(_) => {
            let Some(profile) = declared_profile(loop_definition, checkpoint)? else {
                return Ok(None);
            };
            profile.tracking(binding, false)?
        }
    };
    Ok(Some(AutoReentry {
        values: restore_states(checkpoint, observed)?,
        tolerance: transfer_target_allowance(binding).ok_or_else(binding_failure)?,
        tracking,
    }))
}

/// The declared controller's commissioning profile with the restored record (`declaredProfile`), when the loop's adapter declares one.
fn declared_profile(
    loop_definition: &LoopDefinition,
    checkpoint: &CommissioningCheckpoint,
) -> Result<Option<CommissioningProfile>, Failure> {
    controller_adapter(loop_definition)?
        .declared_profile(&checkpoint.model, &checkpoint.origin.values)
}

/// The allowed output change of the Auto entry (`entryTolerance`, A6.3, A6.4): the allowance of Apply's entry, for the restored record,
/// the noise Apply measured and the error the loop enters with.
fn entry_tolerance(
    loop_definition: &LoopDefinition,
    checkpoint: &CommissioningCheckpoint,
    observed: &BTreeMap<String, TransferValue>,
) -> Result<f64, Failure> {
    auto_entry_tolerance(
        loop_definition,
        &checkpoint.task,
        &checkpoint.origin.values,
        checkpoint.process_noise,
        auto_entry_error(loop_definition, observed),
    )
}

/// The mode the runtime reports the loop in (A6.2), by the checkpoint's mode binding; `None` when the reading holds no BOOL for the mode.
#[must_use]
pub fn reported_mode(
    checkpoint: &CommissioningCheckpoint,
    values: &BTreeMap<String, TransferValue>,
) -> Option<FoundMode> {
    let mode = &checkpoint.binding.mode;
    let reading = match values.get(&mode.symbol) {
        Some(TransferValue::Boolean(value)) => Some(*value),
        _ => None,
    };
    found_mode_of(reading, mode.manual_value)
}

/// Each bound parameter of a Restore's review (§0.1b; B6.4): its symbol and unit as the loop's parameter binding declares them, the value
/// the runtime holds now, and the value the Restore returns it to, from the record Apply found. A value that a record does not hold is
/// not a number.
#[must_use]
pub fn reviewed_parameters(
    loop_definition: &LoopDefinition,
    checkpoint: &CommissioningCheckpoint,
    observed: &ParameterStatus,
) -> Vec<ReviewedParameter> {
    loop_definition
        .parameter_binding
        .iter()
        .flat_map(|binding| binding.parameters.iter())
        .map(|parameter| ReviewedParameter {
            symbol: parameter.symbol.clone(),
            unit: parameter.unit.clone(),
            running: observed
                .values
                .get(&parameter.symbol)
                .copied()
                .unwrap_or(f64::NAN),
            next: checkpoint
                .origin
                .values
                .get(&parameter.symbol)
                .copied()
                .unwrap_or(f64::NAN),
        })
        .collect()
}

/// Refuses a Restore on a physical target without the engineer's approval, before anything is written (A2.5, E10.2).
///
/// # Errors
/// The target is physical and the approval is missing.
pub fn check_restore_approval(physical: bool, physical_approved: bool) -> Result<(), Failure> {
    if physical && !physical_approved {
        return Err(physical_approval_needed());
    }
    Ok(())
}

/// The reason of a confirmed Restore (B6.4, A6.2): the previous values are back, and the sentence names the mode the Restore left the
/// loop in (`modeStatement`).
#[must_use]
pub fn restored_reason(
    found: Option<FoundMode>,
    reenterable: bool,
    returned_to_auto: bool,
) -> String {
    texts::restored_reason(texts::mode_statement(found, reenterable, returned_to_auto))
}

/// The identity of the recording a checkpoint retains, for its stop, its status and its release (E4.6, E4.7): its identifier, its loop
/// and program revisions and its bindings, as the owner journaled them. The limits belong to the start, which alone sends them, so they
/// are the protocol's own here.
#[must_use]
pub fn retained_recording_request(owner: &RecordingRecoveryOwner) -> RecordingRequest {
    RecordingRequest {
        recording_id: owner.recording_id.clone(),
        loop_revision: owner.loop_revision.clone(),
        program_revision: owner.program_revision.clone(),
        bindings: owner
            .bindings
            .iter()
            .map(|binding| SignalBinding {
                id: binding.id.clone(),
                address: binding.address.clone(),
                symbol: binding.symbol.clone(),
                iec_type: binding.iec_type.clone(),
            })
            .collect(),
        maximum_samples: RECORDING_SAMPLE_LIMIT,
        maximum_duration_ms: RECORDING_DURATION_LIMIT_MS,
        task: None,
        record_every: 1,
    }
}

/// The evidence of a recording that the checkpoint retains and the runtime no longer holds (`incompleteRecording` and `stopRecovered`,
/// applyObservation.ts; A8.6, E4.5): no samples, an incomplete acquisition with the runtime's reason, and a runtime boundary as the way it
/// ended, so that it is never read as complete evidence. The settings are restored all the same.
#[must_use]
pub fn incomplete_recording(owner: &RecordingRecoveryOwner, runtime_reason: &str) -> RecordedRun {
    let identity = retained_recording_request(owner);
    RecordedRun {
        recording_id: identity.recording_id,
        loop_revision: identity.loop_revision,
        program_revision: identity.program_revision,
        bindings: identity.bindings,
        stop_reason: StopReason::RuntimeBoundary,
        samples: Vec::new(),
        acquisition: Acquisition {
            method: "runtime_scan_buffer".to_owned(),
            completeness: Completeness::Incomplete,
            observed_sample_count: 0,
            scan_range: None,
            missing_scan_ranges: Vec::new(),
            dropped_observed_samples: 0,
            actual_cycle_timing_available: false,
            freshness_available: false,
            reason: texts::recording_not_held(runtime_reason),
        },
    }
}

#[cfg(test)]
#[path = "checkpoint_restore_tests.rs"]
mod tests;
