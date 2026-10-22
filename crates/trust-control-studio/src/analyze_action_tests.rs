//! The routing of `controlStudio/analyze` (specification 35, §0.1a and B9.9; A7): the tools that apply, a quick calculation's envelope, an
//! operation's final update, the input refusals, the cancelled and the budget-bound calculations. The calculations are trust-control's and
//! are tested there; the study is written only by a kept result of a served loop.

use std::fs;
use std::path::PathBuf;
use std::sync::Arc;
use std::thread;
use std::time::{Duration, Instant};

use serde_json::{json, Value};
use trust_control::analysis::budget::{Budget, ANALYSIS_BUDGET, BUDGET_EXCEEDED};
use trust_control::failure::{Cause, NextAction};
use trust_control::loop_definition::LoopDefinition;
use trust_control::messages::InputsRequest;
use trust_control::messages::{
    AnalysisInputs, AnalysisTool, AnalyzeRequest, FrequencyGrid, OperationState, OperationUpdate,
};
use trust_control::model::{decode_general_model, GeneralModel};
use trust_control::recording::evidence::recording_evidence;
use trust_control::recording::{EvidenceSource, RecordedRun, RecordingFile};
use trust_control::study::analysis_records::current_records;
use trust_control::study::general_model_persistence::SuppliedModelSection;

use super::{answer, calculate, Work};
use crate::busy_loops::LoopClaims;
use crate::commands::{ControlStudio, PAGE};
use crate::inputs_action;
use crate::operation::{Operations, UpdateSink};
use crate::storage;

/// The example loop of the engine's tests.
const EXAMPLE: &str = include_str!("../../../examples/control-pid-commissioning/control-loop.json");

/// The sink of an engine whose operation updates nobody receives.
struct Ignore;

impl UpdateSink for Ignore {
    fn send(&self, _update: OperationUpdate) {}
}

/// A project folder of one test with the example loop, removed when the test ends.
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
        let root =
            std::env::temp_dir().join(format!("trust-analyze-{name}-{}", std::process::id()));
        let _ = fs::remove_dir_all(&root);
        fs::create_dir_all(&root).expect("the project folder");
        fs::write(root.join("control-loop.json"), EXAMPLE).expect("the loop file");
        let loop_definition = LoopDefinition::decode(EXAMPLE).expect("the example loop");
        Self {
            root,
            loop_definition,
        }
    }

    /// A request for the example loop, with `extra` fields added to its arguments.
    fn request(&self, extra: &Value) -> AnalyzeRequest {
        let mut value = json!({
            "projectRoot": self.root.display().to_string(),
            "runtimeId": self.loop_definition.runtime_id,
            "loopId": self.loop_definition.id,
        });
        value
            .as_object_mut()
            .expect("an object")
            .extend(extra.as_object().expect("an object").clone());
        trust_control::json::decode_value::<AnalyzeRequest>(value).expect("a valid request")
    }
}

fn operations() -> Operations {
    Operations::new(Arc::new(Ignore))
}

fn channel(id: &str, unit: &str) -> Value {
    json!({ "id": id, "unit": unit, "operatingRange": { "minimum": -10.0, "maximum": 10.0 } })
}

/// A continuous single-input, single-output state-space model with two states: a supplied model of the request.
fn supplied_model() -> Value {
    json!({
        "kind": "continuous-state-space", "revision": "supplied-1", "source": "provided", "validation": "unvalidated",
        "evidence": [], "delaySeconds": 0.0,
        "inputs": [channel("u", "V")], "outputs": [channel("y", "m")],
        "states": [channel("x1", "m"), channel("x2", "m/s")],
        "a": [[0.0, 1.0], [-2.0, -3.0]], "b": [[0.0], [1.0]], "c": [[1.0, 0.0]], "d": [[0.0]],
    })
}

fn model() -> GeneralModel {
    decode_general_model(&supplied_model()).expect("the supplied model is valid")
}

/// Waits for the operation to end, as a panel that reattaches would, and returns its last update.
fn finished(operations: &Operations, id: &str) -> OperationUpdate {
    let started = Instant::now();
    loop {
        let update = operations.status(id).expect("the operation is known");
        if update.state != OperationState::Running {
            return update;
        }
        assert!(
            started.elapsed() < Duration::from_secs(20),
            "the operation ends"
        );
        thread::sleep(Duration::from_millis(10));
    }
}

#[test]
fn without_a_tool_the_answer_lists_the_tools_that_apply_to_the_supplied_model() {
    let project = Project::new("offers");
    let request = project.request(&json!({ "model": supplied_model() }));
    let value =
        answer(&operations(), &LoopClaims::default(), &request).expect("the tools are listed");
    let tools: Vec<&str> = value["tools"]
        .as_array()
        .expect("a list of tools")
        .iter()
        .map(|offer| offer["tool"].as_str().expect("a tool name"))
        .collect();
    assert_eq!(
        tools,
        vec![
            "poleZero",
            "frequency",
            "leadLag",
            "initialState",
            "stateStructure"
        ]
    );
    assert!(
        !tools.contains(&"measuredFrequency"),
        "no recording is listed"
    );
    let frequency = value["tools"][1].clone();
    assert_eq!(frequency["operation"], json!(true));
    assert_eq!(frequency["abortAction"], json!("Abort analysis"));
    assert_eq!(frequency["defaults"]["grid"]["points"], json!(128));
}

#[test]
fn an_analysis_answer_is_not_current_after_the_model_changes() {
    // B9.9, A8.4: the page sends the revision of the model each answer it holds was calculated for, and the list of tools says which of
    // them no longer belong to the model analysed now (here the request's own, which stands for the study's). Nothing is listed while the
    // answers match, so the page drops an answer only when the engine says it is out of date.
    let project = Project::new("currency");
    let list = |revision: &str, held: &Value| {
        let mut model = supplied_model();
        model["revision"] = json!(revision);
        let request = project.request(&json!({ "model": model, "held": held }));
        answer(&operations(), &LoopClaims::default(), &request).expect("the tools are listed")
    };
    let held = json!([
        { "tool": "poleZero", "model": "supplied-1" },
        { "tool": "frequency", "model": "supplied-2" },
        { "tool": "rootLocus" },
    ]);
    let after_the_change = list("supplied-2", &held);
    assert_eq!(
        after_the_change["stale"],
        json!(["poleZero"]),
        "the answer calculated for the earlier model is stale; the declared analysis does not read the model: {after_the_change}"
    );
    let unchanged = list(
        "supplied-1",
        &json!([{ "tool": "poleZero", "model": "supplied-1" }]),
    );
    assert!(
        unchanged.get("stale").is_none(),
        "an answer for the model's revision stays: {unchanged}"
    );
    let no_answers = list("supplied-1", &json!([]));
    assert!(no_answers.get("stale").is_none(), "{no_answers}");
}

/// B9.9, A8.1, A8.4: the engine records the analysis a calculation answers as the one the Tune page shows, with the inputs it read and the
/// revision of the model it used, and the answer to a change of the inputs names the recorded analysis when the study's model is another
/// one. The revisions are worked out by hand: the study holds `supplied-1`, then `supplied-2`.
#[test]
fn the_engine_records_the_shown_analysis_and_names_what_a_change_made_stale() {
    let project = Project::new("shown-analysis");
    let claims = LoopClaims::default();
    let revision = project.loop_definition.revision.clone();
    let study = || {
        storage::open_study(&project.root, &project.loop_definition.id, &revision)
            .expect("the study")
    };
    let keep_model = |model_revision: &str| {
        let mut value = supplied_model();
        value["revision"] = json!(model_revision);
        let mut next = study();
        next.set_section(&SuppliedModelSection::new(
            decode_general_model(&value).expect("a valid model"),
            None,
        ))
        .expect("the model is kept");
        storage::save_study(&project.root, &next, &[]).expect("the study is saved");
    };
    let change = |claims: &LoopClaims| {
        let request = InputsRequest {
            project_root: project.root.display().to_string(),
            runtime_id: project.loop_definition.runtime_id.clone(),
            loop_id: project.loop_definition.id.clone(),
            inputs: None,
            change: json!({ "tuningPreferences": { "method": "automatic" } })
                .as_object()
                .cloned(),
            channel_selection: None,
            storage_folder: Some(project.root.join("storage").display().to_string()),
        };
        inputs_action::answer(claims, request).expect("the change is recorded")
    };
    keep_model("supplied-1");
    assert_eq!(
        study().analysis,
        None,
        "nothing is shown before a calculation"
    );

    // The page asks for the state structure of the study's own model: the engine records what it answered.
    let calculated = answer(
        &operations(),
        &claims,
        &project.request(&json!({ "tool": "stateStructure" })),
    )
    .expect("the structure is calculated");
    let recorded = study()
        .analysis
        .expect("the answer is recorded as the shown analysis");
    assert_eq!(recorded.tool, AnalysisTool::StateStructure);
    assert_eq!(recorded.model_revision.as_deref(), Some("supplied-1"));
    assert_eq!(
        serde_json::to_value(&recorded.inputs).expect("the inputs encode"),
        calculated["inputs"],
        "the inputs are the ones the engine read, not the ones the page sent"
    );

    // The model is still the study's: the change leaves the analysis current and the answer lists nothing.
    let unchanged = change(&claims);
    assert!(unchanged.get("stale").is_none(), "{unchanged}");

    // The page records the analysis it shows (`controlStudio/page`) without a revision; the engine keeps the one it recorded for the tool.
    let studio = ControlStudio::new(Arc::new(Ignore));
    let paged = |analysis: Value| {
        let reply = studio
            .execute(
                PAGE,
                &[json!({
                    "projectRoot": project.root.display().to_string(),
                    "runtimeId": project.loop_definition.runtime_id,
                    "loopId": project.loop_definition.id,
                    "page": "Tune",
                    "analysis": analysis,
                })],
            )
            .expect("the page command is the engine's");
        assert!(reply.get("failure").is_none(), "{reply}");
    };
    paged(json!({ "tool": "stateStructure", "inputs": {} }));
    assert_eq!(
        study()
            .analysis
            .and_then(|page| page.model_revision)
            .as_deref(),
        Some("supplied-1"),
        "the same tool keeps the revision it was calculated for"
    );

    // The study's model is replaced: the recorded analysis describes a plant that is not the study's any more.
    keep_model("supplied-2");
    let changed = change(&claims);
    assert_eq!(
        changed["stale"],
        json!(["stateStructure"]),
        "the engine names the analysis the change made stale: {changed}"
    );

    // Another tool, only selected, was calculated for no model, so nothing is out of date; the page's request replaces the record.
    paged(json!({ "tool": "poleZero", "inputs": {} }));
    let recorded = study().analysis.expect("the selection is recorded");
    assert_eq!(
        (recorded.tool, recorded.model_revision),
        (AnalysisTool::PoleZero, None)
    );
    assert!(change(&claims).get("stale").is_none());
}

#[test]
fn a_calculation_of_a_model_the_request_supplies_is_not_recorded_as_the_shown_analysis() {
    // The page asks for the study's model. A request that carries a model of its own is an analysis of other evidence: recording it as the
    // analysis the Tune page shows would name the page's tool for a plant that is not the study's.
    let project = Project::new("supplied-not-recorded");
    let request = project.request(&json!({ "model": supplied_model(), "tool": "stateStructure" }));
    answer(&operations(), &LoopClaims::default(), &request).expect("the structure is calculated");
    let study = storage::open_study(
        &project.root,
        &project.loop_definition.id,
        &project.loop_definition.revision,
    )
    .expect("the study");
    assert_eq!(study.analysis, None);
}

#[test]
fn a_quick_tool_answers_at_once_with_its_envelope_and_names_the_model_by_its_revision() {
    let project = Project::new("quick");
    let request = project.request(&json!({
        "model": supplied_model(),
        "tool": "poleZero",
        "inputs": { "view": "minimal" },
    }));
    let value =
        answer(&operations(), &LoopClaims::default(), &request).expect("the view is answered");
    assert_eq!(value["tool"], json!("poleZero"));
    assert_eq!(value["inputs"]["view"], json!("minimal"));
    assert_eq!(value["model"]["revision"], json!("supplied-1"));
    assert_eq!(value["result"]["poleZero"]["view"], json!("minimal"));
    assert!(
        value.get("failure").is_none(),
        "a qualified calculation has no failure: {value}"
    );
}

#[test]
fn an_operation_answers_its_identifier_and_its_final_update_carries_the_envelope() {
    let project = Project::new("operation");
    let operations = operations();
    let request = project.request(&json!({
        "model": supplied_model(),
        "tool": "frequency",
        "inputs": { "grid": { "minimumFrequency": 0.1, "maximumFrequency": 10.0, "points": 8 } },
    }));
    let started =
        answer(&operations, &LoopClaims::default(), &request).expect("the operation starts");
    let id = started["operationId"]
        .as_str()
        .expect("an operation identifier")
        .to_owned();
    let update = finished(&operations, &id);
    assert_eq!(
        update.state,
        OperationState::Finished,
        "{:?}",
        update.failure
    );
    let envelope = update.result.expect("the envelope is the final result");
    assert_eq!(envelope["tool"], json!("frequency"));
    assert_eq!(envelope["defaults"]["grid"]["points"], json!(128));
    let channels = envelope["result"]["frequency"]["channels"]
        .as_array()
        .expect("the channels of the response");
    assert_eq!(
        channels.len(),
        1,
        "a single-input, single-output model has one channel"
    );
    assert_eq!(
        channels[0]["points"].as_array().expect("the points").len(),
        8
    );
}

#[test]
fn an_input_outside_its_rule_is_a_method_failure_that_names_the_input() {
    let project = Project::new("inputs");
    let request = project.request(&json!({
        "model": supplied_model(),
        "tool": "frequency",
        "inputs": { "grid": { "minimumFrequency": 0.1, "maximumFrequency": 10.0, "points": 1 } },
    }));
    let refused = answer(&operations(), &LoopClaims::default(), &request)
        .expect_err("one grid point is refused before anything starts");
    assert!(refused.message.contains("points"), "{}", refused.message);
    assert_eq!(refused.next_action, NextAction::None);
}

#[test]
fn a_tool_that_does_not_apply_and_a_missing_model_are_refused_with_their_next_action() {
    let project = Project::new("refusals");
    let transfer = json!({
        "kind": "transfer-function", "revision": "tf", "source": "provided", "validation": "unvalidated",
        "evidence": [], "delaySeconds": 0.0, "inputs": [channel("u", "V")], "outputs": [channel("y", "m")],
        "numerator": [1.0], "denominator": [1.0, 1.0],
    });
    let structure = project.request(&json!({ "model": transfer, "tool": "stateStructure" }));
    let refused = answer(&operations(), &LoopClaims::default(), &structure)
        .expect_err("a transfer function has no state structure");
    assert!(
        refused.message.contains("does not apply"),
        "{}",
        refused.message
    );
    assert_eq!(refused.next_action, NextAction::ChooseMethod);
    // No model in the request and none in the study: the engineer identifies the plant first (B4.7).
    let missing = project.request(&json!({ "tool": "poleZero" }));
    let no_model =
        answer(&operations(), &LoopClaims::default(), &missing).expect_err("no model is at hand");
    assert_eq!(no_model.next_action, NextAction::Identify);
}

#[test]
fn a_model_that_is_not_a_general_model_is_refused_as_the_request_model() {
    let project = Project::new("invalid-model");
    let request = project.request(&json!({ "model": { "kind": "nonsense" }, "tool": "poleZero" }));
    let refused = answer(&operations(), &LoopClaims::default(), &request)
        .expect_err("the model is not a general model");
    assert!(
        refused.message.contains("not a valid general model"),
        "{}",
        refused.message
    );
}

/// The example loop with a structure the engine does not run yet (V7): the program's own law of `family`, and a `part` that the Rust loop
/// does not decode. `loops::open` refuses such a file as a parked feature, with the cause `controller-capability`.
fn parked_loop_file(family: &str, part: &str) -> String {
    let mut file: Value = serde_json::from_str(EXAMPLE).expect("the example loop");
    file["controller"]["project"] = json!({
        "family": family, "convention": "project-owned-native-controller",
        "initialization": { "kind": "declared-state-v1", "states": [] }, "axes": [],
    });
    file[part] = json!({ "gain": 1.0 });
    file.to_string()
}

/// The identifiers of the tools that an answer lists, in the order it lists them.
fn tool_names(listed: &Value) -> Vec<&str> {
    listed["tools"]
        .as_array()
        .expect("a list of tools")
        .iter()
        .map(|offer| offer["tool"].as_str().expect("a tool name"))
        .collect()
}

#[test]
fn a_model_of_a_parked_loop_answers_the_tools_that_apply_to_it() {
    let project = Project::new("parked-loop");
    fs::write(
        project.root.join("control-loop.json"),
        parked_loop_file("Smith", "smithModel"),
    )
    .expect("the parked loop");
    let quick = project.request(&json!({ "model": supplied_model(), "tool": "poleZero" }));
    let value = answer(&operations(), &LoopClaims::default(), &quick)
        .expect("the quick tool answers from the request's model");
    assert_eq!(value["tool"], json!("poleZero"));
    assert_eq!(value["model"]["revision"], json!("supplied-1"));
    assert!(value.get("failure").is_none(), "{value}");
    let listed = answer(
        &operations(),
        &LoopClaims::default(),
        &project.request(&json!({ "model": supplied_model() })),
    )
    .expect("the tools are listed");
    assert_eq!(
        tool_names(&listed),
        vec!["poleZero", "frequency", "leadLag", "initialState", "stateStructure"],
        "the tools that need only the model are listed, operations included; none of them needs the loop"
    );
    let without_model = project.request(&json!({ "tool": "poleZero" }));
    let failure = answer(&operations(), &LoopClaims::default(), &without_model)
        .expect_err("without a model the loop's own failure stands");
    assert_eq!(failure.cause, Cause::ControllerCapability, "{failure:?}");
}

#[test]
fn a_parked_loop_without_a_model_lists_no_tools_because_its_refusal_is_the_open_answers() {
    let project = Project::new("parked-list-without-model");
    fs::write(
        project.root.join("control-loop.json"),
        parked_loop_file("Smith", "smithModel"),
    )
    .expect("the parked loop");
    let listed = answer(
        &operations(),
        &LoopClaims::default(),
        &project.request(&json!({})),
    )
    .expect("a refused loop lists no tools, and the refusal is the open answer's (B9.9)");
    assert_eq!(tool_names(&listed), [] as [&str; 0], "{listed}");
}

#[test]
fn a_parked_feedforward_loop_lists_and_runs_its_operations_with_the_request_model() {
    let project = Project::new("parked-feedforward");
    fs::write(
        project.root.join("control-loop.json"),
        parked_loop_file("feedforward", "feedforwardModel"),
    )
    .expect("the parked loop");
    let operations = operations();
    let listed = answer(
        &operations,
        &LoopClaims::default(),
        &project.request(&json!({ "model": supplied_model(), "controller": supplied_model() })),
    )
    .expect("the tools are listed");
    assert_eq!(
        tool_names(&listed),
        vec![
            "poleZero",
            "frequency",
            "feedback",
            "leadLag",
            "initialState",
            "stateStructure",
        ],
        "with the request's controller, feedback applies, and the operations are listed as for a served loop"
    );
    let started = answer(
        &operations,
        &LoopClaims::default(),
        &project.request(&json!({
            "model": supplied_model(),
            "tool": "frequency",
            "inputs": { "grid": { "minimumFrequency": 0.1, "maximumFrequency": 10.0, "points": 8 } },
        })),
    )
    .expect("the operation starts");
    let id = started["operationId"]
        .as_str()
        .expect("an operation identifier")
        .to_owned();
    let update = finished(&operations, &id);
    assert_eq!(
        update.state,
        OperationState::Finished,
        "{:?}",
        update.failure
    );
    let envelope = update.result.expect("the envelope is the final result");
    assert_eq!(envelope["tool"], json!("frequency"));
    let channels = envelope["result"]["frequency"]["channels"]
        .as_array()
        .expect("the channels of the response");
    assert_eq!(channels.len(), 1);
    assert_eq!(
        channels[0]["points"].as_array().expect("the points").len(),
        8
    );
    assert!(
        !project.root.join("control-studio").exists(),
        "a parked loop keeps nothing"
    );
}

#[test]
fn an_operation_of_a_parked_loop_claims_the_loop_by_the_identity_its_file_names() {
    let project = Project::new("parked-claim");
    fs::write(
        project.root.join("control-loop.json"),
        parked_loop_file("feedforward", "feedforwardModel"),
    )
    .expect("the parked loop");
    let claims = LoopClaims::default();
    let _held = claims
        .claim(
            &project.loop_definition.project_id,
            &project.loop_definition.id,
            "another operation",
        )
        .expect("the loop is free");
    let operation = project.request(&json!({ "model": supplied_model(), "tool": "frequency" }));
    let refused = answer(&operations(), &claims, &operation);
    assert!(refused.is_err(), "the loop is claimed by its operation");
}

#[test]
fn a_loop_id_the_project_does_not_declare_keeps_its_own_failure() {
    let project = Project::new("missing-loop");
    fs::write(
        project.root.join("control-loop.json"),
        parked_loop_file("Smith", "smithModel"),
    )
    .expect("the parked loop");
    let missing = project.request(&json!({
        "model": supplied_model(), "tool": "poleZero", "loopId": "loop:missing",
    }));
    let failure = answer(&operations(), &LoopClaims::default(), &missing)
        .expect_err("the project declares no such loop");
    assert_eq!(failure.cause, Cause::Implementation, "{failure:?}");
    assert_eq!(failure.next_action, NextAction::OpenLoopSetup);
}

/// The work of a frequency calculation of the supplied model, as the operation receives it.
fn frequency_work(project: &Project, grid: Option<FrequencyGrid>) -> Work {
    let model = model();
    Work {
        project_root: project.root.clone(),
        loop_definition: Some(project.loop_definition.clone()),
        served: false,
        tool: AnalysisTool::Frequency,
        inputs: AnalysisInputs {
            grid,
            ..AnalysisInputs::default()
        },
        defaults: AnalysisInputs::default(),
        records_shown: false,
        model: Some(model),
        model_revision: None,
        controller_revision: None,
        definition: None,
        declared: None,
    }
}

#[test]
fn a_cancelled_calculation_stops_at_its_next_step_and_writes_nothing() {
    let project = Project::new("cancelled");
    let work = frequency_work(
        &project,
        Some(FrequencyGrid {
            minimum_frequency: 0.1,
            maximum_frequency: 10.0,
            points: 8,
        }),
    );
    let elapsed = || Duration::ZERO;
    let cancel = || true;
    let budget = Budget::new(ANALYSIS_BUDGET, &elapsed, &cancel);
    let stopped = calculate(&work, &budget).expect_err("a cancelled calculation is not answered");
    assert_eq!(stopped.message, "Analysis was cancelled.");
    assert!(
        !project.root.join("control-studio").exists(),
        "a cancelled calculation keeps nothing"
    );
}

#[test]
fn a_calculation_past_the_ten_second_budget_fails_with_the_a7_sentence() {
    let project = Project::new("budget");
    let work = frequency_work(
        &project,
        Some(FrequencyGrid {
            minimum_frequency: 0.1,
            maximum_frequency: 10.0,
            points: 8,
        }),
    );
    let elapsed = || Duration::from_secs(11);
    let stop = || false;
    let budget = Budget::new(ANALYSIS_BUDGET, &elapsed, &stop);
    let exceeded = calculate(&work, &budget).expect_err("the budget is exceeded");
    assert_eq!(exceeded.message, BUDGET_EXCEEDED);
}

/// A complete periodic recording of the example loop, as the runtime returns it: 256 scans of 0.1 s whose applied input is `1 + 0.5 sin`
/// and whose measured output is `2 + sin`, so the record reads a steady gain of 2 at 1 rad/s. No sample is on the loop's output limits
/// (0 and 100). The run keeps the runtime's own identifier, which the analysis never names (§0.1a).
fn periodic_run(loop_revision: &str) -> RecordedRun {
    let samples: Vec<Value> = (0_u32..256)
        .map(|index| {
            let tone = (f64::from(index) * 0.1).sin();
            json!({
                "scan": index + 1,
                "runtime_time_ns": i64::from(index) * 100_000_000,
                "captured_at_ms": index,
                "values": [40.0, 2.0 + tone, 1.0 + 0.5 * tone],
                "forced": [false, false, false],
                "quality": ["good", "good", "good"],
            })
        })
        .collect();
    serde_json::from_value(json!({
        "recording_id": "periodic-import",
        "loop_revision": loop_revision,
        "bindings": [
            { "id": "setpoint", "address": "%ML0", "iec_type": "LREAL" },
            { "id": "processVariable", "address": "%IL0", "iec_type": "LREAL" },
            { "id": "manipulatedVariable", "address": "%QL0", "iec_type": "LREAL" },
        ],
        "stop_reason": "sample_limit",
        "samples": samples,
        "acquisition": {
            "method": "runtime_scan_buffer", "completeness": "complete", "observedSampleCount": 256,
            "scanRange": { "first": 1, "last": 256 }, "missingScanRanges": [],
            "droppedObservedSamples": 0, "actualCycleTimingAvailable": true,
            "freshnessAvailable": false, "reason": "",
        },
    }))
    .expect("a recorded run")
}

/// Keeps the periodic recording of the example loop in the project's study, as `importRecording` keeps a file (B10.2), and returns the
/// identifier that the study lists it under.
fn keep_periodic_recording(project: &Project) -> String {
    let revision = project.loop_definition.revision.clone();
    let run = periodic_run(&revision);
    let evidence = recording_evidence(&run, None);
    let file = RecordingFile::new(
        &project.loop_definition.id,
        EvidenceSource::SimulatedPlant,
        run,
        evidence,
    );
    let study = storage::open_study(&project.root, &project.loop_definition.id, &revision)
        .expect("the study");
    storage::save_recording(&project.root, &study, &file).expect("the recording is kept");
    file.id().expect("the study's identifier of the recording")
}

/// Runs the measured response with `inputs` to its end, and returns the envelope of its final update.
fn measured(project: &Project, inputs: &Value) -> Value {
    let operations = operations();
    let request = project.request(&json!({ "tool": "measuredFrequency", "inputs": inputs }));
    let started =
        answer(&operations, &LoopClaims::default(), &request).expect("the operation starts");
    let id = started["operationId"]
        .as_str()
        .expect("an operation identifier")
        .to_owned();
    let update = finished(&operations, &id);
    assert_eq!(
        update.state,
        OperationState::Finished,
        "{:?}",
        update.failure
    );
    update.result.expect("the envelope is the final result")
}

/// The identifier of the recording that the study keeps a measured response on, when it keeps one (B10.2).
fn kept_measured_recording(project: &Project) -> Option<String> {
    let study = storage::open_study(
        &project.root,
        &project.loop_definition.id,
        &project.loop_definition.revision,
    )
    .expect("the study");
    current_records(&study, &project.loop_definition.revision, None)
        .measured_frequency
        .map(|result| result.recording_id)
}

#[test]
fn a_request_that_names_a_kept_recording_by_its_study_id_measures_it_and_keeps_the_result() {
    let project = Project::new("measured-named");
    let recording_id = keep_periodic_recording(&project);
    let envelope = measured(
        &project,
        &json!({ "recordingId": recording_id, "frequencies": [1.0] }),
    );
    let measured_answer = &envelope["result"]["measuredFrequency"];
    assert_eq!(measured_answer["status"], json!("qualified"), "{envelope}");
    assert_eq!(
        measured_answer["recordingId"],
        json!(recording_id),
        "the answer names the study's identifier of the recording it measured"
    );
    assert_eq!(
        kept_measured_recording(&project),
        Some(recording_id),
        "the study keeps the measured response on that recording (B10.2)"
    );
}

#[test]
fn a_request_that_names_no_recording_measures_the_last_periodic_recording_under_its_study_id() {
    let project = Project::new("measured-last");
    let recording_id = keep_periodic_recording(&project);
    let envelope = measured(&project, &json!({ "frequencies": [1.0] }));
    let measured_answer = &envelope["result"]["measuredFrequency"];
    assert_eq!(
        measured_answer["recordingId"],
        json!(recording_id),
        "the answer names the study's identifier of the recording it measured"
    );
    assert_eq!(
        kept_measured_recording(&project),
        Some(recording_id),
        "the study keeps the measured response on that recording (B10.2)"
    );
}
