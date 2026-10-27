//! The `validate` verb (specification 35, §0.1b; B5): runs the independent model check when the model is provisional, then
//! the actual-ST validation of the current and the candidate settings in isolated simulations, and prints each requirement's
//! result and whether Apply is available. The verb sends `controlStudio/validate` through `ControlStudio::execute`, the entry
//! that `serve` uses, so every rule, limit and refusal is the method's. A refusal comes at once; otherwise the method starts an
//! operation, which the verb follows to its final update. Validate never writes to the controller (B5.1).

use std::io::Write;

use serde_json::Value;
use trust_control::failure::{Cause, Failure, NextAction};
use trust_control::messages::ValidateRequest;

use super::{
    follow, json_answer_of, output, places, refused, settle, text_of, Exit, ValidateArgs,
    SELECTED_RUNTIME,
};
use crate::commands::{self, ControlStudio};
use crate::interrupt::Presses;
use crate::settings::{self, Environment};

/// Runs `validate` (§0.1b). While its operation runs, the first Ctrl-C counted in `presses` cancels it and the verb waits for
/// its final update; the second leaves it to the runtime.
pub(super) fn run(
    args: &ValidateArgs,
    environment: Environment<'_>,
    presses: &mut Presses,
    stdout: &mut dyn Write,
    stderr: &mut dyn Write,
) -> Exit {
    let as_json = args.common.json;
    let (studio, updates) = follow::engine();
    let answer = match request(args, environment).and_then(|request| call(&studio, &request)) {
        Ok(answer) => answer,
        Err(failure) => return refused(failure, as_json, presses, stdout, stderr),
    };
    if answer.get("failure").is_some() {
        // The only answer at once is a refusal (B5.5): no current model or candidate, or no isolated simulation.
        let exit = json_answer_of(&answer, as_json, output::validate_text, stdout, stderr);
        return settle(exit, presses.count());
    }
    let Some(operation_id) = answer
        .pointer("/result/operationId")
        .and_then(Value::as_str)
    else {
        return refused(no_operation(), as_json, presses, stdout, stderr);
    };
    let followed = follow::to_end(studio.operations(), operation_id, &updates, presses, stderr);
    let available = matches!(
        &followed,
        Ok(follow::Followed::Finished(result)) if output::apply_available(result)
    );
    finished_status(
        follow::ended(followed, as_json, output::validate_text, stdout, stderr),
        available,
    )
}

/// The request that the arguments make (§0.1b). Validate reaches the runtime for its model check and its isolated simulations,
/// so the request carries the connection that `identify` takes: the runtime of `--endpoint` or the project's own runtime,
/// with the token of `TRUST_CONTROL_TOKEN` or of `--token-file`.
fn request(args: &ValidateArgs, environment: Environment<'_>) -> Result<ValidateRequest, Failure> {
    let places = places(&args.common, environment)?;
    let connection = settings::runtime_connection(
        args.common.endpoint.as_deref(),
        &places.project,
        args.common.token_file.as_deref(),
        environment,
    )?;
    Ok(ValidateRequest {
        project_root: text_of(&places.project),
        runtime_id: SELECTED_RUNTIME.to_string(),
        loop_id: args.loop_id.clone(),
        storage_folder: text_of(&places.journal),
        connection: Some(connection),
    })
}

/// The answer of `controlStudio/validate` to `request`, through the entry that `serve` uses (§0.1a): `{"result": …}` or
/// `{"failure": …}`.
fn call(studio: &ControlStudio, request: &ValidateRequest) -> Result<Value, Failure> {
    let arguments = serde_json::to_value(request).map_err(|error| {
        Failure::new(
            &format!("The validate request cannot be written: {error}."),
            Cause::Implementation,
            NextAction::None,
        )
    })?;
    studio
        .execute(commands::VALIDATE, &[arguments])
        .ok_or_else(|| {
            Failure::new(
                &format!("This engine does not answer {}.", commands::VALIDATE),
                Cause::Implementation,
                NextAction::None,
            )
        })
}

/// The failure of a started validation whose answer names no operation to follow.
fn no_operation() -> Failure {
    Failure::new(
        &format!(
            "{} answered without an operation to follow.",
            commands::VALIDATE
        ),
        Cause::Implementation,
        NextAction::None,
    )
}

/// The status of a validation that finished (§0.1b): 0 only when Apply is available, since a validation that did not pass is
/// not a success. `exit` is the status that the finished answer was written with.
fn finished_status(exit: Exit, apply_available: bool) -> Exit {
    match exit {
        Exit::Finished if !apply_available => Exit::Failed,
        exit => exit,
    }
}

#[cfg(test)]
mod tests {
    use std::ffi::OsString;
    use std::path::PathBuf;

    use clap::error::ErrorKind;
    use clap::Parser;
    use trust_control::runtime::RuntimeConnection;

    use super::*;
    use crate::cli::{CommandLine, Verb};

    /// The request that `validate --loop tank` with `extra` makes in the project folder `project`, which has no runtime file,
    /// with its journal folder beside it. No environment variable is set.
    fn request_in(project: &str, extra: &[&str]) -> Result<ValidateRequest, Failure> {
        let journal = format!("{project}/journal");
        let mut arguments = vec![
            "trust-control-studio",
            "validate",
            "--loop",
            "tank",
            "--project",
            project,
            "--journal-dir",
            journal.as_str(),
        ];
        arguments.extend_from_slice(extra);
        let Verb::Validate(args) = CommandLine::try_parse_from(arguments)
            .expect("accepted")
            .verb
        else {
            panic!("validate is read as validate");
        };
        let no_variables = |_: &str| None::<OsString>;
        let environment: Environment<'_> = &no_variables;
        request(&args, environment)
    }

    #[test]
    fn validate_needs_its_loop_and_takes_no_method_and_no_action() {
        let missing = CommandLine::try_parse_from(["trust-control-studio", "validate"])
            .expect_err("the loop is required");
        assert_eq!(missing.kind(), ErrorKind::MissingRequiredArgument);
        for extra in [
            &["--method", "simc"][..],
            &["--reset"][..],
            &["--setting", "Kp=2.5"][..],
        ] {
            let arguments: Vec<&str> = ["trust-control-studio", "validate", "--loop", "tank"]
                .into_iter()
                .chain(extra.iter().copied())
                .collect();
            let error = CommandLine::try_parse_from(arguments.clone())
                .expect_err("validate takes no such argument");
            assert_eq!(error.kind(), ErrorKind::UnknownArgument, "{arguments:?}");
            assert_eq!(error.exit_code(), 2, "{arguments:?}");
        }
    }

    #[test]
    fn validate_takes_the_loop_and_the_common_arguments() {
        let Verb::Validate(args) = CommandLine::try_parse_from([
            "trust-control-studio",
            "validate",
            "--loop",
            "tank",
            "--json",
            "--project",
            "/project",
            "--endpoint",
            "tcp://127.0.0.1:1",
            "--token-file",
            "/token",
            "--journal-dir",
            "/journal",
        ])
        .expect("accepted")
        .verb
        else {
            panic!("validate is read as validate");
        };
        assert_eq!(args.loop_id, "tank");
        assert_eq!(
            (
                args.common.json,
                args.common.project,
                args.common.endpoint,
                args.common.token_file,
                args.common.journal_dir,
            ),
            (
                true,
                Some(PathBuf::from("/project")),
                Some("tcp://127.0.0.1:1".to_string()),
                Some(PathBuf::from("/token")),
                Some(PathBuf::from("/journal")),
            )
        );
    }

    #[test]
    fn a_validation_needs_the_runtime_and_carries_its_connection_and_folders() {
        let failure = request_in("/no-runtime-project", &[]).expect_err("a runtime is needed");
        assert_eq!(
            failure.next_action,
            NextAction::ConnectRuntime,
            "{}",
            failure.message
        );
        let connected = request_in("/no-runtime-project", &["--endpoint", "tcp://127.0.0.1:1"])
            .expect("the endpoint is the runtime");
        assert_eq!(
            connected,
            ValidateRequest {
                project_root: "/no-runtime-project".to_string(),
                runtime_id: SELECTED_RUNTIME.to_string(),
                loop_id: "tank".to_string(),
                storage_folder: "/no-runtime-project/journal".to_string(),
                connection: Some(RuntimeConnection {
                    endpoint: "tcp://127.0.0.1:1".to_string(),
                    token: None,
                    runtime_id: None,
                }),
            }
        );
    }

    #[test]
    fn a_finished_validation_is_status_0_only_when_apply_is_available() {
        assert_eq!(finished_status(Exit::Finished, true), Exit::Finished);
        assert_eq!(
            finished_status(Exit::Finished, false),
            Exit::Failed,
            "a validation that did not pass is not a success"
        );
        assert_eq!(finished_status(Exit::Failed, false), Exit::Failed);
        assert_eq!(finished_status(Exit::Interrupted, false), Exit::Interrupted);
    }
}
