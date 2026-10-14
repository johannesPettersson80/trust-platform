//! The declared initialization of a controller's states (specification 35, C0 "Controller state initialization" and C1 "Apply
//! and Restore"; annex E6.2 and E6.4): the transfer binding that writes the setpoint, the Manual command and the states, the
//! tracking record that carries them, the commissioning profile that holds both, and the plan that an isolated run starts
//! from. Ported from `controllerInitialization.ts` (`declaredTransferBinding`, `declaredCommissioningProfile`,
//! `declaredInitializationPlan`, and the `DeclaredStateTracking` record); the re-exports of that file are not ported.

use std::collections::{BTreeMap, BTreeSet};

use schemars::JsonSchema;
use serde::{Deserialize, Serialize};

use super::actuator_feedback::with_actuator_feedback;
use super::channel_mapping::declared_channels;
use super::entry_allowance::{noise_aware_transfer_tolerance, AllowanceTerms};
use super::initial_state::{
    initialization_values, operating_input, operating_output, transfer_member,
    ControllerInitialization,
};
use super::initialization_formulas::{initialization_formulas, InitializationFormula};
use super::initialization_plan::{
    bounds, bounds_on, event, start_run, start_timing, StartPlan, StartTiming,
};
use super::observation::commissioning_reference_targets;
use super::pid_measurement::initialized_states;
use super::profile::{CommissioningProfile, ProfileKind, RequiredTracking};
use super::texts;
use super::transfer::{
    rounded_transfer_value, TransferBinding, TransferMember, TransferModeBinding, TransferValue,
};
use super::transfer_tracking::{DeclaredStateKind, TransferTracking};
use crate::failure::{Cause, Failure, NextAction};
use crate::loop_definition::{
    ControllerLaw, InitializedState, LoopDefinition, Parameter, PostScan, StateType,
};
use crate::model::GeneralModel;
use crate::validation::engineer_verification_plan::{
    engineer_verification_plan, EngineerRequirements,
};
use crate::validation::run_validation::{RunCheck, RunRequirement};

/// A further reference of a declared transfer and its process value (annex E6.4 `additional_references`). A loop without
/// channels has none.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct AdditionalReference {
    /// The further setpoint.
    pub reference_symbol: String,
    /// Its process value.
    pub process_variable: String,
}

/// A further Manual command of a declared transfer and the output it holds (annex E6.4 `additional_commands`). A loop
/// without channels has none.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct AdditionalCommand {
    /// The further Manual command.
    pub manual_command_symbol: String,
    /// The output that it holds.
    pub output_symbol: String,
}

/// The tracking record of a declared transfer (annex E6.4 `declared-state-v1`): the setpoint and the Manual command, the
/// process value, whether the output is aligned at the entry, the operating-point values and the fresh sources of the states,
/// and the states the controller's scan may move after the entry.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct DeclaredStateTracking {
    /// The kind word: always `declared-state-v1`.
    pub kind: DeclaredStateKind,
    /// The setpoint, the reference that the transfer writes.
    pub reference_symbol: String,
    /// The Manual command that the transfer writes.
    pub manual_command_symbol: String,
    /// The process value that the transfer reads.
    pub process_variable: String,
    /// Whether the first output is aligned to the held output at the Auto entry (A6.3).
    pub align_output: bool,
    /// The fresh source of each state that has one (C0), when there is one.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub initialization_formulas: Option<BTreeMap<String, InitializationFormula>>,
    /// The operating-point value of each initialized state.
    pub initialization_values: BTreeMap<String, f64>,
    /// The states that the program may move within their bounds after the entry.
    pub bounded_state_symbols: Vec<String>,
    /// Further references, for a multichannel loop.
    pub additional_references: Vec<AdditionalReference>,
    /// The further measurements the transfer reads.
    pub additional_measurements: Vec<String>,
    /// Further Manual commands, for a multichannel loop.
    pub additional_commands: Vec<AdditionalCommand>,
}

/// The loop, model, declared states and parameters that a declared commissioning profile reads: the context that the
/// `TypeScript` closures captured. The formulas are those of the states it initializes.
#[derive(Debug, Clone, PartialEq)]
pub struct DeclaredStateProfile {
    loop_definition: LoopDefinition,
    model: GeneralModel,
    definition: ControllerInitialization,
    parameters: BTreeMap<String, f64>,
    formulas: BTreeMap<String, InitializationFormula>,
}

impl DeclaredStateProfile {
    /// The binding that the declared transfer writes (`declaredTransferBinding`).
    ///
    /// # Errors
    /// See [`declared_transfer_binding`].
    pub fn binding(&self) -> Result<TransferBinding, Failure> {
        declared_transfer_binding(&self.loop_definition, &self.definition)
    }

    /// The reference targets of the loop for a reviewed task (`commissioningReferenceTargets`, B6.2).
    ///
    /// # Errors
    /// See [`commissioning_reference_targets`].
    pub fn targets(&self, task: &EngineerRequirements) -> Result<BTreeMap<String, f64>, Failure> {
        commissioning_reference_targets(&self.loop_definition, &self.model, task)
    }

    /// The tracking record of the transfer (`tracking`): an Auto entry (`initialize`) aligns the output and carries the
    /// operating-point values; a later target change does not, and carries the midpoints of the states' ranges.
    ///
    /// # Errors
    /// The binding has no reference or Manual command, a state's operating-point value does not fit (see
    /// [`initialization_values`]), or the loop's channels do not fit (see [`declared_channels`]).
    pub fn tracking(
        &self,
        binding: &TransferBinding,
        initialize: bool,
    ) -> Result<TransferTracking, Failure> {
        let (Some(reference), Some(manual)) = (binding.states.first(), binding.states.get(1))
        else {
            return Err(references_required());
        };
        let owned = initialized_states(&self.loop_definition, &self.definition);
        let values: BTreeMap<String, f64> = if initialize {
            let all = initialization_values(
                &self.loop_definition,
                &self.model,
                &self.definition,
                &self.parameters,
            )?;
            owned
                .iter()
                .filter_map(|state| {
                    all.get(&state.symbol)
                        .map(|value| (state.symbol.clone(), *value))
                })
                .collect()
        } else {
            owned
                .iter()
                .map(|state| {
                    let middle = if state.iec_type == StateType::Bool {
                        0.0
                    } else {
                        state.minimum / 2.0 + state.maximum / 2.0
                    };
                    rounded_transfer_value(&transfer_member(state), middle)
                        .map(|value| (state.symbol.clone(), value))
                })
                .collect::<Result<BTreeMap<String, f64>, Failure>>()?
        };
        let mut bounded: Vec<String> = owned
            .iter()
            .filter(|state| state.post_scan == PostScan::Bounded)
            .map(|state| state.symbol.clone())
            .collect();
        if self.loop_definition.controller.manual_output_follows_output == Some(true) {
            bounded.push(manual.symbol.clone());
        }
        let channels = declared_channels(&self.loop_definition)?;
        Ok(TransferTracking::DeclaredState(DeclaredStateTracking {
            kind: DeclaredStateKind::DeclaredStateV1,
            reference_symbol: reference.symbol.clone(),
            manual_command_symbol: manual.symbol.clone(),
            process_variable: self.loop_definition.signals.process_variable.symbol.clone(),
            align_output: initialize,
            initialization_formulas: (!self.formulas.is_empty()).then(|| self.formulas.clone()),
            initialization_values: values,
            bounded_state_symbols: bounded,
            additional_references: Vec::new(),
            additional_measurements: channels
                .measurements()
                .into_iter()
                .map(|member| member.symbol)
                .collect(),
            additional_commands: Vec::new(),
        }))
    }
}

/// The transfer binding of a declared initialization (`declaredTransferBinding`, C0): the setpoint and the Manual command are
/// states, followed by the initialized states; the parameters and the process value are guards; the controller output is held
/// within its limits, with its measured actuator as its feedback (A6.3, E6.2).
///
/// # Errors
/// The loop does not declare its parameter record, setpoint, Manual command and type, or mode; its channels do not fit (see
/// [`declared_channels`]); two roles share a symbol, without regard to case; or the actuator feedback does not fit (see
/// `with_actuator_feedback`).
pub fn declared_transfer_binding(
    loop_definition: &LoopDefinition,
    definition: &ControllerInitialization,
) -> Result<TransferBinding, Failure> {
    let Some(parameters) = loop_definition.parameter_binding.as_ref() else {
        return Err(references_required());
    };
    if loop_definition.signals.setpoint.is_none()
        || loop_definition.controller.manual_output_signal.is_none()
        || loop_definition.controller.manual_output_type.is_none()
    {
        return Err(references_required());
    }
    let channels = declared_channels(loop_definition)?;
    let states: Vec<TransferMember> = [channels.reference.clone(), channels.command.clone()]
        .into_iter()
        .chain(
            initialized_states(loop_definition, definition)
                .into_iter()
                .map(transfer_member),
        )
        .collect();
    let guards: Vec<TransferMember> = parameters
        .parameters
        .iter()
        .map(parameter_member)
        .chain(channels.measurements())
        .collect();
    let mode = TransferModeBinding {
        symbol: parameters.mode.symbol.clone(),
        manual_value: parameters.mode.manual_value,
    };
    if !roles_distinct(&mode, &states, &guards, &channels.output) {
        return Err(Failure::new(
            &texts::declared_roles_overlap(),
            Cause::Implementation,
            NextAction::OpenLoopSetup,
        ));
    }
    let binding = TransferBinding {
        loop_id: parameters.loop_id.clone(),
        task_name: parameters.task_name.clone(),
        mode,
        additional_modes: Vec::new(),
        tracking_outputs: BTreeMap::new(),
        states,
        guards,
        output: channels.output,
        additional_outputs: Vec::new(),
    };
    with_actuator_feedback(loop_definition, binding)
}

/// The commissioning profile of a declared initialization (`declaredCommissioningProfile`, C0 and C1): its label, its reference,
/// the formulas of the states that it initializes, and the tracking those formulas require.
///
/// # Errors
/// The loop's channels do not fit (see [`declared_channels`]), or a state's formula does not fit the model and the loop (see
/// [`initialization_formulas`]).
pub fn declared_commissioning_profile(
    loop_definition: &LoopDefinition,
    model: &GeneralModel,
    definition: &ControllerInitialization,
    label: &str,
    parameters: &BTreeMap<String, f64>,
) -> Result<CommissioningProfile, Failure> {
    let channels = declared_channels(loop_definition)?;
    // A state that is also a measurement, such as a PID's own measurement, is read at the boundary and never initialized.
    let owned: BTreeSet<String> = initialized_states(loop_definition, definition)
        .into_iter()
        .map(|state| state.symbol.clone())
        .collect();
    let formulas: BTreeMap<String, InitializationFormula> =
        initialization_formulas(loop_definition, model, definition)?
            .into_iter()
            .filter(|(symbol, _)| owned.contains(symbol))
            .collect();
    let required_tracking = if formulas.is_empty() {
        Vec::new()
    } else {
        vec![RequiredTracking::DeclaredInitializationFormulas]
    };
    Ok(CommissioningProfile {
        label: label.to_owned(),
        references: vec![channels.reference.symbol],
        required_tracking,
        kind: ProfileKind::DeclaredState(DeclaredStateProfile {
            loop_definition: loop_definition.clone(),
            model: model.clone(),
            definition: definition.clone(),
            parameters: parameters.clone(),
            formulas,
        }),
    })
}

/// The values that a declared state's reported values become, rounded as their members are typed (`states`, declared): a BOOL
/// reads as 0 or 1, and a state with no reported value is refused.
///
/// # Errors
/// A state of the binding has no reported value, or a value does not fit its member.
pub fn declared_states(
    binding: &TransferBinding,
    observed: &BTreeMap<String, TransferValue>,
) -> Result<BTreeMap<String, f64>, Failure> {
    binding
        .states
        .iter()
        .map(|member| {
            let number = match observed.get(&member.symbol) {
                Some(TransferValue::Number(value)) => *value,
                Some(TransferValue::Boolean(flag)) => f64::from(u8::from(*flag)),
                None => {
                    return Err(Failure::new(
                        &texts::declared_state_unavailable(&member.symbol),
                        Cause::Runtime,
                        NextAction::None,
                    ))
                }
            };
            Ok((
                member.symbol.clone(),
                rounded_transfer_value(member, number)?,
            ))
        })
        .collect()
}

/// The start plan of a declared initialization (`declaredInitializationPlan`, C0 and C1): the run starts in Manual at the
/// operating point, with the setpoint and the Manual command held and the initialized states at their values, and it enters
/// Auto at the transfer. The allowance is that of A6.4 with the law's three gains, as the `TypeScript` computes it.
///
/// # Errors
/// The binding does not fit (see [`declared_transfer_binding`]); the run or the allowance is refused (see
/// `engineer_verification_plan`); a state has no planned value; or a held value does not fit its member.
pub fn declared_initialization_plan(
    loop_definition: &LoopDefinition,
    model: &GeneralModel,
    task: &EngineerRequirements,
    values: &BTreeMap<String, f64>,
    definition: &ControllerInitialization,
    process_noise: Option<f64>,
) -> Result<StartPlan, Failure> {
    let binding = declared_transfer_binding(loop_definition, definition)?;
    let controller_initial = initialization_values(loop_definition, model, definition, values)?;
    let base =
        engineer_verification_plan(loop_definition, model, task, values, &controller_initial)?;
    let owned = initialized_states(loop_definition, definition);
    let initialized: Vec<(String, f64)> = owned
        .iter()
        .map(|state| {
            base.initial_values
                .get(&state.symbol)
                .copied()
                .map(|value| (state.symbol.clone(), value))
                .ok_or_else(|| {
                    Failure::new(
                        &texts::declared_state_unavailable(&state.symbol),
                        Cause::Runtime,
                        NextAction::None,
                    )
                })
        })
        .collect::<Result<Vec<(String, f64)>, Failure>>()?;
    let timing = start_timing(base.step_ms);
    let (proportional, terms) = allowance_gains(loop_definition, values);
    let tolerance = noise_aware_transfer_tolerance(
        loop_definition,
        task.transfer_tolerance,
        proportional,
        process_noise,
        terms,
    )?;
    let reference = operating_output(model, 0);
    let applied = base
        .initial_values
        .get(&binding.output.symbol)
        .copied()
        .unwrap_or_else(|| operating_input(model, 0));
    let mut starting = vec![
        (
            binding.states[0].symbol.clone(),
            rounded_transfer_value(&binding.states[0], reference)?,
        ),
        (
            binding.states[1].symbol.clone(),
            rounded_transfer_value(&binding.states[1], applied)?,
        ),
    ];
    starting.extend(initialized.iter().cloned());
    let events: Vec<_> = starting
        .iter()
        .map(|(symbol, value)| event(timing.transfer_at_ms, symbol, *value))
        .collect();
    let observed: Vec<String> = binding
        .states
        .iter()
        .map(|member| member.symbol.clone())
        .chain(std::iter::once(binding.mode.symbol.clone()))
        .collect();
    let mode = (binding.mode.symbol.as_str(), binding.mode.manual_value);
    let (run, base_requirements) = start_run(&timing, base, mode, starting, observed, events);
    let requirements = declared_requirements(
        &binding,
        &timing,
        (applied, tolerance),
        &initialized,
        &owned,
        base_requirements,
    );
    Ok(StartPlan {
        reference,
        tolerance,
        run,
        requirements,
    })
}

/// The requirements of a declared start: the output's continuity over the whole run; the Auto mode after the transfer; the
/// initialized states after the transfer, held where they do not move and bounded where they may; and the base plan's bounds
/// over the whole run.
fn declared_requirements(
    binding: &TransferBinding,
    timing: &StartTiming,
    (applied, tolerance): (f64, f64),
    initialized: &[(String, f64)],
    owned: &[&InitializedState],
    base: Vec<RunRequirement>,
) -> Vec<RunRequirement> {
    let auto = f64::from(u8::from(!binding.mode.manual_value));
    let mut requirements = vec![
        bounds(
            "transfer-continuity",
            &binding.output.symbol,
            timing.window,
            applied - tolerance,
            applied + tolerance,
        ),
        bounds(
            "auto-mode",
            &binding.mode.symbol,
            timing.transfer_window,
            auto,
            auto,
        ),
    ];
    for (state, (symbol, value)) in owned.iter().zip(initialized) {
        let (minimum, maximum) = match state.post_scan {
            PostScan::Unchanged => (*value, *value),
            PostScan::Bounded => (state.minimum, state.maximum),
        };
        requirements.push(RunRequirement {
            id: format!("initialized-state:{symbol}"),
            channel: symbol.clone(),
            window: timing.transfer_window,
            check: RunCheck::StateBounds { minimum, maximum },
        });
    }
    requirements.extend(bounds_on(base, timing.window));
    requirements
}

/// The proportional gain of the law, and the allowance terms of its integral and derivative gains (`gains` of the `TypeScript`):
/// a PID law has all three; any other law has none.
fn allowance_gains(
    loop_definition: &LoopDefinition,
    values: &BTreeMap<String, f64>,
) -> (Option<f64>, AllowanceTerms) {
    match loop_definition.controller.law.as_ref() {
        Some(ControllerLaw::Pid(law)) => (
            values.get(&law.proportional_gain_symbol).copied(),
            AllowanceTerms {
                integral_gain: values.get(&law.integral_gain_symbol).copied(),
                derivative_gain: values.get(&law.derivative_gain_symbol).copied(),
                cycle_seconds: Some(loop_definition.controller.task_period_ms / 1000.0),
                entry_error: None,
            },
        ),
        _ => (None, AllowanceTerms::default()),
    }
}

/// A declared parameter as a guard of the transfer (`guards`, E6.2).
fn parameter_member(parameter: &Parameter) -> TransferMember {
    TransferMember {
        symbol: parameter.symbol.clone(),
        iec_type: parameter.iec_type.into(),
        unit: parameter.unit.clone(),
        minimum: parameter.minimum,
        maximum: parameter.maximum,
    }
}

/// Whether the roles of a binding have distinct symbols, without regard to case (`symbols`, `toUpperCase`).
fn roles_distinct(
    mode: &TransferModeBinding,
    states: &[TransferMember],
    guards: &[TransferMember],
    output: &TransferMember,
) -> bool {
    let symbols = std::iter::once(mode.symbol.as_str()).chain(
        states
            .iter()
            .chain(guards.iter())
            .chain(std::iter::once(output))
            .map(|member| member.symbol.as_str()),
    );
    let mut seen = BTreeSet::new();
    symbols
        .into_iter()
        .all(|symbol| seen.insert(symbol.to_uppercase()))
}

/// The refusal of a declared initialization whose reference, Manual command or mode is not declared.
fn references_required() -> Failure {
    Failure::new(
        &texts::declared_references_required(),
        Cause::Implementation,
        NextAction::OpenLoopSetup,
    )
}

#[cfg(test)]
#[path = "declared_initialization_tests.rs"]
mod tests;
