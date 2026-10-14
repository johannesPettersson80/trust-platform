//! Matrix inversion under the rank and conditioning rules of the numerical annex. A square matrix is inverted when its
//! numerical rank is full (`numerical_rank`) and its condition number `σ_max / σ_min` does not exceed the limit of the
//! tolerance. The sampled designs use it for the controllability matrix and for the Riccati control weight `R + BᵀPB`.

use nalgebra::{linalg::SVD, DMatrix};

use crate::linalg::{
    matrix_from_rows, numerical_rank, rejection, rows_from_matrix, DenseMatrix, NumericalRejection,
    NumericalRejectionReason, NumericalTolerance,
};

/// The inverse of a square matrix of full rank whose condition number is within the tolerance's limit.
///
/// # Errors
/// Returns the rejection of [`numerical_rank`] for an invalid tolerance or matrix, `MatrixNotSquare` for a rectangular
/// matrix, `Singular` when the rank is below the order, `IllConditioned` when `σ_max / σ_min` exceeds the limit, and
/// `SolverFailed` when the inverse cannot be computed or is not finite.
pub fn invert(
    matrix: &DenseMatrix,
    tolerance: &NumericalTolerance,
) -> Result<DenseMatrix, NumericalRejection> {
    let rank = numerical_rank(matrix, tolerance)?;
    let order = matrix.len();
    if matrix.iter().any(|row| row.len() != order) {
        return Err(rejection(NumericalRejectionReason::MatrixNotSquare));
    }
    if rank < order {
        return Err(rejection(NumericalRejectionReason::Singular));
    }
    let square = matrix_from_rows(matrix, order)?;
    let singular_values = SVD::new(square.clone(), false, false).singular_values;
    let largest = singular_values
        .iter()
        .copied()
        .fold(f64::NEG_INFINITY, f64::max);
    let smallest = singular_values
        .iter()
        .copied()
        .fold(f64::INFINITY, f64::min);
    let condition_number = largest / smallest;
    if !condition_number.is_finite() || condition_number > tolerance.condition_limit {
        return Err(rejection(NumericalRejectionReason::IllConditioned));
    }
    let inverse = square
        .try_inverse()
        .ok_or_else(|| rejection(NumericalRejectionReason::SolverFailed))?;
    let rows = rows_from_matrix(&inverse);
    if rows.iter().flatten().all(|value| value.is_finite()) {
        Ok(rows)
    } else {
        Err(rejection(NumericalRejectionReason::SolverFailed))
    }
}

/// [`invert`] for a nalgebra matrix, with the same rules.
///
/// # Errors
/// Returns the rejection of [`invert`].
pub(crate) fn invert_matrix(
    matrix: &DMatrix<f64>,
    tolerance: &NumericalTolerance,
) -> Result<DMatrix<f64>, NumericalRejection> {
    let inverse = invert(&rows_from_matrix(matrix), tolerance)?;
    matrix_from_rows(&inverse, matrix.nrows())
}

#[cfg(test)]
#[path = "inverse_tests.rs"]
mod tests;
