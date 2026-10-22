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
use trust_control::identification::campaign::CANCELLED_TEXT;
use trust_control::identification::connected_run::identification_cancelled;
use trust_control::messages::{AnswerKind, OperationStarted, OperationState, OperationUpdate};
use trust_control::stages::StageAvailability;
use trust_control::study::Page;

/// The command of an identification (`identify_action`): its cancelled update carries the identification's sentence.
const IDENTIFY_COMMAND: &str = "identify";

/// The command of an analysis (`analyze_action`): its cancelled update carries the analysis's sentence.
const ANALYZE_COMMAND: &str = "analyze";

/// The sentence of a cancelled analysis (specification 35, B9.9).
const ANALYSIS_CANCELLED_TEXT: &str = "Analysis cancelled.";

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

/// The operations of one engine process.
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
    /// The stage a run is in (B8), shared by every clone of the context; none for an operation that runs no stages.
    stage: Arc<Mutex<Option<Page>>>,
    /// The status of every stage that the operation's failure leaves, which its failed update carries (B9.4); none until the work
    /// names it.
    failure_stages: Arc<Mutex<Option<Vec<StageAvailability>>>>,
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

    /// Names the status of every stage that the operation leaves when it fails, which its failed update carries (B6.4, B6.7, B9.4): an
    /// Apply or a Restore that stopped after its first write leaves a checkpoint that holds the stages, and the page shows that block
    /// without a reopen.
    pub fn stages_on_failure(&self, stages: Vec<StageAvailability>) {
        *self.failure_stages.lock() = Some(stages);
    }

    /// Reports that a run has entered `stage` (B8). Every later update of the operation names that stage, until the next one.
    pub fn enter_stage(&self, stage: Page) {
        *self.stage.lock() = Some(stage);
        self.report(OperationState::Running, None, None, None);
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
            stage: *self.stage.lock(),
            progress,
            result,
            failure,
            message: cancelled_message(&self.command, state),
            stages: if state == OperationState::Failed {
                self.failure_stages.lock().take()
            } else {
                None
            },
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
    /// result or failure is its final update. A cancelled step reports `Cancelled` only when its failure is the cancellation
    /// itself; any other failure, including one after a Cancel, is reported as a failure.
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
            stage: Arc::new(Mutex::new(None)),
            failure_stages: Arc::new(Mutex::new(None)),
        };
        let latest = OperationUpdate {
            operation_id: id.clone(),
            command: command.to_string(),
            state: OperationState::Running,
            stage: None,
            progress: None,
            result: None,
            failure: None,
            message: None,
            stages: None,
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
            Err(failure) if context.is_cancelled() && is_cancellation(&failure) => {
                context.report(OperationState::Cancelled, None, None, None);
            }
            // A failure that is not the cancellation itself, such as a restoration that is not confirmed after a Cancel,
            // is reported with its words: the engineer must see it (B3.9).
            Err(failure) => context.report(OperationState::Failed, None, None, Some(failure)),
        });
        OperationStarted {
            kind: AnswerKind::new(),
            operation_id: id,
        }
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

/// The sentence a cancelled update carries (specification 35, B3, B9.4, B9.9): the identification's own, for the identify command
/// (`CANCELLED_TEXT`), the analysis's, for the analyze command, and the cancellation's own sentence (`cancelled`) for every other
/// command, so that the page shows the engine's words and writes none. An update that is not cancelled carries none.
fn cancelled_message(command: &str, state: OperationState) -> Option<String> {
    if state != OperationState::Cancelled {
        return None;
    }
    Some(match command {
        IDENTIFY_COMMAND => CANCELLED_TEXT.to_string(),
        ANALYZE_COMMAND => ANALYSIS_CANCELLED_TEXT.to_string(),
        _ => cancelled().message,
    })
}

/// Whether `failure` is the answer of a cancelled step: this module's `cancelled`, or the identification's own cancellation
/// (`identification_cancelled`). Any other failure is a failure, even after a Cancel.
fn is_cancellation(failure: &Failure) -> bool {
    *failure == cancelled() || *failure == identification_cancelled()
}

fn unknown(id: &str) -> Failure {
    Failure::new(
        &format!("No Control Studio operation is called {id}."),
        Cause::Implementation,
        NextAction::None,
    )
}

#[cfg(test)]
#[path = "operation_tests.rs"]
mod tests;
