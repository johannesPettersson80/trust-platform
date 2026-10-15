//! The commissioning adapter of a PID group (specification 35, C7; `pidControllerGroup.ts`, `assertPidGroupAdapter`): the checks that
//! make a group one coherent transaction (its members, parameters and states belong to one compiled program, and each is claimed
//! once), and the transfer binding that writes every member's output together. A group's declared states and its tracking go through
//! the declared initialization, which refuses a loop with channel bindings until the channel port (checklist V7) makes it group-aware.

use std::collections::{BTreeMap, BTreeSet};

use super::actuator_feedback::with_actuator_feedback;
use super::initial_state::transfer_member;
use super::pid_measurement::initialized_states;
use super::project_law::additional_mode_members;
use super::texts;
use super::transfer::{TransferBinding, TransferMember, TransferModeBinding, TransferValueType};
use crate::failure::{Cause, Failure, NextAction};
use crate::loop_definition::{
    channel_signal, ChannelInput, ChannelOutput, ChannelSignal, ControllerProject,
    InitializedState, LoopDefinition, ManualOutputType, ParameterBinding, PidControllerGroup,
    PidGroupMember, ProjectSeedBlock,
};

/// The retained states of one `Control.PID` member (`assertLibraryPidStates`).
const RETAINED_STATES: usize = 7;
/// The refusal of a group whose loop has no project, or no parameter record (`assertPidGroupAdapter`).
const NEEDS_PROJECT: &str = "This PID group needs a project-owned coherent parameter and state adapter before tuning or Apply.";
/// The refusal of a group whose members or transfer signals belong to more than one compiled program.
const ONE_PROGRAM: &str = "Coherent PID group application requires all members and transfer signals to belong to the same compiled program.";
/// The refusal of a parameter record that is not exactly the members' parameters, each with one owner.
const PARAMETER_UNION: &str =
    "Declare the complete PID group parameter union with one owner per parameter.";
/// The refusal of a state list that is not exactly the members' states, each with one owner.
const STATE_UNION: &str =
    "Declare the complete PID group retained and application state union with one owner per state.";
/// The refusal of a group that names a channel or a signal which the loop does not declare.
const UNDECLARED_SIGNAL: &str =
    "A PID group names a channel or signal that the loop does not declare.";
/// The refusal of a transfer whose loop setpoint or manipulated variable is not a member's (`declaredControllerChannels`).
const PRIMARY_NOT_MEMBER: &str =
    "The loop's primary reference and output must belong to the declared model channel bindings.";
/// The refusal of a group whose values are asked for as gains: a group writes its values by symbol (C7).
const NOT_BY_GAINS: &str = "A PID group writes its values by symbol, never through gains (C7).";

/// The transfer members of one group member (`declaredControllerChannels`, the channel branch): its setpoint, its Manual command, its
/// controller output with the output's actuator limits, and its process value.
struct MemberChannels {
    reference: TransferMember,
    command: TransferMember,
    output: TransferMember,
    measurement: TransferMember,
}

/// The refusal of a group that the project or the parameter record does not support (C7).
#[must_use]
pub fn group_needs_project() -> Failure {
    refusal(NEEDS_PROJECT)
}

/// The refusal of a group whose values are asked for as gains: a group writes its values by symbol, never through gains (C7).
#[must_use]
pub fn group_gains_refused() -> Failure {
    Failure::new(NOT_BY_GAINS, Cause::Implementation, NextAction::None)
}

/// Checks that a PID group is one coherent transaction (`assertPidGroupAdapter`): its declaration fits the loop's channels; the
/// members, the mode and the transfer signals belong to one compiled program; each member runs the shared mode and has a setpoint;
/// each member's retained states are seven, one of them `NominalOutput`; each parameter and each state has exactly one owner; and the
/// loop's parameter record and the project's states are exactly those owners. The coordinates and units of a member's seven states
/// are not checked here yet (the scalar `Control.PID` checks its own in `library_pid`).
///
/// # Errors
/// The group breaks one of these rules; the refusal names the rule and the symbol, as the `TypeScript` does.
pub fn check_group(
    loop_definition: &LoopDefinition,
    project: &ControllerProject,
    group: &PidControllerGroup,
) -> Result<(), Failure> {
    group
        .check(loop_definition.channels.as_ref())
        .map_err(|message| refusal(&message))?;
    let binding = loop_definition
        .parameter_binding
        .as_ref()
        .ok_or_else(group_needs_project)?;
    check_one_program(loop_definition, binding, project, group)?;
    let mut parameters = BTreeSet::new();
    let mut states = BTreeSet::new();
    for member in &group.members {
        check_member(
            loop_definition,
            binding,
            project,
            member,
            &mut parameters,
            &mut states,
        )?;
    }
    if let Some(ProjectSeedBlock::Decoupled(seed)) = project.seed.as_ref() {
        // A static decoupler's variables belong to the group as a whole (Specification 35 section 17.15).
        for symbol in seed.matrix_symbols.iter().flatten() {
            claim(&mut parameters, symbol, "parameter")?;
        }
    }
    if !complete(
        binding
            .parameters
            .iter()
            .map(|parameter| parameter.symbol.as_str()),
        &parameters,
    ) {
        return Err(refusal(PARAMETER_UNION));
    }
    if !complete(
        project
            .initialization
            .states()
            .iter()
            .map(|state| state.symbol.as_str()),
        &states,
    ) {
        return Err(refusal(STATE_UNION));
    }
    Ok(())
}

/// The members, the mode and the transfer signals belong to one compiled program (`assertPidGroupAdapter`): the first element of
/// each symbol, in upper case, is the same. A missing optional symbol counts as an empty root, as in the `TypeScript`, so it is refused.
fn check_one_program(
    loop_definition: &LoopDefinition,
    binding: &ParameterBinding,
    project: &ControllerProject,
    group: &PidControllerGroup,
) -> Result<(), Failure> {
    let mut roots = BTreeSet::new();
    roots.extend(group.members.iter().map(|member| root(&member.instance)));
    roots.insert(root(&binding.mode.symbol));
    roots.extend(
        binding
            .parameters
            .iter()
            .map(|parameter| root(&parameter.symbol)),
    );
    roots.extend(
        project
            .initialization
            .states()
            .iter()
            .map(|state| root(&state.symbol)),
    );
    for input in loop_definition
        .channels
        .iter()
        .flat_map(|channels| channels.inputs.iter())
    {
        roots.insert(root(signal_of(loop_definition, &input.signal_id)?.symbol));
        let command = input
            .manual_output_signal
            .as_deref()
            .or(loop_definition.controller.manual_output_signal.as_deref());
        roots.insert(root(command.unwrap_or("")));
    }
    for output in loop_definition
        .channels
        .iter()
        .flat_map(|channels| channels.outputs.iter())
    {
        roots.insert(root(signal_of(loop_definition, &output.signal_id)?.symbol));
        let setpoint = match output.setpoint_signal_id.as_deref() {
            Some(id) => signal_of(loop_definition, id)?.symbol,
            None => "",
        };
        roots.insert(root(setpoint));
    }
    if roots.len() == 1 {
        Ok(())
    } else {
        Err(refusal(ONE_PROGRAM))
    }
}

/// Checks one member (the member loop of `assertPidGroupAdapter`): its mode, its parameters, its setpoint, its retained states, and its
/// application states, each claimed once and each in its own program.
fn check_member(
    loop_definition: &LoopDefinition,
    binding: &ParameterBinding,
    project: &ControllerProject,
    member: &PidGroupMember,
    parameters: &mut BTreeSet<String>,
    states: &mut BTreeSet<String>,
) -> Result<(), Failure> {
    let instance = member.instance.as_str();
    if member.mode_symbol != binding.mode.symbol {
        return Err(refusal(&format!(
            "PID group member {instance} must use the declared shared mode for coherent application."
        )));
    }
    for symbol in &member.parameter_symbols {
        claim(parameters, symbol, "parameter")?;
    }
    if output_channel(loop_definition, member)?
        .setpoint_signal_id
        .is_none()
    {
        return Err(refusal(&format!(
            "PID group member {instance} needs its declared setpoint."
        )));
    }
    let own = own_states(project, instance);
    check_retained_states(instance, &own)?;
    let mut owned: BTreeSet<&str> = own.iter().map(|state| state.symbol.as_str()).collect();
    owned.extend(
        member
            .application_state_symbols
            .iter()
            .flatten()
            .map(String::as_str),
    );
    for symbol in owned {
        if !symbol
            .to_uppercase()
            .starts_with(format!("{}.", root(instance)).as_str())
        {
            return Err(refusal(&format!(
                "PID group state {symbol} must belong to {instance}'s owning program."
            )));
        }
        claim(states, symbol, "state")?;
    }
    Ok(())
}

/// A member's own retained states (`assertLibraryPidStates`): exactly seven, one of them `NominalOutput`.
fn check_retained_states(instance: &str, own: &[&InitializedState]) -> Result<(), Failure> {
    if own.len() != RETAINED_STATES {
        return Err(refusal(&texts::library_pid_seven_states(instance)));
    }
    let nominal = format!("{instance}.StateValue.NominalOutput");
    if own.iter().any(|state| state.symbol == nominal) {
        Ok(())
    } else {
        Err(refusal(&texts::library_pid_state_contract(
            instance,
            "NominalOutput",
        )))
    }
}

/// The project's states that a member's instance declares for itself (`<instance>.StateValue.` in the `TypeScript`).
fn own_states<'a>(project: &'a ControllerProject, instance: &str) -> Vec<&'a InitializedState> {
    let prefix = format!("{instance}.StateValue.");
    project
        .initialization
        .states()
        .iter()
        .filter(|state| state.symbol.starts_with(prefix.as_str()))
        .collect()
}

/// Claims a parameter or a state for one owner, without regard to case (`claim`): a second claim is refused.
fn claim(owners: &mut BTreeSet<String>, symbol: &str, kind: &str) -> Result<(), Failure> {
    if owners.insert(symbol.to_uppercase()) {
        Ok(())
    } else {
        Err(refusal(&format!(
            "PID group {kind} {symbol} has more than one owner."
        )))
    }
}

/// Whether the symbols are exactly the owners (`complete`): as many, each distinct without regard to case, and each one an owner.
fn complete<'a>(symbols: impl Iterator<Item = &'a str>, owners: &BTreeSet<String>) -> bool {
    let actual: Vec<String> = symbols.map(str::to_uppercase).collect();
    let distinct: BTreeSet<&String> = actual.iter().collect();
    actual.len() == owners.len()
        && distinct.len() == actual.len()
        && actual.iter().all(|symbol| owners.contains(symbol))
}

/// The program that owns a symbol: its first path element, in upper case (`split(".")[0].toUpperCase()`).
fn root(symbol: &str) -> String {
    symbol.split('.').next().unwrap_or_default().to_uppercase()
}

/// The refusal of a group's declaration or application, with the next step that the engineer takes (C7).
fn refusal(message: &str) -> Failure {
    Failure::new(
        message,
        Cause::ControllerCapability,
        NextAction::OpenLoopSetup,
    )
}

/// The loop signal that a channel names (`loopSignal`).
fn signal_of<'a>(
    loop_definition: &'a LoopDefinition,
    id: &str,
) -> Result<ChannelSignal<'a>, Failure> {
    channel_signal(loop_definition, id).ok_or_else(|| refusal(UNDECLARED_SIGNAL))
}

/// The model input that a member drives.
fn input_channel<'a>(
    loop_definition: &'a LoopDefinition,
    member: &PidGroupMember,
) -> Result<&'a ChannelInput, Failure> {
    loop_definition
        .channels
        .as_ref()
        .and_then(|channels| {
            channels
                .inputs
                .iter()
                .find(|input| input.model_channel_id == member.input_channel_id)
        })
        .ok_or_else(|| refusal(UNDECLARED_SIGNAL))
}

/// The model output that a member measures.
fn output_channel<'a>(
    loop_definition: &'a LoopDefinition,
    member: &PidGroupMember,
) -> Result<&'a ChannelOutput, Failure> {
    loop_definition
        .channels
        .as_ref()
        .and_then(|channels| {
            channels
                .outputs
                .iter()
                .find(|output| output.model_channel_id == member.output_channel_id)
        })
        .ok_or_else(|| refusal(UNDECLARED_SIGNAL))
}

/// The transfer binding of a PID group (`declaredTransferBinding` for the group's channels, C7): the loop's primary setpoint and Manual
/// command, then each other member's setpoint and command, then the project's initialized states; the parameters and every member's
/// process value are guards; the loop's manipulated variable is held, and each other member's output is an additional output (E6.2).
/// The further modes are the project's, then each member's mode that the loop's mode does not name.
///
/// # Errors
/// The loop lacks its parameter record, setpoint, Manual command or its type; a channel or signal that the group names is not
/// declared; the loop's setpoint or manipulated variable is not a member's; a member's signal is not REAL or LREAL; or two roles share
/// a symbol, without regard to case.
pub fn group_transfer_binding(
    loop_definition: &LoopDefinition,
    project: &ControllerProject,
    group: &PidControllerGroup,
) -> Result<TransferBinding, Failure> {
    let parameters = loop_definition
        .parameter_binding
        .as_ref()
        .ok_or_else(group_needs_project)?;
    let references_declared = loop_definition.signals.setpoint.is_some()
        && loop_definition.controller.manual_output_signal.is_some()
        && loop_definition.controller.manual_output_type.is_some();
    if !references_declared {
        return Err(Failure::new(
            &texts::declared_references_required(),
            Cause::Implementation,
            NextAction::OpenLoopSetup,
        ));
    }
    let members = group
        .members
        .iter()
        .map(|member| member_channels(loop_definition, member))
        .collect::<Result<Vec<MemberChannels>, Failure>>()?;
    let (reference, output) = primary_members(loop_definition, &members)?;
    let mode = TransferModeBinding {
        symbol: parameters.mode.symbol.clone(),
        manual_value: parameters.mode.manual_value,
    };
    let additional_modes = further_modes(loop_definition, group, &mode);
    let mut states = vec![
        members[reference].reference.clone(),
        members[output].command.clone(),
    ];
    for (index, member) in members.iter().enumerate() {
        if index != reference {
            states.push(member.reference.clone());
        }
    }
    for (index, member) in members.iter().enumerate() {
        if index != output {
            states.push(member.command.clone());
        }
    }
    states.extend(
        initialized_states(loop_definition, &project.initialization)
            .into_iter()
            .map(transfer_member),
    );
    let mut guards = parameter_guards(parameters);
    guards.extend(members.iter().map(|member| member.measurement.clone()));
    let additional_outputs = members
        .iter()
        .enumerate()
        .filter(|(index, _)| *index != output)
        .map(|(_, member)| member.output.clone())
        .collect();
    let binding = TransferBinding {
        loop_id: parameters.loop_id.clone(),
        task_name: parameters.task_name.clone(),
        mode,
        additional_modes,
        tracking_outputs: BTreeMap::new(),
        states,
        guards,
        output: members[output].output.clone(),
        additional_outputs,
    };
    let binding = with_actuator_feedback(loop_definition, binding)?;
    if roles_overlap(&binding) {
        return Err(Failure::new(
            &texts::declared_roles_overlap(),
            Cause::Implementation,
            NextAction::OpenLoopSetup,
        ));
    }
    Ok(binding)
}

/// One member's transfer members (`declaredControllerChannels`, the channel branch). Its Manual command is the loop's own for the member
/// whose input is the loop's manipulated variable, and the input's own otherwise. Its output keeps the input's actuator limits.
fn member_channels(
    loop_definition: &LoopDefinition,
    member: &PidGroupMember,
) -> Result<MemberChannels, Failure> {
    let input = input_channel(loop_definition, member)?;
    let channel = output_channel(loop_definition, member)?;
    let setpoint = channel.setpoint_signal_id.as_deref().ok_or_else(|| {
        refusal(&format!(
            "PID group member {} needs its declared setpoint.",
            member.instance
        ))
    })?;
    let signal = signal_of(loop_definition, &input.signal_id)?;
    let (command, command_type) =
        if signal.symbol == loop_definition.signals.manipulated_variable.symbol.as_str() {
            (
                loop_definition.controller.manual_output_signal.as_deref(),
                loop_definition.controller.manual_output_type,
            )
        } else {
            (
                input.manual_output_signal.as_deref(),
                input.manual_output_type,
            )
        };
    let (Some(command), Some(command_type)) = (command, command_type) else {
        return Err(refusal(&format!(
            "Declare the compiled Manual command and IEC type for {}.",
            signal.symbol
        )));
    };
    let output = TransferMember {
        minimum: input.actuator.minimum,
        maximum: input.actuator.maximum,
        ..signal_member(signal, true)?
    };
    Ok(MemberChannels {
        reference: signal_member(signal_of(loop_definition, setpoint)?, false)?,
        command: TransferMember {
            symbol: command.to_owned(),
            iec_type: manual_type(command_type),
            ..output.clone()
        },
        measurement: signal_member(signal_of(loop_definition, &channel.signal_id)?, false)?,
        output,
    })
}

/// The member whose setpoint is the loop's setpoint, and the member whose output is the loop's manipulated variable: the transfer holds
/// the two first (`primaryReference`, `primaryOutput`).
fn primary_members(
    loop_definition: &LoopDefinition,
    members: &[MemberChannels],
) -> Result<(usize, usize), Failure> {
    let setpoint = loop_definition
        .signals
        .setpoint
        .as_ref()
        .map(|signal| signal.symbol.as_str());
    let reference = members
        .iter()
        .position(|member| Some(member.reference.symbol.as_str()) == setpoint)
        .ok_or_else(|| refusal(PRIMARY_NOT_MEMBER))?;
    let manipulated = loop_definition.signals.manipulated_variable.symbol.as_str();
    let output = members
        .iter()
        .position(|member| member.output.symbol == manipulated)
        .ok_or_else(|| refusal(PRIMARY_NOT_MEMBER))?;
    Ok((reference, output))
}

/// A signal as a transfer member (`transferSignalMember`): REAL or LREAL, or with `actuator`, also a dimensionless BOOL level from 0
/// to 1 (an ON/OFF output).
#[allow(clippy::float_cmp)] // The BOOL level is recognised by its exact bounds 0 and 1, as the TypeScript checks them.
fn signal_member(signal: ChannelSignal<'_>, actuator: bool) -> Result<TransferMember, Failure> {
    let level = actuator
        && signal.iec_type == "BOOL"
        && signal.unit == "1"
        && signal.range.minimum == 0.0
        && signal.range.maximum == 1.0;
    let iec_type = match signal.iec_type {
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
        symbol: signal.symbol.to_owned(),
        iec_type,
        unit: signal.unit.to_owned(),
        minimum: signal.range.minimum,
        maximum: signal.range.maximum,
    })
}

/// The IEC type of a Manual command.
fn manual_type(command: ManualOutputType) -> TransferValueType {
    match command {
        ManualOutputType::Real => TransferValueType::Real,
        ManualOutputType::Lreal => TransferValueType::Lreal,
        ManualOutputType::Bool => TransferValueType::Bool,
    }
}

/// The parameters of the record as the transfer's guards (`parameterMember`).
fn parameter_guards(binding: &ParameterBinding) -> Vec<TransferMember> {
    binding
        .parameters
        .iter()
        .map(|parameter| TransferMember {
            symbol: parameter.symbol.clone(),
            iec_type: parameter.iec_type.into(),
            unit: parameter.unit.clone(),
            minimum: parameter.minimum,
            maximum: parameter.maximum,
        })
        .collect()
}

/// The further modes of a group's transfer: the project's, then each member's mode that neither the loop's mode nor an earlier one
/// names, with the loop's Manual value. A valid group names the loop's mode for every member, so it adds none.
fn further_modes(
    loop_definition: &LoopDefinition,
    group: &PidControllerGroup,
    mode: &TransferModeBinding,
) -> Vec<TransferModeBinding> {
    let mut modes = additional_mode_members(loop_definition);
    for member in &group.members {
        let named = std::iter::once(mode.symbol.as_str())
            .chain(modes.iter().map(|known| known.symbol.as_str()))
            .any(|symbol| symbol.eq_ignore_ascii_case(&member.mode_symbol));
        if !named {
            modes.push(TransferModeBinding {
                symbol: member.mode_symbol.clone(),
                manual_value: mode.manual_value,
            });
        }
    }
    modes
}

/// Whether two roles of a binding share a symbol, without regard to case: the mode, the further modes, the states, the guards, the
/// output and the further outputs each hold their own symbol (E6.2).
fn roles_overlap(binding: &TransferBinding) -> bool {
    let mut seen = BTreeSet::new();
    let mut symbols = std::iter::once(binding.mode.symbol.as_str())
        .chain(
            binding
                .additional_modes
                .iter()
                .map(|mode| mode.symbol.as_str()),
        )
        .chain(
            binding
                .states
                .iter()
                .chain(&binding.guards)
                .chain(std::iter::once(&binding.output))
                .chain(&binding.additional_outputs)
                .map(|member| member.symbol.as_str()),
        );
    !symbols.all(|symbol| seen.insert(symbol.to_uppercase()))
}

#[cfg(test)]
#[path = "group_adapter_tests.rs"]
mod tests;
