//! Tests of the stored presentation of a fitted model (`modelComparison.ts`): what the page shows must describe its model.

#![allow(clippy::cast_precision_loss)] // Sample indices are small counts, far below 2^53.

use serde_json::{json, Value};

use super::{presentation_describes_model, ComparisonPresentation};
use crate::identification::record_fixtures::fitted_model;
use crate::model::GeneralModel;

/// The presentation of the fitted model `fitted_model(vec![], [2], [3, 1])`, as the study keeps it, with `points` predicted
/// points.
fn presentation(points: usize) -> Value {
    let predicted: Vec<Value> = (0..points)
        .map(|index| {
            let time = index as f64 * 0.1;
            json!({ "timeSeconds": time, "measured": time, "predicted": time, "residual": 0.0 })
        })
        .collect();
    json!({
        "equation": "G(s) = 2 / (3s + 1)",
        "units": { "input": "%", "output": "degC", "gain": "degC/%", "time": "s" },
        "parameters": [{ "name": "gain", "value": 2.0, "unit": "degC/%" }],
        "poles": [-0.333],
        "zeros": [],
        "delaySeconds": 0.0,
        "uncertainty": { "label": "Provisional" },
        "estimation": { "normalizedError": 0.01 },
        "validation": {},
        "predictedResponse": predicted,
        "validityRange": { "inputMinimum": 0.0, "inputMaximum": 100.0, "outputMinimum": 0.0, "outputMaximum": 100.0 },
        "warnings": []
    })
}

fn decoded(value: &Value) -> ComparisonPresentation {
    crate::json::decode_value(value.clone()).expect("the presentation decodes")
}

fn model() -> GeneralModel {
    fitted_model(Vec::new(), vec![2.0], vec![3.0, 1.0])
}

#[test]
fn a_presentation_of_its_own_model_describes_it() {
    let shown = decoded(&presentation(3));
    assert_eq!(shown.presentation.equation, "G(s) = 2 / (3s + 1)");
    assert_eq!(shown.output_id, None);
    assert!(presentation_describes_model(&shown, &model()));
}

#[test]
fn the_presentation_must_agree_with_the_model_units_delay_and_points() {
    let mut wrong_unit = presentation(3);
    wrong_unit["units"]["output"] = json!("%");
    assert!(
        !presentation_describes_model(&decoded(&wrong_unit), &model()),
        "the output unit is the model's"
    );
    let mut negative_delay = presentation(3);
    negative_delay["delaySeconds"] = json!(-0.1);
    assert!(
        !presentation_describes_model(&decoded(&negative_delay), &model()),
        "a delay is not negative"
    );
    assert!(
        !presentation_describes_model(&decoded(&presentation(1)), &model()),
        "a response has two points at least"
    );
    let mut unordered = presentation(3);
    unordered["predictedResponse"][2]["timeSeconds"] = json!(0.05);
    assert!(
        !presentation_describes_model(&decoded(&unordered), &model()),
        "the times increase"
    );
}

#[test]
fn the_traces_of_an_experiment_must_match_the_model_channels() {
    let mut traced = presentation(3);
    traced["experiment"] = json!({
        "recordingId": "experiment",
        "inputs": [{ "id": "MV", "unit": "%", "points": [] }],
        "outputs": [{ "id": "PV", "unit": "degC", "points": [] }]
    });
    assert!(presentation_describes_model(&decoded(&traced), &model()));
    traced["experiment"]["outputs"][0]["unit"] = json!("%");
    assert!(
        !presentation_describes_model(&decoded(&traced), &model()),
        "the traces keep the model's units"
    );
}
