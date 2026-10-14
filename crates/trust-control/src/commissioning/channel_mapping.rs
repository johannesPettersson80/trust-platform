//! The loop's signals as the commissioning's transfers and models see them, for a loop without channel bindings
//! (specification 35, §17.1 and C0; the Rust engine runs no channels yet, checklist V7). Ported from the scalar branches of
//! `declaredControllerChannels.ts` and of `connectedModelChannels` in `loopChannels.ts`. A loop with channel bindings is
//! refused here: its multichannel rules are parked and not ported.

use super::texts;
use super::transfer::{output_limits, TransferMember, TransferValueType};
use crate::failure::{Cause, Failure, NextAction};
use crate::loop_definition::{LoopDefinition, LoopSignal, ManualOutputType};
use crate::model::GeneralModel;

/// The channels of a loop without channel bindings, as a transfer declares them: the setpoint and its process value, the
/// controller output with the output limits, and the Manual command that holds its place (`declaredControllerChannels`).
#[derive(Debug, Clone, PartialEq)]
pub struct DeclaredChannels {
    /// The setpoint, the reference the loop follows.
    pub reference: TransferMember,
    /// The process variable, the measurement the loop controls.
    pub process: TransferMember,
    /// The controller output, within the output limits (A7).
    pub output: TransferMember,
    /// The Manual command: the variable that Manual writes, with the output's bounds.
    pub command: TransferMember,
}

impl DeclaredChannels {
    /// The measurements of a loop without channels: its process variable alone. The `TypeScript` lists the process variable
    /// here, so a declared binding reads it as a guard and the tracking names it (`declaredControllerChannels`).
    #[must_use]
    pub fn measurements(&self) -> Vec<TransferMember> {
        vec![self.process.clone()]
    }
}

/// The plant connection of a loop without channel bindings: the plant input the controller drives and the output it
/// measures (`connectedModelChannels`, the scalar branch).
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct ConnectedChannels<'a> {
    /// The plant input: the manipulated variable.
    pub input: &'a LoopSignal,
    /// The measured output: the process variable.
    pub output: &'a LoopSignal,
}

/// The declared channels of a loop without channel bindings (`declaredControllerChannels`, the scalar branch).
///
/// # Errors
/// The loop has channel bindings; it has no setpoint, a Manual command or a Manual command type; its setpoint, process
/// value or output is not REAL or LREAL (a BOOL output is accepted when it is a dimensionless level from 0 to 1); or an
/// output limit is not set.
pub fn declared_channels(loop_definition: &LoopDefinition) -> Result<DeclaredChannels, Failure> {
    if loop_definition.channels.is_some() {
        return Err(channels_parked());
    }
    let setpoint = loop_definition.signals.setpoint.as_ref().ok_or_else(|| {
        Failure::new(
            &texts::init_reference_missing(),
            Cause::Implementation,
            NextAction::OpenLoopSetup,
        )
    })?;
    let reference = coordinate_member(setpoint, false)?;
    let process = coordinate_member(&loop_definition.signals.process_variable, false)?;
    let (Some(command_symbol), Some(command_type)) = (
        loop_definition.controller.manual_output_signal.as_ref(),
        loop_definition.controller.manual_output_type,
    ) else {
        return Err(manual_command_missing());
    };
    let (minimum, maximum) = output_limits(loop_definition)?;
    let output = TransferMember {
        minimum,
        maximum,
        ..coordinate_member(&loop_definition.signals.manipulated_variable, true)?
    };
    let command = TransferMember {
        symbol: command_symbol.clone(),
        iec_type: manual_type(command_type),
        ..output.clone()
    };
    Ok(DeclaredChannels {
        reference,
        process,
        output,
        command,
    })
}

/// The plant connection of a loop without channel bindings (`connectedModelChannels`, the scalar branch): the model must
/// have one input and one output, which the loop's manipulated variable and process variable connect to.
///
/// # Errors
/// The loop has channel bindings, or the model is not one input and one output.
pub fn connected_model_channels<'a>(
    loop_definition: &'a LoopDefinition,
    model: &GeneralModel,
) -> Result<ConnectedChannels<'a>, Failure> {
    if loop_definition.channels.is_some() {
        return Err(channels_parked());
    }
    let identity = model.identity();
    if identity.inputs.len() != 1 || identity.outputs.len() != 1 {
        return Err(Failure::new(
            &texts::init_model_channels_unmapped(),
            Cause::ModelOrData,
            NextAction::Identify,
        ));
    }
    Ok(ConnectedChannels {
        input: &loop_definition.signals.manipulated_variable,
        output: &loop_definition.signals.process_variable,
    })
}

/// A signal as a transfer member (`transferSignalMember`): REAL or LREAL, or with `actuator`, also a dimensionless BOOL
/// level from 0 to 1 (an ON/OFF output).
#[allow(clippy::float_cmp)] // The BOOL level is recognised by its exact bounds 0 and 1, as the TypeScript checks them.
fn coordinate_member(signal: &LoopSignal, actuator: bool) -> Result<TransferMember, Failure> {
    let level = actuator
        && signal.iec_type == "BOOL"
        && signal.unit == "1"
        && signal.range.minimum == 0.0
        && signal.range.maximum == 1.0;
    let iec_type = match signal.iec_type.as_str() {
        "REAL" => TransferValueType::Real,
        "LREAL" => TransferValueType::Lreal,
        "BOOL" if level => TransferValueType::Bool,
        _ => {
            return Err(Failure::new(
                &texts::init_primary_coordinates(),
                Cause::Implementation,
                NextAction::OpenLoopSetup,
            ))
        }
    };
    Ok(TransferMember {
        symbol: signal.symbol.clone(),
        iec_type,
        unit: signal.unit.clone(),
        minimum: signal.range.minimum,
        maximum: signal.range.maximum,
    })
}

/// The IEC type of a Manual command.
fn manual_type(command_type: ManualOutputType) -> TransferValueType {
    match command_type {
        ManualOutputType::Real => TransferValueType::Real,
        ManualOutputType::Lreal => TransferValueType::Lreal,
        ManualOutputType::Bool => TransferValueType::Bool,
    }
}

/// The refusal of a loop whose Manual command or its type is not declared.
fn manual_command_missing() -> Failure {
    Failure::new(
        &texts::init_manual_command_missing(),
        Cause::Implementation,
        NextAction::OpenLoopSetup,
    )
}

/// The refusal of a loop with channel bindings, which the Rust engine does not run yet.
fn channels_parked() -> Failure {
    Failure::new(
        &texts::init_channels_parked(),
        Cause::ControllerCapability,
        NextAction::None,
    )
}

#[cfg(test)]
#[path = "channel_mapping_tests.rs"]
mod tests;
