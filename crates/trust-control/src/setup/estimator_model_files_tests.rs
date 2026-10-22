//! Tests of the model files an LQG estimator can implement (specification 35, C4 Setup; `estimatorModelFiles.ts`). They were part of the
//! discovery tests.

use serde_json::{json, Value};

use super::{estimator_model_files, EstimatorModelFile, ModelFileText};
use crate::setup::discovery::LoopFileRead;

/// A discrete state-space model text with `states` states and `inputs` inputs and one output, as a project's model file holds it.
fn model_text(states: usize, inputs: usize) -> String {
    let channel = |id: String| json!({ "id": id, "unit": "m", "operatingRange": { "minimum": -1.0, "maximum": 1.0 } });
    let a: Vec<Vec<f64>> = (0..states)
        .map(|row| {
            (0..states)
                .map(|column| if row == column { 0.5 } else { 0.0 })
                .collect()
        })
        .collect();
    let b: Vec<Vec<f64>> = (0..states).map(|_| vec![0.1; inputs]).collect();
    json!({
        "kind": "discrete-state-space", "revision": "model", "source": "provided",
        "validation": "validated", "evidence": ["lesson"], "delaySeconds": 0, "sampleTimeSeconds": 1,
        "inputs": (0..inputs).map(|index| channel(format!("u{index}"))).collect::<Vec<Value>>(),
        "outputs": [channel("y".to_owned())],
        "states": (0..states).map(|index| channel(format!("x{index}"))).collect::<Vec<Value>>(),
        "a": a, "b": b, "c": [vec![1.0; states]], "d": [vec![0.0; inputs]],
        "operatingPoint": { "inputs": vec![0.0; inputs], "outputs": [0.0] },
    })
    .to_string()
}

#[test]
fn the_estimator_model_files_are_state_space_models_with_one_input_one_output_and_three_states_or_fewer(
) {
    // estimatorModelFiles.ts: a state-space model with one input, one output and at most three states, in the order the files are given.
    // A file that is not a model, has two inputs or three states too many, or cannot be read is left out.
    let files = vec![
        ModelFileText {
            file_name: "lqg-model.json".to_owned(),
            read: LoopFileRead::Text(model_text(2, 1)),
        },
        ModelFileText {
            file_name: "notes.json".to_owned(),
            read: LoopFileRead::Text(json!({ "title": "not a model" }).to_string()),
        },
        ModelFileText {
            file_name: "two-inputs.json".to_owned(),
            read: LoopFileRead::Text(model_text(2, 2)),
        },
        ModelFileText {
            file_name: "four-states.json".to_owned(),
            read: LoopFileRead::Text(model_text(4, 1)),
        },
        ModelFileText {
            file_name: "unread.json".to_owned(),
            read: LoopFileRead::Unreadable,
        },
        ModelFileText {
            file_name: "models/tanks.json".to_owned(),
            read: LoopFileRead::Text(model_text(3, 1)),
        },
    ];
    assert_eq!(
        estimator_model_files(&files),
        vec![
            EstimatorModelFile {
                file: "lqg-model.json".to_owned(),
                states: 2
            },
            EstimatorModelFile {
                file: "models/tanks.json".to_owned(),
                states: 3
            },
        ]
    );
}
