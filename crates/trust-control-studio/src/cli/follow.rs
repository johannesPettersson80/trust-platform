//! Following an operation to its final update (specification 35, §0.1b). A verb that started an
//! operation waits on the channel that its engine's updates arrive on: each progress update is one
//! line on standard error, and the final update ends the wait. Between updates, at least every tenth
//! of a second, the Ctrl-C presses are counted. The first press cancels the operation as Cancel does,
//! and the verb waits for its final update: a live experiment ends through its restoration path, and
//! the update comes once the runtime has confirmed it. The second press ends the wait at once and
//! leaves the experiment, its recording and the journal to the runtime, as closing VS Code does
//! (A8.6); the operation's thread is left running and ends with the process.

use std::io::Write;
use std::sync::mpsc::{self, Receiver, RecvTimeoutError, Sender};
use std::sync::Arc;
use std::time::Duration;

use parking_lot::Mutex;
use serde_json::Value;
use trust_control::failure::{Cause, Failure, NextAction};
use trust_control::messages::{OperationState, OperationUpdate};

use super::{answer_of, output, Exit};
use crate::commands::ControlStudio;
use crate::interrupt::Presses;
use crate::operation::{Operations, UpdateSink};

/// The longest wait for an update before the Ctrl-C presses are counted again.
const POLL_INTERVAL: Duration = Duration::from_millis(100);

/// The engine's updates, sent to the verb's thread.
struct UpdateChannel(Mutex<Sender<OperationUpdate>>);

impl UpdateSink for UpdateChannel {
    fn send(&self, update: OperationUpdate) {
        // A verb that has stopped waiting receives nothing more; the journal keeps the operation (A8.5).
        let _ = self.0.lock().send(update);
    }
}

/// A Control Studio engine for one verb, and the receiver that its operation updates arrive on.
pub(super) fn engine() -> (ControlStudio, Receiver<OperationUpdate>) {
    let (sender, updates) = mpsc::channel();
    let studio = ControlStudio::new(Arc::new(UpdateChannel(Mutex::new(sender))));
    (studio, updates)
}

/// How the wait for an operation ended.
#[derive(Debug, PartialEq)]
pub(super) enum Followed {
    /// The operation finished with this result.
    Finished(Value),
    /// The operation failed.
    Failed(Failure),
    /// The operation was cancelled.
    Cancelled,
    /// The second Ctrl-C ended the wait; the operation is left to the runtime.
    Left,
}

/// Waits on `updates` for the final update of operation `operation_id` of `operations`, and writes
/// each of its progress updates as one line on `stderr`. The first Ctrl-C counted in `presses` asks
/// the operation to cancel, and the wait goes on; the second ends the wait at once.
///
/// # Errors
/// The updates stop before the operation's final update.
pub(super) fn to_end(
    operations: &Operations,
    operation_id: &str,
    updates: &Receiver<OperationUpdate>,
    presses: &mut Presses,
    stderr: &mut dyn Write,
) -> Result<Followed, Failure> {
    let mut cancelling = false;
    loop {
        match updates.recv_timeout(POLL_INTERVAL) {
            Ok(update) if update.operation_id == operation_id => {
                if let Some(followed) = final_state(update, stderr) {
                    return Ok(followed);
                }
            }
            // Another operation's update: the verb's engine runs no other operation.
            Ok(_) | Err(RecvTimeoutError::Timeout) => {}
            Err(RecvTimeoutError::Disconnected) => return Err(updates_lost(operation_id)),
        }
        let count = presses.count();
        if count > 1 {
            let _ = writeln!(stderr, "{}", output::LEFT_TEXT);
            return Ok(Followed::Left);
        }
        if count == 1 && !cancelling {
            cancelling = true;
            let line = match operations.cancel(operation_id) {
                Ok(latest) => output::cancel_text(&latest),
                Err(failure) => output::failure_text(&failure),
            };
            let _ = writeln!(stderr, "{line}");
        }
    }
}

/// How `update` ends the wait, or `None` while the operation runs. A progress update and a
/// cancellation are each one line on `stderr`; a result or a failure is the verb's answer.
fn final_state(update: OperationUpdate, stderr: &mut dyn Write) -> Option<Followed> {
    match update.state {
        OperationState::Running => {
            let _ = writeln!(stderr, "{}", output::progress_line(&update));
            None
        }
        OperationState::Finished => Some(Followed::Finished(update.result.unwrap_or_default())),
        OperationState::Failed => Some(Followed::Failed(
            update.failure.unwrap_or_else(unexplained_failure),
        )),
        OperationState::Cancelled => {
            let _ = writeln!(stderr, "{}", output::progress_line(&update));
            Some(Followed::Cancelled)
        }
    }
}

/// The status that a followed operation ends the verb with (§0.1b, D6). Its result or failure is
/// written as [`answer_of`] writes an answer: the result's readable text from `text`, or with
/// `--json` `{"result": …}`. A cancelled operation, and one left by the second Ctrl-C, end the verb
/// with status 130.
pub(super) fn ended(
    followed: Result<Followed, Failure>,
    as_json: bool,
    text: impl FnOnce(&Value) -> String,
    stdout: &mut dyn Write,
    stderr: &mut dyn Write,
) -> Exit {
    match followed {
        Ok(Followed::Finished(result)) => answer_of(Ok(result), as_json, text, stdout, stderr),
        Ok(Followed::Failed(failure)) | Err(failure) => {
            answer_of::<Value>(Err(failure), as_json, text, stdout, stderr)
        }
        Ok(Followed::Cancelled | Followed::Left) => Exit::Interrupted,
    }
}

/// The failure of an operation that failed without saying why.
fn unexplained_failure() -> Failure {
    Failure::new(
        "The operation failed.",
        Cause::Unclassified,
        NextAction::None,
    )
}

/// The failure of a wait whose updates stopped before the operation's final update.
fn updates_lost(operation_id: &str) -> Failure {
    Failure::new(
        &format!("The updates of operation {operation_id} stopped before it ended."),
        Cause::Implementation,
        NextAction::None,
    )
}

#[cfg(test)]
mod tests {
    use std::sync::atomic::{AtomicBool, Ordering};
    use std::thread;

    use serde_json::json;

    use super::*;
    use crate::operation::cancelled;

    const OPERATION: &str = "operation-1";

    fn update(state: OperationState) -> OperationUpdate {
        OperationUpdate {
            operation_id: OPERATION.to_string(),
            command: "identify".to_string(),
            state,
            stage: None,
            progress: None,
            result: None,
            failure: None,
        }
    }

    /// Operations whose updates arrive on the returned receiver.
    fn operations() -> (Operations, Receiver<OperationUpdate>) {
        let (sender, updates) = mpsc::channel();
        (
            Operations::new(Arc::new(UpdateChannel(Mutex::new(sender)))),
            updates,
        )
    }

    /// Operations that run nothing, for a wait on updates that the test sends itself.
    fn idle_operations() -> Operations {
        operations().0
    }

    /// Ctrl-C presses that the test makes by setting the returned flag.
    fn presses() -> (Presses, Arc<AtomicBool>) {
        let flag = Arc::new(AtomicBool::new(false));
        (Presses::from_flag(Arc::clone(&flag)), flag)
    }

    fn text(bytes: Vec<u8>) -> String {
        String::from_utf8(bytes).expect("UTF-8")
    }

    /// The status, standard output and standard error of the verb that `followed` ends.
    fn written(followed: Result<Followed, Failure>, as_json: bool) -> (Exit, String, String) {
        let (mut stdout, mut stderr) = (Vec::<u8>::new(), Vec::<u8>::new());
        let exit = ended(
            followed,
            as_json,
            output::identify_text,
            &mut stdout,
            &mut stderr,
        );
        (exit, text(stdout), text(stderr))
    }

    #[test]
    fn each_progress_update_is_one_line_and_the_result_is_its_text_with_status_0() {
        let (sender, updates) = mpsc::channel();
        let mut running = update(OperationState::Running);
        running.progress = Some("Waiting for a steady baseline".to_string());
        let result =
            json!({ "text": "First-order model · Provisional\ny(s) = 2 / (10s + 1) u(s)" });
        let mut finished = update(OperationState::Finished);
        finished.result = Some(result.clone());
        sender.send(running).expect("queued");
        sender.send(finished).expect("queued");
        let (mut presses, _flag) = presses();
        let mut stderr = Vec::<u8>::new();
        let followed = to_end(
            &idle_operations(),
            OPERATION,
            &updates,
            &mut presses,
            &mut stderr,
        );
        assert_eq!(followed, Ok(Followed::Finished(result.clone())));
        assert_eq!(
            text(stderr),
            "identify (operation-1): Waiting for a steady baseline\n"
        );
        assert_eq!(
            written(Ok(Followed::Finished(result.clone())), false),
            (
                Exit::Finished,
                "First-order model · Provisional\ny(s) = 2 / (10s + 1) u(s)\n".to_string(),
                String::new()
            )
        );
        assert_eq!(
            written(Ok(Followed::Finished(result.clone())), true),
            (
                Exit::Finished,
                format!("{}\n", json!({ "result": result })),
                String::new()
            )
        );
    }

    #[test]
    fn a_failed_operation_is_status_1_with_its_failure_on_standard_error() {
        let failure = Failure::new(
            "The response is too small against the noise. It changed 0.02 °C.",
            Cause::ModelOrData,
            NextAction::OpenLimits,
        );
        let (sender, updates) = mpsc::channel();
        let mut failed = update(OperationState::Failed);
        failed.failure = Some(failure.clone());
        sender.send(failed).expect("queued");
        let (mut presses, _flag) = presses();
        let mut stderr = Vec::<u8>::new();
        let followed = to_end(
            &idle_operations(),
            OPERATION,
            &updates,
            &mut presses,
            &mut stderr,
        );
        assert_eq!(followed, Ok(Followed::Failed(failure.clone())));
        assert_eq!(text(stderr), "", "the failure is the verb's answer");
        let failure_line = format!("{}\n", output::failure_text(&failure));
        assert_eq!(
            written(Ok(Followed::Failed(failure.clone())), false),
            (Exit::Failed, String::new(), failure_line.clone())
        );
        assert_eq!(
            written(Ok(Followed::Failed(failure.clone())), true),
            (
                Exit::Failed,
                format!("{}\n", json!({ "failure": failure })),
                failure_line
            )
        );
    }

    #[test]
    fn an_operation_that_ended_before_the_press_is_not_cancelled_and_ends_with_status_0() {
        let (sender, updates) = mpsc::channel();
        let mut finished = update(OperationState::Finished);
        finished.result = Some(json!({ "text": "Static gain model" }));
        sender.send(finished).expect("queued");
        let (mut presses, flag) = presses();
        flag.store(true, Ordering::SeqCst);
        let mut stderr = Vec::<u8>::new();
        let followed = to_end(
            &idle_operations(),
            OPERATION,
            &updates,
            &mut presses,
            &mut stderr,
        );
        assert_eq!(
            followed,
            Ok(Followed::Finished(json!({ "text": "Static gain model" })))
        );
        assert_eq!(text(stderr), "", "no cancellation was asked");
        assert_eq!(
            written(followed, false),
            (
                Exit::Finished,
                "Static gain model\n".to_string(),
                String::new()
            )
        );
    }

    #[test]
    fn the_first_press_cancels_the_operation_and_the_verb_waits_for_its_end_with_status_130() {
        let (operations, updates) = operations();
        let started = operations.start("identify", |context| {
            while !context.is_cancelled() {
                thread::sleep(Duration::from_millis(5));
            }
            Err(cancelled())
        });
        let (mut presses, flag) = presses();
        flag.store(true, Ordering::SeqCst);
        let mut stderr = Vec::<u8>::new();
        let followed = to_end(
            &operations,
            &started.operation_id,
            &updates,
            &mut presses,
            &mut stderr,
        );
        assert_eq!(followed, Ok(Followed::Cancelled));
        let id = started.operation_id;
        assert_eq!(
            text(stderr),
            format!("Cancellation requested for {id}.\nidentify ({id}): cancelled\n")
        );
        assert_eq!(
            written(followed, false),
            (Exit::Interrupted, String::new(), String::new())
        );
    }

    #[test]
    fn the_second_press_leaves_the_operation_to_the_runtime_at_once_with_status_130() {
        let (operations, updates) = operations();
        let (release, hold) = mpsc::channel::<()>();
        let started = operations.start("identify", move |_| {
            // The operation ignores its cancellation until the test lets it end.
            let _ = hold.recv();
            Err(cancelled())
        });
        let (mut presses, flag) = presses();
        let done = Arc::new(AtomicBool::new(false));
        let keeps_pressing = {
            let (flag, done) = (Arc::clone(&flag), Arc::clone(&done));
            thread::spawn(move || {
                while !done.load(Ordering::SeqCst) {
                    flag.store(true, Ordering::SeqCst);
                    thread::sleep(Duration::from_millis(10));
                }
            })
        };
        let mut stderr = Vec::<u8>::new();
        let followed = to_end(
            &operations,
            &started.operation_id,
            &updates,
            &mut presses,
            &mut stderr,
        );
        done.store(true, Ordering::SeqCst);
        keeps_pressing.join().expect("the presses end");
        drop(release);
        assert_eq!(followed, Ok(Followed::Left));
        assert_eq!(
            text(stderr),
            format!(
                "Cancellation requested for {}.\n{}\n",
                started.operation_id,
                output::LEFT_TEXT
            )
        );
        assert_eq!(
            written(followed, true),
            (Exit::Interrupted, String::new(), String::new())
        );
    }

    #[test]
    fn updates_that_stop_before_the_final_one_fail_the_verb() {
        let (sender, updates) = mpsc::channel::<OperationUpdate>();
        drop(sender);
        let (mut presses, _flag) = presses();
        let mut stderr = Vec::<u8>::new();
        let failure = to_end(
            &idle_operations(),
            OPERATION,
            &updates,
            &mut presses,
            &mut stderr,
        )
        .expect_err("the updates stopped");
        assert_eq!(
            failure.message,
            "The updates of operation operation-1 stopped before it ended."
        );
        assert_eq!(written(Err(failure), false).0, Exit::Failed);
    }
}
