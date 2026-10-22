use std::sync::mpsc;
use std::time::Duration;

use serde_json::json;

use super::*;

struct Channel(Mutex<mpsc::Sender<OperationUpdate>>);

impl UpdateSink for Channel {
    fn send(&self, update: OperationUpdate) {
        let _ = self.0.lock().send(update);
    }
}

fn operations() -> (Operations, mpsc::Receiver<OperationUpdate>) {
    let (sender, receiver) = mpsc::channel();
    (
        Operations::new(Arc::new(Channel(Mutex::new(sender)))),
        receiver,
    )
}

fn next(receiver: &mpsc::Receiver<OperationUpdate>) -> OperationUpdate {
    receiver
        .recv_timeout(Duration::from_secs(10))
        .expect("an update")
}

#[test]
fn an_operation_answers_at_once_then_reports_progress_and_its_result() {
    let (operations, updates) = operations();
    let (release, wait) = mpsc::channel::<()>();
    let started = operations.start("identify", move |context| {
        context.progress("Recording the plant");
        wait.recv().map_err(|_| cancelled())?;
        Ok(json!({ "model": "first-order" }))
    });
    let progress = next(&updates);
    assert_eq!(
        (progress.state, progress.progress.as_deref()),
        (OperationState::Running, Some("Recording the plant"))
    );
    assert_eq!(
        operations
            .status(&started.operation_id)
            .expect("known")
            .progress
            .as_deref(),
        Some("Recording the plant")
    );
    release.send(()).expect("release");
    let finished = next(&updates);
    assert_eq!(
        (finished.state, finished.result),
        (
            OperationState::Finished,
            Some(json!({ "model": "first-order" }))
        )
    );
    assert_eq!(
        operations
            .status(&started.operation_id)
            .expect("known")
            .state,
        OperationState::Finished,
        "status answers a reattaching panel"
    );
}

#[test]
fn a_cancelled_operation_stops_at_its_next_step_and_reports_cancelled() {
    let (operations, updates) = operations();
    let started = operations.start("identify", |context| {
        while !context.is_cancelled() {
            thread::sleep(Duration::from_millis(5));
        }
        Err(cancelled())
    });
    assert_eq!(
        operations
            .cancel(&started.operation_id)
            .expect("known")
            .state,
        OperationState::Running
    );
    assert_eq!(next(&updates).state, OperationState::Cancelled);
    assert!(
        operations.cancel("operation-0").is_err(),
        "an unknown operation is refused"
    );
}

#[test]
fn a_failure_after_a_cancel_is_reported_as_a_failure_not_as_cancelled() {
    let (operations, updates) = operations();
    let started = operations.start("identify", |context| {
        while !context.is_cancelled() {
            thread::sleep(Duration::from_millis(5));
        }
        Err(Failure::new(
            "The experiment's restoration is not confirmed.",
            Cause::Runtime,
            NextAction::None,
        ))
    });
    operations
        .cancel(&started.operation_id)
        .expect("the operation is known");
    let last = next(&updates);
    assert_eq!(last.state, OperationState::Failed);
    assert_eq!(
        last.failure.map(|failure| failure.message),
        Some("The experiment's restoration is not confirmed.".to_string()),
        "the restoration failure keeps its words"
    );
}

#[test]
fn a_failure_is_reported_with_its_next_action() {
    let (operations, updates) = operations();
    operations.start("identify", |_| {
        Err(Failure::unreachable("connection refused"))
    });
    let failed = next(&updates);
    assert_eq!(failed.state, OperationState::Failed);
    assert_eq!(
        failed.failure.map(|failure| failure.next_action),
        Some(NextAction::ConnectRuntime)
    );
}

#[test]
fn a_run_names_its_stage_on_every_update_until_the_next_stage() {
    let (operations, updates) = operations();
    let started = operations.start("run", |context| {
        context.enter_stage(Page::Identify);
        context.progress("Recording the plant");
        context.enter_stage(Page::Tune);
        Ok(json!({ "stages": [] }))
    });
    let entered = next(&updates);
    assert_eq!(
        (entered.state, entered.stage),
        (OperationState::Running, Some(Page::Identify))
    );
    let progress = next(&updates);
    assert_eq!(
        (progress.progress.as_deref(), progress.stage),
        (Some("Recording the plant"), Some(Page::Identify))
    );
    let tune = next(&updates);
    assert_eq!(tune.stage, Some(Page::Tune));
    assert_eq!(
        serde_json::to_value(&tune).expect("the update")["stage"],
        json!("Tune"),
        "the page reads the stage's name"
    );
    let finished = next(&updates);
    assert_eq!(
        (finished.state, finished.stage),
        (OperationState::Finished, Some(Page::Tune))
    );
    assert_eq!(
        operations
            .status(&started.operation_id)
            .expect("known")
            .stage,
        Some(Page::Tune),
        "status names the stage the run is in"
    );
}

#[test]
fn an_operation_that_runs_no_stages_names_none_and_sends_no_stage_field() {
    let (operations, updates) = operations();
    operations.start("identify", |context| {
        context.progress("Recording the plant");
        Ok(json!({}))
    });
    let progress = next(&updates);
    assert_eq!(progress.stage, None);
    let value = serde_json::to_value(&progress).expect("the update");
    assert!(value.get("stage").is_none(), "{value}");
}

/// Runs until it is cancelled, then answers the cancellation.
fn until_cancelled(context: &OperationContext) -> Result<Value, Failure> {
    while !context.is_cancelled() {
        thread::sleep(Duration::from_millis(5));
    }
    Err(cancelled())
}

#[test]
fn a_cancelled_identify_update_carries_the_identification_sentence() {
    let (operations, updates) = operations();
    let started = operations.start("identify", until_cancelled);
    operations
        .cancel(&started.operation_id)
        .expect("the operation is known");
    let ended = next(&updates);
    assert_eq!(
        (
            ended.state,
            ended.progress.as_deref(),
            ended.message.as_deref()
        ),
        (
            OperationState::Cancelled,
            None,
            Some("Identification cancelled.")
        ),
        "the cancelled update keeps its progress empty and names the sentence"
    );
    assert_eq!(
        operations
            .status(&started.operation_id)
            .expect("known")
            .message
            .as_deref(),
        Some("Identification cancelled."),
        "status answers a reattaching panel with the same sentence"
    );
}

/// B3, B9.4, B9.9: the cancelled update of every operation carries the engine's sentence, so that the page shows `message` and writes none of
/// its own ("Operation stopped.", "Analysis cancelled."). The sentences are worked out from the specification: the identification and the
/// analysis have their own words, and every other command the cancellation's.
#[test]
fn every_cancelled_operation_states_its_sentence() {
    for (command, sentence) in [
        ("identify", "Identification cancelled."),
        ("analyze", "Analysis cancelled."),
        ("tune", "The operation was cancelled."),
        ("validate", "The operation was cancelled."),
        ("run", "The operation was cancelled."),
        ("apply", "The operation was cancelled."),
        ("restore", "The operation was cancelled."),
        ("keep", "The operation was cancelled."),
        ("parameters", "The operation was cancelled."),
    ] {
        let (operations, updates) = operations();
        let started = operations.start(command, until_cancelled);
        operations
            .cancel(&started.operation_id)
            .expect("the operation is known");
        let ended = next(&updates);
        assert_eq!(ended.state, OperationState::Cancelled, "{command}");
        assert_eq!(ended.message.as_deref(), Some(sentence), "{command}");
        assert_eq!(
            operations
                .status(&started.operation_id)
                .expect("known")
                .message
                .as_deref(),
            Some(sentence),
            "{command}: status answers a reattaching panel with the same sentence"
        );
    }
}

/// The update of an operation that is running, finished or failed carries no sentence of cancellation: the page would show it as the
/// outcome.
#[test]
fn an_update_that_is_not_cancelled_states_no_cancellation() {
    let (operations, updates) = operations();
    operations.start("tune", |context| {
        context.progress("Designing the controller");
        Ok(json!({}))
    });
    let progress = next(&updates);
    let finished = next(&updates);
    assert_eq!(progress.state, OperationState::Running);
    assert_eq!(finished.state, OperationState::Finished);
    assert_eq!((progress.message, finished.message), (None, None));
}
