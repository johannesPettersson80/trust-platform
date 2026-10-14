//! The selected model as the plant of an isolated Validate run (specification 35, B5.3 and B6.2; runtime protocol annex E8.4): the
//! model's inline plant, which the runtime takes as its `simulation_override`, and the controller's initial values at the model's
//! operating point. Ported from the model route of `engineerVerificationPlan.ts`: its binding for
//! `materializeSelectedModelSimulation`, and its initial values from `pidOperatingState` and `initializationValues`.
//!
//! The Rust engine runs a loop with one plant input, one measured output and no ON/OFF actuator, state feedback or program-owned
//! law (`LoopDefinition::rust_engine_support`), so the binding is the scalar one. The plan's own initial values, the mode, the plant
//! input and the setpoint target, are added by `engineer_verification_plan`.

use std::collections::BTreeMap;

use crate::commissioning::channel_mapping::connected_model_channels;
use crate::commissioning::initial_state::{initialization_values, ControllerInitialization};
use crate::commissioning::library_pid::library_pid_initialization;
use crate::commissioning::pid_measurement::{measured_pid, plant_feedback_target};
use crate::commissioning::pid_operating_state::pid_operating_state;
use crate::failure::{Cause, Failure, NextAction};
use crate::identification::model_compatibility::CompatibilityRefusal;
use crate::identification::texts::compatibility_text;
use crate::json::decode_value;
use crate::loop_definition::{LoopDefinition, PidInitialization};
use crate::model::{
    materialize_selected_model_simulation, ActuatorCapabilities, ActuatorKind, GeneralModel,
    SelectedModelSimulationBinding, SimulationOverride,
};
use crate::validation::scenario_contract::EngineerScenario;

/// The plant id of the selected model's inline plant (`plantId: "selected-model"`).
const PLANT_ID: &str = "selected-model";
/// The sentence of a model that cannot be read as the runtime's plant input.
const MODEL_UNREADABLE: &str = "The selected model could not be read for its isolated plant.";
/// The sentence of controller initial states that overlap a parameter, an observed process channel or the mode.
const STATES_OVERLAP: &str = "Controller initial states cannot overlap parameters, observed process channels or mode ownership.";

/// The model route of one Validate (B5.3, B6.2): the selected model's inline plant and the controller's initial values at its
/// operating point.
#[derive(Debug, Clone, PartialEq)]
pub struct ModelRoute {
    /// The model's inline plant, which the isolated run request carries as its `simulation_override` (E8.4).
    pub simulation: SimulationOverride,
    /// The controller's initial values at the model's operating point, by symbol: the parallel PID's history and the declared states
    /// of a `control-pid-v1` PID (C0, C1). The mode, the plant input and the setpoint target are the plan's own.
    pub initial_values: BTreeMap<String, f64>,
}

/// Builds the model route of a loop on `model` (B5.3, B6.2). `values` are the controller's parameter values, which the initial
/// values need.
///
/// # Errors
/// A declared scenario supplies its own model, which this route does not run; the loop's binding does not fit the model (see
/// [`selected_model_binding`]); the model does not materialize as a plant (its refusal, `ModelOrData`); or the controller's initial
/// values cannot be computed, or overlap a parameter, an observed process channel or the mode.
pub fn model_route(
    loop_definition: &LoopDefinition,
    model: &GeneralModel,
    values: &BTreeMap<String, f64>,
) -> Result<ModelRoute, Failure> {
    refuse_scenario_models(loop_definition)?;
    let binding = selected_model_binding(loop_definition, model)?;
    let model_value = serde_json::to_value(model)
        .map_err(|_| Failure::new(MODEL_UNREADABLE, Cause::Implementation, NextAction::None))?;
    let materialized =
        materialize_selected_model_simulation(&model_value, &binding).map_err(|refusal| {
            Failure::new(
                &refusal.to_string(),
                Cause::ModelOrData,
                NextAction::Identify,
            )
        })?;
    Ok(ModelRoute {
        simulation: materialized.simulation_override,
        initial_values: controller_initial_values(loop_definition, model, values)?,
    })
}

/// The selected model's inline plant binding for a loop (the argument of `materializeSelectedModelSimulation` in
/// `engineerVerificationPlan.ts`): the loop's plant input is the model's command, its measured output is the model's feedback, and
/// its actuator reports the applied value when the loop declares one. The model starts at its operating point (B6.2), so its states
/// and its plant input start from zero deviation.
///
/// # Errors
/// The loop has channel bindings, or the model is not one input and one output (`connected_model_channels`); or the loop declares no
/// output limit, which the actuator's range needs (`CompatibilityRefusal::InputActuatorRange`, `OpenLimits`).
pub fn selected_model_binding(
    loop_definition: &LoopDefinition,
    model: &GeneralModel,
) -> Result<SelectedModelSimulationBinding, Failure> {
    connected_model_channels(loop_definition, model)?;
    let limits = &loop_definition.limits;
    let (Some(minimum), Some(maximum)) = (limits.output_minimum, limits.output_maximum) else {
        return Err(output_range_missing(model));
    };
    let signals = &loop_definition.signals;
    let process = &signals.process_variable;
    let manipulated = &signals.manipulated_variable;
    // The actuator's measured position is the applied value, unless it is the measured output itself (TypeScript
    // `actuatorFeedbackTargets`); an empty address names nothing.
    let applied = signals
        .actuator
        .as_ref()
        .and_then(|actuator| actuator.address.as_deref())
        .filter(|address| !address.is_empty())
        .filter(|address| process.address.as_deref() != Some(*address));
    Ok(SelectedModelSimulationBinding {
        plant_id: PLANT_ID.to_owned(),
        command_sources: vec![manipulated
            .address
            .clone()
            .unwrap_or_else(|| manipulated.symbol.clone())],
        feedback_targets: vec![plant_feedback_target(
            loop_definition,
            &process.symbol,
            process
                .address
                .as_deref()
                .filter(|address| !address.is_empty()),
        )],
        actuator_feedback_targets: Some(vec![applied.map(str::to_owned)]),
        initial_state: vec![0.0; state_count(model)],
        // The plant input starts at its operating point, so its deviation from the operating point is zero (TypeScript
        // `initialInput`, which subtracts the operating input from the input the run starts at).
        initial_input: vec![0.0],
        initial_delayed_input: Some(0.0),
        step_ms: loop_definition.controller.task_period_ms,
        actuators: Some(vec![ActuatorCapabilities {
            kind: ActuatorKind::Continuous,
            minimum,
            maximum,
            rate_up_per_s: limits.rate_up_per_s,
            rate_down_per_s: limits.rate_down_per_s,
            minimum_on_seconds: None,
            minimum_off_seconds: None,
        }]),
    })
}

/// The number of states of the model's plant: the order of its denominator, or the number of its listed states.
fn state_count(model: &GeneralModel) -> usize {
    match model {
        GeneralModel::TransferFunction { transfer, .. } => {
            transfer.denominator.len().saturating_sub(1)
        }
        GeneralModel::ContinuousStateSpace { states, .. }
        | GeneralModel::DiscreteStateSpace { states, .. } => states.len(),
    }
}

/// The refusal of a loop without an output limit, which the actuator's range of the model's plant needs.
fn output_range_missing(model: &GeneralModel) -> Failure {
    let id = model
        .identity()
        .inputs
        .first()
        .map(|channel| channel.id.clone())
        .unwrap_or_default();
    Failure::new(
        &compatibility_text(&CompatibilityRefusal::InputActuatorRange { id }),
        Cause::Settings,
        NextAction::OpenLimits,
    )
}

/// The controller's initial values at the model's operating point (`engineerVerificationPlan`): the parallel PID's history
/// (`pidOperatingState`), then the declared states of a `control-pid-v1` PID (`initializationValues`), which take their symbol.
fn controller_initial_values(
    loop_definition: &LoopDefinition,
    model: &GeneralModel,
    values: &BTreeMap<String, f64>,
) -> Result<BTreeMap<String, f64>, Failure> {
    let mut initial = pid_operating_state(loop_definition, model, values)?;
    if matches!(
        loop_definition.controller.pid_initialization,
        Some(PidInitialization::ControlPid { .. })
    ) {
        let definition = library_pid_initialization(loop_definition)?;
        if states_overlap(loop_definition, &definition) {
            return Err(Failure::new(
                STATES_OVERLAP,
                Cause::Settings,
                NextAction::OpenLoopSetup,
            ));
        }
        initial.extend(initialization_values(
            loop_definition,
            model,
            &definition,
            values,
        )?);
    }
    Ok(initial)
}

/// Whether a declared state takes the symbol of a parameter, of an observed process channel (other than the library PID's own
/// measurement, which the model gives to the PID) or of the mode.
fn states_overlap(loop_definition: &LoopDefinition, definition: &ControllerInitialization) -> bool {
    let Some(binding) = &loop_definition.parameter_binding else {
        return false;
    };
    let signals = &loop_definition.signals;
    let channels = [
        Some(&signals.manipulated_variable),
        Some(&signals.process_variable),
        signals.setpoint.as_ref(),
    ];
    definition.states().iter().any(|state| {
        binding
            .parameters
            .iter()
            .any(|parameter| parameter.symbol == state.symbol)
            || channels.iter().flatten().any(|signal| {
                signal.symbol == state.symbol
                    && measured_pid(loop_definition, &signal.symbol).is_none()
            })
            || state.symbol == binding.mode.symbol
    })
}

/// Refuses a declared scenario that supplies its own model: this route gives every case the selected model's plant, which such a
/// scenario does not run on. A scenario list that does not decode is left to the campaign, which refuses it first.
fn refuse_scenario_models(loop_definition: &LoopDefinition) -> Result<(), Failure> {
    let Some(value) = &loop_definition.verification_scenarios else {
        return Ok(());
    };
    let Ok(scenarios): Result<Vec<EngineerScenario>, _> = decode_value(value.clone()) else {
        return Ok(());
    };
    match scenarios.iter().find(|scenario| scenario.model.is_some()) {
        Some(scenario) => {
            let id = &scenario.id;
            Err(Failure::new(
                &format!("Scenario {id} supplies its own model, which Validate does not run on the selected model."),
                Cause::Settings,
                NextAction::OpenRequirements,
            ))
        }
        None => Ok(()),
    }
}

#[cfg(test)]
#[path = "model_route_tests.rs"]
mod tests;
