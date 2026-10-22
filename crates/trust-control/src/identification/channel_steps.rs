//! The steps of a campaign's plant inputs (`connectedIdentificationChannels.ts`; specification 35, C7 and C8): the loop each input is
//! stepped on, the sample the step reads, the address of its Manual command from the runtime's I/O list, and the method each input's
//! step runs under (`campaign_method`, `InputStep::own_method`), which a resumed campaign plans the same way. `connected_run` re-exports
//! these rules, which the campaign and its recovery call.

use serde_json::Value;

use crate::experiment::{ControlExperimentKind, ExperimentSample};
use crate::failure::{Cause, Failure, NextAction};
use crate::identification::policy::{
    project_scripted, validate_explicit_method, IdentificationMethod,
};
use crate::loop_definition::{ChannelInput, ExperimentKind, LoopDefinition, LoopSignal};

/// The seed stride between the steps of two inputs (`index * 7919` of `preparePlan`), so that no two inputs share an excitation.
const INPUT_SEED_STRIDE: u64 = 7_919;

/// One plant input's step of a campaign (`connectedIdentificationChannels`): the loop the step is planned on, the plant input it
/// moves, and whether that input is the loop's own manipulated variable.
#[derive(Debug, Clone)]
pub struct InputStep {
    /// The loop the step is planned on. For the manipulated variable it is the loop, with the input's own identification step; for
    /// another input it is a variant of the loop that drives that input's Manual command, within its actuator's range (C7, C8).
    pub loop_definition: LoopDefinition,
    /// The symbol of the plant input the step moves.
    pub input_symbol: String,
    /// Whether the input is the loop's own manipulated variable, whose sample the run reads as it is.
    pub manipulated: bool,
    /// The method of the input's own identification step, a manual-output step of that size, when the input declares one (C7, C8).
    /// `None` for an input that takes the campaign's method (`campaign_method`).
    pub own_method: Option<IdentificationMethod>,
}

/// The steps of a loop's plant inputs, in input order (`connectedIdentificationChannels`). A loop with one plant input, and a
/// project-scripted loop whatever its inputs (C12), is stepped as it is. A loop with several inputs has one step per input, each planned
/// on the loop with that input's identification step
/// (`experimentStep`) and its own seed. An input other than the manipulated variable is stepped through its Manual command, whose
/// address is the one the runtime's I/O list `io` gives for its name; a command without an address is stepped by name (§17.1).
///
/// # Errors
/// An input's signal is not declared, or an input other than the manipulated variable declares no Manual command.
pub fn input_steps(
    loop_definition: &LoopDefinition,
    io: &Value,
) -> Result<Vec<InputStep>, Failure> {
    let Some(inputs) = several_inputs(loop_definition) else {
        return Ok(vec![InputStep {
            loop_definition: loop_definition.clone(),
            input_symbol: loop_definition.signals.manipulated_variable.symbol.clone(),
            manipulated: true,
            own_method: None,
        }]);
    };
    inputs
        .iter()
        .enumerate()
        .map(|(index, input)| input_step(loop_definition, input, index, io))
        .collect()
}

/// The step of one input of a loop with several inputs, at its position `index` in the input order.
fn input_step(
    loop_definition: &LoopDefinition,
    input: &ChannelInput,
    index: usize,
    io: &Value,
) -> Result<InputStep, Failure> {
    let signal = loop_signal_of(loop_definition, &input.signal_id).ok_or_else(|| {
        Failure::new(
            &format!(
                "Channel binding {} does not identify a declared signal.",
                input.signal_id
            ),
            Cause::Implementation,
            NextAction::None,
        )
    })?;
    let symbol = signal.symbol.clone();
    let manipulated = symbol == loop_definition.signals.manipulated_variable.symbol;
    let mut variant = loop_definition.clone();
    if let Some(experiment) = variant.experiment.as_mut() {
        // An input with its own step is stepped at it, as a step of that input (`experimentStep`, Specification 35 §17.9).
        if let Some(step) = input.experiment_step {
            experiment.amplitude = step;
            experiment.kind = ExperimentKind::MvStep;
        }
        experiment.seed = experiment.seed.wrapping_add(seed_offset(index));
    }
    if !manipulated {
        let (command, manual_type) = input
            .manual_output_signal
            .clone()
            .zip(input.manual_output_type)
            .ok_or_else(|| {
                Failure::new(
                    &format!("The loop declares no Manual command for {symbol}."),
                    Cause::Settings,
                    NextAction::OpenLoopSetup,
                )
            })?;
        variant.controller.manual_output_address = manual_address(io, &command);
        variant.controller.manual_output_signal = Some(command);
        variant.controller.manual_output_type = Some(manual_type);
        variant.controller.manual_output_follows_output = None;
        variant.signals.manipulated_variable = signal;
        variant.limits.output_minimum = Some(input.actuator.minimum);
        variant.limits.output_maximum = Some(input.actuator.maximum);
    }
    Ok(InputStep {
        loop_definition: variant,
        input_symbol: symbol,
        manipulated,
        own_method: own_step_of(loop_definition, input).map(|_| manual_output_method()),
    })
}

/// The seed offset of the input at `index` in the input order.
fn seed_offset(index: usize) -> u64 {
    u64::try_from(index).map_or(0, |index| index * INPUT_SEED_STRIDE)
}

/// The loop signal a channel identifier names, as the loop's own signals give it (`loopSignal`): the setpoint, the process variable,
/// the manipulated variable or the actuator position, or an additional signal of the loop by its identifier. `None` when the loop
/// declares no such signal.
#[must_use]
pub fn loop_signal_of(loop_definition: &LoopDefinition, id: &str) -> Option<LoopSignal> {
    let signals = &loop_definition.signals;
    let fixed = match id {
        "setpoint" => signals.setpoint.clone(),
        "processVariable" => Some(signals.process_variable.clone()),
        "manipulatedVariable" => Some(signals.manipulated_variable.clone()),
        "actuator" => signals.actuator.clone(),
        _ => None,
    };
    fixed.or_else(|| {
        signals
            .additional
            .iter()
            .flatten()
            .find(|signal| signal.id == id)
            .map(|signal| LoopSignal {
                symbol: signal.symbol.clone(),
                address: signal.address.clone(),
                iec_type: signal.iec_type.clone(),
                direction: signal.direction,
                unit: signal.unit.clone(),
                range: signal.range,
            })
    })
}

/// The address of the runtime's I/O entry named `name` (`io.list`), compared without regard to case: the first entry of the inputs,
/// the outputs and the memory, in that order, that bears the name. `None` when no such entry has an address (§17.1).
#[must_use]
pub fn manual_address(io: &Value, name: &str) -> Option<String> {
    let wanted = name.to_lowercase();
    ["inputs", "outputs", "memory"]
        .into_iter()
        .filter_map(|group| io.get(group).and_then(Value::as_array))
        .flatten()
        .find(|entry| {
            entry
                .get("name")
                .and_then(Value::as_str)
                .is_some_and(|entry_name| entry_name.to_lowercase() == wanted)
        })
        .and_then(|entry| entry.get("address"))
        .and_then(Value::as_str)
        .map(str::to_owned)
}

/// The sample a step is planned on (`channel.sample`): the sample as it is for the loop's own manipulated variable, and for another
/// input the sample with that input's measured value in place of the manipulated variable (`connectedIdentificationChannels`).
///
/// # Errors
/// The sample holds no current measurement of the input.
pub fn step_sample(
    step: &InputStep,
    sample: Option<ExperimentSample>,
) -> Result<Option<ExperimentSample>, Failure> {
    if step.manipulated {
        return Ok(sample);
    }
    let Some(mut sample) = sample else {
        return Ok(None);
    };
    let measured = sample
        .additional
        .iter()
        .find(|channel| channel.signal.symbol == step.input_symbol)
        .map(|channel| channel.signal.clone())
        .ok_or_else(|| {
            Failure::new(
                &format!("No current measured input for {}.", step.input_symbol),
                Cause::Unclassified,
                NextAction::None,
            )
        })?;
    sample.manipulated_variable = measured;
    Ok(Some(sample))
}

/// The method a campaign runs its steps under (C7, C8). A loop with several plant inputs whose every input declares its own
/// identification step runs manual-output steps, so its campaign is a manual-output campaign. Otherwise the campaign runs the method
/// Identify chose for the loop (`loop_method`, from `identification_method`), and the inputs without their own step take it.
#[must_use]
pub fn campaign_method(
    loop_definition: &LoopDefinition,
    loop_method: IdentificationMethod,
) -> IdentificationMethod {
    if every_input_has_own_step(loop_definition) {
        manual_output_method()
    } else {
        loop_method
    }
}

/// The checks of the plant inputs' own identification steps, before anything changes mode or is recorded (C7, C8). An own step is a
/// manual-output step, so the identification policy must permit one, as it permits a declared `mv_step` (`validate_explicit_method`).
/// One recording holds one controller mode: an input without its own step takes the loop's method, and when that method is a setpoint
/// step (Auto), it cannot run beside the manual-output step of an input with its own step, so the campaign is refused by name.
///
/// # Errors
/// The policy does not permit a manual-output step, or an input without its own step would take a setpoint step beside one with its own.
pub fn own_steps_check(
    loop_definition: &LoopDefinition,
    loop_method: IdentificationMethod,
) -> Result<(), Failure> {
    let Some(inputs) = several_inputs(loop_definition) else {
        return Ok(());
    };
    if !inputs
        .iter()
        .any(|input| own_step_of(loop_definition, input).is_some())
    {
        return Ok(());
    }
    validate_explicit_method(loop_definition, ExperimentKind::MvStep)
        .map_err(|error| error.failure())?;
    let unstepped = inputs
        .iter()
        .find(|input| own_step_of(loop_definition, input).is_none());
    match unstepped {
        Some(input) if !loop_method.manual => Err(mixed_modes_failure(loop_definition, input)),
        _ => Ok(()),
    }
}

/// The refusal of a campaign whose input without its own step would take a setpoint step in Auto, beside a manual-output step.
fn mixed_modes_failure(loop_definition: &LoopDefinition, input: &ChannelInput) -> Failure {
    let symbol = loop_signal_of(loop_definition, &input.signal_id)
        .map_or_else(|| input.signal_id.clone(), |signal| signal.symbol);
    Failure::new(
        &format!(
            "Identify would step {symbol} by a setpoint step in Auto, which cannot run in the same recording as the manual-output steps of the inputs with their own step. Give {symbol} its own identification step, so that every input is stepped in Manual."
        ),
        Cause::Settings,
        NextAction::OpenLoopSetup,
    )
}

/// The identification step an input declares as its own (`experimentStep`, Specification 35 §17.9), or `None`. A loop with no
/// experiment, and a project-scripted loop, which owns its excitation (C12), declare none.
fn own_step_of(loop_definition: &LoopDefinition, input: &ChannelInput) -> Option<f64> {
    if loop_definition.experiment.is_none() || project_scripted(loop_definition).is_some() {
        return None;
    }
    input.experiment_step
}

/// The plant inputs of a loop that steps several of them (`input_steps`), or `None` for a loop with one. A project-scripted loop steps
/// none of them: the project's scenario moves every input, and Control Studio writes no excitation of its own (C12).
fn several_inputs(loop_definition: &LoopDefinition) -> Option<&[ChannelInput]> {
    if project_scripted(loop_definition).is_some() {
        return None;
    }
    loop_definition
        .channels
        .as_ref()
        .map(|channels| channels.inputs.as_slice())
        .filter(|inputs| inputs.len() > 1)
}

/// Whether a loop with several plant inputs declares an own identification step for every one of them.
fn every_input_has_own_step(loop_definition: &LoopDefinition) -> bool {
    several_inputs(loop_definition).is_some_and(|inputs| {
        inputs
            .iter()
            .all(|input| own_step_of(loop_definition, input).is_some())
    })
}

/// The method of a manual-output step, which an input's own identification step runs (A6.2, B3.3): a step of the Manual command.
fn manual_output_method() -> IdentificationMethod {
    IdentificationMethod {
        kind: ControlExperimentKind::MvStep,
        manual: true,
        automatic: false,
    }
}

/// Whether the loop has two or more plant inputs or measured outputs, so that its recording is fitted as a multichannel model
/// (`loop.channels?.inputs.length > 1`, and the outputs).
#[must_use]
pub fn several_channels(loop_definition: &LoopDefinition) -> bool {
    loop_definition
        .channels
        .as_ref()
        .is_some_and(|channels| channels.inputs.len() > 1 || channels.outputs.len() > 1)
}

#[cfg(test)]
#[path = "channel_steps_tests.rs"]
mod tests;
