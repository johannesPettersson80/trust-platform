//! Restore previous settings (specification 35, B6.4, B6.7, A6.2, A8.5, A8.6; annex E6.10, E7.5, E7.6, E9.3, E11.4, E11.9, E11.10):
//! the retirement of a checkpoint whose runtime was replaced since Apply, which writes nothing to the runtime and keeps the checkpoint
//! as history; the review of a Restore, which reads the runtime and writes nothing (§0.1b Confirmation); and the one Restore operation
//! that every case uses: `controlStudio/restore`, the `restore` verb, and Apply's failure path. The operation starts from the journaled
//! commissioning checkpoint and the runtime's current answers, as `rollback` of `interruptedCommissioningRecovery.ts` does: it ends the
//! checkpoint's owned operations by their identities (never a replay), returns the loop to Manual when it is not there, rolls the
//! parameter record back to the revision Apply found, stops and keeps a recording the checkpoint still holds, returns the loop to the
//! mode Apply found it in, and ends the checkpoint once the runtime has confirmed each step. A step the runtime already confirmed is not
//! repeated, and Restored is reported only after the runtime's own answers confirm it.
//!
//! Every decision is `trust_control::commissioning::checkpoint_restore`'s. This module reads the runtime, journals each identity before
//! its request is sent (E11.2), sends the writes and keeps the order. From `interruptedCommissioningRecovery.ts` (`review`, `rollback`),
//! `engineerActions/recovery.ts` (`reviewInterruptedRollback`), `applyObservation.ts` (`stopRecovered`) and the recovery of
//! `pidCommissioning.ts` and `proportionalCommissioning.ts`, which this one path replaces.

#![allow(clippy::module_name_repetitions)] // `review_restore`, `run_restore` and `restore_with_approval` are the V4 contract's names.

use std::collections::BTreeMap;

use trust_control::commissioning::adapters::controller_adapter;
use trust_control::commissioning::application::ApplySource;
use trust_control::commissioning::application_offers::{apply_offered_after, primary_action};
use trust_control::commissioning::application_records::{
    approval_block, restore_identity_parts, review_identity, ApplicationReview, RestoreOutcome,
    ReviewConfirmation, ReviewedAction,
};
use trust_control::commissioning::application_texts::review_changed;
use trust_control::commissioning::checkpoint::{CheckpointStage, CommissioningCheckpoint};
use trust_control::commissioning::checkpoint_restore as rules;
use trust_control::commissioning::commissioning_result::{TransferOwner, TransferStatus};
use trust_control::commissioning::found_mode::AutoReentry;
use trust_control::commissioning::gain_schedule_adapter::check_restore_mode;
use trust_control::commissioning::parameter_application::ParameterOwner;
use trust_control::commissioning::parameter_binding::runtime_binding_of;
use trust_control::commissioning::parameter_status::ParameterStatus;
use trust_control::commissioning::proportional_commissioning::restore_again;
use trust_control::commissioning::transfer_tracking::TransferTracking;
use trust_control::experiment::{check_recording_owner_scope, RecordingRecoveryOwner};
use trust_control::failure::{Failure, NextAction};
use trust_control::loop_definition::LoopDefinition;
use trust_control::messages::AnswerKind;
use trust_control::recording::{EvidenceSource, RecordedRun, RecordingRequest};
use trust_control::study::apply::ApplySection;
use trust_control::study::Study;

use super::apply::ApplyContext;
use super::commissioning_journal::SharedJournal;
use super::restore_texts as texts;
use crate::operation::{cancelled, OperationContext};
use crate::runtime_client::parameters::ParameterTransactions;
use crate::runtime_client::runtime_recording::RecordingOwnerPersistence;
use crate::runtime_client::transfers::{TransferOwnerSink, TransferSession, TransferWrite};
use crate::runtime_client::RuntimeClient;

/// A review of a Restore and the checkpoint its confirmation starts from.
struct Reviewed {
    review: ApplicationReview,
    checkpoint: CommissioningCheckpoint,
}

/// Reviews Restore previous settings (§0.1b `restore`; B6.4): reads the journaled checkpoint and the runtime, writes nothing and starts
/// no operation (V4 decision 1). The review names each parameter with its running value and the value the Restore returns it to, the
/// mode the loop was found in, what the Restore does, whether the target is physical, and the identity its confirmation must name.
///
/// # Errors
/// The journal holds no checkpoint with settings to restore; the runtime cannot be reached or read; the runtime was restarted or its
/// program reloaded since Apply, so Restore is unavailable (B6.4; `controlStudio/restore` retires such a checkpoint before it asks for
/// a review, [`retire_after_replacement`]); the checkpoint is not the current controller's; another change replaced the settings Apply
/// wrote; or another operation owns the parameter record or the transfer.
pub fn review_restore(context: &mut ApplyContext<'_>) -> Result<ApplicationReview, Failure> {
    // A loop without a controller law has no settings Apply could have written (C12).
    context.loop_definition.require_controller_law()?;
    let mut client = RuntimeClient::connect(context.connection)?;
    reviewed(&mut client, context).map(|reviewed| reviewed.review)
}

/// Runs the reviewed Restore (§0.1b Confirmation; B6.4): the review is made again from the live runtime and the identity it gives must be
/// `identity`, so a runtime that changed since the review writes nothing. The Restore can be cancelled only before its first step; once
/// it writes, it goes on until the runtime confirms it or refuses (E11.10).
///
/// # Errors
/// The review refuses (see [`review_restore`]); its identity is not `identity`; the operation was cancelled before the Restore began; a
/// physical target has no approval; or a step of the Restore is refused or not confirmed, which leaves the checkpoint journaled so that
/// Restore is tried again (B6.7).
pub fn run_restore(
    context: &mut ApplyContext<'_>,
    operation: &OperationContext,
    identity: &str,
) -> Result<RestoreOutcome, Failure> {
    context.loop_definition.require_controller_law()?;
    let mut client = RuntimeClient::connect(context.connection)?;
    let Reviewed { review, checkpoint } = reviewed(&mut client, context)?;
    if review.identity != identity {
        return Err(review_changed());
    }
    if operation.is_cancelled() {
        return Err(cancelled());
    }
    let journal = SharedJournal::new(&mut *context.journal);
    restore_with_approval(
        &mut client,
        context.loop_definition,
        &journal,
        checkpoint,
        context.physical_approved,
        &|text: &str| operation.progress(text),
    )
}

/// A Restore on a runtime that was replaced since Apply (B6.4, B6.7): the checkpoint it would start from, now kept as history, and its
/// answer.
pub struct Retirement {
    /// The identifier of the retired checkpoint, which the study's receipt names when an Apply's receipt names it (A8.1, A8.5).
    pub checkpoint: String,
    /// The Restore's answer: not restored, with the sentence that says why and the offers that follow (B6.7, B7).
    pub outcome: RestoreOutcome,
}

/// Retires the journaled checkpoint when the runtime was restarted, or its program reloaded, since Apply (B6.4, B6.7; annex E9.3: the
/// program revision changes with the session epoch): nothing of the checkpoint can be restored on the new program, so the Restore says
/// so at once, before any review. The checkpoint is journaled again as history ([`rules::retired_checkpoint`]): it grants nothing and
/// holds no stage back, and a later Apply replaces it (A8.5). Nothing is written to the runtime, and the loop continues from a new
/// validation: the outcome offers neither Restore, Keep in project nor Apply (B7). The engineer's Restore is the explicit confirmation
/// that a replaced runtime needs (annex E11.7): it does not claim that the Apply completed, and it replays none of its writes. A runtime
/// that is still the one Apply wrote to retires nothing, and the Restore is reviewed as usual.
///
/// # Errors
/// The loop has no controller law (C12); the journal holds no checkpoint with settings to restore; the runtime cannot be reached or
/// read; or the journal cannot be written.
pub fn retire_after_replacement(
    context: &mut ApplyContext<'_>,
) -> Result<Option<Retirement>, Failure> {
    let loop_definition = context.loop_definition;
    loop_definition.require_controller_law()?;
    let journal = SharedJournal::new(&mut *context.journal);
    let checkpoint = rules::restorable(journal.checkpoint(), &loop_definition.id)?;
    let mut client = RuntimeClient::connect(context.connection)?;
    let source = target_of(&mut client)?;
    let binding = runtime_binding_of(loop_definition)?;
    let observed = ParameterTransactions::new(&mut client, binding, source).status()?;
    let Err(replaced) = rules::check_runtime_identity(&checkpoint, &observed) else {
        return Ok(None);
    };
    journal.write_checkpoint(&rules::retired_checkpoint(
        &checkpoint,
        &observed.program_revision,
    ))?;
    let apply_available = apply_offered_after(ReviewedAction::Restore);
    Ok(Some(Retirement {
        checkpoint: checkpoint.id,
        outcome: RestoreOutcome {
            kind: AnswerKind::new(),
            restored: false,
            // The whole sentence: what happened, that the checkpoint is kept as history, and that a new validation follows (B6.7).
            reason: replaced.detail.unwrap_or(replaced.message),
            mode: None,
            parameters: observed,
            recording: None,
            restore_available: false,
            keep_available: false,
            apply_available,
            primary_action: primary_action(apply_available, false, false),
            // The stages read the study and the journal once the checkpoint is retired: `apply_action` adds them to its answer.
            stages: None,
        },
    }))
}

/// The one Restore (B6.4; `rollback` of `interruptedCommissioningRecovery.ts`), which [`run_restore`] and Apply's failure path call, from
/// `checkpoint` and the runtime's current answers, in this order:
/// 1. the checkpoint's parameter operation and transfer are cancelled by their identities, never replayed (E7.6, E6.10, E11.4);
/// 2. the parameter record and the transfer must still be the checkpoint's own, and settled;
/// 3. a loop that is not in Manual, or whose reference moved, returns to Manual through a transfer of its states as observed and the
///    references Apply found, its owner journaled before it is sent (E11.2);
/// 4. a record that is not back yet is rolled back to the revision Apply found, the rollback's identity journaled before it is sent
///    (E7.5);
/// 5. a recording the checkpoint still holds is stopped and kept in the checkpoint before its buffer is released (E4.7); one the runtime
///    no longer holds is kept as incomplete evidence, and the Restore goes on (A8.6);
/// 6. a loop found in Auto returns to Auto with the Auto-entry allowance (A6.2, A6.4, E11.9);
/// 7. the checkpoint is journaled as recovered and cleared, since a confirmed Restore ends it (A8.5).
///
/// Each step is confirmed by the runtime's answers (E6.7, E7.4), and a step the runtime already confirmed is not repeated. `progress`
/// receives a sentence for each step that writes. `physical_approved` is the engineer's approval on a physical target: the Restore's own,
/// or the Apply's, whose one approval covers its restoration (A2.5). The outcome carries the recording the checkpoint retains, as the
/// Restore stopped and kept it, or its incomplete evidence (E11.6, A8.6), so that the caller keeps it once the checkpoint is cleared.
///
/// # Errors
/// The target is physical without approval; the runtime cannot be reached; an owned operation cannot be ended; another operation owns
/// the record or the transfer; a write is refused or not confirmed; or the journal cannot be written. The checkpoint then stays
/// journaled with every identity the Restore dispatched, and Restore is tried again (B6.7).
pub(crate) fn restore_with_approval(
    client: &mut RuntimeClient,
    loop_definition: &LoopDefinition,
    journal: &SharedJournal<'_>,
    mut checkpoint: CommissioningCheckpoint,
    physical_approved: bool,
    progress: &dyn Fn(&str),
) -> Result<RestoreOutcome, Failure> {
    let source = target_of(client)?;
    rules::check_restore_approval(source == ApplySource::Physical, physical_approved)?;
    let binding = runtime_binding_of(loop_definition)?;
    let reenterable = rules::reenterable(loop_definition, &checkpoint)?;
    let writes = Writes {
        source,
        physical_approved,
    };

    progress(&texts::cancelling(
        &checkpoint.parameter_operation.id,
        checkpoint.transfer.as_ref().map(|owner| owner.id.as_str()),
    ));
    ParameterTransactions::new(client, binding.clone(), source)
        .cancel_owner(&checkpoint.parameter_operation)?;
    if let Some(owner) = checkpoint.transfer.clone() {
        let mut transfers = transfer_session(client, &checkpoint);
        transfers.restore_owner(owner)?;
        transfers.cancel()?;
    }

    let record = ParameterTransactions::new(client, binding.clone(), source).status()?;
    rules::check_owned_parameters(&checkpoint, &record)?;
    let current = transfer_session(client, &checkpoint).status()?;
    rules::check_transfer_settled(&checkpoint, &current)?;
    if rules::restore_transfer_needed(&checkpoint, &current.values) {
        let values = rules::restore_states(&checkpoint, &current.values)?;
        progress(&texts::to_manual(
            &checkpoint.binding.mode.symbol,
            &checkpoint.references,
        ));
        let write = writes.transfer(
            values,
            true,
            checkpoint.tolerance,
            checkpoint.recovery_tracking.clone(),
            current,
        );
        apply_transfer(client, journal, &mut checkpoint, &write)?;
    }

    let parameters = roll_back(
        &mut ParameterTransactions::new(client, binding, source),
        journal,
        &mut checkpoint,
        physical_approved,
        progress,
    )?;
    keep_retained_recording(client, loop_definition, journal, &mut checkpoint, progress)?;

    let current = transfer_session(client, &checkpoint).status()?;
    let (mut values, returned_to_auto) =
        match rules::restore_auto_entry(loop_definition, &checkpoint, &current.values)? {
            Some(entry) => {
                progress(&texts::to_auto(
                    &checkpoint.binding.mode.symbol,
                    entry.tolerance,
                    entry.entry_error_gain,
                    &checkpoint.binding.output.unit,
                ));
                let write = writes.entry(entry, current);
                let entered = apply_transfer(client, journal, &mut checkpoint, &write)?;
                (entered.values, true)
            }
            None => (current.values, false),
        };
    // The bumpless Auto entry aligned the references to the measurement; the references Apply found return as a reference step,
    // as Apply's target change is made (B6.4; C13 step 8).
    if returned_to_auto {
        let entered = transfer_session(client, &checkpoint).status()?;
        if let Some(step) =
            rules::restore_reference_step(loop_definition, &checkpoint, &entered.values)?
        {
            progress(&texts::to_references(&checkpoint.references));
            let write = writes.transfer(step.values, false, step.tolerance, step.tracking, entered);
            values = apply_transfer(client, journal, &mut checkpoint, &write)?.values;
        }
    }

    checkpoint.stage = CheckpointStage::Recovered;
    journal.write_checkpoint(&checkpoint)?;
    journal.clear_checkpoint(&checkpoint.id)?;
    // The recording the checkpoint retains, as the Restore stopped and kept it, or its incomplete evidence (E11.6, A8.6). A checkpoint
    // that retains no recording hands none over.
    let recording = checkpoint
        .recording_result
        .take()
        .filter(|_| checkpoint.recording.is_some());
    // B6.4: the runtime confirmed the restoration and the checkpoint is cleared, so Restore is not offered again; B6.5: Keep in project
    // is not offered after a Restore; B7: Apply is offered again by a validation that passes afterwards.
    let apply_available = apply_offered_after(ReviewedAction::Restore);
    Ok(RestoreOutcome {
        kind: AnswerKind::new(),
        restored: true,
        reason: rules::restored_reason(checkpoint.found_mode, reenterable, returned_to_auto),
        mode: rules::reported_mode(&checkpoint, &values),
        parameters,
        recording,
        restore_available: false,
        keep_available: false,
        apply_available,
        primary_action: primary_action(apply_available, false, false),
        // The stages read the study and the journal once the Restore has ended: `apply_action` adds them to its answer.
        stages: None,
    })
}

/// The review of a Restore (`review` of `interruptedCommissioningRecovery.ts` and `reviewInterruptedRollback` of
/// `engineerActions/recovery.ts`; B6.4), in the order of their checks; it reads and writes nothing.
fn reviewed(
    client: &mut RuntimeClient,
    context: &mut ApplyContext<'_>,
) -> Result<Reviewed, Failure> {
    let loop_definition = context.loop_definition;
    let checkpoint = rules::restorable(
        SharedJournal::new(&mut *context.journal).checkpoint(),
        &loop_definition.id,
    )?;
    // A schedule's table is written at an Auto entry only, so a Restore into Manual would leave it as Apply wrote it (C10).
    check_restore_mode(checkpoint.schedule_origin.as_ref(), checkpoint.found_mode)?;
    let source = target_of(client)?;
    let binding = runtime_binding_of(loop_definition)?;
    let observed = ParameterTransactions::new(client, binding, source).status()?;
    rules::check_runtime_identity(&checkpoint, &observed)?;
    let adapter = controller_adapter(loop_definition)?;
    rules::check_checkpoint_identity(
        &loop_definition.revision,
        &adapter.transfer_binding()?,
        &adapter.recovery_tracking(&checkpoint.model)?,
        &checkpoint,
    )?;
    rules::check_restore_available(
        &checkpoint,
        &observed,
        applied_record(context.study, &checkpoint.id)?.as_ref(),
    )?;
    rules::check_reviewed_parameters(&checkpoint, &observed)?;
    let transfer = transfer_session(client, &checkpoint).status()?;
    rules::check_transfer_owner(&checkpoint, &transfer)?;
    let reenterable = rules::reenterable(loop_definition, &checkpoint)?;
    let parameters = rules::reviewed_parameters(loop_definition, &checkpoint, &observed);
    // A2.5: the approval names the signals this Restore writes, with their limits, and its restoration (no observation, so no duration).
    let approval = approval_block(loop_definition, ReviewedAction::Restore, &parameters, None);
    let physical = source == ApplySource::Physical;
    let review = ApplicationReview {
        kind: AnswerKind::new(),
        action: ReviewedAction::Restore,
        identity: review_identity(&restore_identity_parts(
            &observed,
            checkpoint.found_mode,
            &checkpoint.origin.values,
            &transfer.binding_revision,
            &checkpoint.id,
        )),
        found_mode: checkpoint.found_mode,
        parameters,
        description: rules::texts::restore_description(
            checkpoint.stage,
            checkpoint.found_mode,
            reenterable,
        ),
        physical,
        confirmation: ReviewConfirmation::of_target(physical),
        operating_point: None,
        approval,
    };
    Ok(Reviewed { review, checkpoint })
}

/// The record that the Apply of `checkpoint_id` left, as its receipt in the study's Apply section holds it (A8.1), when there is one.
fn applied_record(study: &Study, checkpoint_id: &str) -> Result<Option<ParameterStatus>, Failure> {
    Ok(study.section::<ApplySection>()?.and_then(|section| {
        section
            .applications
            .into_iter()
            .find(|application| application.checkpoint == checkpoint_id)
            .and_then(|application| application.result.parameters)
    }))
}

/// The target the Restore's writes go to, from the runtime's own fact (A2.4, E10.1): a simulated plant, or a physical one.
fn target_of(client: &mut RuntimeClient) -> Result<ApplySource, Failure> {
    Ok(match client.status()?.source() {
        EvidenceSource::SimulatedPlant => ApplySource::Simulation,
        EvidenceSource::PhysicalPlant => ApplySource::Physical,
    })
}

/// A transfer session on the checkpoint's binding and the program Apply found (E6.2, E11.1).
fn transfer_session<'c>(
    client: &'c mut RuntimeClient,
    checkpoint: &CommissioningCheckpoint,
) -> TransferSession<'c> {
    TransferSession::new(
        client,
        checkpoint.binding.clone(),
        checkpoint.origin.program_revision.clone(),
    )
}

/// The target and the approval every write of one Restore carries (A2.5, E10.2).
#[derive(Clone, Copy)]
struct Writes {
    source: ApplySource,
    physical_approved: bool,
}

impl Writes {
    /// A transfer write of this Restore, reviewed on `reviewed` (E6.5).
    fn transfer(
        self,
        values: BTreeMap<String, f64>,
        manual: bool,
        tolerance: f64,
        tracking: TransferTracking,
        reviewed: TransferStatus,
    ) -> TransferWrite {
        TransferWrite {
            values,
            manual,
            tolerance,
            entry_error_gain: None,
            tracking: Some(tracking),
            reviewed: Some(reviewed),
            source: self.source,
            physical_approved: self.physical_approved,
        }
    }

    /// The Auto entry `entry` of this Restore, reviewed on `reviewed` (A6.3), with the factor of the error at entry that the
    /// runtime adds to its allowance (E6.9).
    fn entry(self, entry: AutoReentry, reviewed: TransferStatus) -> TransferWrite {
        TransferWrite {
            entry_error_gain: entry.entry_error_gain,
            ..self.transfer(
                entry.values,
                false,
                entry.tolerance,
                entry.tracking,
                reviewed,
            )
        }
    }
}

/// Sends one transfer of the Restore and confirms it, its owner journaled in the checkpoint before it is sent (E11.2). A transfer the
/// runtime refuses, reverts or does not confirm leaves the Restore unconfirmed, and Restore is offered again (B6.7).
fn apply_transfer(
    client: &mut RuntimeClient,
    journal: &SharedJournal<'_>,
    checkpoint: &mut CommissioningCheckpoint,
    write: &TransferWrite,
) -> Result<TransferStatus, Failure> {
    let mut transfers = transfer_session(client, checkpoint);
    let mut sink = CheckpointSink {
        journal,
        checkpoint,
    };
    transfers
        .apply(write, &mut sink)
        .map_err(|failure| restore_again(Some(failure), ""))
}

/// Retains each transfer owner of a Restore in its checkpoint, journaled before the transfer is sent (E11.2, A8.5), so that a Restore
/// interrupted at any step resumes from the checkpoint and never replays the transfer (E11.4).
struct CheckpointSink<'s, 'j> {
    journal: &'s SharedJournal<'j>,
    checkpoint: &'s mut CommissioningCheckpoint,
}

impl TransferOwnerSink for CheckpointSink<'_, '_> {
    fn retain(&mut self, owner: &TransferOwner) -> Result<(), Failure> {
        self.checkpoint.stage = CheckpointStage::Transfer;
        self.checkpoint.transfer = Some(owner.clone());
        self.journal.write_checkpoint(self.checkpoint)
    }
}

/// Rolls the record back to the values Apply found when it is not back yet (E7.5), the rollback's identity journaled in the checkpoint
/// before it is sent (E11.2). Returns the record as the runtime last confirmed it.
fn roll_back(
    transactions: &mut ParameterTransactions<'_>,
    journal: &SharedJournal<'_>,
    checkpoint: &mut CommissioningCheckpoint,
    physical_approved: bool,
    progress: &dyn Fn(&str),
) -> Result<ParameterStatus, Failure> {
    let record = transactions.status()?;
    rules::check_owned_parameters(checkpoint, &record)?;
    if !rules::rollback_needed(checkpoint, &record) {
        return Ok(record);
    }
    let review = transactions.review_rollback()?;
    rules::check_rollback_values(checkpoint, &review)?;
    checkpoint.parameter_operation = ParameterOwner {
        id: review.transaction_id.clone(),
        program: review.observed.program_revision.clone(),
        binding: review.observed.binding_revision.clone(),
    };
    checkpoint.stage = CheckpointStage::Parameters;
    journal.write_checkpoint(checkpoint)?;
    progress(&texts::rolling_back(
        &checkpoint.origin.active_revision,
        &checkpoint.origin.values,
    ));
    transactions.rollback(&review, physical_approved)
}

/// Stops a recording the checkpoint holds without its result, keeps the stopped run in the checkpoint, clears the recording's owner, and
/// only then releases the runtime's buffer (E4.7, E11.6). A recording the runtime no longer holds is kept as incomplete evidence, and the
/// Restore goes on (`stopRecovered`, A8.6). The stop goes to the endpoint of the Restore, which is the one the recording started on
/// (E11.1).
fn keep_retained_recording(
    client: &mut RuntimeClient,
    loop_definition: &LoopDefinition,
    journal: &SharedJournal<'_>,
    checkpoint: &mut CommissioningCheckpoint,
    progress: &dyn Fn(&str),
) -> Result<(), Failure> {
    let Some(owner) = checkpoint
        .recording
        .clone()
        .filter(|_| checkpoint.recording_result.is_none())
    else {
        return Ok(());
    };
    check_recording_owner_scope(&owner, loop_definition)?;
    progress(&texts::stopping_recording(&owner.recording_id));
    let request = rules::retained_recording_request(&owner);
    let (run, held) = stopped_run(client, &owner, &request)?;
    checkpoint.recording_result = Some(run);
    journal.write_checkpoint(checkpoint)?;
    RecordingOwnerPersistence::clear(journal, &owner)?;
    if held {
        // The run is kept in the journal now, so the buffer may go. A release that fails leaves the buffer to the runtime until a later
        // release or its next restart frees it; the samples are kept either way (E4.7, E4.9).
        let _ = client.release_recording(&request);
    }
    Ok(())
}

/// The stopped run of the retained recording, and whether the runtime held it (`stopRecovered`, A8.6). A stop that fails while the
/// runtime still reports the recording, or cannot be asked, is the Restore's failure. A stop the runtime refuses for a recording it no
/// longer reports (E4.6) gives the incomplete evidence of that recording, with the runtime's own reason.
fn stopped_run(
    client: &mut RuntimeClient,
    owner: &RecordingRecoveryOwner,
    request: &RecordingRequest,
) -> Result<(RecordedRun, bool), Failure> {
    let stop = match client.stop_recording(request) {
        Ok(run) => return Ok((run, true)),
        Err(stop) => stop,
    };
    match client.recording_status(request, None) {
        // The runtime answers and refuses the recording's status as well: it no longer holds the recording (E4.6), so the evidence of
        // the recording is incomplete (A8.6).
        Err(status)
            if status.next_action != NextAction::ConnectRuntime
                && status != Failure::invalid_response("control.recording.status") =>
        {
            let reason = status.detail.unwrap_or(status.message);
            Ok((rules::incomplete_recording(owner, &reason), false))
        }
        // The runtime still reports the recording, or cannot be asked: the stop itself failed.
        _ => Err(stop),
    }
}

#[cfg(test)]
mod test_runtime;

#[cfg(test)]
#[path = "restore_tests.rs"]
mod tests;
