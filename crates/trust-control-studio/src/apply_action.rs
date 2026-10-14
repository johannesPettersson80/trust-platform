//! The Apply and Restore methods, `controlStudio/apply` and `controlStudio/restore` (specification 35, §0.1a, §0.1b, B6.1 to B6.4, A8.1
//! and A8.5). Without a review identity a method answers the review at once and writes nothing; with the identity of that review it
//! runs the reviewed change as one operation, claimed on its loop (A8.5) and answered through `controlStudio/update` (A10.4). Each
//! result is kept in the study's Apply section as a receipt (A8.1).
//!
//! Every rule is trust-control's (`commissioning`, `study::apply`) or the workflow's (`workflow::apply`, `workflow::restore`). This
//! module opens the loop, the study, the journal and the runtime connection, makes the calls of the workflow in their order, and
//! writes each receipt. A receipt names the checkpoint that the workflow journaled before its first write (A8.5).

use std::path::{Path, PathBuf};

use serde_json::Value;
use trust_control::commissioning::application_records::ApplyOutcome;
use trust_control::commissioning::checkpoint::CommissioningCheckpoint;
use trust_control::failure::Failure;
use trust_control::identification::connected_run::runtime_required;
use trust_control::loop_definition::LoopDefinition;
use trust_control::messages::{ApplicationAnswer, ApplicationRequest};
use trust_control::runtime::RuntimeConnection;
use trust_control::study::apply::{
    restore_unavailable, ApplicationReceipt, ApplySection, RestorationReceipt,
};

use crate::busy_loops::{LoopClaim, LoopClaims};
use crate::commands::to_value;
use crate::journal::{Journal, JournalEntry};
use crate::loops;
use crate::operation::{OperationContext, Operations};
use crate::storage;
use crate::workflow::apply::{review_apply, run_apply, ApplyContext};
use crate::workflow::apply_observation::reattach_observation;
use crate::workflow::restore::{review_restore, run_restore};

/// The command of the operation that `apply` starts (§0.1a).
const APPLY_COMMAND: &str = "apply";
/// The command of the operation that `restore` starts (§0.1a).
const RESTORE_COMMAND: &str = "restore";
/// The name of the operation that holds the loop, as a refusal names the operation that holds it (A8.5).
const APPLY_OPERATION: &str = "Apply";
/// The name of the Restore operation that holds the loop (A8.5).
const RESTORE_OPERATION: &str = "Restore previous settings";

/// Answers `controlStudio/apply` (B6.1 to B6.3): the review of the change when the request names no review, else the Apply operation,
/// which refuses the identity of a review that the runtime no longer matches. The loop is claimed for the answer (A8.5).
///
/// # Errors
/// The request cannot be read, or the loop cannot be opened; no runtime is connected; an operation of this engine works on the loop;
/// the workflow refuses the review (its refusal names the next action), or the study cannot be read.
pub fn answer(
    operations: &Operations,
    claims: &LoopClaims,
    request: ApplicationRequest,
) -> Result<Value, Failure> {
    let work = ApplicationWork::of(&request)?;
    let claim = work.claim(claims, APPLY_OPERATION)?;
    if request.resume {
        return started(
            operations,
            claim,
            APPLY_COMMAND,
            work,
            run_reattach_operation,
        );
    }
    match request.review {
        None => {
            let review = work.with_context(review_apply)?;
            to_value(&ApplicationAnswer::Review(Box::new(review)))
        }
        Some(identity) => started(
            operations,
            claim,
            APPLY_COMMAND,
            work,
            move |context, work| run_apply_operation(context, work, &identity),
        ),
    }
}

/// Answers `controlStudio/restore` (B6.4): refused at once when the study offers no Restore (no applied settings, or a checkpoint that
/// is closed), the review of the restore when the request names no review, else the Restore operation.
///
/// # Errors
/// As `answer`, and the study offers no Restore: no Apply has applied settings that a Restore could return.
pub fn answer_restore(
    operations: &Operations,
    claims: &LoopClaims,
    request: ApplicationRequest,
) -> Result<Value, Failure> {
    if request.resume {
        return Err(Failure::invalid_request(
            "resume follows a journaled Apply observation; Restore has nothing to resume",
        ));
    }
    let work = ApplicationWork::of(&request)?;
    let claim = work.claim(claims, RESTORE_OPERATION)?;
    // B6.4: Restore starts from the journaled checkpoint, whether the Apply completed, failed or was interrupted at any step.
    if !work.with_journal(|journal| journaled_checkpoint(journal).is_some())? {
        return Err(restore_unavailable());
    }
    match request.review {
        None => {
            let review = work.with_context(review_restore)?;
            to_value(&ApplicationAnswer::Review(Box::new(review)))
        }
        Some(identity) => started(
            operations,
            claim,
            RESTORE_COMMAND,
            work,
            move |context, work| run_restore_operation(context, work, &identity),
        ),
    }
}

/// What Apply, Restore and Keep work with: the loop, its folders and its runtime, and the physical approval of the request. It is
/// owned, so that the operation can move to its thread (A10.4).
struct ApplicationWork {
    loop_definition: LoopDefinition,
    project_root: PathBuf,
    storage_folder: PathBuf,
    connection: RuntimeConnection,
    physical_approved: bool,
}

impl ApplicationWork {
    /// The work of `request`: its loop is opened, and its runtime connection is required (`identify` refuses the same way).
    fn of(request: &ApplicationRequest) -> Result<Self, Failure> {
        let project_root = PathBuf::from(&request.project_root);
        let loop_definition = loops::open(&project_root, &request.runtime_id, &request.loop_id)?;
        let connection = request.connection.clone().ok_or_else(runtime_required)?;
        Ok(Self {
            loop_definition,
            project_root,
            storage_folder: PathBuf::from(&request.storage_folder),
            connection,
            physical_approved: request.physical_approved,
        })
    }

    /// Claims the loop for `operation`, so that no operation of this engine works on it at the same time (A8.5).
    fn claim(&self, claims: &LoopClaims, operation: &str) -> Result<LoopClaim, Failure> {
        claims.claim(
            &self.loop_definition.project_id,
            &self.loop_definition.id,
            operation,
        )
    }

    /// Reads the loop's journal as it is now (A8.5).
    fn with_journal<T>(&self, read: impl FnOnce(&Journal) -> T) -> Result<T, Failure> {
        let journal = Journal::open(
            &self.storage_folder,
            &self.loop_definition.project_id,
            &self.loop_definition.id,
        )?;
        Ok(read(&journal))
    }

    /// Runs `action` with the study and the journal as they are now (A8.2, A8.5), in the context that the workflow needs.
    fn with_context<T>(
        &self,
        action: impl FnOnce(&mut ApplyContext<'_>) -> Result<T, Failure>,
    ) -> Result<T, Failure> {
        let study = storage::open_study(
            &self.project_root,
            &self.loop_definition.id,
            &self.loop_definition.revision,
        )?;
        let mut journal = Journal::open(
            &self.storage_folder,
            &self.loop_definition.project_id,
            &self.loop_definition.id,
        )?;
        let mut context = ApplyContext {
            loop_definition: &self.loop_definition,
            study: &study,
            connection: &self.connection,
            journal: &mut journal,
            physical_approved: self.physical_approved,
            project_root: &self.project_root,
        };
        action(&mut context)
    }
}

/// The Apply operation (B6.1 to B6.3): runs the reviewed change, then keeps its receipt in the study (A8.1). The workflow refuses an
/// identity that no longer matches the runtime, and a refusal keeps no receipt.
fn run_apply_operation(
    context: &OperationContext,
    work: &ApplicationWork,
    identity: &str,
) -> Result<Value, Failure> {
    let outcome = work.with_context(|apply| run_apply(apply, context, identity))?;
    // The receipt is the outcome's: a failure-path Restore may already have cleared the checkpoint from the journal (B6.4).
    record_application(work, &outcome)?;
    to_value(&outcome)
}

/// Keeps an Apply's receipt in the study (A8.1), and beside it the receipt of the restoration the Apply made itself: a cancelled
/// Apply, or one whose observation failed or left its permitted range, returns the previous settings as Restore does (B6.3, B6.4).
fn record_application(work: &ApplicationWork, outcome: &ApplyOutcome) -> Result<(), Failure> {
    let receipt = ApplicationReceipt {
        checkpoint: outcome.checkpoint_id.clone(),
        validation_identity: outcome.validation_identity.clone(),
        settings_applied: outcome.restore_available,
        result: outcome.result.clone(),
    };
    let restoration = outcome.restored.then(|| RestorationReceipt {
        checkpoint: outcome.checkpoint_id.clone(),
        restored: true,
        retired: false,
        reason: outcome.result.reason.clone(),
        mode: None,
    });
    record_section(&work.project_root, &work.loop_definition, |section| {
        section.applications.push(receipt);
        section.restorations.extend(restoration);
    })
}

/// The reattached Apply observation (A8.5, A8.6, B6.3): follows the response the runtime records for the journaled Apply to its end,
/// then keeps the Apply's receipt, which the interrupted engine could not write.
fn run_reattach_operation(
    context: &OperationContext,
    work: &ApplicationWork,
) -> Result<Value, Failure> {
    let outcome = work.with_context(|apply| reattach_observation(apply, context))?;
    record_application(work, &outcome)?;
    to_value(&outcome)
}

/// The Restore operation (B6.4): returns the previous settings from the journaled checkpoint, then keeps its receipt (A8.1). A
/// checkpoint that the runtime's replacement retired is closed without a restore, and its receipt says so.
fn run_restore_operation(
    context: &OperationContext,
    work: &ApplicationWork,
    identity: &str,
) -> Result<Value, Failure> {
    let (outcome, receipt) = work.with_context(|restore| {
        let checkpoint =
            journaled_checkpoint(&*restore.journal).map(|journaled| journaled.id.clone());
        let outcome = run_restore(restore, context, identity)?;
        let receipt = checkpoint.map(|id| {
            // The workflow clears a checkpoint that a confirmed Restore returned, and one that the runtime's replacement retired, so
            // a checkpoint that is no longer journaled and was not restored was retired (B6.4).
            let still_journaled =
                journaled_checkpoint(&*restore.journal).is_some_and(|journaled| journaled.id == id);
            RestorationReceipt {
                checkpoint: id,
                restored: outcome.restored,
                retired: !outcome.restored && !still_journaled,
                reason: outcome.reason.clone(),
                mode: outcome.mode,
            }
        });
        Ok((outcome, receipt))
    })?;
    if let Some(receipt) = receipt {
        record_section(&work.project_root, &work.loop_definition, |section| {
            section.restorations.push(receipt);
        })?;
    }
    to_value(&outcome)
}

/// Starts `run` on `work` as the operation `command`, holding `claim` until it ends (A8.5), and answers the operation's identifier at
/// once (§0.1a). Apply, Restore and Keep share it.
pub(crate) fn started<W: Send + 'static>(
    operations: &Operations,
    claim: LoopClaim,
    command: &str,
    work: W,
    run: impl FnOnce(&OperationContext, &W) -> Result<Value, Failure> + Send + 'static,
) -> Result<Value, Failure> {
    let started = operations.start(command, move |context| {
        let worked = run(context, &work);
        drop(claim);
        worked
    });
    to_value(&ApplicationAnswer::Started {
        operation_id: started.operation_id,
    })
}

/// The checkpoint that the journal holds for the loop, when an Apply journaled one (A8.5).
fn journaled_checkpoint(journal: &Journal) -> Option<&CommissioningCheckpoint> {
    journal.entries().iter().find_map(|entry| match entry {
        JournalEntry::Commissioning { checkpoint } => Some(&**checkpoint),
        _ => None,
    })
}

/// The study's Apply section as it is now: empty before the first Apply (A8.1).
pub(crate) fn apply_section(
    project_root: &Path,
    loop_definition: &LoopDefinition,
) -> Result<ApplySection, Failure> {
    let study = storage::open_study(project_root, &loop_definition.id, &loop_definition.revision)?;
    Ok(study.section::<ApplySection>()?.unwrap_or_default())
}

/// Keeps `change` on the study's Apply section (A8.1). The study is read again for the write, so every other section, and a page the
/// engineer chose meanwhile, is kept as it is.
pub(crate) fn record_section(
    project_root: &Path,
    loop_definition: &LoopDefinition,
    change: impl FnOnce(&mut ApplySection),
) -> Result<(), Failure> {
    let mut study =
        storage::open_study(project_root, &loop_definition.id, &loop_definition.revision)?;
    let mut section = study.section::<ApplySection>()?.unwrap_or_default();
    change(&mut section);
    study.set_section(&section)?;
    storage::save_study(project_root, &study, &[])
}

#[cfg(test)]
#[path = "apply_action_tests.rs"]
mod tests;
