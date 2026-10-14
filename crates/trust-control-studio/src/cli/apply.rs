//! The `apply` verb (specification 35, §0.1b; B6.1 to B6.3). Without `--review` it prints the review of the change and its identity. With
//! `--review <identity>` it applies the reviewed change while the runtime still matches the review, waits for the operating point,
//! transfers, observes the response for the test duration and prints the assessed result; it is status 0 only when that result passed.
//! The verb sends `controlStudio/apply` through `ControlStudio::execute`, the entry that `serve` uses, and adds no rule of its own.

use std::io::Write;

use serde_json::Value;

use super::application::{self, Reviewed};
use super::{application_output, refused, Exit, ReviewArgs};
use crate::commands;
use crate::interrupt::Presses;
use crate::settings::Environment;

/// Runs `apply` (§0.1b). While its operation runs, the first Ctrl-C counted in `presses` cancels it, and the second leaves it to the
/// runtime.
pub(super) fn run(
    args: &ReviewArgs,
    environment: Environment<'_>,
    presses: &mut Presses,
    stdout: &mut dyn Write,
    stderr: &mut dyn Write,
) -> Exit {
    let as_json = args.common.json;
    let request = match application::application_request(args, environment) {
        Ok(request) => request,
        Err(failure) => return refused(failure, as_json, presses, stdout, stderr),
    };
    application::run(
        &Reviewed {
            method: commands::APPLY,
            reviewing: args.review.is_none() && !args.resume,
            as_json,
            review_text: application_output::application_review_text,
            result_text: application_output::apply_text,
            succeeded: apply_succeeded,
            cancelled: apply_cancelled,
        },
        &request,
        presses,
        stdout,
        stderr,
    )
}

/// Whether a finished Apply succeeded: its result passed (B6.3). A failed requirement and a cancelled observation do not.
fn apply_succeeded(outcome: &Value) -> bool {
    outcome.pointer("/result/status").and_then(Value::as_str) == Some("passed")
}

/// Whether an Apply's finished outcome is a cancellation (B6.4, step 9 of C13): its commissioning result is `cancelled`.
fn apply_cancelled(outcome: &Value) -> bool {
    outcome.pointer("/result/status").and_then(Value::as_str) == Some("cancelled")
}

#[cfg(test)]
mod tests {
    use std::ffi::OsString;

    use clap::error::ErrorKind;
    use clap::Parser;
    use serde_json::json;
    use trust_control::failure::{Failure, NextAction};
    use trust_control::messages::ApplicationRequest;
    use trust_control::runtime::RuntimeConnection;

    use super::*;
    use crate::cli::{CommandLine, Verb, SELECTED_RUNTIME};
    use crate::settings::Environment;

    /// The request that `apply` with `extra` makes in the project folder `project`, which has no runtime file. No environment variable
    /// is set.
    fn request_in(project: &str, extra: &[&str]) -> Result<ApplicationRequest, Failure> {
        let journal = format!("{project}/journal");
        let mut arguments = vec![
            "trust-control-studio",
            "apply",
            "--loop",
            "tank",
            "--project",
            project,
            "--journal-dir",
            journal.as_str(),
        ];
        arguments.extend_from_slice(extra);
        let Verb::Apply(args) = CommandLine::try_parse_from(arguments)
            .expect("accepted")
            .verb
        else {
            panic!("apply is read as apply");
        };
        let no_variables = |_: &str| None::<OsString>;
        let environment: Environment<'_> = &no_variables;
        application::application_request(&args, environment)
    }

    #[test]
    fn apply_needs_its_loop_takes_an_optional_review_and_has_no_approval_argument() {
        let missing = CommandLine::try_parse_from(["trust-control-studio", "apply"])
            .expect_err("the loop is required");
        assert_eq!(missing.kind(), ErrorKind::MissingRequiredArgument);
        let approval = CommandLine::try_parse_from([
            "trust-control-studio",
            "apply",
            "--loop",
            "tank",
            "--approve-physical",
        ])
        .expect_err("a physical target is refused on the command line, not approved there (§0.1b)");
        assert_eq!(approval.kind(), ErrorKind::UnknownArgument);
        assert_eq!(approval.exit_code(), 2);
        let Verb::Apply(args) = CommandLine::try_parse_from([
            "trust-control-studio",
            "apply",
            "--loop",
            "tank",
            "--review",
            "sha256:abc",
            "--json",
        ])
        .expect("accepted")
        .verb
        else {
            panic!("apply is read as apply");
        };
        assert_eq!(args.loop_id, "tank");
        assert_eq!(args.review.as_deref(), Some("sha256:abc"));
        assert!(args.common.json);
    }

    #[test]
    fn the_request_names_the_runtime_and_gives_no_physical_approval() {
        let failure = request_in("/no-runtime-project", &[]).expect_err("a runtime is needed");
        assert_eq!(
            failure.next_action,
            NextAction::ConnectRuntime,
            "{}",
            failure.message
        );
        let connected = request_in(
            "/no-runtime-project",
            &["--endpoint", "tcp://127.0.0.1:1", "--review", "sha256:abc"],
        )
        .expect("the endpoint is the runtime");
        assert_eq!(
            connected,
            ApplicationRequest {
                project_root: "/no-runtime-project".to_string(),
                runtime_id: SELECTED_RUNTIME.to_string(),
                loop_id: "tank".to_string(),
                storage_folder: "/no-runtime-project/journal".to_string(),
                connection: Some(RuntimeConnection {
                    endpoint: "tcp://127.0.0.1:1".to_string(),
                    token: None,
                }),
                review: Some("sha256:abc".to_string()),
                physical_approved: false,
                resume: false,
            }
        );
    }

    #[test]
    fn apply_is_a_success_only_when_its_result_passed() {
        assert!(apply_succeeded(
            &json!({ "result": { "status": "passed" } })
        ));
        assert!(!apply_succeeded(
            &json!({ "result": { "status": "failed" } })
        ));
        assert!(!apply_succeeded(
            &json!({ "result": { "status": "cancelled" } })
        ));
        assert!(!apply_succeeded(&json!({})));
    }
}
