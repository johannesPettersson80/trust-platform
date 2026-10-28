//! The method `controlStudio/run` (specification 35, §0.1a and B8): the automatic progression, as one operation with one Cancel.
//!
//! The method claims the loop (one operation per loop at a time, A8.5), starts the operation and answers with its identifier at once.
//! The operation's stages run in `workflow::run`, each through the stage function of its own method, so a run makes the same checks
//! and the same design, campaign and validation as the method of that stage. A run never applies (B8).

use std::path::PathBuf;
use std::sync::Arc;

use serde_json::Value;
use trust_control::failure::Failure;
use trust_control::messages::RunRequest;

use crate::busy_loops::LoopClaims;
use crate::commands::to_value;
use crate::loops;
use crate::operation::Operations;
use crate::runtime_client::runtime_recording::PendingReleases;
use crate::workflow::run::{sequence, RunWork};

/// The command of the operation that `run` starts (§0.1a).
const RUN_COMMAND: &str = "run";
/// The name of the operation that holds the loop, as a refusal names the operation that holds it (A8.5).
const OPERATION_NAME: &str = "Run";

/// Answers `controlStudio/run`: starts the run as one operation and answers with its identifier at once. The run's final update
/// carries the stages after it and the input it waits for, when a stage needs one (B8); a failure of a stage stops it and keeps every
/// result the stages obtained. The recordings the operation stops are released through `pending`, the engine's one list (E4.7).
///
/// # Errors
/// The loop cannot be opened, or an operation of this engine works on it (A8.5). What a stage refuses is the operation's own failure,
/// not this answer's.
pub fn answer(
    operations: &Operations,
    pending: &Arc<PendingReleases>,
    claims: &LoopClaims,
    request: RunRequest,
) -> Result<Value, Failure> {
    let RunRequest {
        project_root,
        runtime_id,
        loop_id,
        storage_folder,
        connection,
        through,
        identify_again,
    } = request;
    let project_root = PathBuf::from(project_root);
    let loop_definition = loops::open(&project_root, &runtime_id, &loop_id)?;
    let claim = claims.claim(
        &loop_definition.project_id,
        &loop_definition.id,
        OPERATION_NAME,
    )?;
    let work = RunWork {
        project_root,
        storage_folder: PathBuf::from(storage_folder),
        pending: Arc::clone(pending),
        loop_definition,
        connection,
        through,
        identify_again,
    };
    // The claim ends before the operation reports its end; a panic of the work ends it too, because the claim is dropped with the work.
    let started = operations.start(RUN_COMMAND, move |context| {
        let worked = sequence(context, &work);
        drop(claim);
        worked
    });
    to_value(&started)
}

#[cfg(test)]
#[path = "run_action_tests.rs"]
mod tests;
