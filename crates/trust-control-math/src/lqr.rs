//! Discrete linear-quadratic regulators (specification 35, C4; numerical annex N5.14). The steady-state gain comes from the
//! algebraic Riccati equation `P = AᵀPA − AᵀPB(R + BᵀPB)⁻¹BᵀPA + Q`, iterated from `P = Q` until it settles, with the gain
//! `K = (R + BᵀPB)⁻¹BᵀPA` and the law `u = −K·x`. A finite horizon of `N` steps runs the same step backwards from the
//! terminal weight `P_N`, giving the gains `K_0 … K_{N−1}`; the runtime repeats the first gain `K_0`.

use nalgebra::DMatrix;
use schemars::JsonSchema;
use serde::{Deserialize, Serialize};

use crate::design_matrix::{
    check_shapes, is_finite_matrix, is_matrix, max_abs, max_difference, to_matrix,
};
use crate::inverse::invert_matrix;
use crate::linalg::{
    eigenvalues, rows_from_matrix, ComplexValue, DenseMatrix, NumericalRejectionReason,
    NumericalTolerance,
};

/// The most Riccati iterations the steady-state solution may take before it is refused.
const MAXIMUM_ITERATIONS: usize = 10_000;

/// A finite horizon: `steps` control steps `N` and the terminal weight `P_N`, an n×n matrix.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct FiniteHorizon {
    /// The number of control steps `N`.
    pub steps: usize,
    /// The terminal weight `P_N`.
    pub terminal_weights: DenseMatrix,
}

/// The product limits of a finite-horizon design: the most steps, states and inputs it accepts.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct HorizonLimits {
    /// The largest number of steps.
    pub maximum_steps: usize,
    /// The largest number of states.
    pub maximum_states: usize,
    /// The largest number of inputs.
    pub maximum_inputs: usize,
}

/// The stages of a finite-horizon design: the gains `K_0 … K_{N−1}` and the Riccati matrices `P_0 … P_N`, in step order.
#[derive(Debug, Clone, PartialEq)]
pub struct FiniteStages {
    /// The gains `K_k`, from `K_0` to `K_{N−1}`.
    pub gain_sequence: Vec<DenseMatrix>,
    /// The Riccati matrices `P_k`, from `P_0` to the terminal `P_N`.
    pub riccati_sequence: Vec<DenseMatrix>,
    /// Whether the loop with the first gain repeated is stable: every pole of `A − B·K_0` is strictly inside the unit
    /// circle.
    pub repeated_first_gain_stable: bool,
}

/// A solved LQR.
#[derive(Debug, Clone, PartialEq)]
pub struct LqrSolution {
    /// The gain `K` (m×n) for `u = −K·x`: the steady-state gain, or the first gain `K_0` of a finite horizon.
    pub gain: DenseMatrix,
    /// The Riccati matrix `P`: the steady-state solution, or the first stage `P_0` of a finite horizon.
    pub riccati: DenseMatrix,
    /// The eigenvalues of `A − B·K`.
    pub closed_loop_poles: Vec<ComplexValue>,
    /// The largest entry of the residual of the algebraic equation at the solution; `None` for a finite horizon.
    pub riccati_residual: Option<f64>,
    /// The stages of a finite horizon; `None` for the steady-state solution.
    pub finite: Option<FiniteStages>,
}

/// Why an LQR is refused.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum LqrRefusal {
    /// The shapes do not fit the problem, a value is not finite, or the closed-loop eigenvalues cannot be computed.
    Numerical(NumericalRejectionReason),
    /// The finite horizon's steps or size are outside their limits, or a weight is not symmetric and of the required
    /// definiteness.
    InvalidFiniteHorizon,
    /// `R + BᵀPB` has no inverse at the tolerance.
    SingularControlWeight,
    /// The steady-state recursion did not settle within its iteration limit.
    RiccatiNotConverged,
    /// The steady-state solution does not stabilise the loop: a pole is not strictly inside the unit circle, or its
    /// eigenvalues cannot be computed.
    NonstabilisingSolution,
    /// A stage of the finite-horizon recursion is not finite.
    NonfiniteRiccatiStage,
    /// A weight is not symmetric, or its eigenvalues are not of the definiteness its role requires (`Q` and the terminal
    /// weight not negative, `R` positive), on the steady-state path (C4). The finite horizon refuses the same weights as
    /// [`LqrRefusal::InvalidFiniteHorizon`].
    MalformedWeight,
}

/// The LQR gain for `x(k+1) = Ax(k) + Bu(k)` with the cost `Σ xᵀQx + uᵀRu`, where `Q` is n×n and `R` is m×m. Without a
/// horizon it is the steady-state solution of the algebraic Riccati equation; with one it is the first gain of the
/// finite-horizon recursion, and every stage is returned.
///
/// # Errors
/// Returns [`LqrRefusal`]: `Numerical` for a shape or a value that does not fit (a steady-state problem), and
/// `InvalidFiniteHorizon` for a finite horizon whose steps, size or weights are refused. The steady-state solution is
/// refused as `MalformedWeight` for a weight that is not admissible, and as `SingularControlWeight`, `RiccatiNotConverged`
/// or `NonstabilisingSolution`; a finite horizon as `SingularControlWeight` or `NonfiniteRiccatiStage`.
pub fn solve_discrete_lqr(
    a: &DenseMatrix,
    b: &DenseMatrix,
    q: &DenseMatrix,
    r: &DenseMatrix,
    tolerance: &NumericalTolerance,
    horizon: Option<&FiniteHorizon>,
    limits: HorizonLimits,
) -> Result<LqrSolution, LqrRefusal> {
    match horizon {
        None => {
            let (order, inputs) = steady_shape(a, b, q, r).map_err(LqrRefusal::Numerical)?;
            if !is_weight(q, order, false, tolerance) || !is_weight(r, inputs, true, tolerance) {
                return Err(LqrRefusal::MalformedWeight);
            }
            let problem = Problem::new(a, b, q, r, order, inputs).map_err(LqrRefusal::Numerical)?;
            solve_steady_state(&problem, tolerance)
        }
        Some(horizon) => solve_finite_horizon(a, b, q, r, tolerance, horizon, limits),
    }
}

/// The matrices of a problem in nalgebra form.
struct Problem {
    a: DMatrix<f64>,
    b: DMatrix<f64>,
    q: DMatrix<f64>,
    r: DMatrix<f64>,
}

impl Problem {
    fn new(
        a: &DenseMatrix,
        b: &DenseMatrix,
        q: &DenseMatrix,
        r: &DenseMatrix,
        order: usize,
        inputs: usize,
    ) -> Result<Self, NumericalRejectionReason> {
        Ok(Self {
            a: to_matrix(a, order)?,
            b: to_matrix(b, inputs)?,
            q: to_matrix(q, order)?,
            r: to_matrix(r, inputs)?,
        })
    }
}

/// The state and input counts of a steady-state problem, after its shapes and values are checked.
fn steady_shape(
    a: &DenseMatrix,
    b: &DenseMatrix,
    q: &DenseMatrix,
    r: &DenseMatrix,
) -> Result<(usize, usize), NumericalRejectionReason> {
    let order = a.len();
    let inputs = b.first().map_or(0, Vec::len);
    if order == 0 || inputs == 0 {
        return Err(NumericalRejectionReason::InconsistentDimensions);
    }
    check_shapes(&[
        (a, order, order),
        (b, order, inputs),
        (q, order, order),
        (r, inputs, inputs),
    ])?;
    Ok((order, inputs))
}

fn solve_steady_state(
    problem: &Problem,
    tolerance: &NumericalTolerance,
) -> Result<LqrSolution, LqrRefusal> {
    let mut riccati = problem.q.clone();
    let mut settled = false;
    for _ in 0..MAXIMUM_ITERATIONS {
        let next =
            riccati_step(problem, &riccati, tolerance).ok_or(LqrRefusal::SingularControlWeight)?;
        let converged = max_difference(&next, &riccati)
            <= tolerance.absolute + tolerance.relative * max_abs(&next);
        riccati = next;
        if converged {
            settled = true;
            break;
        }
    }
    if !settled {
        return Err(LqrRefusal::RiccatiNotConverged);
    }
    let inverse = invert_matrix(&control_weight(problem, &riccati), tolerance)
        .map_err(|_| LqrRefusal::SingularControlWeight)?;
    let gain = &inverse * problem.b.transpose() * &riccati * &problem.a;
    let next =
        riccati_step(problem, &riccati, tolerance).ok_or(LqrRefusal::SingularControlWeight)?;
    let residual = &riccati - next;
    let closed_loop = &problem.a - &problem.b * &gain;
    let spectrum = eigenvalues(&rows_from_matrix(&closed_loop), tolerance)
        .map_err(|_| LqrRefusal::NonstabilisingSolution)?;
    if spectrum
        .values
        .iter()
        .any(|pole| pole.real.hypot(pole.imaginary) >= 1.0)
    {
        return Err(LqrRefusal::NonstabilisingSolution);
    }
    Ok(LqrSolution {
        gain: rows_from_matrix(&gain),
        riccati: rows_from_matrix(&riccati),
        closed_loop_poles: spectrum.values,
        riccati_residual: Some(max_abs(&residual)),
        finite: None,
    })
}

fn solve_finite_horizon(
    a: &DenseMatrix,
    b: &DenseMatrix,
    q: &DenseMatrix,
    r: &DenseMatrix,
    tolerance: &NumericalTolerance,
    horizon: &FiniteHorizon,
    limits: HorizonLimits,
) -> Result<LqrSolution, LqrRefusal> {
    let order = a.len();
    let inputs = b.first().map_or(0, Vec::len);
    let valid = (1..=limits.maximum_steps).contains(&horizon.steps)
        && (1..=limits.maximum_states).contains(&order)
        && (1..=limits.maximum_inputs).contains(&inputs)
        && is_matrix(a, order, order)
        && is_matrix(b, order, inputs)
        && is_weight(q, order, false, tolerance)
        && is_weight(r, inputs, true, tolerance)
        && is_weight(&horizon.terminal_weights, order, false, tolerance);
    if !valid {
        return Err(LqrRefusal::InvalidFiniteHorizon);
    }
    let problem =
        Problem::new(a, b, q, r, order, inputs).map_err(|_| LqrRefusal::InvalidFiniteHorizon)?;
    let mut riccati = to_matrix(&horizon.terminal_weights, order)
        .map_err(|_| LqrRefusal::InvalidFiniteHorizon)?;
    let mut riccati_sequence = vec![rows_from_matrix(&riccati)];
    let mut gains: Vec<DMatrix<f64>> = Vec::with_capacity(horizon.steps);
    for _ in 0..horizon.steps {
        let inverse = invert_matrix(&control_weight(&problem, &riccati), tolerance)
            .map_err(|_| LqrRefusal::SingularControlWeight)?;
        let gain = &inverse * problem.b.transpose() * &riccati * &problem.a;
        let next = riccati_update(&problem, &riccati, &inverse);
        if !is_finite_matrix(&gain) || !is_finite_matrix(&next) {
            return Err(LqrRefusal::NonfiniteRiccatiStage);
        }
        gains.push(gain);
        riccati = next;
        riccati_sequence.push(rows_from_matrix(&riccati));
    }
    // The recursion ran backwards from P_N, so the stages are reversed into step order.
    gains.reverse();
    riccati_sequence.reverse();
    let first_gain = gains
        .first()
        .cloned()
        .ok_or(LqrRefusal::InvalidFiniteHorizon)?;
    let closed_loop = &problem.a - &problem.b * &first_gain;
    let spectrum = eigenvalues(&rows_from_matrix(&closed_loop), tolerance)
        .map_err(|_| LqrRefusal::Numerical(NumericalRejectionReason::SolverFailed))?;
    let repeated_first_gain_stable = spectrum
        .values
        .iter()
        .all(|pole| pole.real.hypot(pole.imaginary) < 1.0);
    let riccati_first = riccati_sequence
        .first()
        .cloned()
        .ok_or(LqrRefusal::InvalidFiniteHorizon)?;
    Ok(LqrSolution {
        gain: rows_from_matrix(&first_gain),
        riccati: riccati_first,
        closed_loop_poles: spectrum.values,
        riccati_residual: None,
        finite: Some(FiniteStages {
            gain_sequence: gains.iter().map(rows_from_matrix).collect(),
            riccati_sequence,
            repeated_first_gain_stable,
        }),
    })
}

/// A weight is an n×n matrix, symmetric to the absolute tolerance, whose eigenvalues are real and positive (`strict`,
/// for `R`) or not negative (for `Q` and the terminal weight), to the tolerance.
fn is_weight(
    matrix: &DenseMatrix,
    size: usize,
    strict: bool,
    tolerance: &NumericalTolerance,
) -> bool {
    if !is_matrix(matrix, size, size) {
        return false;
    }
    let symmetric = matrix.iter().enumerate().all(|(row, values)| {
        values
            .iter()
            .enumerate()
            .all(|(column, value)| (value - matrix[column][row]).abs() <= tolerance.absolute)
    });
    if !symmetric {
        return false;
    }
    eigenvalues(matrix, tolerance).is_ok_and(|spectrum| {
        spectrum.values.iter().all(|pole| {
            pole.imaginary.abs() <= tolerance.absolute
                && if strict {
                    pole.real > 0.0
                } else {
                    pole.real >= -tolerance.absolute
                }
        })
    })
}

/// One Riccati step, `None` when `R + BᵀPB` has no inverse at the tolerance.
fn riccati_step(
    problem: &Problem,
    p: &DMatrix<f64>,
    tolerance: &NumericalTolerance,
) -> Option<DMatrix<f64>> {
    let inverse = invert_matrix(&control_weight(problem, p), tolerance).ok()?;
    Some(riccati_update(problem, p, &inverse))
}

/// `R + BᵀPB`.
fn control_weight(problem: &Problem, p: &DMatrix<f64>) -> DMatrix<f64> {
    &problem.r + problem.b.transpose() * p * &problem.b
}

/// `AᵀPA − AᵀPB·(R + BᵀPB)⁻¹·BᵀPA + Q`, given the inverse of `R + BᵀPB`.
fn riccati_update(problem: &Problem, p: &DMatrix<f64>, inverse: &DMatrix<f64>) -> DMatrix<f64> {
    let a_transpose_p = problem.a.transpose() * p;
    let correction = &a_transpose_p * &problem.b * inverse * problem.b.transpose() * p * &problem.a;
    a_transpose_p * &problem.a - correction + &problem.q
}

#[cfg(test)]
#[path = "lqr_tests.rs"]
mod tests;
