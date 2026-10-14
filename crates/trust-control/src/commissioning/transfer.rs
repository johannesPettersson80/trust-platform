//! The transfer of Control Studio's commissioning (specification 35, A6.3 and B6.1; annex E6.2): the binding that one
//! transfer writes, its modes, states, guards and outputs, built from a loop's declared signals for proportional and PID
//! commissioning (`proportionalTransfer.ts`, `pidTransfer.ts`). The tracking that goes with a binding is in
//! `transfer_tracking`.

// The fixed type names of the V4 contract begin with the name of this module.
#![allow(clippy::module_name_repetitions)]

use std::collections::BTreeMap;

use schemars::JsonSchema;
use serde::{Deserialize, Serialize};

use super::actuator_feedback::with_actuator_feedback;
use super::texts;
use crate::failure::{Cause, Failure, NextAction};
use crate::loop_definition::{
    ControllerLaw, LoopDefinition, LoopSignal, ManualOutputType, Parameter, ParameterBinding,
    PidInitialization, RealType, StateReference,
};

/// The IEC type of a transfer member (E6.2): `REAL`, `LREAL` or `BOOL`.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "UPPERCASE")]
pub enum TransferValueType {
    /// `REAL`, single precision: an exact value is compared after rounding to single precision (A6.4).
    Real,
    /// `LREAL`.
    Lreal,
    /// `BOOL`, 0 or 1 only.
    Bool,
}

impl TransferValueType {
    /// The type of an IEC type word; `None` for anything but `REAL`, `LREAL` and `BOOL`.
    #[must_use]
    pub fn from_iec(word: &str) -> Option<Self> {
        match word {
            "REAL" => Some(Self::Real),
            "LREAL" => Some(Self::Lreal),
            "BOOL" => Some(Self::Bool),
            _ => None,
        }
    }
}

impl From<RealType> for TransferValueType {
    fn from(value: RealType) -> Self {
        match value {
            RealType::Real => Self::Real,
            RealType::Lreal => Self::Lreal,
        }
    }
}

/// One member of a transfer, a state, a guard or an output, with its IEC type, unit and bounds (E6.2).
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct TransferMember {
    /// The variable's path.
    pub symbol: String,
    /// The variable's IEC type.
    pub iec_type: TransferValueType,
    /// The variable's unit; `1` for a BOOL.
    pub unit: String,
    /// The smallest value the variable may take.
    pub minimum: f64,
    /// The largest value the variable may take.
    pub maximum: f64,
}

/// A mode of a transfer: a BOOL variable, and the value that means Manual (E6.2).
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct TransferModeBinding {
    /// The mode variable's path.
    pub symbol: String,
    /// The value that means Manual.
    pub manual_value: bool,
}

/// The binding of one transfer (annex E6.2): the loop, its task, its mode and the members it writes or checks. An
/// omitted optional field of the protocol is an empty list or map here.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct TransferBinding {
    /// The loop the transfer belongs to.
    pub loop_id: String,
    /// The task that owns every member.
    pub task_name: String,
    /// The loop's mode.
    pub mode: TransferModeBinding,
    /// Further controllers' modes, switched at the same task boundary.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub additional_modes: Vec<TransferModeBinding>,
    /// Each tracked output's feedback guard: output symbol to guard symbol (A6.3).
    #[serde(default, skip_serializing_if = "BTreeMap::is_empty")]
    pub tracking_outputs: BTreeMap<String, String>,
    /// The states the transfer writes.
    pub states: Vec<TransferMember>,
    /// The members the transfer checks and reads.
    pub guards: Vec<TransferMember>,
    /// The controller output the transfer holds.
    pub output: TransferMember,
    /// Further controller outputs, held with `output`.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub additional_outputs: Vec<TransferMember>,
}

/// A value a transfer status reports for a member or a mode: a number, or a BOOL (annex E6.3).
#[derive(Debug, Clone, Copy, PartialEq)]
pub enum TransferValue {
    /// A number.
    Number(f64),
    /// A BOOL.
    Boolean(bool),
}

impl TransferValue {
    /// The number this value holds; `None` for a BOOL (the `TypeScript`'s `typeof value === "number"`).
    #[must_use]
    pub fn as_number(self) -> Option<f64> {
        match self {
            Self::Number(value) => Some(value),
            Self::Boolean(_) => None,
        }
    }
}

/// The value a transfer writes for `member`: a REAL is rounded to single precision (`Math.fround`), and the result must be
/// finite and inside the member's bounds (`roundedTransferValue`, A6.4, E6.2).
///
/// # Errors
/// A BOOL member takes 0 or 1 only, and the rounded value must be finite and inside the member's bounds.
pub fn rounded_transfer_value(member: &TransferMember, value: f64) -> Result<f64, Failure> {
    let binary = value.fract() == 0.0 && (0.0..=1.0).contains(&value);
    if member.iec_type == TransferValueType::Bool && !binary {
        return Err(Failure::new(
            &texts::transfer_bool_not_binary(),
            Cause::Runtime,
            NextAction::None,
        ));
    }
    let rounded = match member.iec_type {
        TransferValueType::Real => round_to_single(value),
        TransferValueType::Lreal | TransferValueType::Bool => value,
    };
    if !rounded.is_finite() || !(member.minimum..=member.maximum).contains(&rounded) {
        return Err(Failure::new(
            &texts::transfer_outside_range(&member.symbol),
            Cause::Settings,
            NextAction::OpenLoopSetup,
        ));
    }
    Ok(rounded)
}

/// The nearest single-precision value, as an `f64` (`Math.fround`).
#[allow(clippy::cast_possible_truncation)] // Rounding to single precision is the purpose of this cast.
fn round_to_single(value: f64) -> f64 {
    f64::from(value as f32)
}

/// The output's minimum and maximum: the loop's output limits (A7).
///
/// # Errors
/// One of the output limits is not set; Auto commissioning needs the output range.
pub fn output_limits(loop_definition: &LoopDefinition) -> Result<(f64, f64), Failure> {
    match (
        loop_definition.limits.output_minimum,
        loop_definition.limits.output_maximum,
    ) {
        (Some(minimum), Some(maximum)) => Ok((minimum, maximum)),
        _ => Err(Failure::new(
            &texts::transfer_output_limits_missing(),
            Cause::Settings,
            NextAction::OpenLimits,
        )),
    }
}

/// The gain parameter of a proportional law (`proportionalParameter`, `engineerVerificationPlan.ts`). The layer that owns
/// that file is ported later; the transfer needs this decision now, so it is ported here. The analysis-based controller of
/// that file is a parked feature and is not ported.
///
/// # Errors
/// The loop has no proportional law or no setpoint, or its gain is not one of the bound parameters.
pub fn proportional_parameter(loop_definition: &LoopDefinition) -> Result<&Parameter, Failure> {
    let unset = || {
        Failure::new(
            &texts::transfer_proportional_unset(),
            Cause::Implementation,
            NextAction::OpenLoopSetup,
        )
    };
    let (Some(ControllerLaw::Proportional { gain_symbol }), Some(_)) = (
        loop_definition.controller.law.as_ref(),
        loop_definition.signals.setpoint.as_ref(),
    ) else {
        return Err(unset());
    };
    loop_definition
        .parameter_binding
        .as_ref()
        .and_then(|binding| {
            binding
                .parameters
                .iter()
                .find(|parameter| parameter.symbol == *gain_symbol)
        })
        .ok_or_else(unset)
}

/// The explicit PI/PID state contract of a loop: its integral and derivative states and the setpoint weights of the
/// `parallel-pid-v1` law (`pidInitializationAdapter`, `pidInitialization.ts`; annex E6.4). The decision is ported here so
/// that the transfer and its tracking need no other file; the PID initialization layer reuses it.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct ParallelPidContract<'a> {
    /// The integral state.
    pub integral_state: &'a StateReference,
    /// The derivative state.
    pub derivative_state: &'a StateReference,
    /// β of the law, `P = Kp·(β·SP − PV)`.
    pub proportional_setpoint_weight: f64,
    /// γ of the law, `D = Kd·(γ·dSP/dt − dPV/dt)`.
    pub derivative_setpoint_weight: f64,
}

/// The `parallel-pid-v1` contract of a loop with a PID law and a setpoint (`pidInitializationAdapter`).
///
/// # Errors
/// The loop does not declare the `parallel-pid-v1` initialization, a PID law and a setpoint.
pub fn parallel_pid_contract(
    loop_definition: &LoopDefinition,
) -> Result<ParallelPidContract<'_>, Failure> {
    match (
        loop_definition.controller.pid_initialization.as_ref(),
        loop_definition.controller.law.as_ref(),
        loop_definition.signals.setpoint.as_ref(),
    ) {
        (
            Some(PidInitialization::ParallelPid {
                integral_state,
                derivative_state,
                proportional_setpoint_weight,
                derivative_setpoint_weight,
                ..
            }),
            Some(ControllerLaw::Pid(_)),
            Some(_),
        ) => Ok(ParallelPidContract {
            integral_state,
            derivative_state,
            proportional_setpoint_weight: *proportional_setpoint_weight,
            derivative_setpoint_weight: *derivative_setpoint_weight,
        }),
        _ => Err(Failure::new(
            &texts::transfer_pid_adapter_required(),
            Cause::ControllerCapability,
            NextAction::ChooseMethod,
        )),
    }
}

/// A REAL or LREAL primary signal as a transfer member, with its own range (`member` of the `TypeScript`).
fn signal_member(signal: &LoopSignal) -> Option<TransferMember> {
    let iec_type = TransferValueType::from_iec(&signal.iec_type)
        .filter(|iec_type| *iec_type != TransferValueType::Bool)?;
    Some(TransferMember {
        symbol: signal.symbol.clone(),
        iec_type,
        unit: signal.unit.clone(),
        minimum: signal.range.minimum,
        maximum: signal.range.maximum,
    })
}

/// A controller state of the PID contract as a transfer member.
fn state_member(state: &StateReference) -> TransferMember {
    TransferMember {
        symbol: state.symbol.clone(),
        iec_type: state.iec_type.into(),
        unit: state.unit.clone(),
        minimum: state.minimum,
        maximum: state.maximum,
    }
}

/// A parameter of the record as a transfer guard.
fn parameter_member(parameter: &Parameter) -> TransferMember {
    TransferMember {
        symbol: parameter.symbol.clone(),
        iec_type: parameter.iec_type.into(),
        unit: parameter.unit.clone(),
        minimum: parameter.minimum,
        maximum: parameter.maximum,
    }
}

/// The Manual command: the manual output's variable and its REAL or LREAL type. `None` when the loop does not name one, or
/// names a BOOL (a BOOL output cannot be transferred bumplessly).
fn command_binding(loop_definition: &LoopDefinition) -> Option<(String, TransferValueType)> {
    let controller = &loop_definition.controller;
    let symbol = controller.manual_output_signal.clone()?;
    let iec_type = match controller.manual_output_type? {
        ManualOutputType::Real => TransferValueType::Real,
        ManualOutputType::Lreal => TransferValueType::Lreal,
        ManualOutputType::Bool => return None,
    };
    Some((symbol, iec_type))
}

fn mode_binding(parameters: &ParameterBinding) -> TransferModeBinding {
    TransferModeBinding {
        symbol: parameters.mode.symbol.clone(),
        manual_value: parameters.mode.manual_value,
    }
}

/// The binding of a proportional commissioning (`proportionalTransferBinding`, A6.3). The reference and the Manual command
/// are states; the gain and the process value are guards; the controller output is held, within the output limits. An
/// actuator that is measured is tracked as the output's feedback (`withActuatorFeedback`).
///
/// # Errors
/// The loop has no proportional gain, a reference, a mode or a REAL or LREAL Manual command; a primary signal is not REAL or
/// LREAL; or an output limit is not set.
pub fn proportional_transfer_binding(
    loop_definition: &LoopDefinition,
) -> Result<TransferBinding, Failure> {
    proportional_parameter(loop_definition)?;
    let (Some(parameters), Some(reference), Some((manual, manual_type))) = (
        loop_definition.parameter_binding.as_ref(),
        loop_definition.signals.setpoint.as_ref(),
        command_binding(loop_definition),
    ) else {
        return Err(Failure::new(
            &texts::transfer_configure_binding(),
            Cause::Implementation,
            NextAction::OpenLoopSetup,
        ));
    };
    let reference = signal_member(reference).ok_or_else(primary_failure)?;
    let (minimum, maximum) = output_limits(loop_definition)?;
    let output = TransferMember {
        minimum,
        maximum,
        ..signal_member(&loop_definition.signals.manipulated_variable)
            .ok_or_else(primary_failure)?
    };
    let process_variable =
        signal_member(&loop_definition.signals.process_variable).ok_or_else(primary_failure)?;
    let binding = TransferBinding {
        loop_id: parameters.loop_id.clone(),
        task_name: parameters.task_name.clone(),
        mode: mode_binding(parameters),
        additional_modes: Vec::new(),
        tracking_outputs: BTreeMap::new(),
        states: vec![
            reference,
            TransferMember {
                symbol: manual,
                iec_type: manual_type,
                ..output.clone()
            },
        ],
        guards: parameters
            .parameters
            .iter()
            .map(parameter_member)
            .chain(std::iter::once(process_variable))
            .collect(),
        output,
        additional_outputs: Vec::new(),
    };
    with_actuator_feedback(loop_definition, binding)
}

/// The binding of a PID commissioning (`pidTransferBinding`, A6.3): as the proportional binding, with the integral and
/// derivative states of the `parallel-pid-v1` contract as states after the Manual command.
///
/// # Errors
/// The loop has no `parallel-pid-v1` contract, or its parameter, reference, mode or Manual command is not bound as the
/// contract needs; a signal or state is not REAL or LREAL; or an output limit is not set.
pub fn pid_transfer_binding(loop_definition: &LoopDefinition) -> Result<TransferBinding, Failure> {
    let contract = parallel_pid_contract(loop_definition)?;
    let (Some(parameters), Some(reference), Some((manual, manual_type))) = (
        loop_definition.parameter_binding.as_ref(),
        loop_definition.signals.setpoint.as_ref(),
        command_binding(loop_definition),
    ) else {
        return Err(Failure::new(
            &texts::transfer_pid_incompatible(),
            Cause::Implementation,
            NextAction::OpenLoopSetup,
        ));
    };
    let reference = signal_member(reference).ok_or_else(pid_states_failure)?;
    let (minimum, maximum) = output_limits(loop_definition)?;
    let output = TransferMember {
        minimum,
        maximum,
        ..signal_member(&loop_definition.signals.manipulated_variable)
            .ok_or_else(pid_states_failure)?
    };
    let process_variable =
        signal_member(&loop_definition.signals.process_variable).ok_or_else(pid_states_failure)?;
    let binding = TransferBinding {
        loop_id: parameters.loop_id.clone(),
        task_name: parameters.task_name.clone(),
        mode: mode_binding(parameters),
        additional_modes: Vec::new(),
        tracking_outputs: BTreeMap::new(),
        states: vec![
            reference,
            TransferMember {
                symbol: manual,
                iec_type: manual_type,
                ..output.clone()
            },
            state_member(contract.integral_state),
            state_member(contract.derivative_state),
        ],
        guards: parameters
            .parameters
            .iter()
            .map(parameter_member)
            .chain(std::iter::once(process_variable))
            .collect(),
        output,
        additional_outputs: Vec::new(),
    };
    with_actuator_feedback(loop_definition, binding)
}

fn primary_failure() -> Failure {
    Failure::new(
        &texts::transfer_primary_not_real(),
        Cause::Implementation,
        NextAction::OpenLoopSetup,
    )
}

fn pid_states_failure() -> Failure {
    Failure::new(
        &texts::transfer_pid_states_not_real(),
        Cause::Implementation,
        NextAction::OpenLoopSetup,
    )
}

#[cfg(test)]
#[path = "transfer_tests.rs"]
mod tests;
