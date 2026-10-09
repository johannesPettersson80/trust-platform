//! Long operations (specification 35, §0.1a). A command that takes longer than a moment answers at
//! once with an operation identifier. The operation runs on its own thread, because it holds a
//! blocking conversation with the runtime; it reports its progress, its result or its failure
//! through the update sink, and stops at its next step once cancelled. `status` answers a panel
//! that reattaches.

use std::collections::HashMap;
use std::sync::atomic::{AtomicBool, AtomicU64, Ordering};
use std::sync::Arc;
use std::thread;

use parking_lot::Mutex;
use serde_json::Value;
use trust_control::failure::{Cause, Failure, NextAction};
use trust_control::messages::{OperationStarted, OperationState, OperationUpdate};

/// Where operation updates go: the `trust/controlStudio/update` notification in the language
/// server, a list in tests.
pub trait UpdateSink: Send + Sync {
    /// Delivers one update.
    fn send(&self, update: OperationUpdate);
}

struct Entry {
    latest: OperationUpdate,
    cancelled: Arc<AtomicBool>,
}

/// The operations of one language server.
pub struct Operations {
    entries: Arc<Mutex<HashMap<String, Entry>>>,
    sink: Arc<dyn UpdateSink>,
    next: AtomicU64,
}

/// What a running operation holds: its identity, its cancellation and its way to report.
#[derive(Clone)]
pub struct OperationContext {
    id: String,
    command: String,
    cancelled: Arc<AtomicBool>,
    entries: Arc<Mutex<HashMap<String, Entry>>>,
    sink: Arc<dyn UpdateSink>,
}

impl OperationContext {
    /// The operation's identifier.
    #[must_use]
    pub fn id(&self) -> &str {
        &self.id
    }

    /// Whether the operation was cancelled; the work checks this between its steps.
    #[must_use]
    pub fn is_cancelled(&self) -> bool {
        self.cancelled.load(Ordering::SeqCst)
    }

    /// Reports what the operation is doing.
    pub fn progress(&self, text: &str) {
        self.report(OperationState::Running, Some(text.to_string()), None, None);
    }

    fn report(
        &self,
        state: OperationState,
        progress: Option<String>,
        result: Option<Value>,
        failure: Option<Failure>,
    ) {
        let update = OperationUpdate {
            operation_id: self.id.clone(),
            command: self.command.clone(),
            state,
            progress,
            result,
            failure,
        };
        if let Some(entry) = self.entries.lock().get_mut(&self.id) {
            entry.latest = update.clone();
        }
        self.sink.send(update);
    }
}

impl Operations {
    /// The registry, reporting through `sink`.
    pub fn new(sink: Arc<dyn UpdateSink>) -> Self {
        Self {
            entries: Arc::new(Mutex::new(HashMap::new())),
            sink,
            next: AtomicU64::new(1),
        }
    }

    /// Starts `work` for the command `command` and answers with its identifier at once. The work's
    /// result or failure is its final update; a failure after a cancellation reports it cancelled.
    pub fn start<F>(&self, command: &str, work: F) -> OperationStarted
    where
        F: FnOnce(&OperationContext) -> Result<Value, Failure> + Send + 'static,
    {
        let id = format!("operation-{}", self.next.fetch_add(1, Ordering::SeqCst));
        let context = OperationContext {
            id: id.clone(),
            command: command.to_string(),
            cancelled: Arc::new(AtomicBool::new(false)),
            entries: Arc::clone(&self.entries),
            sink: Arc::clone(&self.sink),
        };
        let latest = OperationUpdate {
            operation_id: id.clone(),
            command: command.to_string(),
            state: OperationState::Running,
            progress: None,
            result: None,
            failure: None,
        };
        self.entries.lock().insert(
            id.clone(),
            Entry {
                latest,
                cancelled: Arc::clone(&context.cancelled),
            },
        );
        thread::spawn(move || match work(&context) {
            Ok(result) => context.report(OperationState::Finished, None, Some(result), None),
            Err(_) if context.is_cancelled() => {
                context.report(OperationState::Cancelled, None, None, None)
            }
            Err(failure) => context.report(OperationState::Failed, None, None, Some(failure)),
        });
        OperationStarted { operation_id: id }
    }

    /// The operation's latest update.
    ///
    /// # Errors
    /// No operation has that identifier.
    pub fn status(&self, id: &str) -> Result<OperationUpdate, Failure> {
        self.entries
            .lock()
            .get(id)
            .map(|entry| entry.latest.clone())
            .ok_or_else(|| unknown(id))
    }

    /// Asks a running operation to stop at its next step. An operation that has ended is left as
    /// it ended.
    ///
    /// # Errors
    /// No operation has that identifier.
    pub fn cancel(&self, id: &str) -> Result<OperationUpdate, Failure> {
        let entries = self.entries.lock();
        let entry = entries.get(id).ok_or_else(|| unknown(id))?;
        if entry.latest.state == OperationState::Running {
            entry.cancelled.store(true, Ordering::SeqCst);
        }
        Ok(entry.latest.clone())
    }
}

/// The failure a cancelled step ends its work with.
#[must_use]
pub fn cancelled() -> Failure {
    Failure::new(
        "The operation was cancelled.",
        Cause::Unclassified,
        NextAction::None,
    )
}

fn unknown(id: &str) -> Failure {
    Failure::new(
        &format!("No Control Studio operation is called {id}."),
        Cause::Implementation,
        NextAction::None,
    )
}

#[cfg(test)]
mod tests {
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
}
