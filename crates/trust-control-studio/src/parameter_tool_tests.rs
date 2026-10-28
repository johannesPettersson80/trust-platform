//! The parameter tool's answers against a runtime (specification 35, C12; annex E7.3 to E7.5, E11.8): a write's final answer says how
//! it ended in the engine's words, a confirmation applies or restores by the review it names, and every answer names the tool's offer,
//! which follows the loop's claim and journal and the review it gives. The page renders all of it and keeps no phase of its own.

use std::fs;
use std::path::PathBuf;
use std::sync::{mpsc, Arc};
use std::time::Duration;

use parking_lot::Mutex;
use serde_json::{json, Value};
use trust_control::experiment::{
    ControlEvidenceSource, RecordingRecoveryBinding, RecordingRecoveryOwner,
};
use trust_control::messages::{OperationState, OperationUpdate, ParameterWorkRequest};

use super::*;
use crate::commands::{ControlStudio, OPEN};
use crate::journal::JournalEntry;
use crate::operation::UpdateSink;
use crate::runtime_client::fake_runtime::{answer as reply, refusal, FakeRuntime};

/// Lesson 21 (a safe, informative experiment): it declares `parameterPurpose: "experiment"`, so its tool writes its settings, the step
/// `P1.SelectedStepPoints` (5 to 15) and the hold `P1.HoldSeconds` (20 to 40 s), both REAL.
const LESSON_21: &str = include_str!(
    "../../../examples/control/05-experiments-identification-and-validation/21-safe-informative-experiment/control-loop.json"
);
/// The project and the loop of lesson 21.
const PROJECT_ID: &str = "project:control-lesson-21";
const LOOP_ID: &str = "control-lesson-21";
/// The engine's sentences of a write's outcome (C12, E7.4) and of a review that no longer matches (E11.8).
const APPLIED: &str = "Reviewed values observed after ST execution.";
const RESTORED: &str = "Previous values restored and observed after ST execution.";
const NOT_CONFIRMED: &str = "The reviewed values were not confirmed after controller execution.";
const CHANGED: &str =
    "The parameter record or the loop changed since the review. Review the values again before the write.";
/// The engine's progress of a write while it waits for the task boundary (C12, E7.4).
const WRITING: &str = "Waiting for the owning task and observed values…";
/// The engine's sentence that a project test left the experiment settings released (C12).
const RELEASED: &str =
    "The previous project test restored its declared values. Edit and review them before the next trial.";

/// The parameter record of a simulated runtime (E7.2): the revision of the values in force, the values, the values retained from
/// before the last write with their revision, the last transaction and its state.
struct Record {
    active: u32,
    values: Value,
    previous: Option<(u32, Value)>,
    transaction: Option<String>,
    status: &'static str,
    /// Whether a write takes effect at the task boundary. When it does not, the runtime rejects it with `reason` and keeps the values
    /// in force (E7.4).
    takes_writes: bool,
    reason: Option<String>,
}

impl Record {
    /// The record as `control.parameters.status` answers it (E7.2).
    fn status_answer(&self) -> Value {
        json!({
            "supports_request_id": true, "transaction_id": self.transaction, "program_revision": "program:1",
            "binding_revision": "binding:1", "active_revision": format!("active:{}", self.active), "values": self.values,
            "status": self.status,
            "previous_revision": self.previous.as_ref().map(|(revision, _)| format!("active:{revision}")),
            "previous_values": self.previous.as_ref().map(|(_, values)| values.clone()),
            "reason": if self.status == "rejected" { self.reason.clone() } else { None }
        })
    }

    /// An apply under `transaction` of `values` (E7.3), which the task boundary takes or rejects (E7.4).
    fn apply(&mut self, transaction: &str, values: &Value) {
        self.transaction = Some(transaction.to_owned());
        if self.takes_writes {
            self.previous = Some((self.active, self.values.clone()));
            self.active += 1;
            self.values = values.clone();
            self.status = "applied";
        } else {
            self.status = "rejected";
        }
    }

    /// A rollback under `transaction` to the retained previous values (E7.5).
    fn rollback(&mut self, transaction: &str) {
        self.transaction = Some(transaction.to_owned());
        if let Some((revision, values)) = self.previous.take() {
            self.previous = Some((self.active, self.values.clone()));
            self.active = revision;
            self.values = values;
            self.status = "applied";
        }
    }
}

/// A simulated runtime (E1.3) whose record holds the step 10 and the hold 30 at `active:1` and takes each write at once, or rejects it
/// with `rejection` when `takes_writes` is false.
fn runtime(takes_writes: bool, rejection: Option<&str>) -> FakeRuntime {
    let record = Mutex::new(Record {
        active: 1,
        values: json!({ "P1.SelectedStepPoints": 10.0, "P1.HoldSeconds": 30.0 }),
        previous: None,
        transaction: None,
        status: "idle",
        takes_writes,
        reason: rejection.map(str::to_owned),
    });
    FakeRuntime::start(move |request| match request["type"].as_str()? {
        "status" => Some(reply(
            request,
            json!({
                "session_epoch": 7, "simulation_enabled": true, "simulation_only_io": true,
                "simulation_time_scale": 1, "simulation_achieved_time_scale": null,
                "program_revision": "program:1", "tasks": [{ "name": "MainTask", "interval_ms": 100 }],
                "cycle_interval_ms": 100
            }),
        )),
        "control.parameters.status" => Some(reply(request, record.lock().status_answer())),
        "control.parameters.apply" => {
            let params = &request["params"];
            record
                .lock()
                .apply(params["transaction_id"].as_str()?, &params["values"]);
            Some(reply(
                request,
                json!({ "status": "pending", "confirmed": false }),
            ))
        }
        "control.parameters.rollback" => {
            record
                .lock()
                .rollback(request["params"]["transaction_id"].as_str()?);
            Some(reply(
                request,
                json!({ "status": "pending", "confirmed": false }),
            ))
        }
        _ => Some(refusal(
            request,
            "invalid_parameter_request",
            "Not in this test.",
        )),
    })
}

/// The updates of an engine's operations, in the order the test receives them.
struct Updates(Mutex<mpsc::Sender<OperationUpdate>>);

impl UpdateSink for Updates {
    fn send(&self, update: OperationUpdate) {
        let _ = self.0.lock().send(update);
    }
}

/// A project folder holding lesson 21's loop, and the engine that answers its parameter requests; removed when the test ends.
struct Engine {
    root: PathBuf,
    operations: Operations,
    updates: mpsc::Receiver<OperationUpdate>,
    claims: LoopClaims,
}

impl Engine {
    fn new(name: &str) -> Self {
        let root = std::env::temp_dir().join(format!(
            "trust-parameter-tool-{name}-{}",
            std::process::id()
        ));
        let _ = fs::remove_dir_all(&root);
        fs::create_dir_all(&root).expect("the project folder");
        fs::write(root.join("control-loop.json"), LESSON_21).expect("the loop file");
        let (sender, updates) = mpsc::channel();
        Self {
            root,
            operations: Operations::new(Arc::new(Updates(Mutex::new(sender)))),
            updates,
            claims: LoopClaims::default(),
        }
    }

    /// The folder of the loop's journals.
    fn storage(&self) -> PathBuf {
        self.root.join("storage")
    }

    /// The answer of `controlStudio/parameters` to `action` against `runtime`, with the fields of `extra` besides.
    fn ask(&self, runtime: &FakeRuntime, action: &str, extra: &Value) -> Result<Value, Failure> {
        let mut request = json!({
            "projectRoot": self.root.display().to_string(),
            "runtimeId": "simulator",
            "loopId": LOOP_ID,
            "storageFolder": self.storage().display().to_string(),
            "connection": runtime.connection(None),
            "action": action
        });
        request
            .as_object_mut()
            .expect("a request is an object")
            .extend(extra.as_object().expect("the fields are an object").clone());
        let request: ParameterWorkRequest =
            serde_json::from_value(request).expect("the request decodes");
        answer(&self.operations, &self.claims, request)
    }

    /// The final answer of the write that `confirm` starts for the review `review`, with `values` when they are given.
    fn confirm(&self, runtime: &FakeRuntime, review: &Value, values: Option<&Value>) -> Value {
        let mut extra = json!({ "review": review });
        if let Some(values) = values {
            extra["values"] = values.clone();
        }
        let started = self
            .ask(runtime, "confirm", &extra)
            .expect("the write starts");
        assert!(started["operationId"].is_string(), "{started}");
        loop {
            let update = self
                .updates
                .recv_timeout(Duration::from_secs(30))
                .expect("an update of the write");
            if update.state != OperationState::Running {
                assert_eq!(update.state, OperationState::Finished, "{update:?}");
                return update.result.expect("the write's final answer");
            }
        }
    }
}

impl Drop for Engine {
    fn drop(&mut self) {
        let _ = fs::remove_dir_all(&self.root);
    }
}

/// The review text of the step 12 and the hold 35, as the engineer types it.
const TYPED: &str = r#"{"P1.SelectedStepPoints": 12, "P1.HoldSeconds": 35}"#;

/// The offer of a free tool with no review (C12): review next; inspect, review and restore.
fn free_offer() -> Value {
    json!({ "nextStep": "review", "available": ["inspect", "review", "restore"] })
}

/// The offer after a review (C12): its confirmation next; every action.
fn reviewed_offer() -> Value {
    json!({ "nextStep": "apply", "available": ["inspect", "review", "apply", "restore"] })
}

/// The offer while a project test holds the settings (C12): another inspection, and nothing else.
fn frozen_offer() -> Value {
    json!({ "nextStep": "inspect", "available": ["inspect"] })
}

// C12, E7.4: a confirmed write finishes with its outcome, `applied`, and the engine's sentence of what the runtime's record shows: an
// Apply's reviewed values, or a Restore's previous values, observed after ST execution. A page that decided "applied" from the record
// and wrote the sentence itself is not needed for either.
#[test]
fn a_finished_parameter_write_answers_its_outcome_sentence() {
    let engine = Engine::new("outcome");
    let runtime = runtime(true, None);
    let reviewed = engine
        .ask(&runtime, "review", &json!({ "json": TYPED }))
        .expect("reviewed");
    let applied = engine.confirm(&runtime, &reviewed["identity"], Some(&reviewed["values"]));
    assert_eq!(applied["status"], json!("applied"), "{applied}");
    assert_eq!(applied["message"], json!(APPLIED), "{applied}");
    assert_eq!(
        applied["values"],
        json!({ "P1.HoldSeconds": 35.0, "P1.SelectedStepPoints": 12.0 }),
        "the values the runtime confirmed"
    );
    assert_eq!(applied["observed"]["status"], json!("applied"), "{applied}");

    let restore_review = engine
        .ask(&runtime, "reviewRestore", &json!({}))
        .expect("the values from before are retained");
    let restored = engine.confirm(&runtime, &restore_review["identity"], None);
    assert_eq!(restored["status"], json!("applied"), "{restored}");
    assert_eq!(restored["message"], json!(RESTORED), "{restored}");
    assert_eq!(
        restored["values"],
        json!({ "P1.HoldSeconds": 30.0, "P1.SelectedStepPoints": 10.0 }),
        "the values from before the write"
    );
}

// C12, E7.4: a write whose values the runtime does not report after the task executed fails, and its final answer says so in the
// engine's words: the runtime's reason when it gives one, else that the reviewed values were not confirmed. It shows no values as
// written, and the review is spent, so the tool offers a review again. A page that wrote its own sentence for this case is not needed.
#[test]
fn a_write_without_observed_values_fails_with_the_engine_sentence() {
    for (rejection, sentence) in [
        (None, NOT_CONFIRMED),
        (
            Some("controller changed parameter values before confirmation: P1.HoldSeconds"),
            "controller changed parameter values before confirmation: P1.HoldSeconds",
        ),
    ] {
        let engine = Engine::new("not-observed");
        let runtime = runtime(false, rejection);
        let reviewed = engine
            .ask(&runtime, "review", &json!({ "json": TYPED }))
            .expect("reviewed");
        let failed = engine.confirm(&runtime, &reviewed["identity"], Some(&reviewed["values"]));
        assert_eq!(failed["status"], json!("failed"), "{failed}");
        assert_eq!(failed["message"], json!(sentence), "{failed}");
        assert!(
            failed.get("values").is_none(),
            "nothing was written: {failed}"
        );
        assert!(failed.get("observed").is_none(), "{failed}");
        assert_eq!(failed["offer"], free_offer(), "{failed}");
        assert_eq!(
            runtime.requests_of("control.parameters.apply").len(),
            1,
            "the write was sent once and never again (E11.4)"
        );
    }
}

// C12, E11.8: the page confirms the review it shows by its identity, and the engine decides from that review whether the write applies
// the values an Apply's review read or restores the values from before. The identity binds the action, so a Restore's identity
// restores even when the values of an earlier Apply review come with it, and an identity no review gives now writes nothing.
#[test]
fn confirm_applies_or_restores_by_the_review_it_names() {
    let engine = Engine::new("confirm");
    let runtime = runtime(true, None);
    let sent = |kind: &str| runtime.requests_of(kind).len();
    let reviewed = engine
        .ask(&runtime, "review", &json!({ "json": TYPED }))
        .expect("reviewed");
    let applied = engine.confirm(&runtime, &reviewed["identity"], Some(&reviewed["values"]));
    assert_eq!(applied["message"], json!(APPLIED), "{applied}");
    assert_eq!(
        (
            sent("control.parameters.apply"),
            sent("control.parameters.rollback")
        ),
        (1, 0),
        "an Apply's review applies"
    );

    let restore_review = engine
        .ask(&runtime, "reviewRestore", &json!({}))
        .expect("the values from before are retained");
    let restored = engine.confirm(
        &runtime,
        &restore_review["identity"],
        Some(&reviewed["values"]),
    );
    assert_eq!(restored["message"], json!(RESTORED), "{restored}");
    assert_eq!(
        (
            sent("control.parameters.apply"),
            sent("control.parameters.rollback")
        ),
        (1, 1),
        "a Restore's review restores, whatever values come with it"
    );

    let refused = engine.confirm(&runtime, &json!("sha256:another-review"), None);
    assert_eq!(refused["status"], json!("failed"), "{refused}");
    assert_eq!(refused["message"], json!(CHANGED), "{refused}");
    assert_eq!(
        (
            sent("control.parameters.apply"),
            sent("control.parameters.rollback")
        ),
        (1, 1),
        "nothing is written for a review that no record gives now"
    );
}

// C12, A8.5: every answer names the tool's offer from the engine's state. A free tool offers a review after an inspection and an edit,
// the confirmation after a review, and a review again after a write; while a project test holds the settings, by the loop's claim or by
// an unfinished operation in its journal, the tool offers only another inspection.
#[test]
fn parameter_tool_actions_follow_its_state() {
    let engine = Engine::new("offer");
    let runtime = runtime(true, None);
    let inspected = engine
        .ask(&runtime, "inspect", &json!({}))
        .expect("inspected");
    assert_eq!(inspected["offer"], free_offer(), "{inspected}");
    let reviewed = engine
        .ask(&runtime, "review", &json!({ "json": TYPED }))
        .expect("reviewed");
    assert_eq!(reviewed["offer"], reviewed_offer(), "{reviewed}");
    let edited = engine
        .ask(&runtime, "invalidate", &json!({}))
        .expect("the edit is answered");
    assert_eq!(
        edited["offer"],
        free_offer(),
        "an edit spends the review: {edited}"
    );
    let written = engine.confirm(&runtime, &reviewed["identity"], Some(&reviewed["values"]));
    assert_eq!(
        written["offer"],
        free_offer(),
        "a write spends its review: {written}"
    );
    let restore_review = engine
        .ask(&runtime, "reviewRestore", &json!({}))
        .expect("the values from before are retained");
    assert_eq!(
        restore_review["offer"],
        reviewed_offer(),
        "{restore_review}"
    );

    // A project test claims the loop.
    let test = engine
        .claims
        .claim(PROJECT_ID, LOOP_ID, "Identify plant")
        .expect("a free loop");
    for action in ["inspect", "invalidate"] {
        let held = engine.ask(&runtime, action, &json!({})).expect("answered");
        assert_eq!(
            held["offer"],
            frozen_offer(),
            "{action} while the test runs: {held}"
        );
    }
    drop(test);

    // The test ended without its restoration confirmed: the loop's journal holds its recording.
    let loop_definition = loops::open(&engine.root, "simulator", LOOP_ID).expect("lesson 21 opens");
    let mut journal =
        Journal::open(&engine.storage(), PROJECT_ID, LOOP_ID).expect("the loop's journal");
    journal
        .record(JournalEntry::RecordingOwner {
            owner: RecordingRecoveryOwner {
                schema_version: 1,
                recording_id: "recording-9".to_owned(),
                project_id: PROJECT_ID.to_owned(),
                runtime_id: "simulator".to_owned(),
                loop_id: LOOP_ID.to_owned(),
                loop_revision: loop_definition.revision.clone(),
                source: ControlEvidenceSource::Simulation,
                program_revision: None,
                start_acknowledgement_unconfirmed: None,
                bindings: vec![RecordingRecoveryBinding {
                    id: "processVariable".to_owned(),
                    address: Some("%IL0".to_owned()),
                    symbol: None,
                    iec_type: "LREAL".to_owned(),
                }],
            },
        })
        .expect("the test's recording is journaled");
    let held = engine
        .ask(&runtime, "inspect", &json!({}))
        .expect("inspected");
    assert_eq!(held["offer"], frozen_offer(), "{held}");
    journal
        .clear("recording-9")
        .expect("the restoration is confirmed and the journal cleared");
    let free = engine
        .ask(&runtime, "inspect", &json!({}))
        .expect("inspected");
    assert_eq!(free["offer"], free_offer(), "{free}");
}

/// An engine whose updates nobody follows.
struct Ignore;

impl UpdateSink for Ignore {
    fn send(&self, _update: OperationUpdate) {}
}

/// R4 (C12: "before the engine's first answer the tool's step is Inspect, which reads only"; the offer, the progress of a write and the
/// release of the settings are the engine's): the parameter tool states each step it takes. The open answer's form names the tool's
/// first offer, Inspect alone; a write reports its progress in the engine's words before its end; and the question whether a project
/// test left the settings released answers the sentence with the offer that follows, a review, and while a test holds the loop the
/// reason with another inspection only. The wrong behaviour caught: a page that starts the tool from an offer of its own
/// (`FIRST_OFFER`), writes the progress of a write itself, or decides after a test's end what the tool offers (N11).
#[test]
fn the_parameter_tool_states_each_step_it_takes() {
    let engine = Engine::new("steps");
    let runtime = runtime(true, None);
    let studio = ControlStudio::new(Arc::new(Ignore));
    let opened = studio
        .execute(
            OPEN,
            &[json!({
                "projectRoot": engine.root.display().to_string(),
                "runtimeId": "simulator",
                "loopId": LOOP_ID,
                "storageFolder": engine.storage().display().to_string(),
            })],
        )
        .expect("answered");
    assert_eq!(
        opened["result"]["projectTestForm"]["parameterOffer"],
        json!({ "nextStep": "inspect", "available": ["inspect"] }),
        "{opened}"
    );

    let reviewed = engine
        .ask(&runtime, "review", &json!({ "json": TYPED }))
        .expect("reviewed");
    let started = engine
        .ask(
            &runtime,
            "confirm",
            &json!({ "review": reviewed["identity"], "values": reviewed["values"] }),
        )
        .expect("the write starts");
    assert!(started["operationId"].is_string(), "{started}");
    let mut progress = Vec::new();
    loop {
        let update = engine
            .updates
            .recv_timeout(Duration::from_secs(30))
            .expect("an update of the write");
        if update.state != OperationState::Running {
            assert_eq!(update.state, OperationState::Finished, "{update:?}");
            break;
        }
        progress.extend(update.progress);
    }
    assert_eq!(progress, [WRITING], "the write's progress is the engine's");

    let released = engine
        .ask(&runtime, "released", &json!({}))
        .expect("answered");
    assert_eq!(released["settingsReleased"], json!(RELEASED), "{released}");
    assert_eq!(released["offer"], free_offer(), "{released}");
    let test = engine
        .claims
        .claim(PROJECT_ID, LOOP_ID, "Identify plant")
        .expect("a free loop");
    let held = engine
        .ask(&runtime, "released", &json!({}))
        .expect("answered");
    assert!(held.get("settingsReleased").is_none(), "{held}");
    assert_eq!(
        held["message"],
        json!("Identify plant is running in this engine on this loop. Wait for it to end, then try again."),
        "{held}"
    );
    assert_eq!(held["offer"], frozen_offer(), "{held}");
    drop(test);
}
