//! Split existing RK4 integration only at independently delayed input changes.
use std::ops::Range;

use super::{model::rk4_state_space, InputHistory, ModelWorkspace};

pub(super) fn advance(
    state: &mut [f64],
    interval: Range<i64>,
    history: &InputHistory,
    initial_inputs: &[f64],
    matrices: (&[Vec<f64>], &[Vec<f64>]),
    workspace: &mut ModelWorkspace,
    delayed_inputs: &mut [f64],
) {
    let mut cursor = interval.start;
    history.values_at(cursor, initial_inputs, delayed_inputs);
    while cursor < interval.end {
        let boundary = history.next_boundary(cursor, interval.end);
        let seconds = (boundary - cursor) as f64 / 1_000_000_000.0;
        rk4_state_space(
            state,
            delayed_inputs,
            seconds,
            matrices.0,
            matrices.1,
            workspace,
        );
        cursor = boundary;
        // At the final endpoint this vector is used by the existing D output.
        history.values_at(cursor, initial_inputs, delayed_inputs);
    }
}
