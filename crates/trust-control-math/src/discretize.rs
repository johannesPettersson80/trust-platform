//! Discretization of a continuous state-space model at its sample period `T`: forward Euler,
//! backward Euler, Tustin (bilinear) and the exact zero-order hold. Each method states its formula.
//! The zero-order hold is the matrix exponential of `T·[[A, B], [0, 0]]`, which holds the input
//! constant over each period.

use nalgebra::DMatrix;
use schemars::JsonSchema;
use serde::{Deserialize, Serialize};

use crate::linalg::{
    matrix_from_rows, rejection, rows_from_matrix, NumericalRejection, NumericalRejectionReason,
};
use crate::model::StateSpace;

/// How a continuous model is turned into a discrete one at its sample period `T`.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "snake_case")]
pub enum DiscretizationMethod {
    /// `A_d = I + T·A` and `B_d = T·B`.
    ForwardEuler,
    /// `A_d = (I − T·A)⁻¹` and `B_d = (I − T·A)⁻¹·T·B`.
    BackwardEuler,
    /// `A_d = (I − T/2·A)⁻¹(I + T/2·A)` and `B_d = (I − T/2·A)⁻¹·T·B`.
    Tustin,
    /// `A_d = e^{AT}` and `B_d = ∫₀ᵀ e^{Aτ}dτ · B`, read from the upper blocks of
    /// `exp(T·[[A, B], [0, 0]])`.
    ExactZoh,
}

/// A discrete model: its matrices, its sample period and the method that produced them.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, JsonSchema)]
pub struct DiscreteStateSpaceModel {
    /// The discrete matrices `A_d` and `B_d`, with `C` and `D` copied from the continuous model.
    #[serde(flatten)]
    pub system: StateSpace,
    /// The sample period `T` in seconds.
    pub sample_time_seconds: f64,
    /// The method that produced `A_d` and `B_d`.
    pub method: DiscretizationMethod,
}

/// Discretizes a continuous model at sample period `T` with the given method.
///
/// # Errors
/// Returns `NonfiniteInput` when `T` is not finite and positive or a matrix value is not finite,
/// `InconsistentDimensions` when the model has no states or its shapes disagree, `Singular` when
/// `I − T·A` (backward Euler) or `I − T/2·A` (Tustin) has no inverse, and `SolverFailed` when a
/// discrete matrix is not finite.
pub fn discretize_state_space(
    model: &StateSpace,
    sample_time_seconds: f64,
    method: DiscretizationMethod,
) -> Result<DiscreteStateSpaceModel, NumericalRejection> {
    if !sample_time_seconds.is_finite() || sample_time_seconds <= 0.0 {
        return Err(rejection(NumericalRejectionReason::NonfiniteInput));
    }
    let states = model.states();
    let inputs = model.inputs();
    if states == 0 || inputs == 0 || model.outputs() == 0 {
        return Err(rejection(NumericalRejectionReason::InconsistentDimensions));
    }
    if !model
        .a
        .iter()
        .chain(&model.b)
        .chain(&model.c)
        .chain(&model.d)
        .flatten()
        .all(|value| value.is_finite())
    {
        return Err(rejection(NumericalRejectionReason::NonfiniteInput));
    }
    let a = matrix_from_rows(&model.a, states)?;
    let b = matrix_from_rows(&model.b, inputs)?;
    let t = sample_time_seconds;
    let (a_d, b_d) = match method {
        DiscretizationMethod::ForwardEuler => {
            let identity = DMatrix::<f64>::identity(states, states);
            (identity + a.scale(t), b.scale(t))
        }
        DiscretizationMethod::BackwardEuler => {
            let identity = DMatrix::<f64>::identity(states, states);
            let resolvent = (identity - a.scale(t))
                .try_inverse()
                .ok_or_else(|| rejection(NumericalRejectionReason::Singular))?;
            let b_d = &resolvent * b.scale(t);
            (resolvent, b_d)
        }
        DiscretizationMethod::Tustin => {
            let identity = DMatrix::<f64>::identity(states, states);
            let left = identity.clone() - a.scale(t / 2.0);
            let right = identity + a.scale(t / 2.0);
            let resolvent = left
                .try_inverse()
                .ok_or_else(|| rejection(NumericalRejectionReason::Singular))?;
            let b_d = &resolvent * b.scale(t);
            (&resolvent * right, b_d)
        }
        DiscretizationMethod::ExactZoh => exact_zoh(&a, &b, t),
    };
    if !a_d.iter().chain(b_d.iter()).all(|value| value.is_finite()) {
        return Err(rejection(NumericalRejectionReason::SolverFailed));
    }
    Ok(DiscreteStateSpaceModel {
        system: StateSpace {
            a: rows_from_matrix(&a_d),
            b: rows_from_matrix(&b_d),
            c: model.c.clone(),
            d: model.d.clone(),
        },
        sample_time_seconds,
        method,
    })
}

/// The exact zero-order hold. The exponential of `T·[[A, B], [0, 0]]` has `A_d = e^{AT}` in its
/// upper-left block and `B_d = ∫₀ᵀ e^{Aτ}dτ·B` in the block to its right.
fn exact_zoh(
    a: &DMatrix<f64>,
    b: &DMatrix<f64>,
    sample_time_seconds: f64,
) -> (DMatrix<f64>, DMatrix<f64>) {
    let states = a.nrows();
    let inputs = b.ncols();
    let mut augmented = DMatrix::<f64>::zeros(states + inputs, states + inputs);
    augmented.view_range_mut(0..states, 0..states).copy_from(a);
    augmented
        .view_range_mut(0..states, states..states + inputs)
        .copy_from(b);
    let exponential = augmented.scale(sample_time_seconds).exp();
    (
        exponential.view_range(0..states, 0..states).into_owned(),
        exponential
            .view_range(0..states, states..states + inputs)
            .into_owned(),
    )
}

#[cfg(test)]
mod tests;
