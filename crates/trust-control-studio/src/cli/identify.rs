//! The `identify` verb (specification 35, §0.1b; B3): a live campaign with the loop's own method or
//! the one named, a kept recording fitted again (B3.6), the journaled campaign followed to its
//! end (A8.5), or a new working identification study (A8.4). The verb sends
//! `controlStudio/identify` through `ControlStudio::execute`, the entry that `serve` uses, so every
//! rule, limit and refusal is the method's. A reset answers at once; every other action follows its
//! operation to the final update.

use std::io::Write;

use serde_json::Value;
use trust_control::failure::{Cause, Failure, NextAction};
use trust_control::identification::acquisition_settings::declared_settings;
use trust_control::messages::{IdentifyAction, IdentifyRequest};
use trust_control::runtime::RuntimeConnection;

use super::{
    follow, json_answer_of, output, places, refused, settle, text_of, Exit, IdentifyArgs, Places,
    Requested, SELECTED_RUNTIME,
};
use crate::commands::{self, ControlStudio};
use crate::interrupt::Presses;
use crate::loops;
use crate::settings::{self, Environment};

/// Runs `identify` (§0.1b). While its operation runs, the first Ctrl-C counted in `presses` cancels
/// it and the verb waits for its final update; the second leaves it to the runtime.
pub(super) fn run(
    args: &IdentifyArgs,
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
    if args.requested() == Requested::Reset || answer.get("failure").is_some() {
        let exit = answered_at_once(&answer, as_json, stdout, stderr);
        return settle(exit, presses.count());
    }
    let Some(operation_id) = answer
        .pointer("/result/operationId")
        .and_then(Value::as_str)
    else {
        return refused(no_operation(), as_json, presses, stdout, stderr);
    };
    let followed = follow::to_end(studio.operations(), operation_id, &updates, presses, stderr);
    follow::ended(followed, as_json, output::identify_text, stdout, stderr)
}

/// The request that the arguments make (§0.1b). A live campaign and a resume reach the runtime, so they carry its connection:
/// the runtime of `--endpoint` or of the project's `runtime.toml`. A live campaign starts from the loop's declared settings with
/// the method asked for (D8). A kept recording fitted again and a reset need no runtime, so their request carries none (E11.1).
fn request(args: &IdentifyArgs, environment: Environment<'_>) -> Result<IdentifyRequest, Failure> {
    let places = places(&args.common, environment)?;
    let (action, connection) = match args.requested() {
        Requested::Live(method) => {
            let connection = connection_of(args, &places, environment)?;
            let loop_definition = loops::open(&places.project, SELECTED_RUNTIME, &args.loop_id)?;
            // The command line names no review and no approval: a physical target is refused here, as in the panel (A2.5).
            let action = IdentifyAction::Identify {
                settings: Some(declared_settings(&loop_definition, method)),
                review: None,
                physical_approved: false,
            };
            (action, Some(connection))
        }
        Requested::Recording(recording_id) => (
            IdentifyAction::OpenRecording {
                recording_id: Some(recording_id.to_string()),
                inputs: None,
                outputs: None,
            },
            None,
        ),
        Requested::Resume => (
            IdentifyAction::Resume,
            Some(connection_of(args, &places, environment)?),
        ),
        Requested::Reset => (IdentifyAction::Reset, None),
    };
    Ok(IdentifyRequest {
        project_root: text_of(&places.project),
        runtime_id: SELECTED_RUNTIME.to_string(),
        loop_id: args.loop_id.clone(),
        storage_folder: text_of(&places.journal),
        connection,
        runtime_mode: None,
        action,
    })
}

/// The runtime of `--endpoint` or of the project's `runtime.toml`, with the token of `TRUST_CONTROL_TOKEN` or `--token-file`
/// (§0.1b): a refusal names how to connect one (Connect runtime).
fn connection_of(
    args: &IdentifyArgs,
    places: &Places,
    environment: Environment<'_>,
) -> Result<RuntimeConnection, Failure> {
    settings::runtime_connection(
        args.common.endpoint.as_deref(),
        &places.project,
        args.common.token_file.as_deref(),
        environment,
    )
}

/// The answer of `controlStudio/identify` to `request`, through the entry that `serve` uses
/// (§0.1a): `{"result": …}` or `{"failure": …}`.
fn call(studio: &ControlStudio, request: &IdentifyRequest) -> Result<Value, Failure> {
    let arguments = serde_json::to_value(request).map_err(|error| {
        Failure::new(
            &format!("The identify request cannot be written: {error}."),
            Cause::Implementation,
            NextAction::None,
        )
    })?;
    studio
        .execute(commands::IDENTIFY, &[arguments])
        .ok_or_else(|| {
            Failure::new(
                &format!("This engine does not answer {}.", commands::IDENTIFY),
                Cause::Implementation,
                NextAction::None,
            )
        })
}

/// Writes an answer that came at once: a reset's result as one readable line (A8.4), or a refusal.
fn answered_at_once(
    answer: &Value,
    as_json: bool,
    stdout: &mut dyn Write,
    stderr: &mut dyn Write,
) -> Exit {
    json_answer_of(
        answer,
        as_json,
        |_| output::RESET_TEXT.to_string(),
        stdout,
        stderr,
    )
}

/// The failure of a started identification whose answer names no operation to follow.
fn no_operation() -> Failure {
    Failure::new(
        &format!(
            "{} answered without an operation to follow.",
            commands::IDENTIFY
        ),
        Cause::Implementation,
        NextAction::None,
    )
}

#[cfg(test)]
mod tests {
    use std::ffi::OsString;

    use clap::Parser;
    use serde_json::json;

    use super::*;
    use crate::cli::{CommandLine, Verb};

    /// The status, standard output and standard error of an answer that came at once.
    fn written(answer: &Value, as_json: bool) -> (Exit, String, String) {
        let (mut stdout, mut stderr) = (Vec::<u8>::new(), Vec::<u8>::new());
        let exit = answered_at_once(answer, as_json, &mut stdout, &mut stderr);
        (
            exit,
            String::from_utf8(stdout).expect("UTF-8"),
            String::from_utf8(stderr).expect("UTF-8"),
        )
    }

    #[test]
    fn a_reset_reads_as_one_line_or_with_json_as_the_answer_itself_with_status_0() {
        let answer =
            json!({ "result": { "stages": [{ "stage": "Identify", "status": "needs-data" }] } });
        assert_eq!(
            written(&answer, false),
            (
                Exit::Finished,
                "The working identification study was started again; the previous study is in history.\n"
                    .to_string(),
                String::new()
            )
        );
        assert_eq!(
            written(&answer, true),
            (Exit::Finished, format!("{answer}\n"), String::new())
        );
    }

    #[test]
    fn a_refusal_is_status_1_with_its_failure_text_and_with_json_the_answer_on_standard_output() {
        let failure = Failure::new(
            "Loop tank has an unfinished operation. Resume it or cancel it first.",
            Cause::Settings,
            NextAction::Identify,
        );
        let answer = json!({ "failure": failure });
        let failure_line = format!("{}\n", output::failure_text(&failure));
        assert_eq!(
            written(&answer, false),
            (Exit::Failed, String::new(), failure_line.clone())
        );
        assert_eq!(
            written(&answer, true),
            (Exit::Failed, format!("{answer}\n"), failure_line)
        );
    }

    /// The request that `identify --loop tank` with `extra` makes in the project folder `project`, which has no runtime file,
    /// with its journal folder beside it. No environment variable is set.
    fn request_in(project: &str, extra: &[&str]) -> Result<IdentifyRequest, Failure> {
        let journal = format!("{project}/journal");
        let mut arguments = vec![
            "trust-control-studio",
            "identify",
            "--loop",
            "tank",
            "--project",
            project,
            "--journal-dir",
            journal.as_str(),
        ];
        arguments.extend_from_slice(extra);
        let Verb::Identify(args) = CommandLine::try_parse_from(arguments)
            .expect("accepted")
            .verb
        else {
            panic!("identify is read as identify");
        };
        let no_variables = |_: &str| None::<OsString>;
        let environment: Environment<'_> = &no_variables;
        request(&args, environment)
    }

    #[test]
    fn a_kept_recording_fitted_again_and_a_reset_need_no_runtime_and_carry_no_connection() {
        let recording = format!("rec-{}", "0".repeat(32));
        let fitted = request_in("/no-runtime-project", &["--recording", recording.as_str()])
            .expect("no runtime is needed");
        assert_eq!(fitted.connection, None, "{fitted:?}");
        assert_eq!(
            fitted.action,
            IdentifyAction::OpenRecording {
                recording_id: Some(recording.clone()),
                inputs: None,
                outputs: None,
            }
        );
        let reset = request_in("/no-runtime-project", &["--reset"]).expect("no runtime is needed");
        assert_eq!(
            (reset.connection, reset.action),
            (None, IdentifyAction::Reset)
        );
    }

    #[test]
    fn a_live_campaign_and_a_resume_reach_the_runtime_and_are_refused_without_one_to_connect_it() {
        let live: &[&str] = &[];
        let resume: &[&str] = &["--resume"];
        for extra in [live, resume] {
            let failure =
                request_in("/no-runtime-project", extra).expect_err("a runtime is needed");
            assert_eq!(
                failure.next_action,
                NextAction::ConnectRuntime,
                "{extra:?}: {}",
                failure.message
            );
        }
        let connected = request_in(
            "/no-runtime-project",
            &["--resume", "--endpoint", "tcp://127.0.0.1:1"],
        )
        .expect("the endpoint is the runtime");
        assert_eq!(
            connected.connection,
            Some(RuntimeConnection {
                endpoint: "tcp://127.0.0.1:1".to_string(),
                token: None,
                runtime_id: None,
            })
        );
        assert_eq!(connected.action, IdentifyAction::Resume);
    }
}
