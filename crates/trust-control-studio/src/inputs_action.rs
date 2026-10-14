//! The method `controlStudio/inputs` (specification 35, §0.1a; A8.1, A8.4 and B4.2 to B4.3): it records the inputs the study holds,
//! as the page's editors change them, and answers at once with the stages and with what the change invalidated.
//!
//! It starts no operation and runs no stage, so it reaches no runtime. It refuses the change while an operation of the loop runs
//! or is unconfirmed, because the page makes the operation's inputs read-only until it ends (workflow specification, Identify).
//! Every rule is trust-control's: the section's checks, what a change invalidates, and the stages the study then has.

use std::path::PathBuf;

use serde_json::Value;
use trust_control::failure::Failure;
use trust_control::messages::{InputsRequest, InputsResult};
use trust_control::stages::stage_availability;
use trust_control::study::inputs::{
    held_requirements, invalidation, stage_evidence_of, InputsSection,
};
use trust_control::study::tune::TuneSection;

use crate::busy_loops::LoopClaims;
use crate::commands::to_value;
use crate::{loops, storage};

/// The name of the operation that holds the loop, as a refusal names it (A8.5). The inputs cannot change while it runs.
const OPERATION_NAME: &str = "Inputs";

/// Answers `controlStudio/inputs`: records the request's inputs in the loop's study, replacing the earlier inputs as a whole, and
/// answers the stages after the change and what it invalidated (A8.4). Nothing else is written, and nothing runs.
///
/// # Errors
/// The loop cannot be opened; an operation of this engine works on the loop (the inputs are read-only while it runs); the study or
/// its sections cannot be read; an input breaks a rule of its own (trust-control's checks, with the next action they name); or the
/// study cannot be written.
pub fn answer(claims: &LoopClaims, request: InputsRequest) -> Result<Value, Failure> {
    let InputsRequest {
        project_root,
        runtime_id,
        loop_id,
        inputs,
    } = request;
    let project_root = PathBuf::from(project_root);
    let loop_definition = loops::open(&project_root, &runtime_id, &loop_id)?;
    // The claim ends when this answer returns: a change is refused only while an operation of the loop holds it.
    let _claim = claims.claim(
        &loop_definition.project_id,
        &loop_definition.id,
        OPERATION_NAME,
    )?;
    let study = storage::open_study(
        &project_root,
        &loop_definition.id,
        &loop_definition.revision,
    )?;
    let tune = study.section::<TuneSection>()?;
    let before = study.section::<InputsSection>()?;
    let invalidated = invalidation(
        held_requirements(before.as_ref(), tune.as_ref()),
        held_requirements(Some(&inputs), tune.as_ref()),
    );
    let mut next = study.clone();
    // A section that breaks one of its rules is refused here, and nothing is written (B10.4).
    next.set_section(&inputs)?;
    storage::save_study(&project_root, &next, &[])?;
    let stages = stage_availability(&stage_evidence_of(&loop_definition, &next)?);
    to_value(&InputsResult {
        stages,
        invalidated,
    })
}

#[cfg(test)]
#[path = "inputs_action_tests.rs"]
mod tests;
