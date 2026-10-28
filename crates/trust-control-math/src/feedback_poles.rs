//! The closed-loop poles of a selected-channel negative feedback loop of delay-free linear models, from the
//! exact state-space interconnection (N7.11).
//!
//! The plant channel and the controller channel are realizations `(A, B, C, D)` of one input and one
//! output each. With `1 + Dp·Dc` nonsingular, the closed loop is
//! `[x; xc]' = M [x; xc]` with `M` built from the two realizations, and its poles are the eigenvalues of `M`.
//! Nothing is cancelled: the controller's internal states and all the plant's states stay in the loop.

use crate::linalg::{
    eigenvalues, ComplexValue, DenseMatrix, NumericalRejection, NumericalTolerance,
};
use crate::model::{transfer_function_state_space, ConstructionError, StateSpace};

/// The tolerance of the eigenvalue solver of the closed loop: the library's root tolerance (N7.11).
pub const EIGENVALUE_TOLERANCE: NumericalTolerance = NumericalTolerance {
    absolute: 1e-12,
    relative: 1e-10,
    condition_limit: 1e12,
};

/// The relative size, against the larger of one and `|Dp·Dc|`, below which `1 + Dp·Dc` is singular.
const ALGEBRAIC_LOOP_TOLERANCE: f64 = 1e-10;

/// A single-input, single-output realization `(A, B, C, D)`.
#[derive(Debug, Clone, PartialEq)]
pub struct Realization {
    /// The state matrix, `n × n`.
    pub a: DenseMatrix,
    /// The input column, of length `n`.
    pub b: Vec<f64>,
    /// The output row, of length `n`.
    pub c: Vec<f64>,
    /// The direct feedthrough.
    pub d: f64,
}

impl Realization {
    /// The realization of input `input` to output `output` of a state-space model. A missing entry reads
    /// as zero, so the realization is always defined.
    #[must_use]
    pub fn from_channel(system: &StateSpace, input: usize, output: usize) -> Self {
        Self {
            a: system.a.clone(),
            b: system
                .b
                .iter()
                .map(|row| row.get(input).copied().unwrap_or(0.0))
                .collect(),
            c: system.c.get(output).cloned().unwrap_or_default(),
            d: system
                .d
                .get(output)
                .and_then(|row| row.get(input))
                .copied()
                .unwrap_or(0.0),
        }
    }

    /// The controllable companion realization of the transfer function `N(s)/D(s)` (N7.11): the
    /// normalised coefficients of [`transfer_function_state_space`].
    ///
    /// # Errors
    /// Returns the [`ConstructionError`] of the companion form: an empty or improper polynomial, or a zero
    /// leading denominator coefficient.
    pub fn from_transfer(
        numerator: &[f64],
        denominator: &[f64],
    ) -> Result<Self, ConstructionError> {
        let companion = transfer_function_state_space(numerator, denominator)?;
        Ok(Self::from_channel(&companion.system, 0, 0))
    }
}

/// Whether the algebraic loop `1 + Dp·Dc` of the two feedthroughs is singular or ill-conditioned, which
/// the feedback analysis refuses before it closes the loop (N7.11).
#[must_use]
pub fn algebraic_loop_is_singular(plant_feedthrough: f64, controller_feedthrough: f64) -> bool {
    let product = plant_feedthrough * controller_feedthrough;
    let loop_gain = 1.0 + product;
    !loop_gain.is_finite()
        || loop_gain.abs() <= ALGEBRAIC_LOOP_TOLERANCE * 1.0_f64.max(product.abs())
}

/// The closed-loop poles of `plant` and `controller` in negative feedback, the eigenvalues of the exact
/// interconnection matrix. With no states at all the loop has no poles.
///
/// The caller checks [`algebraic_loop_is_singular`] first: the interconnection divides by `1 + Dp·Dc`.
///
/// # Errors
/// Returns the [`NumericalRejection`] of the eigenvalue solver.
pub fn closed_loop_poles(
    plant: &Realization,
    controller: &Realization,
) -> Result<Vec<ComplexValue>, NumericalRejection> {
    let plant_states = plant.a.len();
    let controller_states = controller.a.len();
    let size = plant_states + controller_states;
    if size == 0 {
        return Ok(Vec::new());
    }
    let feedthrough = 1.0 + plant.d * controller.d;
    let matrix: DenseMatrix = (0..size)
        .map(|row| {
            (0..size)
                .map(|column| {
                    interconnection_entry(plant, controller, feedthrough, plant_states, row, column)
                })
                .collect()
        })
        .collect();
    Ok(eigenvalues(&matrix, &EIGENVALUE_TOLERANCE)?.values)
}

/// The entry of the interconnection matrix at `row` and `column`, for the plant states first and the
/// controller states after them (N7.11).
fn interconnection_entry(
    plant: &Realization,
    controller: &Realization,
    feedthrough: f64,
    plant_states: usize,
    row: usize,
    column: usize,
) -> f64 {
    let n = plant_states;
    match (row < n, column < n) {
        (true, true) => {
            matrix_entry(&plant.a, row, column)
                - at(&plant.b, row) * controller.d * at(&plant.c, column) / feedthrough
        }
        (true, false) => at(&plant.b, row) * at(&controller.c, column - n) / feedthrough,
        (false, true) => -at(&controller.b, row - n) * at(&plant.c, column) / feedthrough,
        (false, false) => {
            matrix_entry(&controller.a, row - n, column - n)
                - at(&controller.b, row - n) * plant.d * at(&controller.c, column - n) / feedthrough
        }
    }
}

/// The entry of a vector at `index`, or zero beyond its end.
fn at(values: &[f64], index: usize) -> f64 {
    values.get(index).copied().unwrap_or(0.0)
}

/// The entry of a matrix at `row` and `column`, or zero beyond its end.
fn matrix_entry(matrix: &[Vec<f64>], row: usize, column: usize) -> f64 {
    matrix
        .get(row)
        .and_then(|line| line.get(column))
        .copied()
        .unwrap_or(0.0)
}

#[cfg(test)]
#[path = "feedback_poles_tests.rs"]
mod tests;
