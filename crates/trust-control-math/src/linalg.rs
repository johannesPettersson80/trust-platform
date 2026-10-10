//! Dense real linear algebra for control calculations: least-squares fits, numerical rank and the
//! eigenvalues of a real square matrix. Each function states the formula it computes and the
//! rejection it returns when the formula does not apply. The decompositions are nalgebra's.

use nalgebra::{
    linalg::{Schur, SVD},
    DMatrix, DVector,
};
use schemars::JsonSchema;
use serde::{Deserialize, Serialize};

/// A dense real matrix as rows: `matrix[i][j]` is row `i`, column `j`.
pub type DenseMatrix = Vec<Vec<f64>>;

/// Tolerances that decide rank and conditioning. A singular value counts as zero unless it exceeds
/// `max(absolute, relative · σ_max)`; a ratio `σ_max / σ_min` above `condition_limit` is
/// ill-conditioned. Every field must be finite, `absolute` and `relative` positive and
/// `condition_limit` above one.
#[derive(Debug, Clone, Copy, PartialEq, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "camelCase")]
pub struct NumericalTolerance {
    /// The absolute part of the singular-value threshold.
    pub absolute: f64,
    /// The part of the threshold that scales with the largest singular value.
    pub relative: f64,
    /// The largest accepted ratio of the largest to the smallest singular value.
    pub condition_limit: f64,
}

/// Why a numerical computation is refused.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "kebab-case")]
pub enum NumericalRejectionReason {
    /// A value or a tolerance is not finite, or a tolerance is not positive.
    NonfiniteInput,
    /// The rows do not share one length, or the matrix or one of its dimensions is empty.
    InconsistentDimensions,
    /// A square matrix was required.
    MatrixNotSquare,
    /// Fewer singular values exceed the threshold than there are unknowns.
    Singular,
    /// The ratio of the largest to the smallest singular value exceeds the condition limit.
    IllConditioned,
    /// The decomposition did not converge, or a result is not finite.
    SolverFailed,
}

/// A refused numerical computation.
#[derive(Debug, Clone, Copy, PartialEq, Eq, thiserror::Error)]
#[error("numerical computation rejected: {reason:?}")]
pub struct NumericalRejection {
    /// Why the computation was refused.
    pub reason: NumericalRejectionReason,
}

/// The least-squares solution `x` that minimises `‖Ax − b‖₂`.
#[derive(Debug, Clone, PartialEq)]
pub struct LeastSquaresSolution {
    /// The solution `x`, one entry per column of `A`.
    pub solution: Vec<f64>,
    /// `‖Ax − b‖₂` at the solution.
    pub residual_norm: f64,
    /// The number of singular values above the threshold.
    pub rank: usize,
    /// The largest singular value divided by the smallest.
    pub condition_number: f64,
}

/// The least-squares outcome: the solution, or the refusal.
pub type LeastSquaresResult = Result<LeastSquaresSolution, NumericalRejection>;

/// A complex number as its real and imaginary parts.
#[derive(Debug, Clone, Copy, PartialEq, Serialize, Deserialize, JsonSchema)]
pub struct ComplexValue {
    /// The real part.
    pub real: f64,
    /// The imaginary part.
    pub imaginary: f64,
}

/// The eigenvalues of a square matrix.
#[derive(Debug, Clone, PartialEq)]
pub struct EigenvalueSolution {
    /// The eigenvalues, in the order their diagonal blocks appear in the real Schur form.
    pub values: Vec<ComplexValue>,
}

/// The eigenvalue outcome: the eigenvalues, or the refusal.
pub type EigenvalueResult = Result<EigenvalueSolution, NumericalRejection>;

/// The convergence tolerance of the decompositions: nalgebra's default, times five.
const DECOMPOSITION_EPSILON: f64 = f64::EPSILON * 5.0;

/// Solves `min ‖Ax − b‖₂` by the singular value decomposition `A = UΣVᵀ`, giving `x = VΣ⁺Uᵀb`.
/// A singular value counts as zero unless it exceeds `threshold = max(absolute, relative · σ_max)`.
/// The matrix must have at least as many rows as columns, and the solution is refused as singular
/// when fewer singular values exceed the threshold than there are columns, and as ill-conditioned
/// when `σ_max / σ_min` exceeds the condition limit.
///
/// # Errors
/// Returns the [`NumericalRejection`] that names why the problem cannot be solved.
pub fn solve_least_squares(
    coefficients: &DenseMatrix,
    observations: &[f64],
    tolerance: &NumericalTolerance,
) -> LeastSquaresResult {
    let (rows, columns) = validate_rectangular(coefficients, tolerance)?;
    if observations.len() != rows {
        return Err(rejection(NumericalRejectionReason::InconsistentDimensions));
    }
    if !observations.iter().all(|value| value.is_finite()) {
        return Err(rejection(NumericalRejectionReason::NonfiniteInput));
    }
    if rows < columns {
        return Err(rejection(NumericalRejectionReason::InconsistentDimensions));
    }
    let decomposition = SVD::try_new(
        matrix_from_rows(coefficients, columns)?,
        true,
        true,
        DECOMPOSITION_EPSILON,
        0,
    )
    .ok_or_else(|| rejection(NumericalRejectionReason::SolverFailed))?;
    let singular_values: Vec<f64> = decomposition.singular_values.iter().copied().collect();
    let maximum = largest_value(&singular_values);
    let threshold = tolerance.absolute.max(tolerance.relative * maximum);
    let rank = singular_values
        .iter()
        .filter(|value| **value > threshold)
        .count();
    if rank < columns {
        return Err(rejection(NumericalRejectionReason::Singular));
    }
    let minimum = singular_values
        .iter()
        .copied()
        .fold(f64::INFINITY, f64::min);
    let condition_number = maximum / minimum;
    if !condition_number.is_finite() || condition_number > tolerance.condition_limit {
        return Err(rejection(NumericalRejectionReason::IllConditioned));
    }
    let right_hand_side = DVector::from_column_slice(observations);
    let solved = decomposition
        .solve(&right_hand_side, threshold)
        .map_err(|_| rejection(NumericalRejectionReason::SolverFailed))?;
    let solution: Vec<f64> = solved.iter().copied().collect();
    let residual_norm = coefficients
        .iter()
        .zip(observations)
        .map(|(row, observation)| {
            let residual = dot(row, &solution) - observation;
            residual * residual
        })
        .sum::<f64>()
        .sqrt();
    if !residual_norm.is_finite() || !solution.iter().all(|value| value.is_finite()) {
        return Err(rejection(NumericalRejectionReason::SolverFailed));
    }
    Ok(LeastSquaresSolution {
        solution,
        residual_norm,
        rank,
        condition_number,
    })
}

/// The numerical rank: the number of singular values above `max(absolute, relative · σ_max)`.
///
/// # Errors
/// Returns the [`NumericalRejection`] for an invalid tolerance, an empty, ragged or non-finite
/// matrix, or a decomposition that did not converge.
pub fn numerical_rank(
    matrix: &DenseMatrix,
    tolerance: &NumericalTolerance,
) -> Result<usize, NumericalRejection> {
    let (_, columns) = validate_rectangular(matrix, tolerance)?;
    let decomposition = SVD::try_new(
        matrix_from_rows(matrix, columns)?,
        false,
        false,
        DECOMPOSITION_EPSILON,
        0,
    )
    .ok_or_else(|| rejection(NumericalRejectionReason::SolverFailed))?;
    let singular_values: Vec<f64> = decomposition.singular_values.iter().copied().collect();
    let threshold = tolerance
        .absolute
        .max(tolerance.relative * largest_value(&singular_values));
    Ok(singular_values
        .iter()
        .filter(|value| **value > threshold)
        .count())
}

/// The eigenvalues of a real square matrix, taken from its real Schur form `A = QTQᵀ`: they are the
/// 1×1 and 2×2 diagonal blocks of the quasi-triangular matrix `T`, as complex numbers.
///
/// # Errors
/// Returns the [`NumericalRejection`] for an invalid tolerance, a matrix that is empty, ragged or not
/// finite, a matrix that is not square, or a Schur iteration that did not converge.
pub fn eigenvalues(matrix: &DenseMatrix, tolerance: &NumericalTolerance) -> EigenvalueResult {
    let (rows, columns) = validate_rectangular(matrix, tolerance)?;
    if rows != columns {
        return Err(rejection(NumericalRejectionReason::MatrixNotSquare));
    }
    let schur = Schur::try_new(matrix_from_rows(matrix, columns)?, f64::EPSILON, 0)
        .ok_or_else(|| rejection(NumericalRejectionReason::SolverFailed))?;
    let values: Vec<ComplexValue> = schur
        .complex_eigenvalues()
        .iter()
        .map(|value| ComplexValue {
            real: value.re,
            imaginary: value.im,
        })
        .collect();
    if values
        .iter()
        .any(|value| !value.real.is_finite() || !value.imaginary.is_finite())
    {
        return Err(rejection(NumericalRejectionReason::SolverFailed));
    }
    Ok(EigenvalueSolution { values })
}

/// The shape `(rows, columns)` of a rectangular, non-empty, finite matrix, checked after the tolerance.
fn validate_rectangular(
    matrix: &DenseMatrix,
    tolerance: &NumericalTolerance,
) -> Result<(usize, usize), NumericalRejection> {
    if !valid_tolerance(tolerance) {
        return Err(rejection(NumericalRejectionReason::NonfiniteInput));
    }
    let Some(first) = matrix.first() else {
        return Err(rejection(NumericalRejectionReason::InconsistentDimensions));
    };
    let columns = first.len();
    if columns == 0 || matrix.iter().any(|row| row.len() != columns) {
        return Err(rejection(NumericalRejectionReason::InconsistentDimensions));
    }
    if matrix.iter().flatten().any(|value| !value.is_finite()) {
        return Err(rejection(NumericalRejectionReason::NonfiniteInput));
    }
    Ok((matrix.len(), columns))
}

fn valid_tolerance(tolerance: &NumericalTolerance) -> bool {
    tolerance.absolute.is_finite()
        && tolerance.absolute > 0.0
        && tolerance.relative.is_finite()
        && tolerance.relative > 0.0
        && tolerance.condition_limit.is_finite()
        && tolerance.condition_limit > 1.0
}

fn largest_value(values: &[f64]) -> f64 {
    values.iter().copied().fold(f64::NEG_INFINITY, f64::max)
}

/// A refusal with its reason.
pub(crate) fn rejection(reason: NumericalRejectionReason) -> NumericalRejection {
    NumericalRejection { reason }
}

/// The matrix of the given rows as a `DMatrix`, refused when a row does not have `columns` entries.
pub(crate) fn matrix_from_rows(
    rows: &[Vec<f64>],
    columns: usize,
) -> Result<DMatrix<f64>, NumericalRejection> {
    if rows.iter().any(|row| row.len() != columns) {
        return Err(rejection(NumericalRejectionReason::InconsistentDimensions));
    }
    let flat: Vec<f64> = rows.iter().flatten().copied().collect();
    Ok(DMatrix::from_row_slice(rows.len(), columns, &flat))
}

/// The rows of a `DMatrix`.
pub(crate) fn rows_from_matrix(matrix: &DMatrix<f64>) -> DenseMatrix {
    (0..matrix.nrows())
        .map(|row| {
            (0..matrix.ncols())
                .map(|column| matrix[(row, column)])
                .collect()
        })
        .collect()
}

/// The dot product `Σ leftᵢ · rightᵢ` over the entries that both slices have.
pub(crate) fn dot(left: &[f64], right: &[f64]) -> f64 {
    left.iter().zip(right).map(|(a, b)| a * b).sum::<f64>()
}

/// The product `left · right`: entry `(i, j)` is `Σₖ leftᵢₖ · rightₖⱼ`.
pub(crate) fn multiply(left: &[Vec<f64>], right: &[Vec<f64>]) -> DenseMatrix {
    let columns = right.first().map_or(0, Vec::len);
    left.iter()
        .map(|row| {
            (0..columns)
                .map(|column| {
                    row.iter()
                        .zip(right)
                        .map(|(value, right_row)| value * right_row[column])
                        .sum::<f64>()
                })
                .collect()
        })
        .collect()
}

/// The identity matrix `I` of the given size.
pub(crate) fn identity(size: usize) -> DenseMatrix {
    (0..size)
        .map(|row| {
            (0..size)
                .map(|column| f64::from(u8::from(row == column)))
                .collect()
        })
        .collect()
}

#[cfg(test)]
mod tests;
