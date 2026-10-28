//! The `cancel` verb (specification 35, §0.1b; D6, D7). It cancels one operation journaled for a
//! loop, never an operation of this process, which has none: it reattaches by the journaled
//! identifier (E11.5). The engine's `cancel_loop` starts the cancellation as an operation of this
//! process: a running experiment ends through its restoration path (E5.5), and the journal is
//! cleared once the outcome is kept (E11.6). The verb follows that operation as `identify` follows
//! its own: its success is said only when the operation finished, and its failure is the verb's
//! failure. It starts nothing when nothing is journaled, and it names the operation it cancels.

use std::io::Write;

use trust_control::failure::Failure;
use trust_control::identification::connected_run::not_journaled;
use trust_control::messages::{CancelResult, OperationStarted};

use super::{
    answer_of, follow, output, places, refused, settle, CancelArgs, Exit, SELECTED_RUNTIME,
};
use crate::commands::ControlStudio;
use crate::identify_action::{cancel_loop, CancelRequest};
use crate::interrupt::Presses;
use crate::journal::{Journal, JournalEntry};
use crate::loops;
use crate::settings::{self, Environment};

/// Runs `cancel` (§0.1b). While the cancellation runs, the first Ctrl-C counted in `presses` cancels
/// it and the verb waits for its final update; the second leaves it to the runtime.
pub(super) fn run(
    args: &CancelArgs,
    environment: Environment<'_>,
    presses: &mut Presses,
    stdout: &mut dyn Write,
    stderr: &mut dyn Write,
) -> Exit {
    let as_json = args.common.json;
    let (studio, updates) = follow::engine();
    match start(args, environment, &studio) {
        Ok(Cancelling::Started(started)) => {
            let followed = follow::to_end(
                studio.operations(),
                &started.operation_id,
                &updates,
                presses,
                stderr,
            );
            follow::ended(followed, as_json, output::cancelled_text, stdout, stderr)
        }
        Ok(Cancelling::Nothing(text)) => {
            let nothing = CancelResult {
                cancelled: Vec::new(),
            };
            let exit = answer_of(Ok(nothing), as_json, |_| text, stdout, stderr);
            settle(exit, presses.count())
        }
        Ok(Cancelling::Usage(text)) => {
            let _ = writeln!(stderr, "{text}");
            settle(Exit::Usage, presses.count())
        }
        Err(failure) => refused(failure, as_json, presses, stdout, stderr),
    }
}

/// What `cancel` does once it has read the loop's journal: the cancellation it started, or an answer
/// without one.
#[derive(Debug, PartialEq, Eq)]
enum Cancelling {
    /// The cancellation runs as this operation.
    Started(OperationStarted),
    /// Nothing is journaled for the loop; the words say so (status 0).
    Nothing(String),
    /// A usage error (status 2), with its words.
    Usage(String),
}

/// Reads the loop's journal and, when it names the operation to cancel, starts its cancellation on
/// the runtime of `--endpoint` or the project's own runtime, as an operation of `studio` with
/// the studio's own list of recordings to release (D7).
fn start(
    args: &CancelArgs,
    environment: Environment<'_>,
    studio: &ControlStudio,
) -> Result<Cancelling, Failure> {
    let places = places(&args.common, environment)?;
    let loop_definition = loops::open(&places.project, SELECTED_RUNTIME, &args.loop_id)?;
    let journal = Journal::open(
        &places.journal,
        &loop_definition.project_id,
        &loop_definition.id,
    )?;
    let journaled: Vec<&str> = journal
        .entries()
        .iter()
        .map(JournalEntry::operation_id)
        .collect();
    let operation_id = match target(
        &loop_definition.id,
        &journaled,
        args.operation_id.as_deref(),
    ) {
        Ok(operation_id) => operation_id,
        Err(answer) => return Ok(answer),
    };
    let connection = settings::runtime_connection(
        args.common.endpoint.as_deref(),
        &places.project,
        args.common.token_file.as_deref(),
        environment,
    )?;
    cancel_loop(
        studio.operations(),
        studio.pending_releases(),
        studio.loop_claims(),
        CancelRequest {
            connection: &connection,
            project_root: &places.project,
            storage_folder: &places.journal,
            loop_definition: &loop_definition,
            operation_id: &operation_id,
        },
    )
    .map(Cancelling::Started)
}

/// The operation that `cancel` cancels, given the operations `journaled` for loop `loop_id` and the one `named` on the command
/// line. A named operation must be journaled, even when nothing is (D6). Without a name, nothing journaled is nothing to cancel
/// (D7), one journaled operation is cancelled, and several need a name (D6). What is not cancelled is the answer instead.
fn target(loop_id: &str, journaled: &[&str], named: Option<&str>) -> Result<String, Cancelling> {
    match named {
        Some(operation_id) if journaled.contains(&operation_id) => Ok(operation_id.to_string()),
        Some(operation_id) => {
            let refusal = not_journaled(loop_id, operation_id).message;
            Err(Cancelling::Usage(if journaled.is_empty() {
                refusal
            } else {
                format!("{refusal} {}", journaled_text(loop_id, journaled))
            }))
        }
        None => match journaled {
            [] => Err(Cancelling::Nothing(format!(
                "No operation is journaled for loop {loop_id}."
            ))),
            [one] => Ok((*one).to_string()),
            _ => Err(Cancelling::Usage(format!(
                "Name the operation to cancel. {}",
                journaled_text(loop_id, journaled)
            ))),
        },
    }
}

/// The words that list the operations journaled for loop `loop_id`, at least one.
fn journaled_text(loop_id: &str, journaled: &[&str]) -> String {
    match journaled {
        [one] => format!("The operation journaled for loop {loop_id} is {one}."),
        _ => format!(
            "The operations journaled for loop {loop_id} are {}.",
            journaled.join(", ")
        ),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn usage(text: &str) -> Cancelling {
        Cancelling::Usage(text.to_string())
    }

    #[test]
    fn an_empty_journal_without_a_named_operation_has_nothing_to_cancel_and_says_so() {
        assert_eq!(
            target("tank", &[], None),
            Err(Cancelling::Nothing(
                "No operation is journaled for loop tank.".to_string()
            ))
        );
    }

    #[test]
    fn the_one_journaled_operation_or_the_named_one_is_cancelled() {
        assert_eq!(
            target("tank", &["recording-1"], None),
            Ok("recording-1".to_string())
        );
        assert_eq!(
            target("tank", &["recording-1"], Some("recording-1")),
            Ok("recording-1".to_string())
        );
        assert_eq!(
            target(
                "tank",
                &["recording-1", "experiment-2"],
                Some("experiment-2")
            ),
            Ok("experiment-2".to_string()),
            "the named one among several is the one cancelled"
        );
    }

    #[test]
    fn a_missing_or_unknown_operation_is_a_usage_error_that_lists_the_journaled_ones() {
        assert_eq!(
            target("tank", &[], Some("operation-1")),
            Err(usage(
                "Operation operation-1 is not journaled for loop tank."
            )),
            "a named operation must be journaled, even when nothing is"
        );
        assert_eq!(
            target("tank", &["recording-1", "experiment-2"], None),
            Err(usage("Name the operation to cancel. The operations journaled for loop tank are recording-1, experiment-2."))
        );
        assert_eq!(
            target("tank", &["recording-1"], Some("operation-1")),
            Err(usage("Operation operation-1 is not journaled for loop tank. The operation journaled for loop tank is recording-1."))
        );
        assert_eq!(
            target(
                "tank",
                &["recording-1", "experiment-2"],
                Some("operation-1")
            ),
            Err(usage("Operation operation-1 is not journaled for loop tank. The operations journaled for loop tank are recording-1, experiment-2."))
        );
    }

    #[test]
    fn a_cancel_that_cancelled_nothing_answers_an_empty_list_in_json_never_null() {
        let (mut stdout, mut stderr) = (Vec::<u8>::new(), Vec::<u8>::new());
        let exit = answer_of(
            Ok(CancelResult {
                cancelled: Vec::new(),
            }),
            true,
            |_| "No operation is journaled for loop tank.".to_string(),
            &mut stdout,
            &mut stderr,
        );
        assert_eq!(exit, Exit::Finished);
        assert_eq!(
            String::from_utf8(stdout).expect("UTF-8"),
            "{\"result\":{\"cancelled\":[]}}\n"
        );
        assert!(stderr.is_empty(), "nothing is refused");
    }
}
