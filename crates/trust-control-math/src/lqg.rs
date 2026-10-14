//! The steady-state Kalman estimator and the discrete LQG design (specification 35, C4; numerical annex N5.14). The
//! estimator iterates the covariance from `P = Q` until it settles: `P⁻ = APAᵀ + Q`, `S = CP⁻Cᵀ + R`, `L = P⁻CᵀS⁻¹` and
//! `P = (I − LC)P⁻`, where `Q` is the process noise and `R` the measurement noise. The LQG controller combines the LQR gain
//! `K` with the estimator gain `L`; by the separation principle its closed-loop poles are those of `A − BK` (the
//! controller) and of `(I − LC)A` (the estimator).

use nalgebra::DMatrix;

use crate::design_matrix::{check_shapes, max_abs, max_difference, to_matrix};
use crate::inverse::invert_matrix;
use crate::linalg::{
    eigenvalues, rows_from_matrix, ComplexValue, DenseMatrix, NumericalRejectionReason,
    NumericalTolerance,
};
use crate::lqr::{solve_discrete_lqr, FiniteHorizon, HorizonLimits, LqrRefusal, LqrSolution};

/// The most covariance iterations the steady-state estimator may take before it is refused.
const MAXIMUM_ITERATIONS: usize = 10_000;

/// The steady-state Kalman estimator: its gain `L` (n×p) and the settled covariance (n×n).
#[derive(Debug, Clone, PartialEq)]
pub struct KalmanGain {
    /// The gain `L`, for the correction `x̂ ← x̂ + L(y − Cx̂)`.
    pub gain: DenseMatrix,
    /// The settled covariance `P` after the correction.
    pub covariance: DenseMatrix,
    /// The largest change of the covariance in the last iteration.
    pub riccati_residual: f64,
}

/// Why the steady-state estimator is refused.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum KalmanRefusal {
    /// The shapes do not fit the estimator, or a value is not finite.
    Numerical(NumericalRejectionReason),
    /// The innovation covariance `CP⁻Cᵀ + R` has no inverse at the tolerance.
    SingularInnovation,
    /// The covariance did not settle within the iteration limit.
    RiccatiNotConverged,
}

/// The steady-state Kalman gain of `x(k+1) = Ax(k) + w(k)`, `y(k) = Cx(k) + v(k)`, with process noise covariance `Q` (n×n)
/// and measurement noise covariance `R` (p×p).
///
/// # Errors
/// Returns [`KalmanRefusal::Numerical`] for a shape or a value that does not fit, [`KalmanRefusal::SingularInnovation`]
/// when the innovation covariance has no inverse, and [`KalmanRefusal::RiccatiNotConverged`] when the covariance does
/// not settle within the iteration limit.
pub fn steady_state_kalman(
    a: &DenseMatrix,
    c: &DenseMatrix,
    process_noise: &DenseMatrix,
    measurement_noise: &DenseMatrix,
    tolerance: &NumericalTolerance,
) -> Result<KalmanGain, KalmanRefusal> {
    let order = a.len();
    let outputs = c.len();
    if order == 0 || outputs == 0 {
        return Err(KalmanRefusal::Numerical(
            NumericalRejectionReason::InconsistentDimensions,
        ));
    }
    check_shapes(&[
        (a, order, order),
        (c, outputs, order),
        (process_noise, order, order),
        (measurement_noise, outputs, outputs),
    ])
    .map_err(KalmanRefusal::Numerical)?;
    let state_matrix = to_matrix(a, order).map_err(KalmanRefusal::Numerical)?;
    let output_matrix = to_matrix(c, order).map_err(KalmanRefusal::Numerical)?;
    let process_matrix = to_matrix(process_noise, order).map_err(KalmanRefusal::Numerical)?;
    let measurement_matrix =
        to_matrix(measurement_noise, outputs).map_err(KalmanRefusal::Numerical)?;
    let identity = DMatrix::<f64>::identity(order, order);

    let mut covariance = process_matrix.clone();
    for _ in 0..MAXIMUM_ITERATIONS {
        let predicted = &state_matrix * &covariance * state_matrix.transpose() + &process_matrix;
        let innovation =
            &output_matrix * &predicted * output_matrix.transpose() + &measurement_matrix;
        let inverse =
            invert_matrix(&innovation, tolerance).map_err(|_| KalmanRefusal::SingularInnovation)?;
        let gain = &predicted * output_matrix.transpose() * &inverse;
        let next = (&identity - &gain * &output_matrix) * &predicted;
        let residual = max_difference(&next, &covariance);
        covariance = next;
        if residual <= tolerance.absolute + tolerance.relative * max_abs(&covariance) {
            return Ok(KalmanGain {
                gain: rows_from_matrix(&gain),
                covariance: rows_from_matrix(&covariance),
                riccati_residual: residual,
            });
        }
    }
    Err(KalmanRefusal::RiccatiNotConverged)
}

/// The LQG problem: the model `A, B, C`, the LQR weights `Q` and `R`, the noise covariances, and the finite horizon of
/// the LQR when there is one.
#[derive(Debug, Clone, Copy)]
pub struct LqgProblem<'a> {
    /// The state matrix `A` (n×n).
    pub a: &'a DenseMatrix,
    /// The input matrix `B` (n×m).
    pub b: &'a DenseMatrix,
    /// The output matrix `C` (p×n).
    pub c: &'a DenseMatrix,
    /// The state weight `Q` of the LQR (n×n).
    pub q: &'a DenseMatrix,
    /// The input weight `R` of the LQR (m×m).
    pub r: &'a DenseMatrix,
    /// The process noise covariance of the estimator (n×n).
    pub process_noise: &'a DenseMatrix,
    /// The measurement noise covariance of the estimator (p×p).
    pub measurement_noise: &'a DenseMatrix,
    /// The finite horizon of the LQR, or `None` for the steady-state gain.
    pub horizon: Option<&'a FiniteHorizon>,
    /// The product limits of a finite horizon.
    pub limits: HorizonLimits,
}

/// A solved LQG design: the LQR, the Kalman estimator, and the poles of the controller, the estimator and the closed loop.
#[derive(Debug, Clone, PartialEq)]
pub struct LqgDesign {
    /// The LQR gain and its stages.
    pub lqr: LqrSolution,
    /// The steady-state Kalman gain.
    pub kalman: KalmanGain,
    /// The eigenvalues of `A − B·K`.
    pub controller_poles: Vec<ComplexValue>,
    /// The eigenvalues of `(I − L·C)·A`.
    pub estimator_poles: Vec<ComplexValue>,
    /// The poles of the closed loop: the controller's, then the estimator's.
    pub separation_poles: Vec<ComplexValue>,
}

/// Why an LQG design is refused: the refusal of its LQR or of its estimator, or a pole calculation that fails.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum LqgRefusal {
    /// The LQR is refused.
    Lqr(LqrRefusal),
    /// The steady-state estimator is refused.
    Kalman(KalmanRefusal),
    /// The eigenvalues of the controller or the estimator cannot be computed.
    SolverFailed,
}

/// The LQG design of `problem` (N5.14): the LQR gain `K` and the Kalman gain `L`, with their poles.
///
/// # Errors
/// Returns [`LqgRefusal::Lqr`] when the LQR is refused, [`LqgRefusal::Kalman`] when the estimator is refused, and
/// [`LqgRefusal::SolverFailed`] when the eigenvalues of the controller or the estimator cannot be computed.
pub fn design_discrete_lqg(
    problem: &LqgProblem<'_>,
    tolerance: &NumericalTolerance,
) -> Result<LqgDesign, LqgRefusal> {
    let lqr = solve_discrete_lqr(
        problem.a,
        problem.b,
        problem.q,
        problem.r,
        tolerance,
        problem.horizon,
        problem.limits,
    )
    .map_err(LqgRefusal::Lqr)?;
    let kalman = steady_state_kalman(
        problem.a,
        problem.c,
        problem.process_noise,
        problem.measurement_noise,
        tolerance,
    )
    .map_err(LqgRefusal::Kalman)?;

    let order = problem.a.len();
    let inputs = problem.b.first().map_or(0, Vec::len);
    let outputs = problem.c.len();
    let state_matrix = to_matrix(problem.a, order).map_err(|_| LqgRefusal::SolverFailed)?;
    let input_matrix = to_matrix(problem.b, inputs).map_err(|_| LqgRefusal::SolverFailed)?;
    let output_matrix = to_matrix(problem.c, order).map_err(|_| LqgRefusal::SolverFailed)?;
    let feedback = to_matrix(&lqr.gain, order).map_err(|_| LqgRefusal::SolverFailed)?;
    let estimator_gain = to_matrix(&kalman.gain, outputs).map_err(|_| LqgRefusal::SolverFailed)?;
    let identity = DMatrix::<f64>::identity(order, order);

    let controller_matrix = &state_matrix - &input_matrix * &feedback;
    let estimator_matrix = (&identity - &estimator_gain * &output_matrix) * &state_matrix;
    let controller_poles = eigenvalues(&rows_from_matrix(&controller_matrix), tolerance)
        .map_err(|_| LqgRefusal::SolverFailed)?
        .values;
    let estimator_poles = eigenvalues(&rows_from_matrix(&estimator_matrix), tolerance)
        .map_err(|_| LqgRefusal::SolverFailed)?
        .values;
    let separation_poles = controller_poles
        .iter()
        .chain(&estimator_poles)
        .copied()
        .collect();
    Ok(LqgDesign {
        lqr,
        kalman,
        controller_poles,
        estimator_poles,
        separation_poles,
    })
}

#[cfg(test)]
#[path = "lqg_tests.rs"]
mod tests;
