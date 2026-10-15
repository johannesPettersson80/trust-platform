//! The state-recovery scenario of a state-feedback law (specification 35, C4 and B5.3; `engineerControllerScenarios.ts`): the nominal runs
//! with the plant's initial state perturbed once, so the law must bring the plant back to its setpoint from a disturbed start. The
//! perturbation is one per cent of each model state's operating range, toward the side of the range it can move to. The case runs on the
//! selected model, as C4's Validate and B5.3 route it, and is judged on the nominal requirements.

use std::collections::BTreeMap;

use crate::failure::{Cause, Failure, NextAction};
use crate::loop_definition::LoopDefinition;
use crate::model::general::ModelChannel;
use crate::model::GeneralModel;
use crate::tuning::state_controller::sampling::{state_channels, state_space};
use crate::validation::engineer_verification_campaign::{CampaignCase, SuppliedPlant};
use crate::validation::engineer_verification_plan::VerificationPlan;
use crate::validation::model_route::model_route_from_state;

/// The identity of the recovery case in the campaign, its table and its plant (C4, B5.3).
pub const STATE_RECOVERY_ID: &str = "state-recovery";

/// The fraction of a model state's operating range that the recovery perturbs the state by.
const PERTURBATION_FRACTION: f64 = 0.01;

/// The refusal of a configured initial state that does not give one value per model state (C4).
const CONFIGURED_STATE_COUNT: &str =
    "State-feedback recovery needs one configured initial value per model state.";
/// The refusal of a model state whose operating range has no span, so that no perturbation can be sized (C4).
const SPAN_MISSING: &str = "State-feedback recovery requires finite nonzero model-state ranges.";
/// The refusal of a state that no perturbation toward either side of its range can move inside the range (C4).
const NO_ROOM: &str = "State-feedback recovery cannot perturb a state inside its declared range.";

/// The plant's initial state of the recovery scenario, in the model's coordinates (C4): each model state's configured initial value,
/// moved by one per cent of its operating range toward the side of the range the move stays inside. `None` for a loop without a
/// state-feedback law, and for a transfer-function model, which has no state to perturb.
///
/// # Errors
/// The configured initial state does not give one value per model state; a state's operating range has no span; or no side of a state's
/// range can take the perturbation.
pub fn perturbed_state(
    loop_definition: &LoopDefinition,
    model: &GeneralModel,
) -> Result<Option<Vec<f64>>, Failure> {
    if loop_definition.controller.state_feedback.is_none() || state_space(model).is_none() {
        return Ok(None);
    }
    let states = state_channels(model);
    let configured = loop_definition
        .channels
        .as_ref()
        .and_then(|channels| channels.initial_state.clone())
        .unwrap_or_else(|| vec![0.0; states.len()]);
    if configured.len() != states.len() {
        return Err(Failure::new(
            CONFIGURED_STATE_COUNT,
            Cause::Settings,
            NextAction::OpenLoopSetup,
        ));
    }
    states
        .iter()
        .zip(configured)
        .map(|(state, initial)| perturb(state, initial))
        .collect::<Result<Vec<f64>, Failure>>()
        .map(Some)
}

/// One state moved by the recovery's perturbation: up when the move stays inside the range, else down, else refused (C4).
fn perturb(state: &ModelChannel, initial: f64) -> Result<f64, Failure> {
    let minimum = state.operating_range.minimum;
    let maximum = state.operating_range.maximum;
    let span = maximum - minimum;
    if !(span > 0.0 && span.is_finite()) {
        return Err(Failure::new(
            SPAN_MISSING,
            Cause::ModelOrData,
            NextAction::Identify,
        ));
    }
    let delta = span * PERTURBATION_FRACTION;
    let up = initial + delta;
    let down = initial - delta;
    if up <= maximum && up >= minimum {
        Ok(up)
    } else if down >= minimum && down <= maximum {
        Ok(down)
    } else {
        Err(Failure::new(
            NO_ROOM,
            Cause::ModelOrData,
            NextAction::Identify,
        ))
    }
}

/// The recovery case of a state-feedback law (C4): the nominal plan, run from the perturbed plant state and judged on the nominal
/// requirements. `None` for a loop without a state law or a transfer-function model.
///
/// # Errors
/// The perturbation cannot be built (see [`perturbed_state`]).
pub fn recovery_case(
    loop_definition: &LoopDefinition,
    model: &GeneralModel,
    nominal: &VerificationPlan,
) -> Result<Option<CampaignCase>, Failure> {
    Ok(
        perturbed_state(loop_definition, model)?.map(|_| CampaignCase {
            id: STATE_RECOVERY_ID.to_owned(),
            plan: nominal.clone(),
            initial_overrides: BTreeMap::new(),
            events: Vec::new(),
            effects: Vec::new(),
            requirements: nominal.requirements.clone(),
        }),
    )
}

/// The plant of the recovery case (C4, B5.3): the selected model with its inline plant starting at the perturbed state, and the
/// controller's initial values at the model's operating point. `None` where [`perturbed_state`] is `None`.
///
/// # Errors
/// The perturbation cannot be built, or the model's route refuses (see `model_route_from_state`).
pub fn recovery_plant(
    loop_definition: &LoopDefinition,
    model: &GeneralModel,
    values: &BTreeMap<String, f64>,
) -> Result<Option<SuppliedPlant>, Failure> {
    let Some(state) = perturbed_state(loop_definition, model)? else {
        return Ok(None);
    };
    let route = model_route_from_state(loop_definition, model, values, &state)?;
    Ok(Some(SuppliedPlant {
        model: model.clone(),
        route,
    }))
}

#[cfg(test)]
#[path = "state_case_tests.rs"]
mod tests;
