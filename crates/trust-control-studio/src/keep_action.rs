//! The Keep method, `controlStudio/keep` (specification 35, §0.1a, §0.1b and B6.5): the running values of the loop's parameter record
//! become the record's initial values in the project, so that a restart or a new download keeps them. Keep is offered only after an
//! Apply whose settings are applied, and not after a Restore (`study::apply`). Without a review identity the method answers the review
//! at once and writes nothing; with the identity it runs the reviewed edit as an operation, claimed on its loop (A8.5), and keeps the
//! receipt in the study (A8.1).
//!
//! The rules are the Keep agent's (`commissioning::keep` and `workflow::keep`), and the project's command is answered by the language
//! tool (`language_tool`). This module reads the running values, makes the calls in their order and writes the receipt.

use std::collections::BTreeMap;
use std::path::PathBuf;

use serde_json::Value;
use trust_control::commissioning::application_offers::primary_action;
use trust_control::commissioning::application_records::ReviewConfirmation;
use trust_control::commissioning::keep_texts;
use trust_control::failure::Failure;
use trust_control::identification::connected_run::runtime_required;
use trust_control::loop_definition::LoopDefinition;
use trust_control::messages::{
    AnswerKind, KeepFileChange, KeepRequest, KeepResult, KeepReviewAnswer,
};
use trust_control::runtime::RuntimeConnection;
use trust_control::study::apply::{keep_unavailable, ApplySection, KeepReceipt};

use crate::application_state::state_now;
use crate::apply_action::{apply_section, record_section, started};
use crate::busy_loops::LoopClaims;
use crate::commands::to_value;
use crate::language_tool::LanguageTool;
use crate::loops;
use crate::operation::Operations;
use crate::runtime_client::RuntimeClient;
use crate::workflow::keep::{confirm_keep, review_keep, KeepOutcome, KeepReview};

/// The command of the operation that `keep` starts (§0.1a).
const KEEP_COMMAND: &str = "keep";
/// The name of the operation that holds the loop, as a refusal names the operation that holds it (A8.5).
const KEEP_OPERATION: &str = "Keep in project";

/// Answers `controlStudio/keep` (B6.5): refused at once when the study offers no Keep, the review of the edit when the request names no
/// review, else the Keep operation, which writes the edit only while the review's identity matches.
///
/// # Errors
/// The loop cannot be opened or no operation of this engine works on it; the study offers no Keep (`study::apply`); no runtime is
/// connected; or the review refuses (the Keep agent names the refusal, such as unsaved changes in a file the edit would change).
pub fn answer(
    operations: &Operations,
    claims: &LoopClaims,
    request: KeepRequest,
) -> Result<Value, Failure> {
    let KeepRequest {
        project_root,
        runtime_id,
        loop_id,
        storage_folder,
        connection,
        review,
        unsaved_files,
        language_tool,
        ..
    } = request;
    let project_root = PathBuf::from(project_root);
    let loop_definition = loops::open(&project_root, &runtime_id, &loop_id)?;
    let claim = claims.claim(
        &loop_definition.project_id,
        &loop_definition.id,
        KEEP_OPERATION,
    )?;
    if apply_section(&project_root, &loop_definition)?
        .keep_checkpoint()
        .is_none()
    {
        return Err(keep_unavailable());
    }
    let connection = connection.ok_or_else(runtime_required)?;
    let work = KeepWork {
        project_root,
        storage_folder: PathBuf::from(storage_folder),
        loop_definition,
        connection,
        unsaved_files: unsaved_files.iter().map(PathBuf::from).collect(),
        tool: LanguageTool::new(language_tool.map(PathBuf::from)),
    };
    match review {
        None => work.review().and_then(|review| review_json(&review)),
        Some(identity) => started(
            operations,
            claim,
            KEEP_COMMAND,
            work,
            move |_context, work| run_keep_operation(work, &identity),
        ),
    }
}

/// What Keep works with (B6.5): the project, the folder of the operation journals and the loop, the runtime whose running values are
/// kept, the files with unsaved changes in an editor, and the language tool. It is owned, so that the operation can move to its thread
/// (A10.4).
struct KeepWork {
    project_root: PathBuf,
    storage_folder: PathBuf,
    loop_definition: LoopDefinition,
    connection: RuntimeConnection,
    unsaved_files: Vec<PathBuf>,
    tool: LanguageTool,
}

impl KeepWork {
    /// The review of the Keep: the edit it would make and its identity (`review_keep`). It writes nothing.
    fn review(&self) -> Result<KeepReview, Failure> {
        let running = self.running_values()?;
        review_keep(
            &self.project_root,
            &self.loop_definition,
            &running,
            &self.unsaved_files,
            &self.tool,
        )
    }

    /// The Keep, when the review made again has the identity `identity` (`confirm_keep`).
    fn confirm(&self, identity: &str) -> Result<KeepOutcome, Failure> {
        let running = self.running_values()?;
        confirm_keep(
            &self.project_root,
            &self.loop_definition,
            &running,
            &self.unsaved_files,
            &self.tool,
            identity,
        )
    }

    /// The running values of the loop's parameter record, read now (E7.2, B6.5).
    fn running_values(&self) -> Result<BTreeMap<String, f64>, Failure> {
        RuntimeClient::connect(&self.connection)?.parameter_values(&self.loop_definition)
    }
}

/// The Keep operation (B6.5): writes the reviewed edit when its identity still matches, then keeps its receipt in the study (A8.1). The
/// checkpoint of the receipt is the Apply whose running values were kept.
fn run_keep_operation(work: &KeepWork, identity: &str) -> Result<Value, Failure> {
    let checkpoint = apply_section(&work.project_root, &work.loop_definition)?
        .keep_checkpoint()
        .ok_or_else(keep_unavailable)?
        .to_string();
    let outcome = work.confirm(identity)?;
    let receipt = KeepReceipt {
        checkpoint,
        record: outcome.record.clone(),
        kept: outcome.kept.clone(),
        unknown: outcome.unknown.clone(),
    };
    record_section(&work.project_root, &work.loop_definition, |section| {
        section.keeps.push(receipt);
    })?;
    // The offers follow the receipts as they stand now: the Keep's own receipt is in them (B6.5). Apply is as the engine offers it now
    // (B7): a validation that passed after the Apply may hold the position.
    let applied = apply_section(&work.project_root, &work.loop_definition)?;
    let (application, _) = state_now(
        &work.project_root,
        &work.storage_folder,
        &work.loop_definition,
    )?;
    keep_outcome_json(&outcome, &applied, application.offers.apply)
}

/// The review of a Keep as its answer (`KeepReviewAnswer`): the record, the values to keep, the fields and files it changes, the
/// sentence that describes what it writes, and the identity that a confirmation gives back.
fn review_json(review: &KeepReview) -> Result<Value, Failure> {
    to_value(&KeepReviewAnswer {
        kind: AnswerKind::new(),
        record: review.record.clone(),
        values: review.values.clone(),
        kept: review.kept.clone(),
        unknown: review.unknown.clone(),
        files: review
            .files
            .iter()
            .map(|file| KeepFileChange {
                path: file.path.display().to_string(),
                before: file.before.clone(),
                after: file.after.clone(),
            })
            .collect(),
        description: keep_texts::review_description(&review.record),
        identity: review.identity.clone(),
        confirmation: ReviewConfirmation::Explicit,
    })
}

/// What a confirmed Keep wrote, as its final result (`KeepResult`): the record, the fields kept, the fields without a setting, the
/// sentence that says what was kept, or that the project already had the values (`keep_texts::kept_reason`), whether Restore and
/// Keep in project are offered now, from the study's receipts `applied`, the Keep's own among them (B6.5, B7), and the action that
/// holds the Validate page's position, with Apply as the engine offers it now (`apply`, B7).
fn keep_outcome_json(
    outcome: &KeepOutcome,
    applied: &ApplySection,
    apply: bool,
) -> Result<Value, Failure> {
    let restore_available = applied.restore_available();
    let keep_available = applied.keep_available();
    to_value(&KeepResult {
        kind: AnswerKind::new(),
        restore_available,
        keep_available,
        primary_action: primary_action(apply, restore_available, keep_available),
        record: outcome.record.clone(),
        kept: outcome.kept.clone(),
        unknown: outcome.unknown.clone(),
        reason: keep_texts::kept_reason(&outcome.record, &outcome.kept, &outcome.unknown),
    })
}

#[cfg(test)]
#[path = "keep_action_tests.rs"]
mod tests;
