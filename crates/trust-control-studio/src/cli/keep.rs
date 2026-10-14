//! The `keep` verb (specification 35, §0.1b; B6.5). Without `--review` it prints the review of the edit and its identity. With
//! `--review <identity>` it keeps the values that the loop's parameter record holds in the running PLC as the record's initial values in
//! the project, and prints the fields kept. The command line has no editor, so it passes no file with unsaved changes: the engine writes
//! a file only while it still holds the text that the review read. The verb sends `controlStudio/keep` through `ControlStudio::execute`,
//! the entry that `serve` uses, and adds no rule of its own.

use std::io::Write;

use serde_json::Value;
use trust_control::failure::Failure;
use trust_control::messages::KeepRequest;

use super::application::{self, Reviewed};
use super::{application_output, refused, Exit, ReviewArgs, SELECTED_RUNTIME};
use crate::commands;
use crate::interrupt::Presses;
use crate::settings::Environment;

/// Runs `keep` (§0.1b). While its operation runs, the first Ctrl-C counted in `presses` cancels it, and the second leaves it to the
/// runtime.
pub(super) fn run(
    args: &ReviewArgs,
    environment: Environment<'_>,
    presses: &mut Presses,
    stdout: &mut dyn Write,
    stderr: &mut dyn Write,
) -> Exit {
    if args.resume {
        let _ = writeln!(
            stderr,
            "--resume follows a journaled Apply; keep has nothing to resume."
        );
        return Exit::Usage;
    }
    let as_json = args.common.json;
    let request = match request(args, environment) {
        Ok(request) => request,
        Err(failure) => return refused(failure, as_json, presses, stdout, stderr),
    };
    application::run(
        &Reviewed {
            method: commands::KEEP,
            reviewing: args.review.is_none(),
            as_json,
            review_text: application_output::keep_review_text,
            result_text: application_output::keep_text,
            succeeded: keep_succeeded,
            cancelled: application::never_cancelled,
        },
        &request,
        presses,
        stdout,
        stderr,
    )
}

/// The request that the arguments of `keep` make (§0.1b). No editor is open, so no file has unsaved changes, and the language tool is
/// the one beside the engine or on the PATH.
///
/// # Errors
/// As `application::target`.
fn request(args: &ReviewArgs, environment: Environment<'_>) -> Result<KeepRequest, Failure> {
    let target = application::target(args, environment)?;
    Ok(KeepRequest {
        project_root: target.project_root,
        runtime_id: SELECTED_RUNTIME.to_string(),
        loop_id: args.loop_id.clone(),
        storage_folder: target.storage_folder,
        connection: Some(target.connection),
        review: args.review.clone(),
        physical_approved: false,
        unsaved_files: Vec::new(),
        language_tool: None,
    })
}

/// Whether a finished Keep succeeded: its outcome exists only once the project's values were written (B6.5).
fn keep_succeeded(_outcome: &Value) -> bool {
    true
}

#[cfg(test)]
mod tests {
    use clap::error::ErrorKind;
    use clap::Parser;

    use super::*;
    use crate::cli::{CommandLine, Verb};

    #[test]
    fn keep_needs_its_loop_and_takes_the_review_identity_only() {
        let missing = CommandLine::try_parse_from(["trust-control-studio", "keep"])
            .expect_err("the loop is required");
        assert_eq!(missing.kind(), ErrorKind::MissingRequiredArgument);
        let approval = CommandLine::try_parse_from([
            "trust-control-studio",
            "keep",
            "--loop",
            "tank",
            "--approve-physical",
        ])
        .expect_err("keep takes no approval argument");
        assert_eq!(approval.kind(), ErrorKind::UnknownArgument);
        let Verb::Keep(args) = CommandLine::try_parse_from([
            "trust-control-studio",
            "keep",
            "--loop",
            "tank",
            "--review",
            "sha256:keep",
        ])
        .expect("accepted")
        .verb
        else {
            panic!("keep is read as keep");
        };
        assert_eq!(args.review.as_deref(), Some("sha256:keep"));
    }

    #[test]
    fn a_keep_request_passes_no_unsaved_file_and_no_language_tool_path() {
        let Verb::Keep(args) = CommandLine::try_parse_from([
            "trust-control-studio",
            "keep",
            "--loop",
            "tank",
            "--endpoint",
            "tcp://127.0.0.1:1",
            "--project",
            "/project",
            "--journal-dir",
            "/journal",
        ])
        .expect("accepted")
        .verb
        else {
            panic!("keep is read as keep");
        };
        let no_variables = |_: &str| None::<std::ffi::OsString>;
        let environment: Environment<'_> = &no_variables;
        let request = request(&args, environment).expect("the endpoint is the runtime");
        assert_eq!(request.unsaved_files, [] as [std::string::String; 0]);
        assert!(request.language_tool.is_none());
        assert!(!request.physical_approved);
        assert_eq!(request.loop_id, "tank");
        assert_eq!(request.runtime_id, SELECTED_RUNTIME);
    }

    #[test]
    fn a_finished_keep_is_a_success() {
        assert!(keep_succeeded(&Value::Null));
    }
}
