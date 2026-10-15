//! The routing of `controlStudio/tune` (specification 35, §0.1a, A8.4, A8.5, B4.1, B4.5 and B4.7): what the method decides before an
//! operation starts, how `controlStudio/open` reports the Tune stage, and how the running settings are read for the comparison.
//! The design is trust-control's (`study::tune`, tested there), and the whole method runs against a real runtime in
//! `tests/control_studio_tune.rs`. No runtime runs here: a connection in these tests names a port nothing listens on, and no
//! project below holds an Identify result, so every request is refused before an operation starts. The one run that designs a candidate
//! here is a program's own PI law (C2), whose project imports a supplied model first (`importModel`): its answer carries
//! `projectCandidate`, keyed by the seed's symbols, and no Control.PID `candidate`.

use std::fs;
use std::path::PathBuf;
use std::sync::{mpsc, Arc};
use std::time::Duration;

use parking_lot::Mutex;
use serde_json::{json, Value};
use trust_control::failure::{Cause, NextAction};
use trust_control::identification::connected_run::loop_busy_refusal;
use trust_control::loop_definition::{
    InitialSource, InitialValue, InitializedState, LoopDefinition, PostScan, StateType,
};
use trust_control::messages::{OperationState, OperationUpdate, TuneChoice, TuneRequest};
use trust_control::runtime::RuntimeConnection;

use super::{answer, running_settings, with_declared_response};
use crate::busy_loops::LoopClaims;
use crate::commands::{ControlStudio, IDENTIFY, METHODS, OPEN, TUNE};
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
        Self::with_loop(name, EXAMPLE)
    }

    /// A project folder of one test whose `control-loop.json` is `text`, which must decode.
    fn with_loop(name: &str, text: &str) -> Self {
        let root = std::env::temp_dir().join(format!("trust-tune-{name}-{}", std::process::id()));
        let _ = fs::remove_dir_all(&root);
        fs::create_dir_all(&root).expect("the project folder");
        fs::write(root.join("control-loop.json"), text).expect("the loop file");
        let loop_definition = LoopDefinition::decode(text).expect("the loop file decodes");
        Self {
            root,
            loop_definition,
        }
    }

    /// The arguments of `controlStudio/identify` that import `model` as the study's model (B3.7), for the loop.
    fn import_request(&self, model: &str) -> Value {
        let mut request = self.request();
        // The identify request names its action as an object with the action's tag and fields (IdentifyRequest).
        request["action"] = json!({ "action": "importModel", "json": model });
        request
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
fn tune_is_a_method_of_the_engine_and_refuses_a_malformed_request() {
    assert!(METHODS.contains(&TUNE), "the engine answers {TUNE}");
    let studio = engine();
    let reply = tune(&studio, json!({ "projectRoot": "/x" }));
    assert!(refusal(&reply).0.contains("invalid arguments"), "{reply}");
    assert!(nothing_started(studio.operations()), "nothing was started");
}

/// B4.5: a connection is a valid part of a Tune request. Tune reads the running settings only once it designs, so a request that
/// is refused for its model is refused as it is without one, and it starts nothing.
#[test]
fn a_connection_is_a_valid_request_part_and_changes_no_refusal_before_the_design() {
    let project = Project::new("connection-refusal");
    let studio = engine();
    let without = tune(&studio, project.request());
    let mut with_connection = project.request();
    with_connection["connection"] =
        json!({ "endpoint": "tcp://127.0.0.1:9", "token": "control-token" });
    let with = tune(&studio, with_connection);
    assert_eq!(refusal(&with), refusal(&without), "{with}");
    assert_eq!(refusal(&with).1, "identify", "{with}");
    assert!(nothing_started(studio.operations()), "nothing was started");
    assert!(!project.has_study(), "a refusal writes nothing");
}

/// B4.5: a Tune request decodes with or without its connection, and the connection keeps the runtime's address and token.
#[test]
fn a_tune_request_decodes_with_or_without_its_connection() {
    let project = Project::new("connection-decode");
    assert_eq!(project.tune_request().connection, None);
    let mut request = project.request();
    request["connection"] = json!({ "endpoint": "tcp://127.0.0.1:9", "token": "control-token" });
    let connected: TuneRequest =
        trust_control::json::decode_value(request).expect("a Tune request with a connection");
    assert_eq!(
        connected.connection,
        Some(RuntimeConnection {
            endpoint: "tcp://127.0.0.1:9".to_string(),
            token: Some("control-token".to_string()),
        })
    );
}

/// B4.5 and B5.3: the running settings are read only through a connection, and a read that fails gives none, so that Tune designs
/// without the comparison and does not fail.
#[test]
fn the_running_settings_are_read_through_a_connection_and_a_failed_read_gives_none() {
    let project = Project::new("running-settings");
    assert_eq!(running_settings(None, &project.loop_definition), None);
    let unreachable = RuntimeConnection {
        endpoint: "tcp://127.0.0.1:1".to_string(),
        token: None,
    };
    assert_eq!(
        running_settings(Some(&unreachable), &project.loop_definition),
        None
    );
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

#[test]
fn tune_takes_the_response_the_study_declares_when_its_request_names_none() {
    // B4.3: a request that names no response reads the study's declared one, as the automatic run does; a response it names is kept.
    let project = Project::new("declared-response");
    let request = project.tune_request();
    assert_eq!(
        with_declared_response(request.clone(), Some(TuneChoice::Faster)).choice,
        Some(TuneChoice::Faster)
    );
    let named = TuneRequest {
        choice: Some(TuneChoice::Balanced),
        ..request.clone()
    };
    assert_eq!(
        with_declared_response(named, Some(TuneChoice::Faster)).choice,
        Some(TuneChoice::Balanced),
        "the response the request names replaces the declared one"
    );
    assert_eq!(with_declared_response(request, None).choice, None);
}

/// The loop identifier of the program's own PI law of these tests.
const OWN_LAW_ID: &str = "own-law-pi";

/// The loop file of a program's own PI law (C2), as the setup writes it: the seed names `P1.Kp` and `P1.Ki`, and the declared state is the
/// integral term, which its `pid-integral` formula starts on `P1.Kp`. It runs on the example's signals (°C and %) at 100 ms.
fn own_law_pi_loop() -> String {
    let integral = InitializedState {
        symbol: "P1.Integral".to_owned(),
        iec_type: StateType::Lreal,
        unit: "%".to_owned(),
        minimum: -110.0,
        maximum: 110.0,
        initial: InitialValue::From(InitialSource::PidIntegral {
            index: 0,
            input_index: 0,
            gain_symbol: "P1.Kp".to_owned(),
            weight_symbol: None,
            weight: None,
        }),
        post_scan: PostScan::Bounded,
    };
    json!({
        "schemaVersion": 2, "id": OWN_LAW_ID, "name": "Own PI law", "revision": "sha256:own-law-pi-1",
        "projectId": "project:own-law-pi", "runtimeId": "simulator",
        "controller": {
            "instance": "P1.Controller", "type": "Control.PID", "revision": "sha256:own-law-controller",
            "action": "direct", "taskPeriodMs": 100, "design": "project",
            "modeSignal": "P1.AutoMode", "modeWhenTrue": "Auto",
            "manualOutputSignal": "P1.ManualOutput", "manualOutputType": "LREAL",
            "project": {
                "family": "PI", "convention": "project-owned-native-controller",
                "initialization": { "kind": "declared-state-v1", "states": [integral], "applicationStates": [] },
                "seed": { "kind": "parallel-pid", "family": "PI", "proportionalSymbol": "P1.Kp", "integralSymbol": "P1.Ki" },
                "axes": [],
                "objective": { "requirementId": "target-error", "direction": "minimize" },
                "maximumCandidates": 4, "maximumCycles": 50000, "wallBudgetMs": 60000
            }
        },
        "signals": {
            "setpoint": { "symbol": "P1.Setpoint", "address": "%ML0", "iecType": "LREAL", "direction": "read",
                "unit": "°C", "range": { "minimum": 0, "maximum": 100 } },
            "processVariable": { "symbol": "P1.ProcessValue", "address": "%IL0", "iecType": "LREAL", "direction": "read",
                "unit": "°C", "range": { "minimum": 0, "maximum": 100 } },
            "manipulatedVariable": { "symbol": "P1.ManipulatedValue", "address": "%QL0", "iecType": "LREAL",
                "direction": "write", "unit": "%", "range": { "minimum": 0, "maximum": 100 } }
        },
        "limits": { "outputMinimum": 0, "outputMaximum": 100 },
        "observedSampleTiming": { "nominalPeriodMs": 100 },
        "parameterBinding": {
            "loop_id": OWN_LAW_ID, "task_name": "MainTask",
            "parameters": [
                { "symbol": "P1.Kp", "iec_type": "LREAL", "unit": "%/°C", "convention": "parallel proportional gain",
                    "minimum": 0, "maximum": 100 },
                { "symbol": "P1.Ki", "iec_type": "LREAL", "unit": "%/(°C*s)", "convention": "parallel integral gain",
                    "minimum": 0, "maximum": 100 }
            ],
            "mode": { "symbol": "P1.AutoMode", "manual_value": false }
        }
    })
    .to_string()
}

/// A supplied model of the own-law loop: `2/(5s + 1)` with a delay of one second, from % to °C, validated (B3.7).
fn supplied_model() -> String {
    json!({
        "kind": "transfer-function", "revision": "own-law-model", "source": "provided",
        "validation": "validated", "evidence": ["independent-course-measurement"], "delaySeconds": 1.0,
        "inputs": [{ "id": "u", "unit": "%", "operatingRange": { "minimum": 0.0, "maximum": 100.0 } }],
        "outputs": [{ "id": "y", "unit": "°C", "operatingRange": { "minimum": 0.0, "maximum": 100.0 } }],
        "operatingPoint": { "inputs": [20.0], "outputs": [20.0] },
        "numerator": [2.0], "denominator": [5.0, 1.0]
    })
    .to_string()
}

/// A sink that hands each operation update to the test, in order.
struct Updates(Mutex<mpsc::Sender<OperationUpdate>>);

impl UpdateSink for Updates {
    fn send(&self, update: OperationUpdate) {
        let _ = self.0.lock().send(update);
    }
}

/// An engine whose operation updates the test receives.
fn engine_with_updates() -> (ControlStudio, mpsc::Receiver<OperationUpdate>) {
    let (sender, receiver) = mpsc::channel();
    (
        ControlStudio::new(Arc::new(Updates(Mutex::new(sender)))),
        receiver,
    )
}

/// Imports the supplied model as the study's model (B3.7) and answers the import.
fn import_model(studio: &ControlStudio, project: &Project) -> Value {
    let reply = studio
        .execute(IDENTIFY, &[project.import_request(&supplied_model())])
        .expect("the engine answers its own method");
    assert!(reply.get("result").is_some(), "the model imports: {reply}");
    reply["result"].clone()
}

/// Asks the engine for `controlStudio/tune` and waits for the operation's final update, which holds the Tune result (B4.3).
fn tune_and_wait(
    studio: &ControlStudio,
    updates: &mpsc::Receiver<OperationUpdate>,
    request: Value,
) -> Value {
    let started = tune(studio, request);
    assert!(
        started.get("result").is_some(),
        "the operation starts: {started}"
    );
    loop {
        let update = updates
            .recv_timeout(Duration::from_secs(60))
            .expect("an update of the Tune operation");
        if update.state == OperationState::Running {
            continue;
        }
        if let Some(failure) = &update.failure {
            panic!(
                "the Tune operation failed: {} ({:?})",
                failure.message, failure.next_action
            );
        }
        return update.result.expect("the Tune operation's result");
    }
}

/// The symbols of an answer's settings or values, in order.
fn symbols_of(settings: &Value) -> Vec<String> {
    let mut symbols: Vec<String> = settings
        .as_object()
        .map(|map| map.keys().cloned().collect())
        .unwrap_or_default();
    symbols.sort();
    symbols
}

#[test]
fn an_own_law_pi_loop_opens_with_tune_validate_and_apply_applicable() {
    // C2 and §0.1a: a served program's law has its controller stages, so Tune, Validate and Apply are applicable when the loop opens.
    // Tune waits for its model. Opening starts nothing and writes nothing.
    let project = Project::with_loop("own-law-open", &own_law_pi_loop());
    let studio = engine();
    let opened = studio
        .execute(OPEN, &[project.request()])
        .expect("answered");
    let stages = &opened["result"]["stages"];
    assert_eq!(status_of(stages, "Tune"), Some("needs-data"), "{opened}");
    for stage in ["Tune", "Validate", "Apply"] {
        assert_ne!(
            status_of(stages, stage),
            Some("not-applicable"),
            "{stage} applies to a program's own law: {opened}"
        );
    }
    assert!(nothing_started(studio.operations()), "nothing was started");
    assert!(!project.has_study(), "opening writes nothing");
}

#[test]
fn tune_automatic_answers_a_project_candidate_by_the_seed_symbols_once_a_model_is_imported() {
    // B4.3 and C2: with the supplied model imported, Automatic designs the program's law. The answer carries the candidate as
    // `projectCandidate`, unapplied, with its values keyed by the seed's variables, and no Control.PID `candidate`.
    let project = Project::with_loop("own-law-automatic", &own_law_pi_loop());
    let (studio, updates) = engine_with_updates();
    let imported = import_model(&studio, &project);
    assert_eq!(imported["modelRevision"], "own-law-model", "{imported}");
    let result = tune_and_wait(&studio, &updates, project.request());
    assert!(
        result.get("candidate").is_none(),
        "a program's law answers no Control.PID record: {result}"
    );
    let candidate = &result["projectCandidate"];
    assert_eq!(candidate["applied"], false, "{result}");
    assert!(
        matches!(candidate["structure"].as_str(), Some("P" | "PI")),
        "a PI seed offers P and PI, and Automatic picks one of them: {result}"
    );
    assert!(
        candidate["gains"]["proportionalGain"].is_number()
            && candidate["gains"]["integralGain"].is_number(),
        "the gains are the candidate's: {result}"
    );
    let symbols = symbols_of(&candidate["values"]);
    assert!(
        symbols.contains(&"P1.Kp".to_owned()),
        "the proportional variable: {result}"
    );
    assert!(
        symbols
            .iter()
            .all(|symbol| symbol == "P1.Kp" || symbol == "P1.Ki"),
        "only the seed's variables: {result}"
    );
    assert_eq!(result["settings"], candidate["values"], "{result}");
}

#[test]
fn tune_with_the_pi_and_the_p_method_answers_the_family_symbols_only() {
    // B4.4 and C2: a named method designs the family the engineer chose, and its answer keeps that family's variables: PI answers the
    // proportional and the integral variable, and P answers the proportional variable alone.
    let project = Project::with_loop("own-law-methods", &own_law_pi_loop());
    let (studio, updates) = engine_with_updates();
    import_model(&studio, &project);
    let mut pi = project.request();
    pi["method"] = json!("SIMC");
    pi["structure"] = json!("PI");
    let answered = tune_and_wait(&studio, &updates, pi);
    assert_eq!(
        answered["projectCandidate"]["structure"], "PI",
        "{answered}"
    );
    assert_eq!(
        symbols_of(&answered["settings"]),
        ["P1.Ki", "P1.Kp"],
        "{answered}"
    );
    let mut p = project.request();
    p["method"] = json!("SIMC");
    p["structure"] = json!("P");
    let answered = tune_and_wait(&studio, &updates, p);
    assert_eq!(answered["projectCandidate"]["structure"], "P", "{answered}");
    assert_eq!(symbols_of(&answered["settings"]), ["P1.Kp"], "{answered}");
    assert!(answered.get("candidate").is_none(), "{answered}");
}
