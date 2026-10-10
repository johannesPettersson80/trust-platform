//! The routing of `controlStudio/tune` (specification 35, §0.1a, A8.4, A8.5, B4.1 and B4.7): what the method decides before an
//! operation starts, and how `controlStudio/open` reports the Tune stage. The design is trust-control's (`study::tune`, tested
//! there), and the whole method runs against a real runtime in `tests/control_studio_tune.rs`. No runtime runs here, because
//! Tune never reaches one (B4.1), and no project below holds an Identify result, so every request is refused before an
//! operation starts. A run that designs and stores a candidate needs a study with an Identify result and is not tested here.

use std::fs;
use std::path::PathBuf;
use std::sync::Arc;

use serde_json::{json, Value};
use trust_control::failure::{Cause, NextAction};
use trust_control::identification::connected_run::loop_busy_refusal;
use trust_control::loop_definition::LoopDefinition;
use trust_control::messages::{OperationUpdate, TuneRequest};

use super::answer;
use crate::busy_loops::LoopClaims;
use crate::commands::{ControlStudio, METHODS, OPEN, TUNE};
use crate::operation::{Operations, UpdateSink};

/// The example project's loop file.
const EXAMPLE: &str = include_str!("../../../examples/control-pid-commissioning/control-loop.json");

/// The sink of an engine whose operations nobody follows: no operation starts in these tests.
struct Ignore;

impl UpdateSink for Ignore {
    fn send(&self, _update: OperationUpdate) {}
}

/// A project folder of one test with the example loop, removed when the test ends, whether or not it passed.
struct Project {
    root: PathBuf,
    loop_definition: LoopDefinition,
}

impl Drop for Project {
    fn drop(&mut self) {
        let _ = fs::remove_dir_all(&self.root);
    }
}

impl Project {
    fn new(name: &str) -> Self {
        let root = std::env::temp_dir().join(format!("trust-tune-{name}-{}", std::process::id()));
        let _ = fs::remove_dir_all(&root);
        fs::create_dir_all(&root).expect("the project folder");
        fs::write(root.join("control-loop.json"), EXAMPLE).expect("the loop file");
        let loop_definition = LoopDefinition::decode(EXAMPLE).expect("the example loop");
        Self {
            root,
            loop_definition,
        }
    }

    /// The arguments of `controlStudio/tune` and of `controlStudio/open` for the example loop: Automatic, no pins.
    fn request(&self) -> Value {
        json!({
            "projectRoot": self.root.display().to_string(),
            "runtimeId": self.loop_definition.runtime_id,
            "loopId": self.loop_definition.id,
            "storageFolder": self.root.join("storage").display().to_string(),
        })
    }

    /// The same arguments as the typed request that `answer` takes.
    fn tune_request(&self) -> TuneRequest {
        trust_control::json::decode_value(self.request()).expect("a Tune request")
    }

    /// Whether the project holds a study folder.
    fn has_study(&self) -> bool {
        self.root.join("control-studio").exists()
    }
}

/// An engine whose operation updates nobody receives.
fn engine() -> ControlStudio {
    ControlStudio::new(Arc::new(Ignore))
}

/// Asks the engine for `controlStudio/tune`.
fn tune(studio: &ControlStudio, request: Value) -> Value {
    studio
        .execute(TUNE, &[request])
        .expect("the engine answers its own method")
}

/// The message and the next action of a refused answer.
fn refusal(reply: &Value) -> (&str, &str) {
    let failure = &reply["failure"];
    (
        failure["message"].as_str().unwrap_or_default(),
        failure["nextAction"].as_str().unwrap_or_default(),
    )
}

/// The status of `stage` in the stages that `controlStudio/open` answers.
fn status_of<'a>(stages: &'a Value, stage: &str) -> Option<&'a str> {
    stages
        .as_array()?
        .iter()
        .find(|item| item["stage"] == stage)?
        .get("status")?
        .as_str()
}

/// Whether no operation was started: the engine numbers its operations from 1.
fn nothing_started(operations: &Operations) -> bool {
    operations.status("operation-1").is_err()
}

#[test]
fn tune_is_a_method_of_the_engine_and_refuses_a_malformed_request_or_a_runtime_connection() {
    assert!(METHODS.contains(&TUNE), "the engine answers {TUNE}");
    let project = Project::new("malformed");
    let studio = engine();
    // Tune never reaches the runtime (B4.1), so its arguments name no connection.
    let mut with_connection = project.request();
    with_connection["connection"] = json!({ "endpoint": "tcp://127.0.0.1:9" });
    for request in [json!({ "projectRoot": "/x" }), with_connection] {
        let reply = tune(&studio, request);
        assert!(refusal(&reply).0.contains("invalid arguments"), "{reply}");
    }
    assert!(nothing_started(studio.operations()), "nothing was started");
}

#[test]
fn tune_of_a_loop_the_project_does_not_declare_is_refused_to_open_the_loop_setup() {
    let project = Project::new("unknown-loop");
    let studio = engine();
    let mut request = project.request();
    request["loopId"] = json!("no-such-loop");
    let reply = tune(&studio, request);
    assert_eq!(refusal(&reply).1, "open-loop-setup", "{reply}");
    assert!(nothing_started(studio.operations()), "nothing was started");
    assert!(!project.has_study(), "a refusal writes nothing");
}

/// B4.7: without a current model Tune is refused at once, with the next action Identify. The refusal starts no operation,
/// writes no study and lets the loop go.
#[test]
fn tune_without_a_model_is_refused_at_once_with_identify_and_lets_the_loop_go() {
    let project = Project::new("no-model");
    let operations = Operations::new(Arc::new(Ignore));
    let claims = LoopClaims::default();
    let refused = answer(&operations, &claims, project.tune_request())
        .expect_err("a study without a model is refused");
    assert_eq!(
        (refused.cause, refused.next_action),
        (Cause::ModelOrData, NextAction::Identify),
        "{refused:?}"
    );
    assert!(nothing_started(&operations), "no operation was started");
    assert!(!project.has_study(), "a refusal writes nothing");
    let loop_definition = &project.loop_definition;
    let _free = claims
        .claim(&loop_definition.project_id, &loop_definition.id, "Tune")
        .expect("the refused request let the loop go");
}

/// A8.5: one operation per loop at a time in one engine. A request for Tune on a loop that another operation of the engine
/// holds is refused at once and names the holder; once that operation has ended, Tune is refused only for what it lacks.
#[test]
fn tune_on_a_loop_this_engine_works_on_is_refused_naming_the_holder_until_it_ends() {
    let project = Project::new("busy");
    let studio = engine();
    let loop_definition = &project.loop_definition;
    let held = studio
        .loop_claims()
        .claim(
            &loop_definition.project_id,
            &loop_definition.id,
            "Identify plant",
        )
        .expect("a free loop");
    let reply = tune(&studio, project.request());
    assert_eq!(
        reply,
        json!({ "failure": loop_busy_refusal("Identify plant") })
    );
    assert!(nothing_started(studio.operations()), "nothing was started");
    drop(held);
    let reply = tune(&studio, project.request());
    assert_eq!(
        refusal(&reply).1,
        "identify",
        "the loop is free again, so Tune is refused for its model: {reply}"
    );
}

/// B4.7 and A8.4: a study without an Identify result has no model, so `controlStudio/open` reports Tune as needing data.
/// Opening starts nothing and writes nothing (A8.6).
#[test]
fn open_reports_the_tune_stage_as_needing_data_while_the_study_holds_no_model() {
    let project = Project::new("open");
    let studio = engine();
    let request = project.request();
    let opened = studio.execute(OPEN, &[request]).expect("answered");
    let stages = &opened["result"]["stages"];
    assert_eq!(status_of(stages, "Tune"), Some("needs-data"), "{opened}");
    assert_eq!(
        status_of(stages, "Identify"),
        Some("needs-data"),
        "{opened}"
    );
    assert!(nothing_started(studio.operations()), "nothing was started");
    assert!(!project.has_study(), "opening writes nothing");
}

/// N5.R: the relay rule is offered only for a model fitted from a qualified relay record. Without a model it is refused for the
/// model first, as every method is, and the refusal starts nothing and writes nothing.
#[test]
fn the_relay_rule_without_a_model_is_refused_for_the_model_and_starts_nothing() {
    let project = Project::new("relay-no-model");
    let studio = engine();
    let mut request = project.request();
    request["method"] = json!("relay");
    let reply = tune(&studio, request);
    assert_eq!(refusal(&reply).1, "identify", "{reply}");
    assert!(nothing_started(studio.operations()), "nothing was started");
    assert!(!project.has_study(), "a refusal writes nothing");
}
