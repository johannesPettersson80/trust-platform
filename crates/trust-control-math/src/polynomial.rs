//! Real polynomials with descending coefficients (highest power first): their products, their sums
//! aligned at the constant term, and their roots (N7.1 and N7.5 of the Control Studio numerical annex).
//!
//! The roots are the eigenvalues of the companion matrix of the monic polynomial, solved by the
//! library's eigenvalue solver. A polynomial whose leading coefficients are zero is read from its
//! first nonzero coefficient, as the characteristic polynomial of a loop is.

use crate::linalg::{
    eigenvalues, ComplexValue, DenseMatrix, NumericalRejection, NumericalTolerance,
};

/// The tolerance of the root solver: the eigenvalue tolerance of the stability margins (N7.11).
const ROOT_TOLERANCE: NumericalTolerance = NumericalTolerance {
    absolute: 1e-12,
    relative: 1e-10,
    condition_limit: 1e12,
};

/// The product `p(s)·q(s)` of two polynomials, descending. An empty factor gives an empty product.
#[must_use]
pub fn convolve(left: &[f64], right: &[f64]) -> Vec<f64> {
    if left.is_empty() || right.is_empty() {
        return Vec::new();
    }
    let mut product = vec![0.0; left.len() + right.len() - 1];
    for (left_index, left_value) in left.iter().enumerate() {
        for (right_index, right_value) in right.iter().enumerate() {
            product[left_index + right_index] += left_value * right_value;
        }
    }
    product
}

/// The sum `p(s) + q(s)`, each polynomial aligned at its constant term: the shorter one is padded
/// with leading zeros, so the sum has the length of the longer one.
#[must_use]
pub fn add_aligned(left: &[f64], right: &[f64]) -> Vec<f64> {
    let length = left.len().max(right.len());
    let padded_left = pad_front(left, length);
    let padded_right = pad_front(right, length);
    padded_left
        .iter()
        .zip(&padded_right)
        .map(|(left_value, right_value)| left_value + right_value)
        .collect()
}

/// The coefficients without their leading zeros. A polynomial that is all zeros is kept as `[0]`,
/// so that it always has a coefficient.
#[must_use]
pub fn trim_leading_zeros(coefficients: &[f64]) -> Vec<f64> {
    match coefficients.iter().position(|value| !is_exact_zero(*value)) {
        Some(start) => coefficients[start..].to_vec(),
        None => vec![0.0],
    }
}

/// The roots of a descending polynomial: the eigenvalues of its companion matrix, after the leading
/// zeros are removed. A polynomial of degree zero has no roots.
///
/// # Errors
/// Returns the [`NumericalRejection`] of the eigenvalue solver when the roots cannot be computed.
pub fn roots(coefficients: &[f64]) -> Result<Vec<ComplexValue>, NumericalRejection> {
    let trimmed = trim_leading_zeros(coefficients);
    let Some((&leading, rest)) = trimmed.split_first() else {
        return Ok(Vec::new());
    };
    let degree = rest.len();
    if degree == 0 {
        return Ok(Vec::new());
    }
    // The monic companion matrix: its first row holds the negated coefficients, and its subdiagonal is one.
    let companion: DenseMatrix = (0..degree)
        .map(|row| {
            (0..degree)
                .map(|column| {
                    if row == 0 {
                        -rest[column] / leading
                    } else if column + 1 == row {
                        1.0
                    } else {
                        0.0
                    }
                })
                .collect::<Vec<f64>>()
        })
        .collect();
    Ok(eigenvalues(&companion, &ROOT_TOLERANCE)?.values)
}

/// The polynomial padded at its front with zeros to `length` coefficients.
fn pad_front(coefficients: &[f64], length: usize) -> Vec<f64> {
    let mut padded = vec![0.0; length - coefficients.len()];
    padded.extend_from_slice(coefficients);
    padded
}

/// Whether a value is exactly zero, `+0` or `-0`. Trimming a leading coefficient compares it exactly.
#[allow(clippy::float_cmp)] // The leading coefficient is trimmed only when it is exactly zero.
fn is_exact_zero(value: f64) -> bool {
    value == 0.0
}

#[cfg(test)]
mod tests;
