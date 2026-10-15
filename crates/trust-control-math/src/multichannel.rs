//! The static gain matrix of a multichannel plant (specification 35, C7; numerical annex N4.4 and N4.5): the steady-state
//! gain of a state-space model, the relative gain array `Λ = K ∘ (K⁻¹)ᵀ`, the one-to-one pairing that its diagonal names, and
//! the static decoupler `D = K⁻¹·diag(K)`. Every matrix keeps the input and output order of the model it comes from.
//!
//! The relative gain array and the decoupler need a square, nonsingular `K`, and both refuse one through the rules of
//! [`crate::inverse::invert`]. The pairing reads the relative gains only, so it never receives a singular case (N4.5).

use nalgebra::linalg::SVD;

use crate::inverse::invert;
use crate::linalg::{
    matrix_from_rows, solve_least_squares, DenseMatrix, NumericalRejection,
    NumericalRejectionReason, NumericalTolerance,
};
use crate::model::StateSpace;

/// The pivot magnitude at or below which a state equation counts as singular (the rule of Control Studio's `solveLinear`).
const SINGULAR_PIVOT: f64 = 1e-12;

/// The penalty that the pairing cost gives a relative gain that is not positive (N4.5 refuses such a pairing).
const NONPOSITIVE_PENALTY: f64 = 1e9;

/// The relative gain array `Λ = K ∘ (K⁻¹)ᵀ` of a square, nonsingular steady-state gain `K` (N4.5): `λᵢⱼ = kᵢⱼ·(K⁻¹)ⱼᵢ`.
/// Its rows and its columns each sum to one.
///
/// # Errors
/// Returns the rejection of [`invert`] when `K` is not square, has no inverse, or is ill-conditioned.
pub fn relative_gain_array(
    gain: &DenseMatrix,
    tolerance: &NumericalTolerance,
) -> Result<DenseMatrix, NumericalRejection> {
    let inverse = invert(gain, tolerance)?;
    Ok(gain
        .iter()
        .enumerate()
        .map(|(row, values)| {
            values
                .iter()
                .enumerate()
                .map(|(column, value)| value * inverse[column][row])
                .collect()
        })
        .collect())
}

/// The static decoupler `D = K⁻¹·diag(K)` of a square, nonsingular steady-state gain `K` (C7): `dᵢⱼ = (K⁻¹)ᵢⱼ·kⱼⱼ`. With
/// `u = D·v`, each member's output moves only its own measurement in steady state, with the gain of its own path.
///
/// # Errors
/// Returns the rejection of [`invert`] when `K` is not square, has no inverse, or is ill-conditioned.
pub fn static_decoupler(
    gain: &DenseMatrix,
    tolerance: &NumericalTolerance,
) -> Result<DenseMatrix, NumericalRejection> {
    let inverse = invert(gain, tolerance)?;
    Ok(inverse
        .iter()
        .map(|row| {
            row.iter()
                .enumerate()
                .map(|(column, value)| value * gain[column][column])
                .collect()
        })
        .collect())
}

/// The one-to-one pairing that the relative gains name (N4.5): for each output row, the input column it pairs with.
///
/// Among all pairings the one of smallest cost is chosen. A positive relative gain `λ` costs `|λ − 1|`, and one that is not
/// positive costs a penalty of `10⁹` plus its magnitude, so the pairing whose diagonal is closest to one wins. Of two equally
/// costly pairings, the one earlier in lexicographic order is kept. An empty matrix pairs nothing.
#[must_use]
pub fn pairing_by_relative_gain(relative_gains: &DenseMatrix) -> Vec<usize> {
    let columns: Vec<usize> = (0..relative_gains.len()).collect();
    let mut best: Option<(f64, Vec<usize>)> = None;
    for pairing in permutations(&columns) {
        let cost = pairing_cost(relative_gains, &pairing);
        let better = match &best {
            None => true,
            Some((best_cost, _)) => cost < *best_cost,
        };
        if better {
            best = Some((cost, pairing));
        }
    }
    best.map(|(_, pairing)| pairing).unwrap_or_default()
}

/// The steady-state gain `G` of a state-space model: `C·(I − A)⁻¹·B + D` for a discrete model, and `−C·A⁻¹·B + D` for a
/// continuous one (C7). The states of each input are solved by [`solve_linear`].
///
/// Returns `None` when a state equation has no solution, that is when an elimination pivot is not above `10⁻¹²`.
#[must_use]
pub fn steady_state_gain(system: &StateSpace, discrete: bool) -> Option<DenseMatrix> {
    let coefficients: DenseMatrix = system
        .a
        .iter()
        .enumerate()
        .map(|(row, values)| {
            values
                .iter()
                .enumerate()
                .map(|(column, value)| {
                    if !discrete {
                        -value
                    } else if row == column {
                        1.0 - value
                    } else {
                        -value
                    }
                })
                .collect()
        })
        .collect();
    let input_count = system.inputs();
    let states: Vec<Vec<f64>> = (0..input_count)
        .map(|input| {
            let right: Vec<f64> = system.b.iter().map(|row| row[input]).collect();
            solve_linear(&coefficients, &right)
        })
        .collect::<Option<Vec<Vec<f64>>>>()?;
    Some(
        (0..system.outputs())
            .map(|output| {
                (0..input_count)
                    .map(|input| {
                        let through_states = system.c[output]
                            .iter()
                            .zip(&states[input])
                            .fold(0.0, |sum, (weight, state)| sum + weight * state);
                        through_states + system.d[output][input]
                    })
                    .collect()
            })
            .collect(),
    )
}

/// The steady-state gain that settled trials give (C7 "Measured paths"), with the evidence of its fit.
#[derive(Debug, Clone, PartialEq)]
pub struct StaticGainFit {
    /// The gain `K`: one row per output, one column per input.
    pub gain: DenseMatrix,
    /// The numerical rank of the input steps.
    pub input_rank: usize,
    /// The condition number `σ_max / σ_min` of the input steps.
    pub input_condition_number: f64,
    /// The norm of the residuals of the fit over every output.
    pub residual_norm: f64,
}

/// The least-squares steady-state gain of settled trials (C7, N4.5). `inputs` has one row per trial and one column per input, and
/// `outputs` one row per trial and one column per output. Each output is fitted on its own, `inputs · kᵢ = outputs column i`, so the
/// gain's row `i` is `kᵢ`. The rank and the conditioning of `inputs` are those of every fit.
///
/// # Errors
/// Returns the rejection of [`solve_least_squares`] for an output that cannot be fitted, and `InconsistentDimensions` when there are
/// fewer than two trials, the trials and the outputs do not pair, a row has another length, or a list is empty.
pub fn identify_static_gain(
    inputs: &DenseMatrix,
    outputs: &DenseMatrix,
    tolerance: &NumericalTolerance,
) -> Result<StaticGainFit, NumericalRejection> {
    let inconsistent = NumericalRejection {
        reason: NumericalRejectionReason::InconsistentDimensions,
    };
    let input_width = inputs.first().map_or(0, Vec::len);
    let output_width = outputs.first().map_or(0, Vec::len);
    let rectangular = inputs.iter().all(|row| row.len() == input_width)
        && outputs.iter().all(|row| row.len() == output_width);
    if inputs.len() < 2
        || inputs.len() != outputs.len()
        || input_width == 0
        || output_width == 0
        || !rectangular
    {
        return Err(inconsistent);
    }
    let mut gain = Vec::with_capacity(output_width);
    let mut residual_squared = 0.0;
    let mut conditioning = None;
    for output in 0..output_width {
        let column: Vec<f64> = outputs.iter().map(|row| row[output]).collect();
        let fit = solve_least_squares(inputs, &column, tolerance)?;
        residual_squared += fit.residual_norm * fit.residual_norm;
        if conditioning.is_none() {
            conditioning = Some((fit.rank, fit.condition_number));
        }
        gain.push(fit.solution);
    }
    let (input_rank, input_condition_number) = conditioning.ok_or(inconsistent)?;
    Ok(StaticGainFit {
        gain,
        input_rank,
        input_condition_number,
        residual_norm: residual_squared.sqrt(),
    })
}

/// The condition number `σ_max / σ_min` of a square matrix (N4.5): the conditioning that a relative gain array keeps. `None` for an
/// empty or rectangular matrix, or when the ratio is not finite.
#[must_use]
pub fn condition_number(matrix: &DenseMatrix) -> Option<f64> {
    let order = matrix.len();
    if order == 0 || matrix.iter().any(|row| row.len() != order) {
        return None;
    }
    let square = matrix_from_rows(matrix, order).ok()?;
    let singular = SVD::new(square, false, false).singular_values;
    let largest = singular.iter().copied().fold(f64::NEG_INFINITY, f64::max);
    let smallest = singular.iter().copied().fold(f64::INFINITY, f64::min);
    let ratio = largest / smallest;
    ratio.is_finite().then_some(ratio)
}

/// The pairing that the relative gains name (N4.5): among the pairings whose diagonal relative gains are all positive, the one whose
/// diagonal is closest to one, by the smallest sum of `|λᵢπ(i) − 1|`. Of two equally close pairings, the earlier in lexicographic order
/// is named. `None` when no pairing has every diagonal element positive, so that no pairing is recommended.
#[must_use]
pub fn named_pairing(relative_gains: &DenseMatrix) -> Option<Vec<usize>> {
    let columns: Vec<usize> = (0..relative_gains.len()).collect();
    let mut best: Option<(f64, Vec<usize>)> = None;
    for pairing in permutations(&columns) {
        let positive = pairing
            .iter()
            .enumerate()
            .all(|(row, &column)| relative_gains[row][column] > 0.0);
        if !positive {
            continue;
        }
        let cost: f64 = pairing
            .iter()
            .enumerate()
            .map(|(row, &column)| (relative_gains[row][column] - 1.0).abs())
            .sum();
        let better = match &best {
            None => true,
            Some((best_cost, _)) => cost < *best_cost,
        };
        if better {
            best = Some((cost, pairing));
        }
    }
    best.map(|(_, pairing)| pairing)
}

/// Every permutation of `items`, in lexicographic order of their positions.
fn permutations(items: &[usize]) -> Vec<Vec<usize>> {
    if items.is_empty() {
        return vec![Vec::new()];
    }
    let mut all = Vec::new();
    for (index, &first) in items.iter().enumerate() {
        let mut rest = items.to_vec();
        rest.remove(index);
        for tail in permutations(&rest) {
            let mut pairing = Vec::with_capacity(items.len());
            pairing.push(first);
            pairing.extend(tail);
            all.push(pairing);
        }
    }
    all
}

/// The cost of a pairing: for each output row, the cost of the relative gain of the column it pairs with.
fn pairing_cost(relative_gains: &DenseMatrix, pairing: &[usize]) -> f64 {
    pairing
        .iter()
        .enumerate()
        .map(|(row, &column)| {
            let value = relative_gains[row][column];
            if value > 0.0 {
                (value - 1.0).abs()
            } else {
                NONPOSITIVE_PENALTY + value.abs()
            }
        })
        .sum()
}

/// Solves `matrix·x = right` by Gaussian elimination with partial pivoting, as Control Studio's `solveLinear` does: the pivot
/// of each column is the row of largest magnitude, and a pivot whose magnitude is not above `10⁻¹²` makes the system singular.
#[allow(clippy::needless_range_loop)] // The elimination reads and writes rows and columns by position, as its formula states.
fn solve_linear(matrix: &DenseMatrix, right: &[f64]) -> Option<Vec<f64>> {
    let size = right.len();
    let mut rows: DenseMatrix = matrix
        .iter()
        .zip(right)
        .map(|(row, value)| {
            let mut augmented = row.clone();
            augmented.push(*value);
            augmented
        })
        .collect();
    for column in 0..size {
        let mut pivot = column;
        for row in column + 1..size {
            if rows[row][column].abs() > rows[pivot][column].abs() {
                pivot = row;
            }
        }
        let magnitude = rows[pivot][column].abs();
        if magnitude.is_nan() || magnitude <= SINGULAR_PIVOT {
            return None;
        }
        rows.swap(column, pivot);
        let pivot_row = rows[column].clone();
        for row in column + 1..size {
            let factor = rows[row][column] / pivot_row[column];
            for (cell, pivot_cell) in rows[row][column..=size]
                .iter_mut()
                .zip(&pivot_row[column..=size])
            {
                *cell -= factor * pivot_cell;
            }
        }
    }
    let mut result = vec![0.0; size];
    for row in (0..size).rev() {
        let mut sum = rows[row][size];
        for known in row + 1..size {
            sum -= rows[row][known] * result[known];
        }
        result[row] = sum / rows[row][row];
    }
    Some(result)
}

#[cfg(test)]
#[path = "multichannel_tests.rs"]
mod tests;
