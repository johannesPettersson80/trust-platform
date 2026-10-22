//! The engine's decisions on the loop files of the workspace folders (`project_loops_action.rs`): each file's state and open request,
//! the folder that declares a loop, and the loops of an equipment.

use std::fs;
use std::path::{Path, PathBuf};

use serde_json::{json, Value};
use trust_control::setup::messages::{LoopIdentity, ProjectLoopsRequest};

use super::answer;

/// The example loop of the course's PID commissioning: id `control-pid-commissioning`, runtime `simulator`, controller `P1.Controller`.
const EXAMPLE: &str = include_str!("../../../examples/control-pid-commissioning/control-loop.json");
const LOOP_ID: &str = "control-pid-commissioning";
const PROJECT_ID: &str = "project:control-pid-commissioning";

fn project(name: &str) -> PathBuf {
    let root = std::env::temp_dir().join(format!(
        "trust-control-project-loops-{name}-{}",
        std::process::id()
    ));
    let _ = fs::remove_dir_all(&root);
    fs::create_dir_all(root.join("control-loops")).expect("project folder");
    root
}

/// The example loop with the fields a test changes.
fn loop_text(change: impl FnOnce(&mut Value)) -> String {
    let mut value: Value = serde_json::from_str(EXAMPLE).expect("the example loop");
    change(&mut value);
    serde_json::to_string_pretty(&value).expect("json")
}

fn request(roots: &[&Path], runtime_id: &str) -> ProjectLoopsRequest {
    ProjectLoopsRequest {
        project_roots: roots
            .iter()
            .map(|root| root.display().to_string())
            .collect(),
        runtime_id: runtime_id.to_string(),
        equipment: None,
        loop_identity: None,
    }
}

fn answered(request: &ProjectLoopsRequest) -> Value {
    answer(request).expect("an answer")
}

#[test]
fn a_folder_lists_its_root_loop_then_its_folder_loops_each_decided_with_its_open_request() {
    let root = project("order");
    fs::write(root.join("control-loop.json"), loop_text(|_| {})).expect("root loop");
    fs::write(root.join("control-loops/b-broken.json"), "{ not json").expect("broken loop");

    let value = answered(&request(&[&root], "simulator"));
    let resolutions = &value["folders"][0]["resolutions"];
    assert_eq!(resolutions[0]["state"], "accepted");
    assert_eq!(resolutions[0]["request"]["schemaVersion"], 1);
    assert_eq!(resolutions[0]["request"]["source"], "ConfiguredLoop");
    assert_eq!(resolutions[0]["request"]["scope"]["projectId"], PROJECT_ID);
    assert_eq!(resolutions[0]["request"]["scope"]["runtimeId"], "simulator");
    assert_eq!(resolutions[0]["request"]["loopDefinition"]["id"], LOOP_ID);
    assert!(resolutions[0].get("reason").is_none());
    assert_eq!(resolutions[1]["state"], "invalid");
    assert!(resolutions[1]["configurationPath"]
        .as_str()
        .is_some_and(|path| path.ends_with("b-broken.json")));
    assert!(resolutions[1].get("request").is_none());
}

#[test]
fn a_folder_with_no_loop_file_has_one_missing_resolution_for_its_root_file() {
    let root = project("missing");
    let value = answered(&request(&[&root], "simulator"));
    let resolutions = &value["folders"][0]["resolutions"];
    assert_eq!(resolutions.as_array().map(Vec::len), Some(1));
    assert_eq!(resolutions[0]["state"], "missing");
    assert!(resolutions[0]["configurationPath"]
        .as_str()
        .is_some_and(|path| path.ends_with("control-loop.json")));
    assert_eq!(
        resolutions[0]["reason"],
        "This project has no loop definition in control-loop.json or control-loops/. Configure a loop to bind its signals."
    );
}

#[test]
fn a_loop_of_another_runtime_is_refused_naming_the_selected_target() {
    let root = project("runtime");
    fs::write(
        root.join("control-loops/tank.json"),
        loop_text(|value| value["runtimeId"] = json!("other-runtime")),
    )
    .expect("loop of another runtime");

    let value = answered(&request(&[&root], "simulator"));
    let resolution = &value["folders"][0]["resolutions"][0];
    assert_eq!(resolution["state"], "invalid");
    let reason = resolution["reason"].as_str().expect("a reason");
    assert!(reason.contains("the loop uses other-runtime"), "{reason}");
    assert!(
        reason.contains("but simulator is the selected target"),
        "{reason}"
    );
}

#[test]
fn two_files_declaring_one_loop_identifier_are_both_refused_as_duplicates() {
    let root = project("duplicate");
    fs::write(root.join("control-loop.json"), loop_text(|_| {})).expect("root loop");
    fs::write(root.join("control-loops/copy.json"), loop_text(|_| {})).expect("copy");

    let value = answered(&request(&[&root], "simulator"));
    let resolutions = &value["folders"][0]["resolutions"];
    for resolution in resolutions.as_array().expect("resolutions") {
        assert_eq!(resolution["state"], "invalid");
        assert!(resolution["reason"]
            .as_str()
            .is_some_and(|reason| reason.starts_with(&format!(
                "Duplicate control loop ID {LOOP_ID} in this project."
            ))));
    }
}

#[test]
fn the_declaring_folder_is_the_one_whose_loop_has_the_revision_else_the_first_with_the_loop() {
    let first = project("declare-first");
    let second = project("declare-second");
    fs::write(
        first.join("control-loop.json"),
        loop_text(|value| value["revision"] = json!("sha256:first")),
    )
    .expect("first folder");
    fs::write(
        second.join("control-loops/tank.json"),
        loop_text(|value| value["revision"] = json!("sha256:second")),
    )
    .expect("second folder");
    let folders = [first.as_path(), second.as_path()];
    let identity = |revision: &str| LoopIdentity {
        id: LOOP_ID.to_string(),
        project_id: PROJECT_ID.to_string(),
        revision: revision.to_string(),
    };

    let mut by_revision = request(&folders, "simulator");
    by_revision.loop_identity = Some(identity("sha256:second"));
    assert_eq!(
        answered(&by_revision)["declaringRoot"],
        second.display().to_string()
    );

    let mut other_revision = request(&folders, "simulator");
    other_revision.loop_identity = Some(identity("sha256:third"));
    assert_eq!(
        answered(&other_revision)["declaringRoot"],
        first.display().to_string()
    );

    let mut unknown = request(&folders, "simulator");
    unknown.loop_identity = Some(LoopIdentity {
        id: "loop:elsewhere".to_string(),
        project_id: PROJECT_ID.to_string(),
        revision: "sha256:first".to_string(),
    });
    assert!(answered(&unknown).get("declaringRoot").is_none());
}

#[test]
fn an_equipment_gets_the_accepted_loops_of_its_controller_or_the_reason_there_are_none() {
    let root = project("equipment");
    fs::write(root.join("control-loop.json"), loop_text(|_| {})).expect("root loop");

    let mut found = request(&[&root], "simulator");
    found.equipment = Some("p1".to_string());
    let value = answered(&found);
    assert_eq!(
        value["equipment"]["requests"].as_array().map(Vec::len),
        Some(1)
    );
    assert_eq!(
        value["equipment"]["requests"][0]["loopDefinition"]["id"],
        LOOP_ID
    );
    assert!(value["equipment"].get("reason").is_none());

    let mut none = request(&[&root], "simulator");
    none.equipment = Some("P2".to_string());
    let value = answered(&none);
    assert_eq!(value["equipment"]["requests"], json!([]));
    assert_eq!(
        value["equipment"]["reason"],
        "No configured Control Studio loop matches P2 on the selected runtime. Configure its actual controller signals first."
    );
}

#[test]
fn an_equipment_with_no_match_gives_the_reason_of_a_refused_loop_file_first() {
    let root = project("equipment-refused");
    fs::write(root.join("control-loops/broken.json"), "{ not json").expect("broken loop");

    let mut none = request(&[&root], "simulator");
    none.equipment = Some("P2".to_string());
    let value = answered(&none);
    assert_eq!(value["equipment"]["requests"], json!([]));
    assert!(
        value["equipment"]["reason"]
            .as_str()
            .is_some_and(|reason| reason.starts_with("broken.json")),
        "{value}"
    );
}

/// The binding revision the example loop declares.
fn example_revision() -> String {
    let value: Value = serde_json::from_str(EXAMPLE).expect("the example loop");
    value["revision"].as_str().expect("a revision").to_string()
}

fn identity(id: &str, project_id: &str, revision: &str) -> LoopIdentity {
    LoopIdentity {
        id: id.to_string(),
        project_id: project_id.to_string(),
        revision: revision.to_string(),
    }
}

#[test]
fn the_loop_an_identity_names_is_answered_with_its_accepted_file_and_no_failure() {
    let root = project("identity-accepted");
    fs::write(root.join("control-loop.json"), loop_text(|_| {})).expect("root loop");

    let mut asked = request(&[&root], "simulator");
    asked.loop_identity = Some(identity(LOOP_ID, PROJECT_ID, &example_revision()));
    let value = answered(&asked);
    let resolution = &value["resolution"];
    assert_eq!(resolution["state"], "accepted");
    assert_eq!(resolution["request"]["loopDefinition"]["id"], LOOP_ID);
    assert!(resolution.get("failure").is_none(), "{value}");
    assert_eq!(value["declaringRoot"], root.display().to_string());
    assert!(
        value.get("open").is_none(),
        "an identity names no workspace opening"
    );
}

#[test]
fn a_loop_no_file_declares_is_missing_with_its_text_and_the_open_loop_setup_next_action() {
    let root = project("identity-missing");
    fs::write(root.join("control-loop.json"), loop_text(|_| {})).expect("root loop");

    let mut asked = request(&[&root], "simulator");
    asked.loop_identity = Some(identity("other-loop", PROJECT_ID, &example_revision()));
    let value = answered(&asked);
    assert!(value.get("declaringRoot").is_none(), "{value}");
    let resolution = &value["resolution"];
    assert_eq!(resolution["state"], "missing");
    assert!(
        resolution["reason"].as_str().is_some_and(
            |reason| reason.starts_with("This project declares no usable loop other-loop.")
        ),
        "{value}"
    );
    assert_eq!(resolution["failure"]["nextAction"], "open-loop-setup");
    assert!(resolution.get("request").is_none());
}

#[test]
fn a_refused_file_that_declares_the_identity_is_its_decision_with_its_failure() {
    let root = project("identity-refused");
    fs::write(
        root.join("control-loops/tank.json"),
        loop_text(|value| value["runtimeId"] = json!("other-runtime")),
    )
    .expect("loop of another runtime");

    let mut asked = request(&[&root], "simulator");
    asked.loop_identity = Some(identity(LOOP_ID, PROJECT_ID, &example_revision()));
    let value = answered(&asked);
    assert!(value.get("declaringRoot").is_none(), "{value}");
    let resolution = &value["resolution"];
    assert_eq!(resolution["state"], "invalid");
    assert!(
        resolution["reason"]
            .as_str()
            .is_some_and(|reason| reason.contains("but simulator is the selected target")),
        "{value}"
    );
    assert_eq!(resolution["failure"]["cause"], "implementation");
    assert_eq!(resolution["failure"]["nextAction"], "open-loop-setup");
}

#[test]
fn a_workspace_with_one_configured_loop_opens_it_and_offers_no_choice() {
    let root = project("open-one");
    fs::write(root.join("control-loop.json"), loop_text(|_| {})).expect("root loop");

    let value = answered(&request(&[&root], "simulator"));
    assert_eq!(value["open"]["resolution"]["state"], "accepted");
    assert_eq!(value["open"]["projectRoot"], root.display().to_string());
    assert!(value.get("choices").is_none(), "{value}");
}

#[test]
fn a_workspace_with_several_configured_loops_offers_them_as_choices_and_opens_none() {
    let root = project("choices");
    fs::write(root.join("control-loop.json"), loop_text(|_| {})).expect("root loop");
    fs::write(
        root.join("control-loops/second.json"),
        loop_text(|value| {
            value["id"] = json!("second-loop");
            value["parameterBinding"]["loop_id"] = json!("second-loop");
        }),
    )
    .expect("second loop");

    let value = answered(&request(&[&root], "simulator"));
    assert!(value.get("open").is_none(), "{value}");
    let choices = value["choices"].as_array().expect("the choices");
    assert_eq!(choices.len(), 2, "{value}");
    assert_eq!(choices[1]["resolution"]["state"], "accepted");
    assert_eq!(
        choices[1]["resolution"]["request"]["loopDefinition"]["id"],
        "second-loop"
    );
}

#[test]
fn a_workspace_with_no_configured_loop_opens_its_first_folders_missing_decision() {
    let root = project("open-none");
    let value = answered(&request(&[&root], "simulator"));
    assert_eq!(value["open"]["resolution"]["state"], "missing");
    assert_eq!(
        value["open"]["resolution"]["reason"],
        "This project has no loop definition in control-loop.json or control-loops/. Configure a loop to bind its signals."
    );
    assert!(value.get("choices").is_none(), "{value}");
}

#[test]
fn a_workspace_with_no_project_folder_opens_the_sentence_that_asks_for_one() {
    let value = answered(&request(&[], "simulator"));
    assert_eq!(value["open"]["resolution"]["state"], "missing");
    assert_eq!(
        value["open"]["resolution"]["reason"],
        "Open a truST project before opening Control Studio."
    );
    assert_eq!(value["open"]["resolution"]["failure"]["nextAction"], "none");
    assert_eq!(value["open"]["projectRoot"], "");
}
