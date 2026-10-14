//! The `restore` verb (specification 35, §0.1b; B6.4). Without `--review` it prints the review of the restore and its identity. With
//! `--review <identity>` it returns the parameter record, and the mode from before Apply, to the retained pre-Apply revision through the
//! runtime, and prints Restored once the runtime confirms it. It is status 0 only when the runtime confirmed the restore. The verb sends
//! `controlStudio/restore` through `ControlStudio::execute`, the entry that `serve` uses, and adds no rule of its own.

use std::io::Write;

use serde_json::Value;

use super::application::{self, Reviewed};
use super::{application_output, refused, Exit, ReviewArgs};
use crate::commands;
use crate::interrupt::Presses;
use crate::settings::Environment;

/// Runs `restore` (§0.1b). While its operation runs, the first Ctrl-C counted in `presses` cancels it, and the second leaves it to the
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
            method: commands::RESTORE,
            reviewing: args.review.is_none(),
            as_json,
            review_text: application_output::application_review_text,
            result_text: application_output::restore_text,
            succeeded: restore_succeeded,
            cancelled: application::never_cancelled,
        },
        &request,
        presses,
        stdout,
        stderr,
    )
}

/// Whether a finished Restore succeeded: the runtime confirmed the previous settings back (B6.4).
fn restore_succeeded(outcome: &Value) -> bool {
    outcome.get("restored").and_then(Value::as_bool) == Some(true)
}

#[cfg(test)]
mod tests {
    use clap::error::ErrorKind;
    use clap::Parser;
    use serde_json::json;

    use super::*;
    use crate::cli::{CommandLine, Verb};

    #[test]
    fn restore_needs_its_loop_and_takes_the_review_identity_only() {
        let missing = CommandLine::try_parse_from(["trust-control-studio", "restore"])
            .expect_err("the loop is required");
        assert_eq!(missing.kind(), ErrorKind::MissingRequiredArgument);
        let method = CommandLine::try_parse_from([
            "trust-control-studio",
            "restore",
            "--loop",
            "tank",
            "--method",
            "simc",
        ])
        .expect_err("restore takes no method");
        assert_eq!(method.kind(), ErrorKind::UnknownArgument);
        let Verb::Restore(args) = CommandLine::try_parse_from([
            "trust-control-studio",
            "restore",
            "--loop",
            "tank",
            "--review",
            "sha256:abc",
        ])
        .expect("accepted")
        .verb
        else {
            panic!("restore is read as restore");
        };
        assert_eq!(args.review.as_deref(), Some("sha256:abc"));
    }

    #[test]
    fn a_restore_is_a_success_only_when_the_runtime_confirmed_it() {
        assert!(restore_succeeded(&json!({ "restored": true })));
        assert!(!restore_succeeded(&json!({ "restored": false })));
        assert!(!restore_succeeded(&json!({})));
    }
}
