//! The routing of `controlStudio/preview` (specification 35, §0.1a and B4.5): the engine answers it as a method, at once and without
//! an operation; it claims no loop and keeps nothing. The design is trust-control's (`study::tune::preview`), tested there. A
//! preview of a study with a current model needs an Identify result, which these tests do not build, so the previews here are
//! refused for their missing model (B4.7) or before the design starts.

use std::fs;
use std::path::PathBuf;
use std::sync::Arc;

use serde_json::{json, Value};
use trust_control::loop_definition::LoopDefinition;
use trust_control::messages::OperationUpdate;

use crate::commands::{ControlStudio, METHODS, PAGE, PREVIEW};
use crate::operation::{Operations, UpdateSink};
use crate::storage;

/// The example loop of the engine's tests.
const EXAMPLE: &str = include_str!("../../../examples/control-pid-commissioning/control-loop.json");

/// The sink of an engine whose operation updates nobody receives.
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
        let root =
            std::env::temp_dir().join(format!("trust-preview-{name}-{}", std::process::id()));
        let _ = fs::remove_dir_all(&root);
        fs::create_dir_all(&root).expect("the project folder");
        fs::write(root.join("control-loop.json"), EXAMPLE).expect("the loop file");
        let loop_definition = LoopDefinition::decode(EXAMPLE).expect("the example loop");
        Self {
            root,
            loop_definition,
        }
    }

    /// The arguments that name the example loop; a preview adds its settings to them.
    fn arguments(&self) -> Value {
        json!({
            "projectRoot": self.root.display().to_string(),
            "runtimeId": self.loop_definition.runtime_id,
            "loopId": self.loop_definition.id,
        })
    }

    /// The study file of the example loop, which exists once a study has been saved.
    fn study_file(&self) -> PathBuf {
        storage::study_directory(&self.root, &self.loop_definition.id).join("study.json")
    }
}

/// Whether no operation was started: the engine numbers its operations from 1.
fn nothing_started(operations: &Operations) -> bool {
    operations.status("operation-1").is_err()
}

#[test]
fn preview_is_a_method_of_the_engine_and_a_malformed_request_is_refused_as_invalid_arguments() {
    assert!(METHODS.contains(&PREVIEW), "the engine answers {PREVIEW}");
    let studio = ControlStudio::new(Arc::new(Ignore));
    let mut with_method = json!({
        "projectRoot": "/x",
        "runtimeId": "simulator",
        "loopId": "tank",
        "settings": { "Kp": 2.5 },
    });
    with_method["method"] = json!("simc");
    let not_numbers = json!({
        "projectRoot": "/x",
        "runtimeId": "simulator",
        "loopId": "tank",
        "settings": { "Kp": "two" },
    });
    for request in [json!({ "projectRoot": "/x" }), with_method, not_numbers] {
        let reply = studio
            .execute(PREVIEW, &[request])
            .expect("the engine answers its own method");
        assert!(
            reply["failure"]["message"]
                .as_str()
                .is_some_and(|message| message.contains("invalid arguments")),
            "{reply}"
        );
        assert!(reply.get("result").is_none(), "{reply}");
    }
    assert!(nothing_started(studio.operations()), "nothing was started");
}

#[test]
fn preview_without_a_model_is_refused_at_once_and_leaves_the_study_file_unchanged() {
    let project = Project::new("preview-no-model");
    let studio = ControlStudio::new(Arc::new(Ignore));
    // Setting the page saves the study, so that there is a study file to compare after the preview.
    let mut page = project.arguments();
    page["page"] = json!("Tune");
    studio.execute(PAGE, &[page]).expect("the engine answers");
    let before = fs::read(project.study_file()).expect("the saved study");
    let mut request = project.arguments();
    request["settings"] = json!({ "Kp": 2.5, "Ki": 0.5 });
    let reply = studio
        .execute(PREVIEW, &[request])
        .expect("the engine answers its own method");
    assert_eq!(reply["failure"]["nextAction"], "identify", "{reply}");
    assert!(reply.get("result").is_none(), "{reply}");
    assert!(nothing_started(studio.operations()), "nothing was started");
    let loop_definition = &project.loop_definition;
    let _free = studio
        .loop_claims()
        .claim(&loop_definition.project_id, &loop_definition.id, "Tune")
        .expect("the preview claimed no loop");
    assert_eq!(
        fs::read(project.study_file()).expect("the saved study"),
        before,
        "the preview left the study file as it was"
    );
}

#[test]
fn preview_of_a_loop_the_project_does_not_declare_is_refused_to_open_the_loop_setup() {
    let project = Project::new("preview-unknown-loop");
    let studio = ControlStudio::new(Arc::new(Ignore));
    let mut request = project.arguments();
    request["loopId"] = json!("no-such-loop");
    request["settings"] = json!({ "Kp": 2.5 });
    let reply = studio
        .execute(PREVIEW, &[request])
        .expect("the engine answers its own method");
    assert_eq!(reply["failure"]["nextAction"], "open-loop-setup", "{reply}");
    assert!(!project.study_file().exists(), "a refusal writes nothing");
}
