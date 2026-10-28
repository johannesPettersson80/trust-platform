//! The fixtures that the tests of the engine's methods (`commands_tests.rs`) and of the open answer (`open_action_tests.rs`) share: an
//! engine that ignores operation updates, the example project with its loop file, and the Identify section of the example loop.

use std::fs;

use serde_json::{json, Value};
use trust_control::experiment::ControlExperimentKind;
use trust_control::identification::identify_result::identify_signals;
use trust_control::identification::model_comparison::ComparisonPresentation;
use trust_control::loop_definition::LoopDefinition;
use trust_control::messages::OperationUpdate;
use trust_control::model::decode_general_model;
use trust_control::recording::EvidenceSource;
use trust_control::study::identify::{IdentifyMethod, IdentifySection};

use crate::operation::UpdateSink;

pub(crate) struct Ignore;

impl UpdateSink for Ignore {
    fn send(&self, _update: OperationUpdate) {}
}

pub(crate) const EXAMPLE: &str =
    include_str!("../../../examples/control-pid-commissioning/control-loop.json");

/// A project folder with the example loop, and the arguments of `open` and `page` for it.
pub(crate) fn example_project(name: &str) -> (std::path::PathBuf, LoopDefinition, Value) {
    let root = std::env::temp_dir().join(format!("trust-control-{name}-{}", std::process::id()));
    let _ = fs::remove_dir_all(&root);
    fs::create_dir_all(&root).expect("project");
    fs::write(root.join("control-loop.json"), EXAMPLE).expect("loop");
    let example = LoopDefinition::decode(EXAMPLE).expect("example");
    let request = json!({
        "projectRoot": root.display().to_string(),
        "runtimeId": example.runtime_id,
        "loopId": example.id,
        "storageFolder": root.join("storage").display().to_string(),
    });
    (root, example, request)
}

/// The estimation recording of the Identify sections the tests keep, and the validation recording of a qualified one.
const ESTIMATION: &str = "rec-00112233445566778899aabbccddeeff";
const VALIDATION: &str = "rec-ffeeddccbbaa99887766554433221100";

/// What the page shows of the fitted model `1 / (s + 1)`, as the identify fixtures of `identify_action_tests` show it.
fn presentation() -> ComparisonPresentation {
    trust_control::json::decode_value(json!({
        "equation": "G(s) = 1 / (1s + 1)",
        "units": { "input": "V", "output": "m", "gain": "m/V", "time": "s" },
        "parameters": [
            { "name": "Gain", "value": 1.0, "unit": "m/V" },
            { "name": "Time constant 1", "value": 1.0, "unit": "s" },
            { "name": "Dead time", "value": 0.0, "unit": "s" }
        ],
        "poles": [-1.0], "zeros": [], "delaySeconds": 0.0,
        "uncertainty": { "label": "Sample-resolution envelope from the estimation record; not statistical confidence" },
        "estimation": { "normalizedError": 0.1 }, "validation": {},
        "predictedResponse": [
            { "timeSeconds": 0.0, "measured": 0.0, "predicted": 0.0, "residual": 0.0 },
            { "timeSeconds": 2.0, "measured": 1.0, "predicted": 0.9, "residual": 0.1 }
        ],
        "validityRange": { "inputMinimum": 0.0, "inputMaximum": 1.0, "outputMinimum": 0.0, "outputMaximum": 1.0 },
        "warnings": []
    }))
    .expect("the presentation decodes")
}

/// The Identify section of the example loop: the fitted model `1 / (s + 1)` from a command in V to a position in m. A
/// `validated` section names a validation recording, as a model that Validate qualified does (B5.2), so it is not provisional.
pub(crate) fn identified_section(example: &LoopDefinition, validated: bool) -> IdentifySection {
    let channel = |id: &str, unit: &str| json!({ "id": id, "unit": unit, "operatingRange": { "minimum": -10.0, "maximum": 10.0 } });
    let (validation, evidence) = if validated {
        (
            "validated",
            json!(["sha256:recording", "sha256:validation"]),
        )
    } else {
        ("unvalidated", json!(["sha256:recording"]))
    };
    let model = decode_general_model(&json!({
        "kind": "transfer-function", "revision": "model:fitted:1", "source": "fitted",
        "validation": validation, "evidence": evidence, "delaySeconds": 0.0,
        "inputs": [channel("command", "V")], "outputs": [channel("position", "m")],
        "numerator": [1.0], "denominator": [1.0, 1.0]
    }))
    .expect("a fitted model");
    IdentifySection::new(
        IdentifyMethod::Live(ControlExperimentKind::MvStep),
        EvidenceSource::SimulatedPlant,
        identify_signals(example, ControlExperimentKind::MvStep),
        ESTIMATION,
        validated.then_some(VALIDATION),
        model,
        presentation(),
    )
}
