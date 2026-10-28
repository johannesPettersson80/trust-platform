//! The state-space and piecewise-linear editors' rules (specification 47 §9; specification 46
//! §5.1.1, §7.6, §7.6.1, §7.7).

use serde_json::json;

use super::{draft, found};
use crate::control::plant_handlers::editor::{piecewise, state_space};

fn state_space_draft(
    parameters: serde_json::Value,
    initial_state: serde_json::Value,
) -> serde_json::Map<String, serde_json::Value> {
    draft(json!({"model": "state_space", "parameters": parameters, "initialState": initial_state}))
}

#[test]
fn the_dimensions_follow_the_matrices() {
    // Specification 47 §9: states from A's rows, inputs from B's columns, outputs from C's rows,
    // each at least 1. Catches inputs counted from B's rows.
    let parameters = json!({"a": [[0, 1], [-1, 0]], "b": [[0], [1]], "c": [[1, 0]], "d": [[0]]});
    assert_eq!(
        state_space::dimensions(parameters.as_object().unwrap()),
        state_space::Dimensions {
            states: 2,
            inputs: 1,
            outputs: 1
        }
    );
    assert_eq!(
        state_space::dimensions(json!({}).as_object().unwrap()),
        state_space::Dimensions {
            states: 1,
            inputs: 1,
            outputs: 1
        },
        "an empty model still has one of each"
    );
}

#[test]
fn every_matrix_cell_and_starting_state_is_checked_at_its_place() {
    // Specification 47 §9: the editor identifies the exact invalid cell. Catches a shape or a
    // non-finite cell reported without its cell, or not at all.
    let valid = json!({"a": [[0, 1], [-1, 0]], "b": [[0], [1]], "c": [[1, 0]], "d": [[0]]});
    assert!(state_space::cells(&state_space_draft(valid.clone(), json!([0, 0]))).is_empty());
    let mut wrong_shape = valid.clone();
    wrong_shape["b"] = json!([[1]]);
    assert_eq!(
        found(&state_space::cells(&state_space_draft(
            wrong_shape,
            json!([0, 0])
        ))),
        [("parameters.b".to_string(), "B must be 2 × 1.".to_string())]
    );
    let mut nan_cell = valid.clone();
    nan_cell["a"] = json!([[0, null], [-1, 0]]);
    assert_eq!(
        found(&state_space::cells(&state_space_draft(
            nan_cell,
            json!([0, null])
        ))),
        [
            (
                "parameters.a[0][1]".to_string(),
                "A[1,2] must be a finite number.".to_string()
            ),
            (
                "initial_state[1]".to_string(),
                "Initial state[2] must be a finite number.".to_string()
            ),
        ]
    );
    assert_eq!(
        found(&state_space::cells(&state_space_draft(valid, json!([0])))),
        [(
            "initial_state".to_string(),
            "Initial state must contain 2 values.".to_string()
        )]
    );
}

fn delays(
    delays: serde_json::Value,
    step_ms: Option<serde_json::Value>,
    project: Option<f64>,
) -> Vec<(String, String)> {
    let mut value = json!({
        "model": "state_space",
        "parameters": {"a": [[0]], "b": [[1]], "c": [[1]], "d": [[0]], "input_delays_s": delays},
        "initialState": [0],
    });
    if let Some(step) = step_ms {
        value["stepMs"] = step;
    }
    found(&state_space::input_delays(&draft(value), project))
}

#[test]
fn input_delays_hold_one_valid_delay_per_input() {
    // Specification 46 §7.6.1: one finite, nonnegative delay in seconds per input. Catches a delay
    // vector of the wrong length, or a negative or unreadable delay, accepted.
    assert_eq!(
        delays(json!([0, 1]), None, Some(10.0)),
        [(
            "parameters.input_delays_s".to_string(),
            "Input delays need exactly 1 values, one per input.".to_string()
        )]
    );
    assert_eq!(
        delays(json!([-1]), None, Some(10.0)),
        [(
            "parameters.input_delays_s[0]".to_string(),
            "Input 1 delay must be 0 s or greater.".to_string()
        )]
    );
    assert_eq!(
        delays(json!([null]), None, Some(10.0)),
        [(
            "parameters.input_delays_s[0]".to_string(),
            "Input 1 delay must be a finite number of seconds.".to_string()
        )]
    );
}

#[test]
fn input_delays_respect_the_transport_history_limit_at_the_effective_step() {
    // Specification 46 §5.1.1: ceil(delay / step) + 3 entries per stored input, at most 100,000.
    // At the project's 10 ms step, 999.97 s needs 99,997 + 3 = 100,000 entries (allowed) and
    // 999.98 s needs 99,998 + 3 = 100,001 (refused); a 20 ms plant override halves the need
    // (49,999 + 3). Catches an off-by-one at the limit, or the override ignored.
    assert!(delays(json!([999.97]), None, Some(10.0)).is_empty());
    assert_eq!(
        delays(json!([999.98]), None, Some(10.0)),
        [(
            "parameters.input_delays_s".to_string(),
            "Input delays exceed the 100,000-entry history limit at a 10 ms plant step. Reduce a delay or increase the plant step."
                .to_string()
        )]
    );
    assert!(delays(json!([999.98]), Some(json!(20)), Some(10.0)).is_empty());
    // Without a known step the limit is not judged (specification 47 §9), nor for zero delays.
    assert!(delays(json!([1.0e6]), None, None).is_empty());
    assert!(delays(json!([0]), None, Some(10.0)).is_empty());
}

#[test]
fn a_resize_keeps_cells_and_delays_and_asks_before_removing_any() {
    // Specification 47 §9: growing keeps every value and adds zeros; shrinking asks first and names
    // removed nonzero delays. Catches a silent removal, or new cells left empty.
    let one = json!({"a": [[0]], "b": [[1]], "c": [[1]], "d": [[0]], "input_delays_s": [0.5]});
    let one = one.as_object().unwrap();
    let grown = state_space::Dimensions {
        states: 2,
        inputs: 2,
        outputs: 1,
    };
    assert_eq!(state_space::resize_confirmation(one, grown), None);
    let (parameters, state) = state_space::resize(one, &[json!(3)], grown);
    assert_eq!(parameters["a"], json!([[0, 0], [0, 0]]));
    assert_eq!(parameters["b"], json!([[1, 0], [0, 0]]));
    assert_eq!(parameters["c"], json!([[1, 0]]));
    assert_eq!(parameters["d"], json!([[0, 0]]));
    assert_eq!(parameters["input_delays_s"], json!([0.5, 0]));
    assert_eq!(state, [json!(3), json!(0)]);

    let two =
        json!({"a": [[0]], "b": [[1, 2]], "c": [[1]], "d": [[0, 0]], "input_delays_s": [0.5, 2]});
    let shrunk = state_space::Dimensions {
        states: 1,
        inputs: 1,
        outputs: 1,
    };
    assert_eq!(
        state_space::resize_confirmation(two.as_object().unwrap(), shrunk).as_deref(),
        Some("Reducing inputs removes matrix columns and 1 nonzero input delay.")
    );
    let zero_delay =
        json!({"a": [[0]], "b": [[1, 2]], "c": [[1]], "d": [[0, 0]], "input_delays_s": [0.5, 0]});
    assert_eq!(
        state_space::resize_confirmation(zero_delay.as_object().unwrap(), shrunk).as_deref(),
        Some("Reducing dimensions removes matrix or initial-state values.")
    );
}

#[test]
fn a_pasted_block_reads_rows_by_line_and_cells_by_separator() {
    // Specification 47 §9 (paste into a matrix). Catches a comma, tab or space separator not
    // accepted, a ragged block accepted, or text taken as a number.
    assert_eq!(
        state_space::parse_paste("1\t2\n3\t4"),
        Some(vec![vec![1.0, 2.0], vec![3.0, 4.0]])
    );
    assert_eq!(
        state_space::parse_paste("1, 2\r\n3,4"),
        Some(vec![vec![1.0, 2.0], vec![3.0, 4.0]])
    );
    assert_eq!(state_space::parse_paste("1,,2"), Some(vec![vec![1.0, 2.0]]));
    assert_eq!(
        state_space::parse_paste(",1"),
        Some(vec![vec![0.0, 1.0]]),
        "an empty cell is 0"
    );
    assert_eq!(state_space::parse_paste("1 2\n3"), None);
    assert_eq!(state_space::parse_paste("a b"), None);
    assert_eq!(state_space::parse_paste("1e3"), Some(vec![vec![1000.0]]));
}

#[test]
fn a_curve_is_paired_points_with_increasing_inputs() {
    // Specification 46 §7.7 and specification 47 §9: at least two finite points with strictly
    // increasing inputs; a new point one input step after the last; a move only when the inputs
    // still increase. Catches a reordered curve accepted, or a valid move refused.
    let parameters = json!({"input_points": [-1, 0, 1], "output_points": [-2, 0, 3]});
    let points = piecewise::points(parameters.as_object().unwrap());
    let point = |input: f64, output: f64| piecewise::Point { input, output };
    assert_eq!(
        points,
        [point(-1.0, -2.0), point(0.0, 0.0), point(1.0, 3.0)]
    );
    assert_eq!(
        piecewise::append(&points),
        [
            point(-1.0, -2.0),
            point(0.0, 0.0),
            point(1.0, 3.0),
            point(2.0, 3.0)
        ]
    );
    assert_eq!(
        piecewise::moved(
            &[point(1.0, 10.0), point(0.0, 0.0), point(2.0, 20.0)],
            1,
            -1
        ),
        Some(vec![point(0.0, 0.0), point(1.0, 10.0), point(2.0, 20.0)])
    );
    assert_eq!(
        piecewise::moved(&points, 1, -1),
        None,
        "the inputs would no longer increase"
    );
    assert_eq!(
        piecewise::with_points(parameters.as_object().unwrap(), &points),
        parameters.as_object().unwrap().clone(),
        "the points save as the same two vectors"
    );
    let issues =
        |parameters: serde_json::Value| found(&piecewise::checks(parameters.as_object().unwrap()));
    assert_eq!(
        issues(json!({"input_points": [-1, -1], "output_points": [0, 1]})),
        [(
            "parameters.input_points[1]".to_string(),
            "Input point 2 must be greater than input point 1.".to_string()
        )]
    );
    assert_eq!(
        issues(json!({"input_points": [0], "output_points": [0]})),
        [(
            "parameters.input_points[0]".to_string(),
            "Add at least two input/output points.".to_string()
        )]
    );
    assert_eq!(
        issues(json!({"input_points": [0, 0], "output_points": [0]})),
        [
            (
                "parameters.output_points[1]".to_string(),
                "Output point 2 must be a finite number.".to_string()
            ),
            (
                "parameters.input_points[1]".to_string(),
                "Input point 2 must be greater than input point 1.".to_string()
            ),
            (
                "parameters.output_points".to_string(),
                "Input and output points must contain the same number of values.".to_string()
            ),
        ]
    );
}
