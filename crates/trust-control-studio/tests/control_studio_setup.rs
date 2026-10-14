//! Loop setup through the engine's methods on a copy of lesson 30's heater project (stage V5; specification 35, B1.2 to B1.4,
//! §0.1b; numerical annex N6). The project is `examples/control/06-pid-tuning-methods/30-model-based-pi-tuning` with its loop
//! file `control-loop.json` removed from the copy, so the project offers a loop to set up. The compiled catalog comes from the
//! built `trust-lsp` (`trust-lsp command`, specification 14 §7.8). No runtime runs: setup reads the project and writes loop
//! files only (§0.1b), so it runs at no speed of its own.
//!
//! The test proves three things: discovery proposes the PID structure with the prefill the program's code states; a draft of
//! those choices writes one loop file that `LoopDefinition` decodes and accepts for the runtime `simulator`; and the same draft
//! again is refused and changes nothing.

#![allow(clippy::expect_used)]

mod common;

use std::fs;
use std::path::{Path, PathBuf};
use std::sync::Arc;

use serde_json::{json, Value};
use trust_control::loop_definition::{LoopDefinition, LoopScope};
use trust_control::messages::OperationUpdate;
use trust_control_studio::commands::{ControlStudio, SETUP_DISCOVER, SETUP_SAVE};
use trust_control_studio::operation::UpdateSink;

/// The lesson's project, shipped in the repository.
const EXAMPLE: &str = "examples/control/06-pid-tuning-methods/30-model-based-pi-tuning";
/// The runtime the loop is set up for: the simulator the lessons use (`cli.rs`'s `SELECTED_RUNTIME`).
const RUNTIME: &str = "simulator";
/// The loop file that the copy does not keep, so that setup proposes one.
const LOOP_FILE: &str = "control-loop.json";
/// The loop that the draft below declares.
const LOOP_ID: &str = "loop:model-based-pi-tuning";
/// The loop file that setup writes for that loop.
const SAVED_FILE: &str = "control-loops/model-based-pi-tuning.json";

/// An engine that discards its operation updates; setup starts no operation.
fn studio() -> ControlStudio {
    struct Silent;

    impl UpdateSink for Silent {
        fn send(&self, _update: OperationUpdate) {}
    }

    ControlStudio::new(Arc::new(Silent))
}

/// A copy of the example in a folder of its own, without its loop file.
fn project_copy(name: &str) -> PathBuf {
    let source = Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("../..")
        .join(EXAMPLE);
    let target =
        std::env::temp_dir().join(format!("trust-control-setup-{name}-{}", std::process::id()));
    let _ = fs::remove_dir_all(&target);
    common::copy_dir(&source, &target);
    let _ = fs::remove_file(target.join(LOOP_FILE));
    target
}

/// The method's answer for `command` with `argument`, required to be a result.
fn result_of(studio: &ControlStudio, command: &str, argument: Value) -> Value {
    let answer = studio
        .execute(command, &[argument])
        .expect("the method answers");
    match answer.get("result") {
        Some(result) => result.clone(),
        None => panic!("{command} refused: {answer}"),
    }
}

/// The request of either method for `project`, with the compiler the test built.
fn request(project: &Path) -> Value {
    json!({
        "projectRoot": project.display().to_string(),
        "runtimeId": RUNTIME,
        "languageTool": common::trust_lsp().display().to_string(),
    })
}

/// The draft of the choices that the lesson's code states: the controller `P1.Controller` with its parameter record, its
/// measurement (its own `StateValue.ProcessValue`), its output and setpoint, the Auto/Manual switch and the manual output.
fn draft() -> Value {
    json!({
        "structure": "pid",
        "name": "Model-based PI tuning",
        "instance": "P1.Controller",
        "action": "direct",
        "parameterRecord": "P1.Parameters",
        "processVariable": {
            "symbol": "P1.Controller.StateValue.ProcessValue",
            "unit": "degC",
            "minimum": 0,
            "maximum": 80
        },
        "manipulatedVariable": {
            "symbol": "P1.AppliedPowerPct",
            "unit": "%",
            "minimum": 0,
            "maximum": 100
        },
        "setpoint": { "symbol": "P1.SetpointC" },
        "mode": { "symbol": "P1.AutoMode", "whenTrue": "Auto" },
        "manualOutput": "P1.ManualOutput",
        // The program initializes P1.Controller itself, behind `IF NOT Initialized` (B1.3, C1).
        "startupFlag": "P1.Initialized"
    })
}

#[test]
fn discovery_proposes_the_pid_structure_with_the_prefill_the_code_states() {
    let project = project_copy("discover");
    let studio = studio();
    let result = result_of(&studio, SETUP_DISCOVER, request(&project));
    let form = &result["formModel"];
    assert_eq!(form["structure"], "pid", "one library PID is the structure");
    assert_eq!(form["pids"][0]["instance"], "P1.Controller");
    assert_eq!(form["pids"][0]["parameterRecord"], "P1.Parameters");
    assert_eq!(form["pids"][0]["output"], "P1.AppliedPowerPct");
    let prefill = &form["prefill"];
    assert_eq!(prefill["instance"], "P1.Controller");
    assert_eq!(prefill["parameterRecord"], "P1.Parameters");
    assert_eq!(prefill["manipulatedVariable"], "P1.AppliedPowerPct");
    assert_eq!(prefill["setpoint"], "P1.SetpointC");
    assert_eq!(
        prefill["processValue"], "P1.Controller.StateValue.ProcessValue",
        "the PID's own measurement, whatever the program computed it from"
    );
    assert_eq!(result["loops"], json!([]), "the copy has no loop yet");
    assert_eq!(result["rejected"], json!([]));
    let _ = fs::remove_dir_all(&project);
}

#[test]
fn a_draft_writes_one_loop_file_that_the_loop_contract_accepts() {
    let project = project_copy("save");
    let studio = studio();
    let mut save = request(&project);
    save["draft"] = draft();
    let saved = result_of(&studio, SETUP_SAVE, save);
    assert_eq!(saved["loops"].as_array().map(Vec::len), Some(1));
    assert_eq!(saved["loops"][0]["id"], LOOP_ID);
    assert_eq!(saved["loops"][0]["file"], SAVED_FILE);

    let text = fs::read_to_string(project.join(SAVED_FILE)).expect("the loop file is written");
    let loop_definition = LoopDefinition::decode(&text).expect("the loop file decodes");
    assert_eq!(loop_definition.id, LOOP_ID);
    loop_definition
        .validate(&LoopScope {
            project_id: loop_definition.project_id.clone(),
            runtime_id: RUNTIME.to_string(),
        })
        .expect("the loop contract accepts the saved loop");

    let discovered = result_of(&studio, SETUP_DISCOVER, request(&project));
    assert_eq!(
        discovered["loops"][0]["loopId"], LOOP_ID,
        "the saved loop is discovered"
    );
    assert_eq!(discovered["rejected"], json!([]));
    let _ = fs::remove_dir_all(&project);
}

#[test]
fn saving_the_same_draft_again_is_refused_and_changes_nothing() {
    let project = project_copy("save-twice");
    let studio = studio();
    let mut save = request(&project);
    save["draft"] = draft();
    result_of(&studio, SETUP_SAVE, save.clone());
    let before = fs::read_to_string(project.join(SAVED_FILE)).expect("the first save wrote it");

    let answer = studio
        .execute(SETUP_SAVE, &[save])
        .expect("the method answers");
    assert!(
        answer.get("failure").is_some(),
        "the second save is refused: {answer}"
    );
    let after = fs::read_to_string(project.join(SAVED_FILE)).expect("the file is still there");
    assert_eq!(before, after, "the refused save leaves the file as it was");
    let _ = fs::remove_dir_all(&project);
}
