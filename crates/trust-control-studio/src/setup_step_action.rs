//! The step-by-step loop setup's method `controlStudio/setupStep` (specification 35, B1.3, C12): the question the engine asks next, the words
//! of a refused answer, or the finished draft and the sentence the engineer confirms before the save. The engine reads the compiled
//! project as `setupDiscover` does; the session's rules are trust-control's (`setup::step_session`), and `setupSave` writes the draft.

use std::path::Path;

use serde_json::Value;
use trust_control::failure::{Cause, Failure, NextAction};
use trust_control::setup::messages::SetupStepRequest;
use trust_control::setup::step_session::{self, StepAction};

use crate::commands::to_value;
use crate::setup_action::compiled_catalog;

/// Answers `controlStudio/setupStep` (B1.3): the reply to the session and the engineer's answer, or Back, or the start of the setup.
///
/// # Errors
/// The session is not one the engine wrote; the compiled project cannot be read; or the reply cannot be written.
pub fn answer(request: &SetupStepRequest) -> Result<Value, Failure> {
    let session = step_session::session_of(request.session.clone())
        .map_err(|reason| Failure::new(&reason, Cause::Settings, NextAction::OpenLoopSetup))?;
    let catalog = compiled_catalog(
        request.language_tool.as_deref(),
        Path::new(&request.project_root),
    )?;
    let action = match (&request.answer, request.back) {
        (_, true) => StepAction::Back(request.answer.as_ref().map(|answer| answer.value.clone())),
        (Some(answer), false) => StepAction::Answer(answer.value.clone()),
        (None, false) => StepAction::Start,
    };
    to_value(&step_session::respond(
        session,
        &catalog,
        action,
        &request.runtime_id,
    ))
}
