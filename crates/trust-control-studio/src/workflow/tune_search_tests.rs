//! Specification 35, B4.3 and C5: a finite search designs to the requirements the study holds and, while it holds none, to the
//! record Automatic fills from the model, which the Identify answer names: one record by one rule, as every Tune designs to it.

use std::fs;
use std::path::PathBuf;
use std::sync::Arc;

use serde_json::{json, Value};
use trust_control::loop_definition::LoopDefinition;
use trust_control::messages::{OperationUpdate, TuneRequest};
use trust_control::tuning::requirements::TuningRequirements;

use super::search_work;
use crate::commands::{ControlStudio, IDENTIFY};
use crate::operation::UpdateSink;

/// Lesson 89's loop: a program's PID whose notch Tune searches (C5).
const SEARCH_LOOP: &str = include_str!(
    "../../../../examples/control/07-classical-stability-and-frequency-design/89-mechanical-resonance/control-loop.json"
);

/// A supplied plant on the loop's signals that settles: 0.05 rad per N·m with a 2 s lag and a 0.1 s delay, at 0 N·m and 0 rad.
const SETTLING_PLANT: &str = r#"{
  "kind": "transfer-function",
  "source": "provided",
  "validation": "unvalidated",
  "evidence": [],
  "delaySeconds": 0.1,
  "inputs": [{ "id": "P1.TorqueNm", "unit": "N·m", "operatingRange": { "minimum": -20, "maximum": 20 } }],
  "outputs": [{ "id": "P1.AngleRad", "unit": "rad", "operatingRange": { "minimum": -2, "maximum": 2 } }],
  "numerator": [0.05],
  "denominator": [2, 1],
  "operatingPoint": { "inputs": [0], "outputs": [0] },
  "revision": "settling-plant"
}"#;

/// The sink of an engine whose operations nobody follows.
struct Ignore;

impl UpdateSink for Ignore {
    fn send(&self, _update: OperationUpdate) {}
}

/// A project folder with the search loop, removed when the test ends.
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
        let root = trust_test_support::unique_temp_path(&format!("trust-search-{name}"));
        fs::create_dir_all(&root).expect("the project folder");
        fs::write(root.join("control-loop.json"), SEARCH_LOOP).expect("the loop file");
        let loop_definition = LoopDefinition::decode(SEARCH_LOOP).expect("the loop file decodes");
        Self {
            root,
            loop_definition,
        }
    }

    /// The arguments of the loop's requests.
    fn request(&self) -> Value {
        json!({
            "projectRoot": self.root.display().to_string(),
            "runtimeId": self.loop_definition.runtime_id,
            "loopId": self.loop_definition.id,
            "storageFolder": self.root.join("storage").display().to_string(),
        })
    }
}

/// Catches the search refusing a study that holds no requirements ("The finite search needs the requirements"), which left the
/// command line, whose verbs do not set requirements, no way to tune such a loop; and a search that designs to another record than
/// the one the Identify answer names.
#[test]
fn a_search_without_declared_requirements_designs_to_the_record_automatic_fills() {
    let project = Project::new("automatic-requirements");
    let studio = ControlStudio::new(Arc::new(Ignore));
    let mut import = project.request();
    import["action"] = json!({ "action": "importModel", "json": SETTLING_PLANT });
    let imported = studio
        .execute(IDENTIFY, &[import])
        .expect("the engine answers its own method");
    let named: TuningRequirements =
        trust_control::json::decode_value(imported["result"]["requirements"].clone())
            .unwrap_or_else(|error| {
                panic!("the Identify answer names the requirements ({error}): {imported}")
            });
    // A connection that names a port nothing listens on: the search reads the runtime only once it starts.
    let mut tune = project.request();
    tune["connection"] = json!({ "endpoint": "tcp://127.0.0.1:9", "token": "control-token" });
    let request: TuneRequest = trust_control::json::decode_value(tune).expect("a Tune request");

    let work = match search_work(&request, project.loop_definition.clone()) {
        Ok(work) => work,
        Err(failure) => panic!("the search designs without declared requirements: {failure:?}"),
    };
    assert_eq!(work.requirements, named);
    let criteria = work
        .requirements
        .automatic_criteria
        .as_ref()
        .expect("Automatic filled the test");
    assert_eq!(criteria.model_revision, "settling-plant");
    for field in ["referenceChange", "settlingTimeSeconds", "durationSeconds"] {
        assert!(
            criteria.fields.iter().any(|filled| filled == field),
            "{field} is Automatic's: {criteria:?}"
        );
    }
}
