//! The gains of the three families (C4; numerical annex N5.14): pole placement by Ackermann's formula, the LQR, and the LQG
//! with its Kalman estimator. An LQR or LQG uses the declared state and input weights, or the automatic weights, which
//! normalise each coordinate by its declared operating span (`1/span²`). Every weight and covariance must be finite,
//! symmetric and definite as its role requires, and an LQG estimator must be the selected sampled model exactly.

use trust_control_math::linalg::{eigenvalues, DenseMatrix};
use trust_control_math::lqg::{design_discrete_lqg, LqgProblem, LqgRefusal};
use trust_control_math::lqr::{solve_discrete_lqr, FiniteHorizon, FiniteStages, LqrRefusal};
use trust_control_math::state_feedback::place_state_feedback_poles;

use super::binding::StateControllerBinding;
use super::sampling::{state_channels, state_space, SampledModel};
use super::{
    is_symmetric_square, texts, StateControllerFamily, DESIGN_TOLERANCE, HORIZON_LIMITS,
    SYMMETRY_TOLERANCE,
};
use crate::failure::{Cause, Failure, NextAction};
use crate::model::GeneralModel;

/// The gains of a design: the state gain `K` (m×n), the observer gain `L` (n×p, LQG only), and the stages of a finite
/// horizon when there is one.
#[derive(Debug, Clone, PartialEq)]
pub struct DesignedGains {
    /// The state gain `K`.
    pub gain: DenseMatrix,
    /// The observer gain `L` of an LQG law.
    pub observer: Option<DenseMatrix>,
    /// The stages of a finite horizon.
    pub stages: Option<FiniteStages>,
}

/// The gains of the family of `binding` on `sampled`. The finite horizon applies to the LQR and the LQG only.
///
/// # Errors
/// Returns the failure of the family's rule: desired poles outside the unit circle, an uncontrollable plant, a weight that
/// is not admissible, an estimator that is not the selected model, or a design that does not qualify.
pub fn design_gains(
    binding: &StateControllerBinding,
    finite_horizon: Option<&FiniteHorizon>,
    model: &GeneralModel,
    sampled: &SampledModel,
    response_time_seconds: f64,
) -> Result<DesignedGains, Failure> {
    match binding.family {
        StateControllerFamily::StateFeedback => {
            state_feedback(binding, sampled, response_time_seconds)
        }
        StateControllerFamily::Lqr => lqr(binding, finite_horizon, model, sampled),
        StateControllerFamily::Lqg => lqg(binding, finite_horizon, model, sampled),
    }
}

/// Pole placement (N5.14): the declared discrete poles, or the default poles when none are declared. The default pole
/// `i` is `exp(−(4 + i/n)·Δt/T_response)`, so the poles are spread between the response time and a faster one.
fn state_feedback(
    binding: &StateControllerBinding,
    sampled: &SampledModel,
    response_time_seconds: f64,
) -> Result<DesignedGains, Failure> {
    let order = sampled.system.states();
    let desired = binding.desired_poles.clone().unwrap_or_else(|| {
        (0..order)
            .map(|index| {
                let spread = count_as_f64(index) / count_as_f64(order.max(1));
                (-(4.0 + spread) * sampled.sample_time_seconds / response_time_seconds).exp()
            })
            .collect()
    });
    if desired.len() != order
        || desired
            .iter()
            .any(|pole| !pole.is_finite() || pole.abs() >= 1.0)
    {
        return Err(Failure::new(
            texts::STRICT_UNIT_CIRCLE_POLES,
            Cause::Settings,
            NextAction::None,
        ));
    }
    let placement = place_state_feedback_poles(
        &sampled.system.a,
        &sampled.system.b,
        &desired,
        &DESIGN_TOLERANCE,
    )
    .map_err(|refusal| {
        let text = texts::state_feedback_refused(texts::placement_reason(refusal));
        match refusal {
            trust_control_math::state_feedback::PlacementRefusal::Uncontrollable => {
                Failure::new(&text, Cause::ControllerCapability, NextAction::ChooseMethod)
            }
            _ => Failure::new(&text, Cause::Unclassified, NextAction::None),
        }
    })?;
    Ok(DesignedGains {
        gain: placement.gain,
        observer: None,
        stages: None,
    })
}

/// The LQR (N5.14): the declared or automatic weights, and the steady-state or finite-horizon gain.
fn lqr(
    binding: &StateControllerBinding,
    finite_horizon: Option<&FiniteHorizon>,
    model: &GeneralModel,
    sampled: &SampledModel,
) -> Result<DesignedGains, Failure> {
    let (state_weight, input_weight) = lqr_weights(binding, model)?;
    let solution = solve_discrete_lqr(
        &sampled.system.a,
        &sampled.system.b,
        &state_weight,
        &input_weight,
        &DESIGN_TOLERANCE,
        finite_horizon,
        HORIZON_LIMITS,
    )
    .map_err(lqr_failure)?;
    repeated_gain_stable(solution.finite.as_ref())?;
    Ok(DesignedGains {
        gain: solution.gain,
        observer: None,
        stages: solution.finite,
    })
}

/// The refusal of a finite-horizon design whose repeated first gain does not stabilise the loop (N5.14, D-B8): the runtime
/// repeats `K_0`, so a law that the repetition destabilises cannot be run. A steady-state design has no stages to check.
fn repeated_gain_stable(stages: Option<&FiniteStages>) -> Result<(), Failure> {
    match stages {
        Some(stages) if !stages.repeated_first_gain_stable => Err(Failure::new(
            texts::REPEATED_GAIN_UNSTABLE,
            Cause::ControllerCapability,
            NextAction::ChooseMethod,
        )),
        _ => Ok(()),
    }
}

/// The LQG (N5.14): the LQR of the declared weights, with the Kalman estimator of the declared covariances on the model
/// the estimator declares, which must be the selected sampled model.
fn lqg(
    binding: &StateControllerBinding,
    finite_horizon: Option<&FiniteHorizon>,
    model: &GeneralModel,
    sampled: &SampledModel,
) -> Result<DesignedGains, Failure> {
    let (state_weight, input_weight) = lqr_weights(binding, model)?;
    let estimator = binding.estimator_model.as_ref().ok_or_else(|| {
        Failure::new(
            texts::NEEDS_BINDING,
            Cause::Implementation,
            NextAction::OpenLoopSetup,
        )
    })?;
    if !estimator_matches(estimator, sampled) {
        return Err(Failure::new(
            texts::ESTIMATOR_MODEL_DIFFERS,
            Cause::ModelOrData,
            NextAction::OpenLoopSetup,
        ));
    }
    let feedthrough = state_space(model)
        .is_some_and(|system| system.d.iter().flatten().any(|value| value.abs() > 0.0));
    if feedthrough {
        return Err(Failure::new(
            texts::LQG_FEEDTHROUGH,
            Cause::ControllerCapability,
            NextAction::ChooseMethod,
        ));
    }
    let (Some(process_noise), Some(measurement_noise)) =
        (&binding.process_noise, &binding.measurement_noise)
    else {
        return Err(Failure::new(
            texts::COVARIANCE_MISSING,
            Cause::Settings,
            NextAction::None,
        ));
    };
    let states = sampled.system.states();
    let outputs = sampled.system.outputs();
    check_weight(process_noise, states, false)?;
    check_weight(measurement_noise, outputs, true)?;
    let problem = LqgProblem {
        a: &sampled.system.a,
        b: &sampled.system.b,
        c: &sampled.system.c,
        q: &state_weight,
        r: &input_weight,
        process_noise,
        measurement_noise,
        horizon: finite_horizon,
        limits: HORIZON_LIMITS,
    };
    let design = design_discrete_lqg(&problem, &DESIGN_TOLERANCE).map_err(lqg_failure)?;
    repeated_gain_stable(design.lqr.finite.as_ref())?;
    Ok(DesignedGains {
        gain: design.lqr.gain,
        observer: Some(design.kalman.gain),
        stages: design.lqr.finite,
    })
}

/// The state and input weights of an LQR or LQG law: the declared ones, or the automatic ones, and each must be admissible.
fn lqr_weights(
    binding: &StateControllerBinding,
    model: &GeneralModel,
) -> Result<(DenseMatrix, DenseMatrix), Failure> {
    let states = state_channels(model);
    let inputs = &model.identity().inputs;
    let state_weight = match &binding.state_weights {
        Some(weight) => weight.clone(),
        None => automatic_weights(states.iter().map(|channel| channel.operating_range))?,
    };
    let input_weight = match &binding.input_weights {
        Some(weight) => weight.clone(),
        None => automatic_weights(inputs.iter().map(|channel| channel.operating_range))?,
    };
    check_weight(&state_weight, states.len(), false)?;
    check_weight(&input_weight, inputs.len(), true)?;
    Ok((state_weight, input_weight))
}

/// The diagonal weight whose entry `i` is `1/span²` of the coordinate `i` (N5.14, automatic weights). A coordinate with no
/// span has no such weight, so the design is refused before it runs (D-B13).
pub(super) fn automatic_weights(
    ranges: impl Iterator<Item = crate::model::general::OperatingRange>,
) -> Result<DenseMatrix, Failure> {
    let diagonal = ranges
        .map(|range| {
            let span = range.maximum - range.minimum;
            (span > 0.0).then_some(1.0 / (span * span)).ok_or_else(|| {
                Failure::new(
                    texts::ZERO_OPERATING_SPAN,
                    Cause::ModelOrData,
                    NextAction::Identify,
                )
            })
        })
        .collect::<Result<Vec<f64>, Failure>>()?;
    Ok(diagonal
        .iter()
        .enumerate()
        .map(|(row, value)| {
            diagonal
                .iter()
                .enumerate()
                .map(|(column, _)| if row == column { *value } else { 0.0 })
                .collect()
        })
        .collect())
}

/// A weight or covariance of `size` rows: finite and symmetric, with eigenvalues that are real and positive (`strict`) or
/// not negative, to the tolerance of the design (`positiveWeight`).
fn check_weight(matrix: &DenseMatrix, size: usize, strict: bool) -> Result<(), Failure> {
    if !is_symmetric_square(matrix, size) || !matrix.iter().flatten().all(|v| v.is_finite()) {
        return Err(Failure::new(
            texts::WEIGHTS_NOT_MATRICES,
            Cause::Settings,
            NextAction::None,
        ));
    }
    let definite = eigenvalues(matrix, &DESIGN_TOLERANCE).is_ok_and(|spectrum| {
        spectrum.values.iter().all(|value| {
            value.imaginary.abs() <= 1e-9
                && if strict {
                    value.real > 0.0
                } else {
                    value.real >= -SYMMETRY_TOLERANCE
                }
        })
    });
    if definite {
        Ok(())
    } else {
        Err(Failure::new(
            texts::WEIGHTS_NOT_DEFINITE,
            Cause::Settings,
            NextAction::None,
        ))
    }
}

/// Whether the estimator's model is the sampled model: the same sample period and the same matrices, exactly.
#[allow(clippy::float_cmp)] // The estimator must run the selected model's coefficients exactly (C4), not nearly.
fn estimator_matches(estimator: &super::binding::EstimatorModel, sampled: &SampledModel) -> bool {
    let same_matrix = |left: &DenseMatrix, right: &DenseMatrix| {
        left.len() == right.len()
            && left.iter().zip(right).all(|(left_row, right_row)| {
                left_row.len() == right_row.len()
                    && left_row.iter().zip(right_row).all(|(l, r)| l == r)
            })
    };
    estimator.sample_time_seconds == sampled.sample_time_seconds
        && same_matrix(&estimator.a, &sampled.system.a)
        && same_matrix(&estimator.b, &sampled.system.b)
        && same_matrix(&estimator.c, &sampled.system.c)
}

/// The failure of a refused LQR: an invalid horizon is a setting, a numerical fault is unclassified, and the other
/// refusals mean the method cannot do what is asked.
fn lqr_failure(refusal: LqrRefusal) -> Failure {
    let text = texts::lqr_refused(texts::lqr_reason(refusal));
    match refusal {
        LqrRefusal::InvalidFiniteHorizon | LqrRefusal::MalformedWeight => {
            Failure::new(&text, Cause::Settings, NextAction::None)
        }
        LqrRefusal::Numerical(_) => Failure::new(&text, Cause::Unclassified, NextAction::None),
        _ => Failure::new(&text, Cause::ControllerCapability, NextAction::ChooseMethod),
    }
}

/// The failure of a refused LQG design, classified as [`lqr_failure`] classifies its LQR.
fn lqg_failure(refusal: LqgRefusal) -> Failure {
    let text = texts::lqg_refused(texts::lqg_reason(refusal));
    match refusal {
        LqgRefusal::Lqr(LqrRefusal::InvalidFiniteHorizon) => {
            Failure::new(&text, Cause::Settings, NextAction::None)
        }
        LqgRefusal::Lqr(LqrRefusal::Numerical(_)) | LqgRefusal::SolverFailed => {
            Failure::new(&text, Cause::Unclassified, NextAction::None)
        }
        _ => Failure::new(&text, Cause::ControllerCapability, NextAction::ChooseMethod),
    }
}

/// A count as a float: the counts of a model are far below 2⁵³, so the conversion is exact.
#[allow(clippy::cast_precision_loss)] // A state count is at most 32 (C4), far below 2⁵³.
fn count_as_f64(count: usize) -> f64 {
    count as f64
}
