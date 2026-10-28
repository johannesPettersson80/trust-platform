//! Whether a model can tune a loop (`engineerModelCompatibility.ts`): the model's evidence, its inputs and outputs against the loop's
//! channels (units and ranges), and the sample time of a discrete model. A loop without channel bindings has one plant input and one
//! measured output, and its model must have one of each. A loop with channel bindings is qualified channel by channel: every model
//! input and output must be bound to a loop channel, and each is checked against its own signal (`assertQualifiedModelResult`; C7, C8).
//! The engineer's compatibility check follows the same rule: Validate binds every model input and output through the loop's channels.

use crate::loop_definition::{channel_signal, LoopChannels, LoopDefinition};
use crate::model::{GeneralModel, ModelIdentity, ModelSource, ModelValidation};

/// Why a model cannot be used with a loop. The sentences are in `identification::texts`.
#[derive(Debug, Clone, PartialEq)]
pub enum CompatibilityRefusal {
    /// The model has no measurement evidence, and it is not a fit whose evidence is its own recording.
    Evidence,
    /// The model's channels are not the loop's: one input and one output without bindings, or every model channel bound.
    UnmappedChannels,
    /// The model's input range does not meet the plant input's actuator range.
    InputActuatorRange {
        /// The model input's identifier.
        id: String,
    },
    /// The model's input unit is not the plant input's unit.
    InputUnit {
        /// The model input's identifier.
        id: String,
        /// The plant input's symbol.
        symbol: String,
    },
    /// The model's output range does not meet the measured output's range.
    OutputMeasurementRange {
        /// The model output's identifier.
        id: String,
    },
    /// The model's output unit is not the measured output's unit.
    OutputUnit {
        /// The model output's identifier.
        id: String,
        /// The measured output's symbol.
        symbol: String,
    },
    /// The loop's setpoint is not in the measured output's unit.
    SetpointUnit {
        /// The measured output's symbol.
        symbol: String,
    },
    /// The loop declares no setpoint for its measured output, so the model controls nothing.
    NoControlledOutput,
    /// A discrete model's sample time is not the controller's task period.
    SampleTime,
}

/// A model input and the loop input it connects to: the plant input's symbol and unit, and the actuator's range when the loop gives one.
struct InputConnection {
    /// The plant input's symbol.
    symbol: String,
    /// The plant input's unit.
    unit: String,
    /// The smallest and largest value the actuator accepts; `None` when the loop declares no range for it.
    actuator: Option<(f64, f64)>,
}

/// A model output and the loop output it connects to: the measured output's symbol and unit, its measurement range, and the unit of
/// its setpoint when the loop declares one.
struct OutputConnection {
    /// The measured output's symbol.
    symbol: String,
    /// The measured output's unit.
    unit: String,
    /// The smallest value the measurement can take.
    minimum: f64,
    /// The largest value the measurement can take.
    maximum: f64,
    /// The unit of the loop's setpoint for this output, when it has one.
    reference_unit: Option<String>,
}

/// The loop's connections of a model: its inputs, then its outputs, each in the model's order.
struct Connections {
    /// The model's inputs.
    inputs: Vec<InputConnection>,
    /// The model's outputs.
    outputs: Vec<OutputConnection>,
}

/// Checks that a model may tune the loop (`assertEngineerModelCompatibility`).
///
/// # Errors
/// Returns the [`CompatibilityRefusal`] for the first check that fails: the evidence, the channel mapping, the units
/// and ranges, the controlled output, and the sample time.
pub fn assert_engineer_model_compatibility(
    loop_definition: &LoopDefinition,
    model: &GeneralModel,
) -> Result<(), CompatibilityRefusal> {
    assert_model_evidence(model)?;
    let identity = model.identity();
    // Without channel bindings a loop has one plant input and one measured output; with them, every model channel must be bound.
    if loop_definition.channels.is_none()
        && (identity.inputs.len() != 1 || identity.outputs.len() != 1)
    {
        return Err(CompatibilityRefusal::UnmappedChannels);
    }
    let connections = match &loop_definition.channels {
        Some(channels) if maps_every_channel(channels, identity) => {
            bound_connections(loop_definition, channels, identity)?
        }
        Some(_) => return Err(CompatibilityRefusal::UnmappedChannels),
        None => unbound_connections(loop_definition),
    };
    check_connections(model, &connections)?;
    if !connections
        .outputs
        .iter()
        .any(|output| output.reference_unit.is_some())
    {
        return Err(CompatibilityRefusal::NoControlledOutput);
    }
    if let GeneralModel::DiscreteStateSpace {
        sample_time_seconds,
        ..
    } = model
    {
        let mismatch = (*sample_time_seconds * 1000.0 - loop_definition.controller.task_period_ms)
            .abs()
            > 1e-9;
        if mismatch {
            return Err(CompatibilityRefusal::SampleTime);
        }
    }
    Ok(())
}

/// Checks a model's qualification against the loop (`assertQualifiedModelResult`): its evidence, and then its units and ranges
/// channel by channel, when its channels are the loop's. A loop with channel bindings is checked when every model input and output
/// is bound; a loop without them is checked when the model has one input and one output. Any other model is qualified without the
/// channel check.
///
/// # Errors
/// Returns the [`CompatibilityRefusal`] of the evidence check, or of the units and ranges check of the first channel that fails.
pub fn assert_qualified_model_result(
    loop_definition: &LoopDefinition,
    model: &GeneralModel,
) -> Result<(), CompatibilityRefusal> {
    assert_model_evidence(model)?;
    let identity = model.identity();
    let connections = match &loop_definition.channels {
        Some(channels) if maps_every_channel(channels, identity) => {
            Some(bound_connections(loop_definition, channels, identity)?)
        }
        None if identity.inputs.len() == 1 && identity.outputs.len() == 1 => {
            Some(unbound_connections(loop_definition))
        }
        Some(_) | None => None,
    };
    match connections {
        Some(connections) => check_connections(model, &connections),
        None => Ok(()),
    }
}

/// The model needs measurement evidence, and either a validation or a fit that was not validated against anything.
fn assert_model_evidence(model: &GeneralModel) -> Result<(), CompatibilityRefusal> {
    let identity = model.identity();
    let unvalidated_supplied = identity.validation == ModelValidation::Unvalidated
        && identity.source != ModelSource::Fitted;
    if identity.evidence.is_empty() || unvalidated_supplied {
        return Err(CompatibilityRefusal::Evidence);
    }
    Ok(())
}

/// Whether the loop's channel bindings cover the model: as many inputs and outputs as the model has, and each model channel bound.
fn maps_every_channel(channels: &LoopChannels, identity: &ModelIdentity) -> bool {
    channels.inputs.len() == identity.inputs.len()
        && channels.outputs.len() == identity.outputs.len()
        && identity.inputs.iter().all(|model| {
            channels
                .inputs
                .iter()
                .any(|input| input.model_channel_id == model.id)
        })
        && identity.outputs.iter().all(|model| {
            channels
                .outputs
                .iter()
                .any(|output| output.model_channel_id == model.id)
        })
}

/// The connections of a loop without channel bindings: its manipulated variable and measured output. The actuator range is the
/// output limits, or the ON/OFF levels when the controller is an ON/OFF one.
fn unbound_connections(loop_definition: &LoopDefinition) -> Connections {
    let signals = &loop_definition.signals;
    let limits = &loop_definition.limits;
    let (minimum, maximum) = match loop_definition.controller.on_off.as_ref() {
        Some(on_off) => (Some(on_off.low_output), Some(on_off.high_output)),
        None => (limits.output_minimum, limits.output_maximum),
    };
    let actuator = minimum.zip(maximum);
    let process = &signals.process_variable;
    Connections {
        inputs: vec![InputConnection {
            symbol: signals.manipulated_variable.symbol.clone(),
            unit: signals.manipulated_variable.unit.clone(),
            actuator,
        }],
        outputs: vec![OutputConnection {
            symbol: process.symbol.clone(),
            unit: process.unit.clone(),
            minimum: process.range.minimum,
            maximum: process.range.maximum,
            reference_unit: signals
                .setpoint
                .as_ref()
                .map(|setpoint| setpoint.unit.clone()),
        }],
    }
}

/// The connections of a model whose channels the loop binds (`connectedModelChannels`): each model input is the bound input's signal
/// with its actuator's range, and each model output the bound output's signal with its setpoint's unit.
///
/// # Errors
/// A model channel has no binding, or a binding names no declared signal (`UnmappedChannels`).
fn bound_connections(
    loop_definition: &LoopDefinition,
    channels: &LoopChannels,
    identity: &ModelIdentity,
) -> Result<Connections, CompatibilityRefusal> {
    let inputs = identity
        .inputs
        .iter()
        .map(|model| -> Result<InputConnection, CompatibilityRefusal> {
            let input = channels
                .inputs
                .iter()
                .find(|input| input.model_channel_id == model.id)
                .ok_or(CompatibilityRefusal::UnmappedChannels)?;
            let signal = channel_signal(loop_definition, &input.signal_id)
                .ok_or(CompatibilityRefusal::UnmappedChannels)?;
            Ok(InputConnection {
                symbol: signal.symbol.to_owned(),
                unit: signal.unit.to_owned(),
                actuator: Some((input.actuator.minimum, input.actuator.maximum)),
            })
        })
        .collect::<Result<Vec<_>, _>>()?;
    let outputs = identity
        .outputs
        .iter()
        .map(|model| -> Result<OutputConnection, CompatibilityRefusal> {
            let output = channels
                .outputs
                .iter()
                .find(|output| output.model_channel_id == model.id)
                .ok_or(CompatibilityRefusal::UnmappedChannels)?;
            let signal = channel_signal(loop_definition, &output.signal_id)
                .ok_or(CompatibilityRefusal::UnmappedChannels)?;
            let reference_unit = output
                .setpoint_signal_id
                .as_deref()
                .map(|id| {
                    channel_signal(loop_definition, id)
                        .map(|setpoint| setpoint.unit.to_owned())
                        .ok_or(CompatibilityRefusal::UnmappedChannels)
                })
                .transpose()?;
            Ok(OutputConnection {
                symbol: signal.symbol.to_owned(),
                unit: signal.unit.to_owned(),
                minimum: signal.range.minimum,
                maximum: signal.range.maximum,
                reference_unit,
            })
        })
        .collect::<Result<Vec<_>, _>>()?;
    Ok(Connections { inputs, outputs })
}

/// Checks every model input and output against its connection. A model input must fit its actuator and have the plant input's unit;
/// a model output must overlap its measurement's range, have the measured output's unit, and match its setpoint's unit when the loop
/// has a setpoint. The inputs are checked first, then the outputs, each in the model's order.
///
/// # Errors
/// The first check that fails, named by the model channel and the loop signal it breaks.
fn check_connections(
    model: &GeneralModel,
    connections: &Connections,
) -> Result<(), CompatibilityRefusal> {
    let identity = model.identity();
    for (input, connection) in identity.inputs.iter().zip(&connections.inputs) {
        let Some((minimum, maximum)) = connection.actuator else {
            return Err(CompatibilityRefusal::InputActuatorRange {
                id: input.id.clone(),
            });
        };
        let range = input.operating_range;
        if range.maximum < minimum || range.minimum > maximum {
            return Err(CompatibilityRefusal::InputActuatorRange {
                id: input.id.clone(),
            });
        }
        if input.unit != connection.unit {
            return Err(CompatibilityRefusal::InputUnit {
                id: input.id.clone(),
                symbol: connection.symbol.clone(),
            });
        }
    }
    for (output, connection) in identity.outputs.iter().zip(&connections.outputs) {
        let range = output.operating_range;
        if range.maximum < connection.minimum || range.minimum > connection.maximum {
            return Err(CompatibilityRefusal::OutputMeasurementRange {
                id: output.id.clone(),
            });
        }
        if output.unit != connection.unit {
            return Err(CompatibilityRefusal::OutputUnit {
                id: output.id.clone(),
                symbol: connection.symbol.clone(),
            });
        }
        if connection
            .reference_unit
            .as_ref()
            .is_some_and(|unit| *unit != connection.unit)
        {
            return Err(CompatibilityRefusal::SetpointUnit {
                symbol: connection.symbol.clone(),
            });
        }
    }
    Ok(())
}

#[cfg(test)]
#[path = "model_compatibility_tests.rs"]
mod tests;
