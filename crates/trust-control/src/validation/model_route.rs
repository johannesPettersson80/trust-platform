//! The selected model as the plant of an isolated Validate run (specification 35, B5.3, B6.2 and C8; runtime protocol annex E8.4): the
//! model's inline plant, which the runtime takes as its `simulation_override`, and the controller's initial values at the model's
//! operating point. Ported from the model route of `engineerVerificationPlan.ts`: its binding for
//! `materializeSelectedModelSimulation` (`selected_model_binding`), and its initial values from `pidOperatingState` and
//! `initializationValues`.
//!
//! The binding follows the loop's channels (`bound_channels`, the port of `connectedModelChannels`): every model input is driven by
//! its command source, a measured disturbance is written through the variable its Manual command names, every model output is read
//! from its feedback target, and each input has its actuator and starts at its operating point. A loop without channel bindings is the
//! scalar form: one plant input and one measured output. The plan's own initial values, the mode and the setpoint target are added by
//! `engineer_verification_plan`.

use std::collections::BTreeMap;

use crate::commissioning::adapters::controller_adapter;
use crate::commissioning::channel_mapping::connected_model_channels;
use crate::commissioning::initial_state::{initialization_values, ControllerInitialization};
use crate::commissioning::library_pid::library_pid_initialization;
use crate::commissioning::pid_measurement::{measured_pid, plant_feedback_target};
use crate::commissioning::pid_operating_state::pid_operating_state;
use crate::commissioning::transfer_tracking::TransferTracking;
use crate::failure::{Cause, Failure, NextAction};
use crate::identification::model_compatibility::CompatibilityRefusal;
use crate::identification::texts::compatibility_text;
use crate::loop_definition::{
    channel_signal, measured_disturbance, ChannelInput, ChannelRequirements, ChannelSignal,
    LoopChannels, LoopDefinition, PidInitialization,
};
use crate::model::{
    materialize_selected_model_simulation, ActuatorCapabilities, ActuatorKind, GeneralModel,
    ModelChannel, SelectedModelSimulationBinding, SimulationOverride,
};
use crate::validation::texts;

/// The plant id of the selected model's inline plant (`plantId: "selected-model"`).
const PLANT_ID: &str = "selected-model";
/// The sentence of a model that cannot be read as the runtime's plant input.
const MODEL_UNREADABLE: &str = "The selected model could not be read for its isolated plant.";
/// The sentence of controller initial states that overlap a parameter, an observed process channel or the mode.
const STATES_OVERLAP: &str = "Controller initial states cannot overlap parameters, observed process channels or mode ownership.";
/// The sentence of a PID group whose declared profile does not read as a declared-state record, which its start values need.
const GROUP_PROFILE_UNREADABLE: &str =
    "The PID group's declared profile does not give its start values.";

/// The model route of one Validate (B5.3, B6.2): the selected model's inline plant and the controller's initial values at its
/// operating point.
#[derive(Debug, Clone, PartialEq)]
pub struct ModelRoute {
    /// The model's inline plant, which the isolated run request carries as its `simulation_override` (E8.4).
    pub simulation: SimulationOverride,
    /// The controller's initial values at the model's operating point, by symbol: the parallel PID's history and the declared states
    /// of a `control-pid-v1` PID (C0, C1). The mode, the plant inputs and the setpoint target are the plan's own.
    pub initial_values: BTreeMap<String, f64>,
}

/// The loop's channels bound to a model, in the model's order (`connectedModelChannels`): each model input with the loop signal that
/// drives or reads it, and each model output with the loop signal that measures it.
#[derive(Debug, Clone, PartialEq)]
pub struct BoundChannels<'a> {
    /// The model's inputs, in order.
    pub inputs: Vec<BoundInput<'a>>,
    /// The model's outputs, in order.
    pub outputs: Vec<BoundOutput<'a>>,
    /// The state deviations that the channels declare, in the model's coordinates, when they declare any.
    pub initial_state: Option<&'a [f64]>,
}

/// One model input as the loop binds it: the loop signal it drives or reads, the signal that reports its applied value, its actuator,
/// and the variable that the simulated plant reads its command from.
#[derive(Debug, Clone, PartialEq)]
pub struct BoundInput<'a> {
    /// The model's input.
    pub channel: &'a ModelChannel,
    /// The loop signal the input drives (the controller output) or reads (a measured disturbance).
    pub signal: ChannelSignal<'a>,
    /// The loop signal that reports the input's applied value, when the loop declares one.
    pub applied: Option<ChannelSignal<'a>>,
    /// The input's actuator: its kind, its range and its rate limits.
    pub actuator: ActuatorCapabilities,
    /// Whether the input is a measured disturbance, which the controller reads and never writes (C8).
    pub disturbance: bool,
    /// The program variable that the run writes the input's value to: the Manual command of a measured disturbance when its channel
    /// names one, otherwise the signal itself.
    pub command_symbol: String,
    /// Where the simulated plant reads the input from: the address of `command_symbol` when the loop declares that signal, else the symbol.
    pub command_source: String,
}

/// One model output as the loop binds it: the loop signal that measures it, its setpoint and its own requirements, when it has them.
#[derive(Debug, Clone, PartialEq)]
pub struct BoundOutput<'a> {
    /// The model's output.
    pub channel: &'a ModelChannel,
    /// The loop signal that measures the output.
    pub signal: ChannelSignal<'a>,
    /// The loop signal that holds the output's setpoint, when the loop has one.
    pub reference: Option<ChannelSignal<'a>>,
    /// The output's own requirements, in place of the task's, when its channel declares them.
    pub requirements: Option<ChannelRequirements>,
}

/// Builds the model route of a loop on `model` (B5.3, B6.2). `values` are the controller's parameter values, which the initial
/// values need. A declared scenario with its own model is routed on that model by the caller (`engineer_verification_campaign`).
///
/// # Errors
/// The loop's channels do not bind the model (see [`selected_model_binding`]); the model does not materialize as a plant (its
/// refusal, `ModelOrData`); or the controller's initial values cannot be computed, or overlap a parameter, an observed process channel
/// or the mode.
pub fn model_route(
    loop_definition: &LoopDefinition,
    model: &GeneralModel,
    values: &BTreeMap<String, f64>,
) -> Result<ModelRoute, Failure> {
    let binding = selected_model_binding(loop_definition, model)?;
    model_route_with(loop_definition, model, values, &binding)
}

/// The model route of a loop on `model` whose plant starts at `initial_state`, in the model's coordinates (deviations from the operating
/// point), instead of the channels' own initial state (C4, the state-recovery scenario). The inline plant and the controller's initial
/// values are those of [`model_route`].
///
/// # Errors
/// As [`model_route`].
pub fn model_route_from_state(
    loop_definition: &LoopDefinition,
    model: &GeneralModel,
    values: &BTreeMap<String, f64>,
    initial_state: &[f64],
) -> Result<ModelRoute, Failure> {
    let mut binding = selected_model_binding(loop_definition, model)?;
    binding.initial_state = initial_state.to_vec();
    model_route_with(loop_definition, model, values, &binding)
}

/// The route of `binding` on `model`: its inline plant, and the controller's initial values at the model's operating point.
fn model_route_with(
    loop_definition: &LoopDefinition,
    model: &GeneralModel,
    values: &BTreeMap<String, f64>,
    binding: &SelectedModelSimulationBinding,
) -> Result<ModelRoute, Failure> {
    let model_value = serde_json::to_value(model)
        .map_err(|_| Failure::new(MODEL_UNREADABLE, Cause::Implementation, NextAction::None))?;
    let materialized =
        materialize_selected_model_simulation(&model_value, binding).map_err(|refusal| {
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
/// `engineerVerificationPlan.ts`): every model input is driven by its command source, every model output is read from its feedback
/// target, each input's actuator reports the applied value when the loop declares one, and each input starts at its operating point, so
/// its deviation from the operating point is zero (`TypeScript` `initialInput`). The model starts at its operating point (B6.2), so its
/// states start from zero deviation unless the channels declare them.
///
/// # Errors
/// The channels do not bind the model (see [`bound_channels`]).
pub fn selected_model_binding(
    loop_definition: &LoopDefinition,
    model: &GeneralModel,
) -> Result<SelectedModelSimulationBinding, Failure> {
    let bound = bound_channels(loop_definition, model)?;
    let command_sources: Vec<String> = bound
        .inputs
        .iter()
        .map(|input| input.command_source.clone())
        .collect();
    let feedback_targets: Vec<String> = bound
        .outputs
        .iter()
        .map(|output| {
            plant_feedback_target(
                loop_definition,
                output.signal.symbol,
                output.signal.address.filter(|address| !address.is_empty()),
            )
        })
        .collect();
    let actuator_feedback_targets: Vec<Option<String>> = bound
        .inputs
        .iter()
        .map(|input| applied_target(input, &bound.outputs))
        .collect();
    let actuators: Vec<ActuatorCapabilities> =
        bound.inputs.iter().map(|input| input.actuator).collect();
    Ok(SelectedModelSimulationBinding {
        plant_id: PLANT_ID.to_owned(),
        command_sources,
        feedback_targets,
        actuator_feedback_targets: Some(actuator_feedback_targets),
        initial_state: bound
            .initial_state
            .map_or_else(|| vec![0.0; state_count(model)], <[f64]>::to_vec),
        // Each input starts at its operating point, so its deviation from that point is zero (`TypeScript` `initialInput`).
        initial_input: vec![0.0; bound.inputs.len()],
        initial_delayed_input: Some(0.0),
        step_ms: loop_definition.controller.task_period_ms,
        actuators: Some(actuators),
    })
}

/// The loop's channels bound to `model` (`connectedModelChannels`). A loop without channel bindings has the scalar form: its
/// manipulated variable drives the model's one input, and its process variable is the model's one output. A loop with channel bindings
/// binds each model channel to the loop channel that names it by identity, never by order.
///
/// # Errors
/// The loop's channel bindings break a rule of their shape or binding (`check_bindings`); a loop without bindings has a model that is
/// not one input and one output (`connected_model_channels`), or no output limit for its actuator's range (`OpenLimits`); or a model
/// channel has no binding, a binding names no declared signal, or the bindings do not match the model's channels
/// (`channel_bindings_unmapped`).
pub fn bound_channels<'a>(
    loop_definition: &'a LoopDefinition,
    model: &'a GeneralModel,
) -> Result<BoundChannels<'a>, Failure> {
    let Some(channels) = &loop_definition.channels else {
        return scalar_channels(loop_definition, model);
    };
    channels
        .check_bindings(loop_definition)
        .map_err(|sentence| Failure::new(&sentence, Cause::Settings, NextAction::OpenChannels))?;
    bound_model_channels(loop_definition, channels, model)
}

/// The scalar form of a loop without channel bindings: the manipulated variable is the model's one input and the process variable its
/// one output (`connectedModelChannels`, the scalar branch). The actuator's range is the loop's output limits, and its rate limits are
/// the loop's.
fn scalar_channels<'a>(
    loop_definition: &'a LoopDefinition,
    model: &'a GeneralModel,
) -> Result<BoundChannels<'a>, Failure> {
    connected_model_channels(loop_definition, model)?;
    let identity = model.identity();
    let (Some(input), Some(output)) = (identity.inputs.first(), identity.outputs.first()) else {
        return Err(unmapped());
    };
    let limits = &loop_definition.limits;
    let (Some(minimum), Some(maximum)) = (limits.output_minimum, limits.output_maximum) else {
        return Err(output_range_missing(model));
    };
    let signal = channel_signal(loop_definition, "manipulatedVariable").ok_or_else(unmapped)?;
    let process = channel_signal(loop_definition, "processVariable").ok_or_else(unmapped)?;
    Ok(BoundChannels {
        inputs: vec![BoundInput {
            channel: input,
            signal,
            applied: channel_signal(loop_definition, "actuator"),
            actuator: ActuatorCapabilities {
                kind: ActuatorKind::Continuous,
                minimum,
                maximum,
                rate_up_per_s: limits.rate_up_per_s,
                rate_down_per_s: limits.rate_down_per_s,
                minimum_on_seconds: None,
                minimum_off_seconds: None,
            },
            disturbance: false,
            command_symbol: signal.symbol.to_owned(),
            command_source: command_source_of(loop_definition, signal.symbol),
        }],
        outputs: vec![BoundOutput {
            channel: output,
            signal: process,
            reference: channel_signal(loop_definition, "setpoint"),
            requirements: None,
        }],
        initial_state: None,
    })
}

/// The channel bindings of a loop with channels, matched to the model's channels by identity (`connectedModelChannels`, the bound
/// branch). The counts must agree, so that no binding is left over.
fn bound_model_channels<'a>(
    loop_definition: &'a LoopDefinition,
    channels: &'a LoopChannels,
    model: &'a GeneralModel,
) -> Result<BoundChannels<'a>, Failure> {
    let identity = model.identity();
    if channels.inputs.len() != identity.inputs.len()
        || channels.outputs.len() != identity.outputs.len()
    {
        return Err(unmapped());
    }
    let mut inputs = Vec::with_capacity(identity.inputs.len());
    for channel in &identity.inputs {
        let input = channels
            .inputs
            .iter()
            .find(|input| input.model_channel_id == channel.id)
            .ok_or_else(unmapped)?;
        inputs.push(bound_input(loop_definition, channel, input)?);
    }
    let mut outputs = Vec::with_capacity(identity.outputs.len());
    for channel in &identity.outputs {
        let output = channels
            .outputs
            .iter()
            .find(|output| output.model_channel_id == channel.id)
            .ok_or_else(unmapped)?;
        let signal = channel_signal(loop_definition, &output.signal_id).ok_or_else(unmapped)?;
        let reference = output
            .setpoint_signal_id
            .as_deref()
            .map(|id| channel_signal(loop_definition, id).ok_or_else(unmapped))
            .transpose()?;
        outputs.push(BoundOutput {
            channel,
            signal,
            reference,
            requirements: output.requirements,
        });
    }
    Ok(BoundChannels {
        inputs,
        outputs,
        initial_state: channels.initial_state.as_deref(),
    })
}

/// One model input bound to its channel: its loop signal, its applied value, its actuator, and the command its plant reads. A measured
/// disturbance is written through the Manual command its channel names (C8, D-5); any other input through its own signal.
fn bound_input<'a>(
    loop_definition: &'a LoopDefinition,
    channel: &'a ModelChannel,
    input: &ChannelInput,
) -> Result<BoundInput<'a>, Failure> {
    let signal = channel_signal(loop_definition, &input.signal_id).ok_or_else(unmapped)?;
    let disturbance = measured_disturbance(loop_definition, input);
    let manual_command = if disturbance {
        input.manual_output_signal.as_deref()
    } else {
        None
    };
    let command_symbol = manual_command.unwrap_or(signal.symbol).to_owned();
    let command_source = command_source_of(loop_definition, &command_symbol);
    Ok(BoundInput {
        channel,
        signal,
        applied: applied_signal(loop_definition, input),
        actuator: input.actuator,
        disturbance,
        command_symbol,
        command_source,
    })
}

/// The loop signal that reports an input's applied value: the one its channel names, or for the manipulated variable the loop's actuator
/// (`TypeScript` `connectedModelChannels`).
fn applied_signal<'a>(
    loop_definition: &'a LoopDefinition,
    input: &ChannelInput,
) -> Option<ChannelSignal<'a>> {
    match input.applied_signal_id.as_deref() {
        Some(id) => channel_signal(loop_definition, id),
        None if input.signal_id == "manipulatedVariable" => {
            channel_signal(loop_definition, "actuator")
        }
        None => None,
    }
}

/// Where the simulated plant reads or writes a variable: the address of the loop signal with this symbol, or the symbol itself when the
/// signal has no address (`TypeScript` `input.signal.address ?? input.signal.symbol`). A symbol the loop does not declare is the program
/// variable itself (specification 46, §13.1).
fn command_source_of(loop_definition: &LoopDefinition, symbol: &str) -> String {
    let signals = &loop_definition.signals;
    let primary = [
        Some(&signals.manipulated_variable),
        Some(&signals.process_variable),
        signals.setpoint.as_ref(),
        signals.actuator.as_ref(),
    ]
    .into_iter()
    .flatten()
    .find(|signal| signal.symbol == symbol)
    .map(|signal| signal.address.clone());
    let auxiliary = signals
        .additional
        .iter()
        .flatten()
        .find(|signal| signal.symbol == symbol)
        .map(|signal| signal.address.clone());
    primary
        .or(auxiliary)
        .flatten()
        .unwrap_or_else(|| symbol.to_owned())
}

/// The address that reports an input's applied value, when the loop declares one that is not a measured output's address (`TypeScript`
/// `actuatorFeedbackTargets`). An empty address names nothing.
fn applied_target(input: &BoundInput<'_>, outputs: &[BoundOutput<'_>]) -> Option<String> {
    let address = input
        .applied
        .and_then(|applied| applied.address)
        .filter(|address| !address.is_empty())?;
    let measured = outputs
        .iter()
        .any(|output| output.signal.address == Some(address));
    (!measured).then(|| address.to_owned())
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

/// The refusal of channel bindings that do not match the model's channels, with the action that opens the channels (A11.2).
fn unmapped() -> Failure {
    Failure::new(
        &texts::channel_bindings_unmapped(),
        Cause::Implementation,
        NextAction::OpenChannels,
    )
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
    // A PID group's states start as its declared profile starts them at the candidate's values (C0, C7), as Apply starts them.
    if loop_definition.controller.group.is_some() {
        return group_initial_values(loop_definition, model, values);
    }
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
    if let Some(project) = loop_definition.controller.project.as_ref() {
        // A program's own law starts its declared states as Validate starts them (C2): the integral term at the operating output.
        if states_overlap(loop_definition, &project.initialization) {
            return Err(Failure::new(
                STATES_OVERLAP,
                Cause::Settings,
                NextAction::OpenLoopSetup,
            ));
        }
        initial.extend(initialization_values(
            loop_definition,
            model,
            &project.initialization,
            values,
        )?);
    }
    Ok(initial)
}

/// The start values of a PID group's declared states at the model's operating point (C0, C7): the declared profile of the candidate's
/// values (`ControllerAdapter::declared_profile`), read at its Auto entry as the run's transfer sends them (`tracking`, initialized). A
/// decoupled member's retained output therefore starts at its share of the operating inputs, `v = D⁻¹·u`.
///
/// # Errors
/// The adapter refuses the group, or the profile's binding or tracking does not fit the loop (see `declared_profile`); or the tracking is
/// not a declared-state record.
fn group_initial_values(
    loop_definition: &LoopDefinition,
    model: &GeneralModel,
    values: &BTreeMap<String, f64>,
) -> Result<BTreeMap<String, f64>, Failure> {
    let adapter = controller_adapter(loop_definition)?;
    let Some(profile) = adapter.declared_profile(model, values)? else {
        return Ok(BTreeMap::new());
    };
    let binding = profile.binding()?;
    match profile.tracking(&binding, true)? {
        TransferTracking::DeclaredState(tracking) => Ok(tracking.initialization_values),
        TransferTracking::Proportional(_) | TransferTracking::ParallelPid(_) => Err(Failure::new(
            GROUP_PROFILE_UNREADABLE,
            Cause::Implementation,
            NextAction::None,
        )),
    }
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

#[cfg(test)]
#[path = "model_route_tests.rs"]
mod tests;
