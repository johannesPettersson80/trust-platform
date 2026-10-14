//! The reviewed verbs `apply`, `restore` and `keep` (specification 35, §0.1b): each sends its method through `ControlStudio::execute`, the
//! entry that `serve` uses, so every rule, refusal and confirmation is the method's. Without `--review` the method answers the review,
//! which the verb prints and which writes nothing. With `--review <identity>` the method starts the operation, which the verb follows to
//! its end. A refusal comes at once. A finished result is status 0 only when it is a success, such as an Apply whose result passed.

use std::io::Write;

use clap::Args;
use serde::Serialize;
use serde_json::Value;
use trust_control::failure::{Cause, Failure, NextAction};
use trust_control::messages::ApplicationRequest;
use trust_control::runtime::RuntimeConnection;

use super::{
    follow, json_answer_of, places, refused, settle, text_of, Common, Exit, SELECTED_RUNTIME,
};
use crate::commands::ControlStudio;
use crate::interrupt::Presses;
use crate::settings::{self, Environment};

/// The arguments of `apply`, `restore` and `keep` (§0.1b; B6). Without `--review` the verb prints the review, which writes nothing;
/// with the identity of that review it runs the change, which refuses to run when the runtime or the project no longer matches it.
#[derive(Debug, Args)]
pub struct ReviewArgs {
    /// The arguments that the verbs share.
    #[command(flatten)]
    pub common: Common,
    /// The loop whose settings change, or whose values are kept.
    #[arg(long = "loop", value_name = "ID")]
    pub loop_id: String,
    /// The identity of the reviewed change, from the review that prints without it.
    #[arg(long, value_name = "IDENTITY")]
    pub review: Option<String>,
    /// Follows the loop's journaled Apply observation to its end (A8.5); `apply` only.
    #[arg(long, conflicts_with = "review")]
    pub resume: bool,
}

/// How a reviewed verb sends its method and words its answers (§0.1b).
pub(super) struct Reviewed<'a> {
    /// The method the verb sends: `controlStudio/apply`, `controlStudio/restore` or `controlStudio/keep`.
    pub method: &'a str,
    /// Whether no `--review` was given, so that the method answers the review.
    pub reviewing: bool,
    /// Whether the answers are written as JSON (`--json`).
    pub as_json: bool,
    /// The readable text of the review.
    pub review_text: fn(&Value) -> String,
    /// The readable text of a finished result, or of a refusal's answer.
    pub result_text: fn(&Value) -> String,
    /// Whether a finished result is a success: the verb's status is 0 only then.
    pub succeeded: fn(&Value) -> bool,
    /// Whether a finished result is the cancellation that a Ctrl-C asked for: an Apply ends its operation with its cancelled
    /// result and the restoration it confirmed, and the verb then ends with status 130 (§0.1b).
    pub cancelled: fn(&Value) -> bool,
}

/// The folders and the runtime that the reviewed verbs send (§0.1b): the project and journal folders, and the connection of
/// `--endpoint` or of the project's runtime file, whose token comes from `TRUST_CONTROL_TOKEN` or `--token-file`.
pub(super) struct Target {
    /// The project folder.
    pub project_root: String,
    /// The folder of the operation journals (A8.3).
    pub storage_folder: String,
    /// Where the runtime listens, and its token.
    pub connection: RuntimeConnection,
}

/// The target that the arguments of a reviewed verb make (§0.1b).
///
/// # Errors
/// The current folder cannot be read, or the runtime's connection cannot be read from the arguments, the project, or the environment.
pub(super) fn target(args: &ReviewArgs, environment: Environment<'_>) -> Result<Target, Failure> {
    let places = places(&args.common, environment)?;
    let connection = settings::runtime_connection(
        args.common.endpoint.as_deref(),
        &places.project,
        args.common.token_file.as_deref(),
        environment,
    )?;
    Ok(Target {
        project_root: text_of(&places.project),
        storage_folder: text_of(&places.journal),
        connection,
    })
}

/// The request that the arguments of `apply` or `restore` make (§0.1b): the target of the project, with no physical approval, which the
/// command line never gives: the review refuses a physical target that is not approved (A2.5).
///
/// # Errors
/// As [`target`].
pub(super) fn application_request(
    args: &ReviewArgs,
    environment: Environment<'_>,
) -> Result<ApplicationRequest, Failure> {
    let target = target(args, environment)?;
    Ok(ApplicationRequest {
        project_root: target.project_root,
        runtime_id: SELECTED_RUNTIME.to_string(),
        loop_id: args.loop_id.clone(),
        storage_folder: target.storage_folder,
        connection: Some(target.connection),
        review: args.review.clone(),
        physical_approved: false,
        resume: args.resume,
    })
}

/// Runs a reviewed verb (§0.1b): its method with `request`, then either the review, or the operation followed to its final update.
/// While the operation runs, the first Ctrl-C counted in `presses` cancels it, and the second leaves it to the runtime.
pub(super) fn run<R: Serialize>(
    verb: &Reviewed<'_>,
    request: &R,
    presses: &mut Presses,
    stdout: &mut dyn Write,
    stderr: &mut dyn Write,
) -> Exit {
    let (studio, updates) = follow::engine();
    let answer = match request_value(request).and_then(|value| call(&studio, verb.method, value)) {
        Ok(answer) => answer,
        Err(failure) => return refused(failure, verb.as_json, presses, stdout, stderr),
    };
    if answer.get("failure").is_some() {
        // A refusal comes at once: no review was made and no operation started (B6.7).
        let exit = json_answer_of(&answer, verb.as_json, verb.result_text, stdout, stderr);
        return settle(exit, presses.count());
    }
    if verb.reviewing {
        let exit = json_answer_of(&answer, verb.as_json, verb.review_text, stdout, stderr);
        return settle(exit, presses.count());
    }
    let Some(operation_id) = answer
        .pointer("/result/operationId")
        .and_then(Value::as_str)
    else {
        return refused(
            no_operation(verb.method),
            verb.as_json,
            presses,
            stdout,
            stderr,
        );
    };
    let followed = follow::to_end(studio.operations(), operation_id, &updates, presses, stderr);
    let succeeded = matches!(
        &followed,
        Ok(follow::Followed::Finished(result)) if (verb.succeeded)(result)
    );
    let interrupted = presses.count() > 0
        && matches!(&followed, Ok(follow::Followed::Finished(result)) if (verb.cancelled)(result));
    let exit = follow::ended(followed, verb.as_json, verb.result_text, stdout, stderr);
    if interrupted {
        Exit::Interrupted
    } else {
        finished_status(exit, succeeded)
    }
}

/// A finished result that is never a cancellation: Restore and Keep end a cancelled operation as cancelled.
pub(super) fn never_cancelled(_result: &Value) -> bool {
    false
}

/// The status of a finished reviewed verb (§0.1b): 0 only when its result is a success, since a result that is not one (a failed
/// requirement, a Restore that the runtime did not confirm) is not. `exit` is the status the result was written with.
fn finished_status(exit: Exit, succeeded: bool) -> Exit {
    match exit {
        Exit::Finished if !succeeded => Exit::Failed,
        exit => exit,
    }
}

fn request_value<R: Serialize>(request: &R) -> Result<Value, Failure> {
    serde_json::to_value(request).map_err(|error| {
        Failure::new(
            &format!("The request cannot be written: {error}."),
            Cause::Implementation,
            NextAction::None,
        )
    })
}

/// The answer of `method` to `request`, through the entry that `serve` uses (§0.1a): `{"result": …}` or `{"failure": …}`.
fn call(studio: &ControlStudio, method: &str, request: Value) -> Result<Value, Failure> {
    studio.execute(method, &[request]).ok_or_else(|| {
        Failure::new(
            &format!("This engine does not answer {method}."),
            Cause::Implementation,
            NextAction::None,
        )
    })
}

/// The failure of a started operation whose answer names no operation to follow.
fn no_operation(method: &str) -> Failure {
    Failure::new(
        &format!("{method} answered without an operation to follow."),
        Cause::Implementation,
        NextAction::None,
    )
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_finished_result_is_status_0_only_when_it_is_a_success() {
        assert_eq!(finished_status(Exit::Finished, true), Exit::Finished);
        assert_eq!(
            finished_status(Exit::Finished, false),
            Exit::Failed,
            "a result that is not a success is not status 0"
        );
        assert_eq!(finished_status(Exit::Failed, false), Exit::Failed);
        assert_eq!(finished_status(Exit::Interrupted, false), Exit::Interrupted);
    }

    #[test]
    fn a_method_that_answers_nothing_to_follow_is_an_implementation_failure() {
        let failure = no_operation("controlStudio/apply");
        assert_eq!(failure.cause, Cause::Implementation);
        assert!(failure.message.contains("controlStudio/apply"));
    }
}
