//! The sampled model of a state-controller design (N4.7(a) and N5.14; specification 35, C4). A discrete model is used as it
//! is, at its own sample period, which must be the task period. A continuous model is discretized at the task period by
//! the exact zero-order hold: `A_d = e^{AΔt}` and `B_d = ∫₀^{Δt} e^{Aτ}dτ · B`.

use trust_control_math::discretize::{discretize_state_space, DiscretizationMethod};
use trust_control_math::model::StateSpace;

use super::texts;
use crate::failure::{Cause, Failure, NextAction};
use crate::model::general::ModelChannel;
use crate::model::GeneralModel;

/// The sample period at which a discrete model may be used: a difference from the task period above this is refused.
const SAMPLE_PERIOD_TOLERANCE_SECONDS: f64 = 1e-9;

/// A state-space model at the task period: its matrices and the period they are sampled at.
#[derive(Debug, Clone, PartialEq)]
pub struct SampledModel {
    /// The sampled matrices `A`, `B`, `C` and `D`.
    pub system: StateSpace,
    /// The sample period `Δt` in seconds: the task period.
    pub sample_time_seconds: f64,
}

/// The state-space matrices of a model, or `None` for a transfer function.
#[must_use]
pub fn state_space(model: &GeneralModel) -> Option<&StateSpace> {
    match model {
        GeneralModel::ContinuousStateSpace { system, .. }
        | GeneralModel::DiscreteStateSpace { system, .. } => Some(system),
        GeneralModel::TransferFunction { .. } => None,
    }
}

/// The state channels of a model, in the order of its matrices; empty for a transfer function.
#[must_use]
pub fn state_channels(model: &GeneralModel) -> &[ModelChannel] {
    match model {
        GeneralModel::ContinuousStateSpace { states, .. }
        | GeneralModel::DiscreteStateSpace { states, .. } => states,
        GeneralModel::TransferFunction { .. } => &[],
    }
}

/// The model at the task period `period_seconds`: a discrete model at its own period, or a continuous model discretized
/// by the exact zero-order hold.
///
/// # Errors
/// Returns a failure when the model is not a state-space model, when a discrete model's period differs from the task
/// period, or when the continuous model cannot be discretized.
pub fn sample_model(model: &GeneralModel, period_seconds: f64) -> Result<SampledModel, Failure> {
    let system = state_space(model).ok_or_else(|| {
        Failure::new(
            texts::COORDINATES_UNAVAILABLE,
            Cause::ModelOrData,
            NextAction::ChooseMethod,
        )
    })?;
    if let GeneralModel::DiscreteStateSpace {
        sample_time_seconds,
        ..
    } = model
    {
        if (sample_time_seconds - period_seconds).abs() > SAMPLE_PERIOD_TOLERANCE_SECONDS {
            return Err(Failure::new(
                texts::SAMPLE_PERIODS_DIFFER,
                Cause::ModelOrData,
                NextAction::OpenLoopSetup,
            ));
        }
        return Ok(SampledModel {
            system: system.clone(),
            sample_time_seconds: period_seconds,
        });
    }
    let discrete = discretize_state_space(system, period_seconds, DiscretizationMethod::ExactZoh)
        .map_err(|error| {
        Failure::new(
            &texts::exact_discretization(texts::numerical_reason(error.reason)),
            Cause::ModelOrData,
            NextAction::Identify,
        )
    })?;
    Ok(SampledModel {
        system: discrete.system,
        sample_time_seconds: period_seconds,
    })
}
