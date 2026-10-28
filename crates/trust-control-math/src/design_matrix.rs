//! Shape, finiteness and size helpers for the matrices of the sampled designs (pole placement, LQR and the Kalman
//! estimator). They check and convert; they never decide a design, and the designs share them.

use nalgebra::DMatrix;

use crate::linalg::{matrix_from_rows, DenseMatrix, NumericalRejectionReason};

/// Whether `matrix` has `rows` rows of `columns` finite entries each.
#[must_use]
pub(crate) fn is_matrix(matrix: &DenseMatrix, rows: usize, columns: usize) -> bool {
    matrix.len() == rows
        && matrix
            .iter()
            .all(|row| row.len() == columns && row.iter().all(|value| value.is_finite()))
}

/// Checks that every `(matrix, rows, columns)` has its shape and only finite entries. A non-finite entry is
/// `NonfiniteInput`; otherwise a shape that disagrees is `InconsistentDimensions`.
///
/// # Errors
/// Returns the reason above for the first matrix that is refused.
pub(crate) fn check_shapes(
    matrices: &[(&DenseMatrix, usize, usize)],
) -> Result<(), NumericalRejectionReason> {
    if matrices
        .iter()
        .any(|(matrix, _, _)| matrix.iter().flatten().any(|value| !value.is_finite()))
    {
        return Err(NumericalRejectionReason::NonfiniteInput);
    }
    if matrices
        .iter()
        .all(|(matrix, rows, columns)| is_matrix(matrix, *rows, *columns))
    {
        Ok(())
    } else {
        Err(NumericalRejectionReason::InconsistentDimensions)
    }
}

/// The rows as a nalgebra matrix with `columns` columns.
///
/// # Errors
/// Returns `InconsistentDimensions` when a row does not have `columns` entries.
pub(crate) fn to_matrix(
    rows: &DenseMatrix,
    columns: usize,
) -> Result<DMatrix<f64>, NumericalRejectionReason> {
    matrix_from_rows(rows, columns).map_err(|error| error.reason)
}

/// Whether every entry of `matrix` is finite.
#[must_use]
pub(crate) fn is_finite_matrix(matrix: &DMatrix<f64>) -> bool {
    matrix.iter().all(|value| value.is_finite())
}

/// The largest absolute entry, `max |mᵢⱼ|`, which is zero for an empty matrix.
#[must_use]
pub(crate) fn max_abs(matrix: &DMatrix<f64>) -> f64 {
    matrix
        .iter()
        .fold(0.0, |largest, value| largest.max(value.abs()))
}

/// The largest absolute difference of corresponding entries, `max |aᵢⱼ − bᵢⱼ|`.
#[must_use]
pub(crate) fn max_difference(left: &DMatrix<f64>, right: &DMatrix<f64>) -> f64 {
    left.iter()
        .zip(right.iter())
        .fold(0.0, |largest, (a, b)| largest.max((a - b).abs()))
}
