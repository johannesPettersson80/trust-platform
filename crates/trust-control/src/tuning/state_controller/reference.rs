//! The reference gain `N` of a state-feedback law (N5.14; specification 35, C4). The reference is the input that holds each
//! controlled output at its setpoint in steady state. For each objective `k` the equilibrium equations
//! `(A − I)x + Bu = 0` and `C_k·x + D_k·u = e_k` are solved for the state `x` and the input `u`, and the reference gain is
//! `N·r = u + K·x`, the law `u = −K·x + N·r` holding that equilibrium.

use trust_control_math::linalg::{solve_least_squares, DenseMatrix};
use trust_control_math::model::StateSpace;

use super::sampling::SampledModel;
use super::{texts, DESIGN_TOLERANCE};
use crate::failure::{Cause, Failure, NextAction};

/// The reference gain: one row per input, one column per controlled objective.
///
/// # Errors
/// Returns a failure when an objective names no output of the model, or when the equilibrium of an objective cannot be
/// solved (the objectives do not have steady control authority).
pub fn reference_gain(
    sampled: &SampledModel,
    model: &StateSpace,
    gain: &DenseMatrix,
    controlled: &[usize],
) -> Result<DenseMatrix, Failure> {
    let states = sampled.system.states();
    let inputs = sampled.system.inputs();
    if controlled.iter().any(|&output| output >= model.c.len()) {
        return Err(Failure::new(
            texts::ONE_OBJECTIVE_PER_INPUT,
            Cause::Implementation,
            NextAction::OpenChannels,
        ));
    }
    let mut equilibrium: DenseMatrix = sampled
        .system
        .a
        .iter()
        .zip(&sampled.system.b)
        .enumerate()
        .map(|(row, (a_row, b_row))| {
            let mut entries: Vec<f64> = a_row
                .iter()
                .enumerate()
                .map(|(column, value)| value - f64::from(u8::from(row == column)))
                .collect();
            entries.extend_from_slice(b_row);
            entries
        })
        .collect();
    for &output in controlled {
        let mut entries = model.c[output].clone();
        entries.extend_from_slice(&model.d[output]);
        equilibrium.push(entries);
    }

    let mut columns: Vec<Vec<f64>> = Vec::with_capacity(controlled.len());
    for objective in 0..controlled.len() {
        let mut target = vec![0.0; states];
        target.extend((0..controlled.len()).map(|index| f64::from(u8::from(index == objective))));
        let solution =
            solve_least_squares(&equilibrium, &target, &DESIGN_TOLERANCE).map_err(|_| {
                Failure::new(
                    texts::INSUFFICIENT_AUTHORITY,
                    Cause::TaskInfeasible,
                    NextAction::OpenChannels,
                )
            })?;
        let (state, input) = solution.solution.split_at(states);
        columns.push(
            (0..inputs)
                .map(|row| {
                    let feedback: f64 = gain[row].iter().zip(state).map(|(k, x)| k * x).sum();
                    input[row] + feedback
                })
                .collect(),
        );
    }
    Ok((0..inputs)
        .map(|row| columns.iter().map(|column| column[row]).collect())
        .collect())
}
