//! The design of a state-feedback law on a selected model (B4.4 and C4; numerical annex N5.13 and N5.14). The gains of the
//! declared family are mapped onto the declared parameters, the closed-loop poles are read back from the mapped values,
//! and the result is a provisional candidate. Nothing here writes to a runtime; the candidate is never applied (C4).

use std::collections::BTreeMap;

use schemars::JsonSchema;
use serde::Serialize;
use trust_control_math::linalg::{ComplexValue, DenseMatrix};

use super::binding::{decode_state_controller_binding, StateControllerBinding};
use super::coefficients::{map_coefficients, ratio_unit};
use super::families::{design_gains, DesignedGains};
use super::poles::closed_loop_poles;
use super::preferences::StateDesignPreferences;
use super::reference::reference_gain;
use super::sampling::{sample_model, state_channels, state_space};
use super::{texts, StateControllerFamily};
use crate::failure::{Cause, Failure, NextAction};
use crate::loop_definition::{LoopDefinition, Parameter};
use crate::model::GeneralModel;

/// The status of a state-controller design: provisional until the actual Structured Text verifies it (C4).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, JsonSchema)]
#[serde(rename_all = "lowercase")]
pub enum DesignStatus {
    /// Designed, not yet verified on the actual Structured Text.
    Provisional,
}

/// The candidate: the coefficient values that a design writes, which stay unapplied (C4).
#[derive(Debug, Clone, PartialEq, Serialize, JsonSchema)]
#[serde(rename_all = "camelCase")]
pub struct StateCandidate {
    /// Always `false`: a design never applies its candidate.
    pub applied: bool,
    /// The family of the law.
    pub structure: StateControllerFamily,
    /// The coefficient values, by parameter symbol.
    pub parameters: BTreeMap<String, f64>,
}

/// The model the design was made on.
#[derive(Debug, Clone, PartialEq, Serialize, JsonSchema)]
#[serde(rename_all = "camelCase")]
pub struct SelectedModel {
    /// The model's revision.
    pub revision: String,
    /// Where the model came from and whether it is validated, for example `declared-fitted-validated`.
    pub provenance: String,
}

/// The policy that runs a finite-horizon design (N5.14): the first gain repeats.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, JsonSchema)]
#[serde(rename_all = "kebab-case")]
pub enum HorizonPolicy {
    /// The runtime repeats the first finite gain `K_0`.
    RepeatFirstFiniteGain,
}

/// The stages of a finite-horizon design.
#[derive(Debug, Clone, PartialEq, Serialize, JsonSchema)]
#[serde(rename_all = "camelCase")]
pub struct StateHorizon {
    /// The number of control steps.
    pub steps: usize,
    /// The policy that runs the design.
    pub policy: HorizonPolicy,
    /// The gains `K_0 … K_{N−1}`.
    pub gain_sequence: Vec<DenseMatrix>,
    /// The Riccati matrices `P_0 … P_N`.
    pub riccati_sequence: Vec<DenseMatrix>,
}

/// A state-controller design (C4).
#[derive(Debug, Clone, PartialEq, Serialize, JsonSchema)]
#[serde(rename_all = "camelCase")]
pub struct StateControllerDesign {
    /// The design's status.
    pub status: DesignStatus,
    /// The unapplied candidate.
    pub candidate: StateCandidate,
    /// The model the design was made on.
    pub selected_model: SelectedModel,
    /// Always `true`: the actual Structured Text verifies the candidate before it is applied.
    #[serde(rename = "actualSTVerify")]
    pub actual_st_verify: bool,
    /// What the design does and does not establish.
    pub scope: String,
    /// The closed-loop poles of the mapped coefficients, read back from their values.
    pub closed_loop_poles: Vec<ComplexValue>,
    /// The finite-horizon stages, when the design has a finite horizon.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub horizon: Option<StateHorizon>,
}

/// Designs the state-feedback law that `loop_definition` declares, on `model`.
///
/// `response_time_seconds` sets the default poles of pole placement. `selected_family` replaces the declared family; an LQG
/// law cannot change into another family, or the reverse, without its own adapter. `preferences` are the stored design
/// choices. `controlled_outputs` are the model outputs that the loop declares as controlled objectives, in channel order:
/// their number must equal the model's inputs (the channel reading of `connectedModelChannels`, which the loop's channel
/// stage supplies).
///
/// # Errors
/// Returns a failure, with its next action, when the law is not declared, the model is a transfer function or has a
/// delay, the family changes the estimator structure, a design rule or a numerical step refuses, or a mapped coefficient
/// does not fit its parameter.
pub fn design_state_controller(
    loop_definition: &LoopDefinition,
    model: &GeneralModel,
    response_time_seconds: f64,
    selected_family: Option<StateControllerFamily>,
    preferences: Option<&StateDesignPreferences>,
    controlled_outputs: &[usize],
) -> Result<StateControllerDesign, Failure> {
    let parameter_binding = loop_definition.parameter_binding.as_ref().ok_or_else(|| {
        Failure::new(
            texts::NEEDS_BINDING,
            Cause::Implementation,
            NextAction::OpenLoopSetup,
        )
    })?;
    let declared = loop_definition
        .controller
        .state_feedback
        .as_ref()
        .and_then(|value| decode_state_controller_binding(value, Some(parameter_binding)))
        .ok_or_else(|| {
            Failure::new(
                texts::NEEDS_BINDING,
                Cause::Implementation,
                NextAction::OpenLoopSetup,
            )
        })?;
    let family = selected_family.unwrap_or(declared.family);
    if (declared.family == StateControllerFamily::Lqg) != (family == StateControllerFamily::Lqg) {
        return Err(Failure::new(
            texts::ESTIMATOR_STRUCTURE_CHANGE,
            Cause::ControllerCapability,
            NextAction::ChooseMethod,
        ));
    }
    let binding = with_preferences(declared, preferences, family);
    let identity = model.identity();
    let system = match state_space(model) {
        Some(system) if identity.delay_seconds.abs() <= 0.0 => system,
        _ => {
            return Err(Failure::new(
                texts::NEEDS_BINDING,
                Cause::ModelOrData,
                NextAction::ChooseMethod,
            ))
        }
    };
    let inputs = identity.inputs.len();
    let outputs = identity.outputs.len();

    let sampled = sample_model(model, loop_definition.controller.task_period_ms / 1000.0)?;
    let finite_horizon = preferences.and_then(|stored| stored.finite_horizon.as_ref());
    let designed = design_gains(
        &binding,
        finite_horizon,
        model,
        &sampled,
        response_time_seconds,
    )?;
    if controlled_outputs.len() != inputs
        || controlled_outputs.iter().any(|&output| output >= outputs)
    {
        return Err(Failure::new(
            texts::ONE_OBJECTIVE_PER_INPUT,
            Cause::Implementation,
            NextAction::OpenChannels,
        ));
    }
    let reference = reference_gain(&sampled, system, &designed.gain, controlled_outputs)?;

    let values = map_design_values(
        &binding,
        &designed,
        &reference,
        controlled_outputs,
        model,
        &parameter_binding.parameters,
    )?;
    let poles = closed_loop_poles(&binding, &sampled, &values)?;
    Ok(StateControllerDesign {
        status: DesignStatus::Provisional,
        candidate: StateCandidate {
            applied: false,
            structure: family,
            parameters: values,
        },
        selected_model: SelectedModel {
            revision: identity.revision.clone(),
            provenance: format!(
                "declared-{}-{}",
                identity.source.word(),
                identity.validation.word()
            ),
        },
        actual_st_verify: true,
        scope: texts::DESIGN_SCOPE.to_string(),
        closed_loop_poles: poles,
        horizon: designed.stages.map(|stages| StateHorizon {
            steps: stages.gain_sequence.len(),
            policy: HorizonPolicy::RepeatFirstFiniteGain,
            gain_sequence: stages.gain_sequence,
            riccati_sequence: stages.riccati_sequence,
        }),
    })
}

/// The coefficient values of a design: the gains `K`, the reference gains `N` and, for LQG, the observer gains `L`, each
/// written under the symbol that the declaration names for its position, in the parameter's unit and bounds.
fn map_design_values(
    binding: &StateControllerBinding,
    designed: &DesignedGains,
    reference: &DenseMatrix,
    controlled_outputs: &[usize],
    model: &GeneralModel,
    parameters: &[Parameter],
) -> Result<BTreeMap<String, f64>, Failure> {
    let identity = model.identity();
    let states = state_channels(model);
    let mut values = BTreeMap::new();
    map_coefficients(
        &binding.gain_symbols,
        &designed.gain,
        |row, column| ratio_unit(&identity.inputs[row].unit, &states[column].unit),
        parameters,
        &mut values,
    )?;
    map_coefficients(
        &binding.reference_gain_symbols,
        reference,
        |row, column| {
            ratio_unit(
                &identity.inputs[row].unit,
                &identity.outputs[controlled_outputs[column]].unit,
            )
        },
        parameters,
        &mut values,
    )?;
    if let (Some(symbols), Some(observer)) = (&binding.observer_gain_symbols, &designed.observer) {
        map_coefficients(
            symbols,
            observer,
            |row, column| ratio_unit(&states[row].unit, &identity.outputs[column].unit),
            parameters,
            &mut values,
        )?;
    }
    Ok(values)
}

/// The declared law with the stored choices and the selected family laid over it: each stored choice that is present
/// replaces the declared field, and the family is the selected one.
fn with_preferences(
    mut binding: StateControllerBinding,
    preferences: Option<&StateDesignPreferences>,
    family: StateControllerFamily,
) -> StateControllerBinding {
    binding.family = family;
    if let Some(stored) = preferences {
        if let Some(poles) = &stored.desired_poles {
            binding.desired_poles = Some(poles.clone());
        }
        if let Some(weights) = &stored.state_weights {
            binding.state_weights = Some(weights.clone());
        }
        if let Some(weights) = &stored.input_weights {
            binding.input_weights = Some(weights.clone());
        }
        if let Some(noise) = &stored.process_noise {
            binding.process_noise = Some(noise.clone());
        }
        if let Some(noise) = &stored.measurement_noise {
            binding.measurement_noise = Some(noise.clone());
        }
    }
    binding
}

#[cfg(test)]
#[path = "design_tests.rs"]
mod tests;
