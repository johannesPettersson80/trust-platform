//! Pole placement for a single-input state-feedback law (specification 35, C4; numerical annex N5.14). The gain is
//! Ackermann's formula `K = e_nᵀ·C⁻¹·p(A)`: `C = [B, AB, …, A^{n−1}B]` is the controllability matrix and
//! `p(s) = ∏ᵢ(s − pᵢ)` the characteristic polynomial of the desired poles. The law `u = −K·x` then gives `A − B·K`
//! exactly those poles.

use nalgebra::{DMatrix, DVector};

use crate::design_matrix::{check_shapes, to_matrix};
use crate::inverse::invert;
use crate::linalg::{
    eigenvalues, rows_from_matrix, ComplexValue, DenseMatrix, NumericalRejectionReason,
    NumericalTolerance,
};

/// A single-input state-feedback design: its gain and the poles that `A − B·K` achieves.
#[derive(Debug, Clone, PartialEq)]
pub struct StatePlacement {
    /// The gain `K` for `u = −K·x`, as one row of `n` entries (a 1×n matrix).
    pub gain: DenseMatrix,
    /// The eigenvalues of `A − B·K`.
    pub achieved_poles: Vec<ComplexValue>,
    /// The desired poles, as complex numbers with zero imaginary part.
    pub desired_poles: Vec<ComplexValue>,
    /// The largest distance between the sorted achieved poles and the sorted desired poles.
    pub pole_residual: f64,
    /// The controllability matrix `[B, AB, …, A^{n−1}B]`.
    pub controllability_matrix: DenseMatrix,
}

/// Why pole placement is refused.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum PlacementRefusal {
    /// The shapes are not `A: n×n`, `B: n×1` and `n` desired poles, or a value is not finite.
    InconsistentDimensions,
    /// The controllability matrix has no inverse at the tolerance.
    Uncontrollable,
    /// The eigenvalues of the closed loop `A − B·K` cannot be computed.
    ClosedLoopEigenvalues(NumericalRejectionReason),
}

/// The gain that places the poles of `A − B·K` at `desired_poles` (Ackermann's formula, N5.14).
///
/// # Errors
/// Returns [`PlacementRefusal::InconsistentDimensions`] for a shape or a non-finite value,
/// [`PlacementRefusal::Uncontrollable`] when the controllability matrix has no inverse at the tolerance, and
/// [`PlacementRefusal::ClosedLoopEigenvalues`] when the eigenvalue calculation of the closed loop refuses.
pub fn place_state_feedback_poles(
    a: &DenseMatrix,
    b: &DenseMatrix,
    desired_poles: &[f64],
    tolerance: &NumericalTolerance,
) -> Result<StatePlacement, PlacementRefusal> {
    let order = a.len();
    if order == 0 || desired_poles.len() != order || b.len() != order {
        return Err(PlacementRefusal::InconsistentDimensions);
    }
    check_shapes(&[(a, order, order), (b, order, 1)])
        .map_err(|_| PlacementRefusal::InconsistentDimensions)?;
    if !desired_poles.iter().all(|pole| pole.is_finite()) {
        return Err(PlacementRefusal::InconsistentDimensions);
    }
    let state_matrix = to_matrix(a, order).map_err(|_| PlacementRefusal::InconsistentDimensions)?;
    let input_column = DVector::from_iterator(order, b.iter().map(|row| row[0]));

    let mut columns = Vec::with_capacity(order);
    let mut column = input_column.clone();
    for _ in 0..order {
        columns.push(column.clone());
        column = &state_matrix * &column;
    }
    let controllability_rows = rows_from_matrix(&DMatrix::from_columns(columns.as_slice()));
    let inverse =
        invert(&controllability_rows, tolerance).map_err(|_| PlacementRefusal::Uncontrollable)?;

    // p(A) = Σₖ cₖ·A^{n−k} by Horner's rule, with c₀ = 1 the coefficient of s^n.
    let polynomial = real_root_polynomial(desired_poles);
    let identity = DMatrix::<f64>::identity(order, order);
    let mut characteristic = identity.scale(polynomial[0]);
    for coefficient in &polynomial[1..] {
        characteristic = &characteristic * &state_matrix + identity.scale(*coefficient);
    }
    let characteristic_rows = rows_from_matrix(&characteristic);
    let gain: Vec<f64> = (0..order)
        .map(|column| {
            (0..order)
                .map(|row| inverse[order - 1][row] * characteristic_rows[row][column])
                .sum()
        })
        .collect();

    let gain_matrix = DMatrix::from_row_slice(1, order, &gain);
    let closed_loop = &state_matrix - &input_column * &gain_matrix;
    let spectrum = eigenvalues(&rows_from_matrix(&closed_loop), tolerance)
        .map_err(|error| PlacementRefusal::ClosedLoopEigenvalues(error.reason))?;
    let pole_residual = pole_set_residual(&spectrum.values, desired_poles);
    Ok(StatePlacement {
        gain: vec![gain],
        achieved_poles: spectrum.values,
        desired_poles: desired_poles
            .iter()
            .map(|&real| ComplexValue {
                real,
                imaginary: 0.0,
            })
            .collect(),
        pole_residual,
        controllability_matrix: controllability_rows,
    })
}

/// The coefficients of `∏ᵢ(s − rᵢ)`, highest power first: each root multiplies the polynomial by `(s − r)`.
fn real_root_polynomial(roots: &[f64]) -> Vec<f64> {
    roots.iter().fold(vec![1.0], |coefficients, root| {
        let mut next = vec![0.0; coefficients.len() + 1];
        for (index, value) in coefficients.iter().enumerate() {
            next[index] += value;
            next[index + 1] -= value * root;
        }
        next
    })
}

/// The largest distance between the achieved poles and the desired ones, each set sorted: the achieved poles by real
/// part and then imaginary part, the desired poles ascending.
fn pole_set_residual(achieved: &[ComplexValue], desired: &[f64]) -> f64 {
    let mut achieved = achieved.to_vec();
    achieved.sort_by(|left, right| {
        left.real
            .total_cmp(&right.real)
            .then(left.imaginary.total_cmp(&right.imaginary))
    });
    let mut desired = desired.to_vec();
    desired.sort_by(f64::total_cmp);
    achieved
        .iter()
        .zip(&desired)
        .map(|(pole, target)| (pole.real - target).hypot(pole.imaginary))
        .fold(0.0, f64::max)
}

#[cfg(test)]
#[path = "state_feedback_tests.rs"]
mod tests;
