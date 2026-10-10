//! The product wrappers of delayedModelRealization.ts and frozenModelPrediction.ts, and the plant of a
//! model that they share with the time response and the simulation.

use trust_control_math::model::{transfer_function_state_space, StateSpace, Timing};
use trust_control_math::simulate::{
    delayed_realization, frozen_model_prediction as frozen_prediction, DelayedSystem,
    FrozenPrediction, Realization,
};

use super::general::GeneralModel;
use super::limits::MAXIMUM_DELAY_STATES;
use super::refusal::{companion_refusal, simulation_refusal, ModelRefusal};

/// The plant of a model: a transfer function's controllable companion realization, or a state-space
/// model's own matrices, with its timing and its delay.
///
/// # Errors
/// Returns the sentence for a transfer function that cannot be realized.
pub(crate) fn delayed_system_of(model: &GeneralModel) -> Result<DelayedSystem, ModelRefusal> {
    let delay_seconds = model.identity().delay_seconds;
    let (system, timing) = match model {
        GeneralModel::TransferFunction { transfer, .. } => (
            transfer_function_state_space(&transfer.numerator, &transfer.denominator)
                .map_err(companion_refusal)?
                .system,
            Timing::Continuous,
        ),
        GeneralModel::ContinuousStateSpace { system, .. }
        | GeneralModel::DiscreteStateSpace { system, .. } => (system.clone(), model.timing()),
    };
    Ok(DelayedSystem {
        system,
        timing,
        delay_seconds,
    })
}

/// The matrices and timing of a model's plant, or `None` when the plant cannot be formed.
pub(crate) fn plant_shape(model: &GeneralModel) -> Option<(StateSpace, Timing)> {
    delayed_system_of(model)
        .ok()
        .map(|plant| (plant.system, plant.timing))
}

/// The exact realization of a model at a sample period, from its initial state and its held input.
///
/// # Errors
/// Returns the sentence of the failed rule: the coordinates, the delay or period, a discrete delay
/// that is not whole, the state budget, the exact conversion, or a non-finite matrix.
pub fn delayed_model_realization(
    model: &GeneralModel,
    step_seconds: f64,
    initial_state: &[f64],
    initial_input: &[f64],
) -> Result<Realization, ModelRefusal> {
    let plant = delayed_system_of(model)?;
    delayed_realization(
        &plant,
        step_seconds,
        initial_state,
        initial_input,
        MAXIMUM_DELAY_STATES,
    )
    .map_err(simulation_refusal)
}

/// The frozen-model prediction of a recording. The history rule applies when the model is discrete
/// and every state is a recorded history slot named `history-<index>`.
///
/// # Errors
/// Returns the sentence of the failed rule of the prediction or of the realization.
pub fn frozen_model_prediction(
    model: &GeneralModel,
    inputs: &[Vec<f64>],
    outputs: &[Vec<f64>],
    step_seconds: f64,
) -> Result<FrozenPrediction, ModelRefusal> {
    let plant = delayed_system_of(model)?;
    let history_states = matches!(
        model,
        GeneralModel::DiscreteStateSpace { states, .. }
            if states
                .iter()
                .enumerate()
                .all(|(index, state)| state.id == format!("history-{index}"))
    );
    frozen_prediction(
        &plant,
        model.identity().operating_point.as_ref(),
        inputs,
        outputs,
        step_seconds,
        history_states,
        MAXIMUM_DELAY_STATES,
    )
    .map_err(simulation_refusal)
}
