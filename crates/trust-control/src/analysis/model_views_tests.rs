//! The state structure (`stateStructure`) and the relative gain array (`relativeGain`), B9.9 and N4.5. Ported assertions name the
//! TypeScript test they come from.

use serde_json::{json, Value};

use super::{relative_gain_answer, state_structure_answer, RGA_SINGULAR, RGA_SUMMARY};
use crate::messages::RelativeGainStatus;
use crate::model::{decode_general_model, GeneralModel, GeneralStateStructure};

fn channel(id: &str, unit: &str) -> Value {
    json!({ "id": id, "unit": unit, "operatingRange": { "minimum": -0.05, "maximum": 0.05 } })
}

/// The cascade of `presents the state structure with the declared order ...` (control-studio-three-page-analysis.test.ts): two
/// tanks, the pump flow `pump-flow-deviation` in and the downstream level out, `a = [[−0.04, 0], [0.04, −0.04]]`, `b = [2, 0]`,
/// `c = [0, 1]`, `d = 0`. Its validation is `unvalidated`, because a validated model needs evidence.
fn cascade(c: &Value) -> GeneralModel {
    decode_general_model(&json!({
        "kind": "continuous-state-space", "revision": "cascade", "source": "provided", "validation": "unvalidated",
        "evidence": [], "delaySeconds": 0.0,
        "inputs": [channel("pump-flow-deviation", "m3/s")],
        "outputs": [channel("downstream-level-deviation", "m")],
        "states": [channel("upstream-level-deviation", "m"), channel("downstream-level-deviation", "m")],
        "a": [[-0.04, 0.0], [0.04, -0.04]], "b": [[2.0], [0.0]], "c": c, "d": [[0.0]],
    }))
    .expect("the cascade is valid")
}

fn close(actual: f64, expected: f64) -> bool {
    (actual - expected).abs() < 1e-12
}

fn close_rows(actual: &[Vec<f64>], expected: &[Vec<f64>]) -> bool {
    actual.len() == expected.len()
        && actual.iter().zip(expected).all(|(row, expected_row)| {
            row.len() == expected_row.len()
                && row.iter().zip(expected_row).all(|(a, e)| close(*a, *e))
        })
}

#[test]
fn the_state_structure_has_the_declared_order_the_ranks_out_of_the_state_count_and_the_matrices() {
    // Ported from `presents the state structure with the declared order, ranks out of the state count, matrices in one precision and
    // the scope` (control-studio-three-page-analysis.test.ts).
    let answer = state_structure_answer(&cascade(&json!([[0.0, 1.0]]))).expect("the structure");
    assert_eq!(
        answer.summary,
        vec!["Controllability rank 2 of 2; observability rank 2 of 2.".to_owned()]
    );
    assert_eq!(
        answer.order,
        "State order: upstream-level-deviation (m), downstream-level-deviation (m). Input order: pump-flow-deviation (m3/s). Output order: downstream-level-deviation (m)."
    );
    let names: Vec<&str> = answer
        .matrices
        .iter()
        .map(|matrix| matrix.name.as_str())
        .collect();
    assert_eq!(
        names,
        vec![
            "A",
            "B",
            "C",
            "D",
            "Controllability [B, AB, …]",
            "Observability [C; CA; …]"
        ]
    );
    assert!(close_rows(
        &answer.matrices[0].rows,
        &[vec![-0.04, 0.0], vec![0.04, -0.04]]
    ));
    assert!(close_rows(
        &answer.matrices[1].rows,
        &[vec![2.0], vec![0.0]]
    ));
    assert!(close_rows(&answer.matrices[2].rows, &[vec![0.0, 1.0]]));
    assert!(close_rows(&answer.matrices[3].rows, &[vec![0.0]]));
    assert!(close_rows(
        &answer.matrices[4].rows,
        &[vec![2.0, -0.08], vec![0.0, 0.08]]
    ));
    assert!(close_rows(
        &answer.matrices[5].rows,
        &[vec![0.0, 1.0], vec![0.04, -0.04]]
    ));
    assert!(
        answer.details[1].contains("local unconstrained model"),
        "{:?}",
        answer.details
    );
}

#[test]
fn an_observability_that_misses_a_state_has_rank_one_and_the_controllability_is_still_full() {
    // Ported from `calculates controllability and observability for the declared two-tank model` (control-studio-general-model.test.ts),
    // its second case with `c = [1, 0]`.
    let answer = state_structure_answer(&cascade(&json!([[1.0, 0.0]]))).expect("the structure");
    match answer.structure {
        GeneralStateStructure::Qualified {
            controllability,
            observability,
            ..
        } => {
            assert_eq!(controllability.rank, 2);
            assert_eq!(observability.rank, 1);
        }
        GeneralStateStructure::Unavailable { reason } => panic!("unavailable: {reason}"),
    }
    assert_eq!(
        answer.summary,
        vec!["Controllability rank 2 of 2; observability rank 1 of 2.".to_owned()]
    );
}

#[test]
fn a_transfer_function_has_no_state_structure() {
    let transfer = decode_general_model(&json!({
        "kind": "transfer-function", "revision": "tf", "source": "provided", "validation": "unvalidated",
        "evidence": [], "delaySeconds": 0.0,
        "inputs": [channel("u", "V")], "outputs": [channel("y", "m")],
        "numerator": [1.0], "denominator": [1.0, 1.0],
    }))
    .expect("the transfer function is valid");
    assert!(state_structure_answer(&transfer).is_err());
    assert!(relative_gain_answer(&transfer).is_err());
}

/// A two-input, two-output state-space model with `a = −I` and `c = I`, so that its steady-state gain is `b`.
fn square_gain(b: &Value) -> GeneralModel {
    decode_general_model(&json!({
        "kind": "continuous-state-space", "revision": "square", "source": "provided", "validation": "unvalidated",
        "evidence": [], "delaySeconds": 0.0,
        "inputs": [channel("u1", "V"), channel("u2", "V")], "outputs": [channel("y1", "m"), channel("y2", "m")],
        "states": [channel("x1", "m"), channel("x2", "m")],
        "a": [[-1.0, 0.0], [0.0, -1.0]], "b": b, "c": [[1.0, 0.0], [0.0, 1.0]], "d": [[0.0, 0.0], [0.0, 0.0]],
    }))
    .expect("the square model is valid")
}

#[test]
fn the_relative_gain_array_of_a_square_gain_has_its_closed_form() {
    // Ported from `qualifies square DC relative gain` (control-studio-general-model.test.ts), with two channels: K = [[2, 1], [1, 2]]
    // gives Λ = K ∘ (K⁻¹)ᵀ = [[4/3, −1/3], [−1/3, 4/3]]. The one-channel case of that test is no longer offered (B9.9).
    let answer = relative_gain_answer(&square_gain(&json!([[2.0, 1.0], [1.0, 2.0]])))
        .expect("the array qualifies");
    match answer.status {
        RelativeGainStatus::Qualified { matrix } => assert!(close_rows(
            &matrix,
            &[vec![4.0 / 3.0, -1.0 / 3.0], vec![-1.0 / 3.0, 4.0 / 3.0]]
        )),
        RelativeGainStatus::Unavailable { reason } => panic!("unavailable: {reason}"),
    }
    assert_eq!(answer.summary, vec![RGA_SUMMARY.to_owned()]);
    assert_eq!(answer.input_channels[0].id, "u1");
    assert_eq!(answer.output_channels[1].unit, "m");
}

#[test]
fn a_singular_steady_state_gain_makes_the_array_unavailable_with_its_reason() {
    // N4.5: a singular K is refused, so the array is unavailable, not a number. The reason is the page's words.
    let answer = relative_gain_answer(&square_gain(&json!([[1.0, 1.0], [1.0, 1.0]])))
        .expect("the answer names the array as unavailable");
    match answer.status {
        RelativeGainStatus::Unavailable { reason } => assert_eq!(reason, RGA_SINGULAR),
        RelativeGainStatus::Qualified { .. } => panic!("a singular gain has no array"),
    }
    assert_eq!(answer.summary, vec![RGA_SINGULAR.to_owned()]);
}

#[test]
fn a_single_channel_model_has_no_relative_gain_array() {
    let single = decode_general_model(&json!({
        "kind": "continuous-state-space", "revision": "single", "source": "provided", "validation": "unvalidated",
        "evidence": [], "delaySeconds": 0.0,
        "inputs": [channel("u", "V")], "outputs": [channel("y", "m")],
        "states": [channel("x", "m")], "a": [[-1.0]], "b": [[1.0]], "c": [[1.0]], "d": [[0.0]],
    }))
    .expect("the single-channel model is valid");
    assert!(relative_gain_answer(&single).is_err());
}
