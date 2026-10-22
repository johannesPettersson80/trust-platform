//! The `tune` verb (specification 35, §0.1b; B4): designs the loop's controller candidate from its study's model, with
//! Automatic or the method named, against the requirements the study holds and the criteria Automatic fills (B4.3). The
//! verb sends `controlStudio/tune` through `ControlStudio::execute`, the entry that `serve` uses, so every rule, limit and
//! refusal is the method's. A refusal comes at once (B4.7); otherwise the method starts an operation, which the verb follows
//! to its final update. Tune never writes to the controller (B4.1); with a runtime it reads the running controller's settings for
//! the comparison of the predicted responses (B4.5).

use std::collections::BTreeMap;
use std::io::Write;

use serde_json::Value;
use trust_control::failure::{Cause, Failure, NextAction};
use trust_control::messages::TuneRequest;
use trust_control::runtime::RuntimeConnection;

use super::{
    follow, json_answer_of, places, refused, settle, text_of, tune_output, Exit, Places, TuneArgs,
    TuneChoiceWord, TuneMethodWord, TuneStructureWord, SELECTED_RUNTIME,
};
use crate::commands::{self, ControlStudio};
use crate::interrupt::Presses;
use crate::settings::{self, Environment};

/// Runs `tune` (§0.1b). While its operation runs, the first Ctrl-C counted in `presses` cancels it and the verb waits for its
/// final update; the second leaves it to the runtime.
pub(super) fn run(
    args: &TuneArgs,
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
        // The only answer at once is a refusal (B4.7): no current model, or a method that does not apply.
        let exit = json_answer_of(&answer, as_json, tune_output::tune_text, stdout, stderr);
        return settle(exit, presses.count());
    }
    let Some(operation_id) = answer
        .pointer("/result/operationId")
        .and_then(Value::as_str)
    else {
        return refused(no_operation(), as_json, presses, stdout, stderr);
    };
    let followed = follow::to_end(studio.operations(), operation_id, &updates, presses, stderr);
    follow::ended(followed, as_json, tune_output::tune_text, stdout, stderr)
}

/// The request that the arguments make (§0.1b). With a runtime, `--endpoint` or the project's `runtime.toml`, the request carries
/// its connection, so that Tune reads the running controller's settings for the comparison (B4.5); without one it carries none,
/// and Tune designs without the comparison. `automatic` names no formula method, so the request carries none; the pinned settings
/// of B4.2 have no argument. The settings that `--setting` names go in the request; the engine decides whether the method takes them.
fn request(args: &TuneArgs, environment: Environment<'_>) -> Result<TuneRequest, Failure> {
    let places = places(&args.common, environment)?;
    Ok(TuneRequest {
        project_root: text_of(&places.project),
        runtime_id: SELECTED_RUNTIME.to_string(),
        loop_id: args.loop_id.clone(),
        storage_folder: text_of(&places.journal),
        method: args.method.and_then(TuneMethodWord::method),
        choice: args.choice.map(TuneChoiceWord::choice),
        structure: args.structure.map(TuneStructureWord::structure),
        settings: settings_of(&args.setting),
        pins: None,
        connection: connection_of(args, &places, environment)?,
    })
}

/// The runtime that the arguments name, read as `identify` reads it (§0.1b): `--endpoint`, else the project's `runtime.toml`, with
/// the token of `--token-file` or `TRUST_CONTROL_TOKEN`. A project with no runtime gives none. A runtime or token file that the
/// arguments name must be readable, so a refusal names it, as `identify` refuses it.
///
/// # Errors
/// The `--endpoint` or `--token-file` that the arguments name cannot be read.
fn connection_of(
    args: &TuneArgs,
    places: &Places,
    environment: Environment<'_>,
) -> Result<Option<RuntimeConnection>, Failure> {
    match settings::runtime_connection(
        args.common.endpoint.as_deref(),
        &places.project,
        args.common.token_file.as_deref(),
        environment,
    ) {
        Ok(connection) => Ok(Some(connection)),
        Err(_) if args.common.endpoint.is_none() && args.common.token_file.is_none() => Ok(None),
        Err(failure) => Err(failure),
    }
}

/// The settings that `--setting` names (B4.2): `None` when none is named, else each record symbol with its number. A symbol named
/// twice keeps its last number.
fn settings_of(entries: &[(String, f64)]) -> Option<BTreeMap<String, f64>> {
    if entries.is_empty() {
        return None;
    }
    Some(entries.iter().cloned().collect())
}

/// The answer of `controlStudio/tune` to `request`, through the entry that `serve` uses (§0.1a): `{"result": …}` or
/// `{"failure": …}`.
fn call(studio: &ControlStudio, request: &TuneRequest) -> Result<Value, Failure> {
    let arguments = serde_json::to_value(request).map_err(|error| {
        Failure::new(
            &format!("The tune request cannot be written: {error}."),
            Cause::Implementation,
            NextAction::None,
        )
    })?;
    studio.execute(commands::TUNE, &[arguments]).ok_or_else(|| {
        Failure::new(
            &format!("This engine does not answer {}.", commands::TUNE),
            Cause::Implementation,
            NextAction::None,
        )
    })
}

/// The failure of a started tuning whose answer names no operation to follow.
fn no_operation() -> Failure {
    Failure::new(
        &format!(
            "{} answered without an operation to follow.",
            commands::TUNE
        ),
        Cause::Implementation,
        NextAction::None,
    )
}

#[cfg(test)]
mod tests {
    use std::ffi::OsString;

    use clap::Parser;
    use trust_control::messages::{TuneChoice, TuneMethod};
    use trust_control::tuning::structure::Structure;

    use super::*;
    use crate::cli::{CommandLine, Verb};

    /// The arguments that `tune --loop tank` with `extra` parse to in the project folder `project`, with its journal folder beside
    /// it.
    fn args_in(project: &str, extra: &[&str]) -> TuneArgs {
        let journal = format!("{project}/journal");
        let mut arguments = vec![
            "trust-control-studio",
            "tune",
            "--loop",
            "tank",
            "--project",
            project,
            "--journal-dir",
            journal.as_str(),
        ];
        arguments.extend_from_slice(extra);
        let Verb::Tune(args) = CommandLine::try_parse_from(arguments)
            .expect("accepted")
            .verb
        else {
            panic!("tune is read as tune");
        };
        args
    }

    /// The request that `tune --loop tank` with `extra` makes in the project folder `project`, with its journal folder beside
    /// it. No environment variable is set.
    fn request_in(project: &str, extra: &[&str]) -> TuneRequest {
        let no_variables = |_: &str| None::<OsString>;
        let environment: Environment<'_> = &no_variables;
        request(&args_in(project, extra), environment).expect("the folders are named")
    }

    #[test]
    fn automatic_is_a_request_without_a_method_a_response_or_a_structure() {
        let automatic = request_in("/project", &[]);
        assert_eq!(
            automatic,
            TuneRequest {
                project_root: "/project".to_string(),
                runtime_id: SELECTED_RUNTIME.to_string(),
                loop_id: "tank".to_string(),
                storage_folder: "/project/journal".to_string(),
                method: None,
                choice: None,
                structure: None,
                settings: None,
                pins: None,
                connection: None,
            }
        );
        assert_eq!(
            request_in("/project", &["--method", "automatic"]),
            automatic,
            "automatic names no formula method"
        );
    }

    #[test]
    fn the_words_reach_the_request_and_the_endpoint_names_its_connection() {
        let named = request_in(
            "/project",
            &[
                "--method",
                "amigo",
                "--choice",
                "more-robust",
                "--structure",
                "pd",
                "--endpoint",
                "tcp://127.0.0.1:1",
            ],
        );
        assert_eq!(
            (named.method, named.choice, named.structure, named.pins),
            (
                Some(TuneMethod::Amigo),
                Some(TuneChoice::MoreRobust),
                Some(Structure::PD),
                None
            )
        );
        assert_eq!(
            named.connection,
            Some(RuntimeConnection {
                endpoint: "tcp://127.0.0.1:1".to_string(),
                token: None,
                runtime_id: None,
            }),
            "the endpoint that the arguments name is the runtime the comparison reads (B4.5)"
        );
    }

    #[test]
    fn a_token_file_that_the_arguments_name_must_be_readable() {
        let args = args_in(
            "/project",
            &[
                "--endpoint",
                "tcp://127.0.0.1:1",
                "--token-file",
                "/no-such-token",
            ],
        );
        let no_variables = |_: &str| None::<OsString>;
        let environment: Environment<'_> = &no_variables;
        assert!(
            request(&args, environment).is_err(),
            "a token file that cannot be read refuses the request, as identify refuses it"
        );
    }

    #[test]
    fn the_manual_method_takes_the_settings_named_and_the_relay_rule_takes_none() {
        let manual = request_in(
            "/project",
            &[
                "--method",
                "manual",
                "--setting",
                "Kp=2.5",
                "--setting",
                "Ki=0.5",
            ],
        );
        assert_eq!(
            (manual.method, manual.settings),
            (
                Some(TuneMethod::Manual),
                Some(BTreeMap::from([
                    ("Kp".to_string(), 2.5),
                    ("Ki".to_string(), 0.5)
                ]))
            )
        );
        let relay = request_in("/project", &["--method", "relay"]);
        assert_eq!(
            (relay.method, relay.settings),
            (Some(TuneMethod::Relay), None)
        );
        // Settings with another method reach the engine too, which refuses them (B4.2).
        let simc = request_in("/project", &["--method", "simc", "--setting", "Kp=2.5"]);
        assert_eq!(
            simc.settings,
            Some(BTreeMap::from([("Kp".to_string(), 2.5)]))
        );
    }
}
