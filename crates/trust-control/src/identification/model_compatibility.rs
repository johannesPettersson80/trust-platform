//! Whether a model can tune a loop (`engineerModelCompatibility.ts`): the model's evidence, its input and output against
//! the loop's plant input and measured output (units and ranges), and the sample time of a discrete model. The loop's
//! channel bindings are parked with stage V7, so a loop here has one plant input and one measured output, and the model
//! must have one input and one output as well.

use serde_json::Value;

use crate::loop_definition::LoopDefinition;
use crate::model::{GeneralModel, ModelSource, ModelValidation};

/// Why a model cannot be used with a loop. The sentences are in `identification::texts`.
#[derive(Debug, Clone, PartialEq)]
pub enum CompatibilityRefusal {
    /// The model has no measurement evidence, and it is not a fit whose evidence is its own recording.
    Evidence,
    /// The model does not have one input and one output, which the loop's channel bindings would map.
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

/// The plant input and measured output of a loop as the model's channels connect to them.
struct Connection {
    /// The plant input's symbol.
    input_symbol: String,
    /// The plant input's unit.
    input_unit: String,
    /// The smallest value the actuator accepts.
    actuator_minimum: f64,
    /// The largest value the actuator accepts.
    actuator_maximum: f64,
    /// The measured output's symbol.
    output_symbol: String,
    /// The measured output's unit.
    output_unit: String,
    /// The smallest value the measurement can take.
    output_minimum: f64,
    /// The largest value the measurement can take.
    output_maximum: f64,
    /// The unit of the loop's setpoint, when the loop declares one.
    reference_unit: Option<String>,
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
    let connection = connected(loop_definition, model)?;
    check_units_and_ranges(model, &connection)?;
    if connection.reference_unit.is_none() {
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

/// Checks a model's qualification against the loop (`assertQualifiedModelResult`): its evidence, and, when it has one
/// input and one output, their units and ranges. A model with more channels is qualified without the channel check.
///
/// # Errors
/// Returns the [`CompatibilityRefusal`] of the evidence check, or of the units and ranges check.
pub fn assert_qualified_model_result(
    loop_definition: &LoopDefinition,
    model: &GeneralModel,
) -> Result<(), CompatibilityRefusal> {
    assert_model_evidence(model)?;
    let identity = model.identity();
    if identity.inputs.len() == 1 && identity.outputs.len() == 1 {
        let connection = connected(loop_definition, model)?;
        check_units_and_ranges(model, &connection)?;
    }
    Ok(())
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

/// The loop's plant input and measured output, which the model's single input and output connect to. The actuator range
/// is the output limits, or the ON/OFF levels when the controller is an ON/OFF one.
fn connected(
    loop_definition: &LoopDefinition,
    model: &GeneralModel,
) -> Result<Connection, CompatibilityRefusal> {
    let identity = model.identity();
    if identity.inputs.len() != 1 || identity.outputs.len() != 1 {
        return Err(CompatibilityRefusal::UnmappedChannels);
    }
    let manipulated = &loop_definition.signals.manipulated_variable;
    let process = &loop_definition.signals.process_variable;
    let limits = &loop_definition.limits;
    let (actuator_minimum, actuator_maximum) = match loop_definition.controller.on_off.as_ref() {
        Some(on_off) => (
            on_off
                .get("lowOutput")
                .and_then(Value::as_f64)
                .or(limits.output_minimum),
            on_off
                .get("highOutput")
                .and_then(Value::as_f64)
                .or(limits.output_maximum),
        ),
        None => (limits.output_minimum, limits.output_maximum),
    };
    // Without both bounds the model's input has no usable actuator range to meet (B3.9).
    let (Some(actuator_minimum), Some(actuator_maximum)) = (actuator_minimum, actuator_maximum)
    else {
        let Some(input) = identity.inputs.first() else {
            return Err(CompatibilityRefusal::UnmappedChannels);
        };
        return Err(CompatibilityRefusal::InputActuatorRange {
            id: input.id.clone(),
        });
    };
    Ok(Connection {
        input_symbol: manipulated.symbol.clone(),
        input_unit: manipulated.unit.clone(),
        actuator_minimum,
        actuator_maximum,
        output_symbol: process.symbol.clone(),
        output_unit: process.unit.clone(),
        output_minimum: process.range.minimum,
        output_maximum: process.range.maximum,
        reference_unit: loop_definition
            .signals
            .setpoint
            .as_ref()
            .map(|setpoint| setpoint.unit.clone()),
    })
}

/// The model's input must fit the actuator and have the plant input's unit; its output must overlap the measurement's
/// range, have the measured output's unit, and match the setpoint's unit when the loop has a setpoint.
fn check_units_and_ranges(
    model: &GeneralModel,
    connection: &Connection,
) -> Result<(), CompatibilityRefusal> {
    let identity = model.identity();
    let (Some(input), Some(output)) = (identity.inputs.first(), identity.outputs.first()) else {
        return Err(CompatibilityRefusal::UnmappedChannels);
    };
    let input_range = input.operating_range;
    if input_range.maximum < connection.actuator_minimum
        || input_range.minimum > connection.actuator_maximum
    {
        return Err(CompatibilityRefusal::InputActuatorRange {
            id: input.id.clone(),
        });
    }
    if input.unit != connection.input_unit {
        return Err(CompatibilityRefusal::InputUnit {
            id: input.id.clone(),
            symbol: connection.input_symbol.clone(),
        });
    }
    let output_range = output.operating_range;
    if output_range.maximum < connection.output_minimum
        || output_range.minimum > connection.output_maximum
    {
        return Err(CompatibilityRefusal::OutputMeasurementRange {
            id: output.id.clone(),
        });
    }
    if output.unit != connection.output_unit {
        return Err(CompatibilityRefusal::OutputUnit {
            id: output.id.clone(),
            symbol: connection.output_symbol.clone(),
        });
    }
    if connection
        .reference_unit
        .as_ref()
        .is_some_and(|unit| *unit != connection.output_unit)
    {
        return Err(CompatibilityRefusal::SetpointUnit {
            symbol: connection.output_symbol.clone(),
        });
    }
    Ok(())
}

#[cfg(test)]
#[path = "model_compatibility_tests.rs"]
mod tests;
