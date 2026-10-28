//! The multichannel ARX fit of a recording (`identifiedStateModel.ts`, `identifyStateModel`; specification 35, C7 and C8, and the
//! numerical annex N10). An autoregressive model with exogenous inputs of each order from 1 to a maximum is solved by least
//! squares, one solve per output, over the lagged outputs and the lagged inputs, the inputs at lag zero included. The
//! companion-form state space of the first order that meets the error limit is the model. Every order is judged by its
//! free-running prediction of the comparison record: the independent record when the caller has one, the estimation record
//! otherwise.
//!
//! A record is a list of rows on an even grid, one row per sample. Each row holds the model's inputs first and then its outputs.
//! The fit works on deviations from the first row of the estimation record, which is the operating point the model is measured
//! from, so the records should start there. The channel names, the sampling of the recordings and the sentences of the refusals
//! belong to the product (`trust-control`).

use super::count_f64;
use crate::linalg::{solve_least_squares, DenseMatrix, NumericalTolerance};
use crate::model::StateSpace;

/// The tolerances of the coefficient solves: a singular value counts as zero below `max(10⁻¹⁰, 10⁻⁹·σ_max)`, and a condition
/// number above `10¹⁰` refuses the order (the options of the `TypeScript` `solveLeastSquares` call).
const SOLVE_TOLERANCE: NumericalTolerance = NumericalTolerance {
    absolute: 1e-10,
    relative: 1e-9,
    condition_limit: 1e10,
};

/// The most regressors one output's solve may hold: an order whose regressors exceed it is not compared, and nor is a later one.
const MAXIMUM_REGRESSORS: usize = 64;

/// The rows the estimation record must hold per regressor: an order with fewer rows is not compared, and nor is a later one.
const ROWS_PER_REGRESSOR: usize = 3;

/// The model orders the caller may ask for.
const ORDER_LIMIT: std::ops::RangeInclusive<usize> = 1..=8;

/// The smallest span a comparison output may have when its error is normalised.
const MINIMUM_SPAN: f64 = 1e-9;

/// What one order gave on the comparison record.
#[derive(Debug, Clone, PartialEq)]
pub enum Outcome {
    /// The regressors do not determine the coefficients of every output, so the order gives no model.
    Indistinguishable,
    /// The order was fitted and its free-running errors on the comparison record were measured.
    Evaluated {
        /// The root-mean-square free-running error of each output, divided by the output's span on the comparison record.
        normalized_errors: Vec<f64>,
        /// Whether every normalised error is finite and at most the error limit.
        adequate: bool,
    },
}

/// One order that was compared.
#[derive(Debug, Clone, PartialEq)]
pub struct Comparison {
    /// The order compared.
    pub order: usize,
    /// What the order gave.
    pub outcome: Outcome,
}

/// The fitted model of the first adequate order.
#[derive(Debug, Clone, PartialEq)]
pub struct Fit {
    /// The order of the selected model.
    pub order: usize,
    /// The companion-form state space, with `p·order + m·(order − 1)` states, `m` inputs and `p` outputs. The first `p` states
    /// are the outputs at the current sample, the next `p` the outputs one sample earlier, and so on; the input history follows,
    /// the inputs at one sample earlier first.
    pub model: StateSpace,
    /// The operating point the model is measured from: the first row of the estimation record, its inputs and then its outputs.
    pub operating_point: Vec<f64>,
    /// Every order compared, in order. The orders after the selected one are compared too, as the `TypeScript` compares them.
    pub comparisons: Vec<Comparison>,
    /// The free-running prediction of each output on the comparison record, one list per output, in the records' units. The
    /// first entry of each list predicts the sample at index `order`; the list holds one entry per sample from there on.
    pub predictions: Vec<Vec<f64>>,
}

/// Why no model is given.
#[derive(Debug, Clone, PartialEq, thiserror::Error)]
pub enum Rejection {
    /// The model order limit is outside 1 to 8, or the error limit is not finite and between zero and one.
    #[error("the model order limit or the error limit is outside its range")]
    InvalidLimit,
    /// A record is empty, its rows differ in length, a value is not finite, or it holds no input or no output.
    #[error("a record is empty, ragged, not finite, or holds no input or no output")]
    InvalidRecords,
    /// No order that was compared meets the error limit on the comparison record; the comparisons say how each order fared.
    #[error("no compared order meets the error limit on the comparison record")]
    NoAdequateOrder {
        /// Every order compared, in order.
        comparisons: Vec<Comparison>,
    },
    /// The selected model's matrices are not a state space.
    #[error("the selected model is not a state space")]
    Unrepresentable,
}

/// The fitted dynamics and the evaluation of one order on the comparison record.
struct Candidate {
    order: usize,
    a: DenseMatrix,
    b: DenseMatrix,
    predictions: Vec<Vec<f64>>,
    normalized_errors: Vec<f64>,
    adequate: bool,
}

/// Fits the multichannel ARX model of `estimation` and checks it on `comparison` (`identifyStateModel`). For each order from one
/// up to `maximum_order`, while its regressors stay within 64 and the estimation record holds three rows per regressor, each
/// output's next value is regressed by least squares on the outputs and the inputs at lags zero to `order − 1`. The first order
/// whose free-running errors meet `maximum_normalized_error` on the comparison record is the model; each output's error is its
/// root-mean-square error over the span of its values on that record.
///
/// # Errors
/// [`Rejection::InvalidLimit`] and [`Rejection::InvalidRecords`] for arguments outside their rules,
/// [`Rejection::NoAdequateOrder`] when no compared order meets the error limit, and [`Rejection::Unrepresentable`] when the
/// selected matrices are not a state space.
pub fn fit(
    estimation: &[Vec<f64>],
    comparison: &[Vec<f64>],
    inputs: usize,
    maximum_order: usize,
    maximum_normalized_error: f64,
) -> Result<Fit, Rejection> {
    // A limit that is not a number fails this comparison and is refused.
    let error_limit_valid = maximum_normalized_error > 0.0 && maximum_normalized_error < 1.0;
    if !ORDER_LIMIT.contains(&maximum_order) || !error_limit_valid {
        return Err(Rejection::InvalidLimit);
    }
    let outputs = channel_count(estimation, comparison, inputs)?;
    let origin = estimation[0].clone();
    let train = centered(estimation, &origin);
    let held = centered(comparison, &origin);
    let mut comparisons = Vec::new();
    let mut selected: Option<Candidate> = None;
    for order in 1..=maximum_order {
        let count = order * (inputs + outputs);
        if count > MAXIMUM_REGRESSORS || train.len() < order + ROWS_PER_REGRESSOR * count {
            break;
        }
        match candidate(
            &train,
            &held,
            inputs,
            outputs,
            order,
            maximum_normalized_error,
        ) {
            None => comparisons.push(Comparison {
                order,
                outcome: Outcome::Indistinguishable,
            }),
            Some(found) => {
                comparisons.push(Comparison {
                    order,
                    outcome: Outcome::Evaluated {
                        normalized_errors: found.normalized_errors.clone(),
                        adequate: found.adequate,
                    },
                });
                if found.adequate && selected.is_none() {
                    selected = Some(found);
                }
            }
        }
    }
    let Some(found) = selected else {
        return Err(Rejection::NoAdequateOrder { comparisons });
    };
    let model = selected_model(&found, inputs, outputs)?;
    let predictions: Vec<Vec<f64>> = found
        .predictions
        .iter()
        .enumerate()
        .map(|(output, values)| {
            values
                .iter()
                .map(|value| value + origin[inputs + output])
                .collect()
        })
        .collect();
    Ok(Fit {
        order: found.order,
        model,
        operating_point: origin,
        comparisons,
        predictions,
    })
}

/// The number of outputs: the row width less the inputs. Both records must be non-empty, rectangular and finite, and the
/// estimation record must hold an operating point.
fn channel_count(
    estimation: &[Vec<f64>],
    comparison: &[Vec<f64>],
    inputs: usize,
) -> Result<usize, Rejection> {
    let width = estimation.first().map_or(0, Vec::len);
    let consistent = |rows: &[Vec<f64>]| {
        !rows.is_empty()
            && rows
                .iter()
                .all(|row| row.len() == width && row.iter().all(|value| value.is_finite()))
    };
    if inputs == 0 || width <= inputs || !consistent(estimation) || !consistent(comparison) {
        return Err(Rejection::InvalidRecords);
    }
    Ok(width - inputs)
}

/// The rows less the operating point, value by value.
fn centered(rows: &[Vec<f64>], origin: &[f64]) -> Vec<Vec<f64>> {
    rows.iter()
        .map(|row| {
            row.iter()
                .zip(origin)
                .map(|(value, base)| value - base)
                .collect()
        })
        .collect()
}

/// The fit and evaluation of one order, or `None` when the regressors do not determine every output's coefficients.
fn candidate(
    train: &[Vec<f64>],
    held: &[Vec<f64>],
    inputs: usize,
    outputs: usize,
    order: usize,
    limit: f64,
) -> Option<Candidate> {
    let regressors = regressors(train, inputs, order);
    let coefficients = (0..outputs)
        .map(|output| {
            let targets: Vec<f64> = train[order..]
                .iter()
                .map(|row| row[inputs + output])
                .collect();
            solve_least_squares(&regressors, &targets, &SOLVE_TOLERANCE)
                .ok()
                .map(|solved| solved.solution)
        })
        .collect::<Option<Vec<Vec<f64>>>>()?;
    let (a, b) = companion(&coefficients, inputs, outputs, order);
    let (predictions, errors) = free_run(&a, &b, held, inputs, outputs, order);
    let normalized_errors = normalized(&errors, held, inputs, order);
    let adequate = normalized_errors
        .iter()
        .all(|error| error.is_finite() && *error <= limit);
    Some(Candidate {
        order,
        a,
        b,
        predictions,
        normalized_errors,
        adequate,
    })
}

/// The regressor rows: for each sample `k` from `order − 1` to the one before the last, the outputs at lags zero to
/// `order − 1`, then the inputs at the same lags. Row `j` predicts the sample after `k`.
fn regressors(train: &[Vec<f64>], inputs: usize, order: usize) -> DenseMatrix {
    (order - 1..train.len() - 1)
        .map(|k| {
            (0..order)
                .flat_map(|lag| train[k - lag][inputs..].iter().copied())
                .chain((0..order).flat_map(|lag| train[k - lag][..inputs].iter().copied()))
                .collect::<Vec<f64>>()
        })
        .collect()
}

/// The companion form of the fitted coefficients (the matrices `a` and `b` of `identifiedStateModel.ts`). The output states hold
/// the outputs at lags zero to `order − 1`; the input states hold the inputs at lags one to `order − 1`. The output states each
/// take the value of the one before, the first input block takes the current inputs, and each further input block takes the one
/// before it.
fn companion(
    coefficients: &[Vec<f64>],
    inputs: usize,
    outputs: usize,
    order: usize,
) -> (DenseMatrix, DenseMatrix) {
    let output_states = outputs * order;
    let states = output_states + inputs * (order - 1);
    let mut a = vec![vec![0.0; states]; states];
    let mut b = vec![vec![0.0; inputs]; states];
    for (row, coefficient) in coefficients.iter().enumerate() {
        a[row][..output_states].copy_from_slice(&coefficient[..output_states]);
        b[row].copy_from_slice(&coefficient[output_states..output_states + inputs]);
        a[row][output_states..].copy_from_slice(&coefficient[output_states + inputs..]);
    }
    for (state, row) in a.iter_mut().enumerate().take(output_states).skip(outputs) {
        row[state - outputs] = 1.0;
    }
    if order > 1 {
        for (input, row) in b.iter_mut().skip(output_states).take(inputs).enumerate() {
            row[input] = 1.0;
        }
    }
    for (state, row) in a.iter_mut().enumerate().skip(output_states + inputs) {
        row[state - inputs] = 1.0;
    }
    (a, b)
}

/// The state of the first sample the free run starts from, at index `order − 1`: the outputs at lags zero to `order − 1`, then
/// the inputs at lags one to `order − 1`.
fn initial_state(held: &[Vec<f64>], inputs: usize, order: usize) -> Vec<f64> {
    let outputs_part = (0..order).flat_map(|lag| held[order - 1 - lag][inputs..].iter().copied());
    let inputs_part =
        (0..order - 1).flat_map(|lag| held[order - 2 - lag][..inputs].iter().copied());
    outputs_part.chain(inputs_part).collect()
}

/// The free-running prediction of the outputs over the comparison record: the state is advanced by `a` and `b` under the recorded
/// inputs, and each output's prediction of the next sample is compared with its measured value. The predictions are deviations
/// from the operating point; the errors are the sums of squared deviations. A record with no predicted sample gives none.
fn free_run(
    a: &DenseMatrix,
    b: &DenseMatrix,
    held: &[Vec<f64>],
    inputs: usize,
    outputs: usize,
    order: usize,
) -> (Vec<Vec<f64>>, Vec<f64>) {
    let mut predictions = vec![Vec::new(); outputs];
    let mut errors = vec![0.0; outputs];
    if held.len() <= order {
        return (predictions, errors);
    }
    let mut state = initial_state(held, inputs, order);
    for k in order - 1..held.len() - 1 {
        state = a
            .iter()
            .zip(b)
            .map(|(dynamics_row, input_row)| {
                dot(dynamics_row, &state) + dot(input_row, &held[k][..inputs])
            })
            .collect();
        for (output, (prediction, error)) in predictions.iter_mut().zip(&mut errors).enumerate() {
            let predicted = state[output];
            let deviation = predicted - held[k + 1][inputs + output];
            prediction.push(predicted);
            *error += deviation * deviation;
        }
    }
    (predictions, errors)
}

/// The normalised error of each output: its root-mean-square free-running error over the predicted samples, divided by its span on
/// the comparison record, with the span floored at `10⁻⁹`. A comparison record with no predicted sample gives an error that is not
/// finite, so the order is not adequate.
fn normalized(errors: &[f64], held: &[Vec<f64>], inputs: usize, order: usize) -> Vec<f64> {
    let terms = count_f64(held.len().saturating_sub(order));
    errors
        .iter()
        .enumerate()
        .map(|(output, error)| {
            let (lowest, highest) = held
                .iter()
                .map(|row| row[inputs + output])
                .fold((f64::INFINITY, f64::NEG_INFINITY), |(low, high), value| {
                    (low.min(value), high.max(value))
                });
            (error / terms).sqrt() / (highest - lowest).max(MINIMUM_SPAN)
        })
        .collect()
}

/// The state space of the selected candidate: its matrices, the output matrix that reads the first `p` states, and no feedthrough.
fn selected_model(
    found: &Candidate,
    inputs: usize,
    outputs: usize,
) -> Result<StateSpace, Rejection> {
    let states = found.a.len();
    let c: DenseMatrix = (0..outputs)
        .map(|output| {
            (0..states)
                .map(|state| if state == output { 1.0 } else { 0.0 })
                .collect::<Vec<f64>>()
        })
        .collect();
    let d = vec![vec![0.0; inputs]; outputs];
    StateSpace::new(found.a.clone(), found.b.clone(), c, d).map_err(|_| Rejection::Unrepresentable)
}

/// The dot product of two equal-length vectors.
fn dot(left: &[f64], right: &[f64]) -> f64 {
    left.iter().zip(right).map(|(l, r)| l * r).sum()
}

#[cfg(test)]
#[path = "arx_tests.rs"]
mod tests;
