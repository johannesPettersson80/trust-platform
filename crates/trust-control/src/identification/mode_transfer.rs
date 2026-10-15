//! The transfers by which Identify moves a loop to the mode its method needs and back to the mode it found it in (the rules of
//! `ConnectedIdentificationMode` in `connectedIdentificationMode.ts`; specification 35, A6.2 to A6.4 and B3.4 step 1; runtime
//! protocol annex E6.2, E6.4 and E6.9): the binding each transfer writes, the states and the tracking it carries, the output
//! change it allows, the references and commands written back before the loop returns, the reference step after it returned to
//! Auto, and the checks that the loop and the program are still the ones the mode was changed on. The runtime calls, the owner and
//! the journal are the engine's (`workflow/identify_mode.rs`); each rule here takes the runtime's answers as data that has already
//! been read.
//!
//! The controller's own states track Manual in its Structured Text, so a mode transfer carries them as they are: the tracking is
//! `declared-state-v1` without alignment, every state but the references and commands is bounded, and only the guarded mode and
//! command writes grant authority (A6.3, A6.4).

use std::collections::{BTreeMap, BTreeSet};

use super::texts;
use crate::commissioning::actuator_feedback::transfer_target_allowance;
use crate::commissioning::adapters::controller_adapter;
use crate::commissioning::channel_mapping::declared_channels;
use crate::commissioning::checkpoint::IdentificationModeCheckpoint;
use crate::commissioning::commissioning_result::TransferStatus;
use crate::commissioning::declared_initialization::{declared_states, DeclaredStateTracking};
use crate::commissioning::entry_allowance::{noise_aware_transfer_tolerance, AllowanceTerms};
use crate::commissioning::found_mode::{found_mode_of, FoundMode};
use crate::commissioning::project_law::gain_symbols;
use crate::commissioning::proportional_commissioning::binding_failure;
use crate::commissioning::transfer::{TransferBinding, TransferValue};
use crate::commissioning::transfer_tracking::{DeclaredStateKind, TransferTracking};
use crate::failure::{Cause, Failure, NextAction};
use crate::loop_definition::LoopDefinition;

/// The suffix of the transfer loop of Identify's mode change (`:identification-mode`), so that its transaction record is apart
/// from Apply's (annex E12.2: one record per transfer loop).
const MODE_LOOP_SUFFIX: &str = ":identification-mode";

/// The suffix of the transfer loop that writes the references and commands back (`:commands`).
const COMMANDS_LOOP_SUFFIX: &str = ":commands";

/// The smallest output change a mode transfer allows (`1e-6`).
const MINIMUM_OUTPUT_CHANGE: f64 = 1e-6;

/// The binding of Identify's mode transfer (`binding` of `ConnectedIdentificationMode`): the transfer binding of the loop's
/// controller adapter (A6.3, C0), on a transfer loop of its own, the adapter's loop identifier followed by
/// `:identification-mode`.
///
/// # Errors
/// The loop's controller has no adapter, or the adapter's binding does not fit the loop (see `ControllerAdapter::transfer_binding`).
pub fn mode_binding(loop_definition: &LoopDefinition) -> Result<TransferBinding, Failure> {
    let binding = controller_adapter(loop_definition)?.transfer_binding()?;
    Ok(TransferBinding {
        loop_id: format!("{}{MODE_LOOP_SUFFIX}", binding.loop_id),
        ..binding
    })
}

/// Whether `status` reports the loop in the mode a method needs (`enter`): Manual when `manual`, Auto otherwise, read from the
/// binding's mode against its Manual value.
#[must_use]
pub fn in_mode(binding: &TransferBinding, status: &TransferStatus, manual: bool) -> bool {
    let wanted = if manual {
        binding.mode.manual_value
    } else {
        !binding.mode.manual_value
    };
    status.values.get(&binding.mode.symbol) == Some(&TransferValue::Boolean(wanted))
}

/// Whether the loop was found in Auto (`toAuto` of `restore`): the found mode of the mode value that `original` reports
/// (`found_mode_of`, A6.2). A mode that `original` does not report is unknown, and a loop of unknown mode stays in Manual.
#[must_use]
pub fn found_in_auto(binding: &TransferBinding, original: &TransferStatus) -> bool {
    let mode = original
        .values
        .get(&binding.mode.symbol)
        .and_then(|value| match value {
            TransferValue::Boolean(flag) => Some(*flag),
            TransferValue::Number(_) => None,
        });
    found_mode_of(mode, binding.mode.manual_value) == Some(FoundMode::Auto)
}

/// The states a mode transfer writes (`stateValues`): every state of the binding at the value `status` reports, as the runtime
/// requires the exact state set (annex E6.5). A BOOL reads as 0 or 1, and each value is rounded as its member is typed.
///
/// # Errors
/// A state has no reported value, or its value does not fit its member (see `declared_states`).
pub fn mode_states(
    binding: &TransferBinding,
    status: &TransferStatus,
) -> Result<BTreeMap<String, f64>, Failure> {
    declared_states(binding, &status.values)
}

/// The tracking of a mode transfer (`tracking` of `ConnectedIdentificationMode`, annex E6.4 `declared-state-v1`): the reference
/// and the Manual command of the loop's channels, its process value, no alignment of the output, and every other state of the
/// binding initialized at the value `status` reports and bounded, since the program moves it in every scan. A Manual command
/// that the program copies from the output (§17.1) is bounded as well, before a transfer to Manual and after one to Auto.
///
/// # Errors
/// The loop's channels do not fit (see `declared_channels`), or a state's value does not fit its member (see [`mode_states`]).
pub fn mode_tracking(
    loop_definition: &LoopDefinition,
    binding: &TransferBinding,
    status: &TransferStatus,
) -> Result<TransferTracking, Failure> {
    let channels = declared_channels(loop_definition)?;
    let controls = [
        channels.reference.symbol.as_str(),
        channels.command.symbol.as_str(),
    ];
    let observed_states = mode_states(binding, status)?;
    let dynamic: Vec<String> = binding
        .states
        .iter()
        .map(|member| member.symbol.clone())
        .filter(|symbol| !controls.contains(&symbol.as_str()))
        .collect();
    let initialization_values = dynamic
        .iter()
        .filter_map(|symbol| {
            observed_states
                .get(symbol)
                .map(|value| (symbol.clone(), *value))
        })
        .collect();
    let mut bounded_state_symbols = dynamic;
    if loop_definition.controller.manual_output_follows_output == Some(true) {
        bounded_state_symbols.push(channels.command.symbol.clone());
    }
    Ok(TransferTracking::DeclaredState(DeclaredStateTracking {
        kind: DeclaredStateKind::DeclaredStateV1,
        reference_symbol: channels.reference.symbol.clone(),
        manual_command_symbol: channels.command.symbol.clone(),
        process_variable: loop_definition.signals.process_variable.symbol.clone(),
        align_output: false,
        initialization_formulas: None,
        initialization_values,
        bounded_state_symbols,
        additional_references: Vec::new(),
        additional_measurements: channels
            .measurements()
            .into_iter()
            .map(|member| member.symbol)
            .collect(),
        additional_commands: Vec::new(),
    }))
}

/// The output change a mode transfer allows at its first execution (`maximum_output_change`, annex E6.9; `transfer` of
/// `ConnectedIdentificationMode`): the larger of `1e-6`, the loop's faster rate limit over one execution of its task, and, for an
/// Auto entry reviewed on `observed`, the controller's own response in that execution ([`auto_entry_response`]). A transfer to
/// Manual, or an Auto entry without a reviewed status, allows no response.
#[must_use]
pub fn mode_output_change(
    loop_definition: &LoopDefinition,
    manual: bool,
    observed: Option<&TransferStatus>,
    process_noise: Option<f64>,
) -> f64 {
    let limits = &loop_definition.limits;
    let rate = limits
        .rate_up_per_s
        .unwrap_or(0.0)
        .max(limits.rate_down_per_s.unwrap_or(0.0));
    let rated = rate * loop_definition.controller.task_period_ms / 1000.0;
    let response = observed.filter(|_| !manual).map_or(0.0, |observed| {
        auto_entry_response(loop_definition, observed, process_noise)
    });
    MINIMUM_OUTPUT_CHANGE.max(rated).max(response)
}

/// The response of a PID in its first execution after an Auto entry (`autoEntryResponse`, A6.4): the allowance of
/// `noise_aware_transfer_tolerance` with the gains `observed` reports and the measured output noise `process_noise`, that is the
/// larger of the exact check and `2·|Kp|·n + |Ki|·Δt·n + 4·|Kd|·n/Δt`, plus the integral step on the error the controller enters
/// with, `|Ki|·Δt·|r − y|`, which identification moved the measurement away from the setpoint to make. Δt is the loop's task
/// period, r its setpoint and y its process value as `observed` reports them. The gains are the law's, or a program's seed
/// variables (C2). A loop without an integral or derivative term (no law, or a proportional law), or a response that is not a
/// finite number (a gain or a value that `observed` does not report), allows no response: zero.
#[must_use]
pub fn auto_entry_response(
    loop_definition: &LoopDefinition,
    observed: &TransferStatus,
    process_noise: Option<f64>,
) -> f64 {
    let Some(symbols) = gain_symbols(loop_definition)
        .filter(|symbols| symbols.integral.is_some() || symbols.derivative.is_some())
    else {
        return 0.0;
    };
    let Some(setpoint) = loop_definition.signals.setpoint.as_ref() else {
        return 0.0;
    };
    let value = |symbol: &str| observed_number(observed, symbol);
    let cycle_seconds = loop_definition.controller.task_period_ms / 1000.0;
    let measured = value(&loop_definition.signals.process_variable.symbol);
    let error = (value(&setpoint.symbol) - measured).abs();
    // An absent integral term contributes nothing; a reported-missing gain stays NaN and so allows no response.
    let integral_gain = symbols.integral.map_or(0.0, value);
    let allowance = noise_aware_transfer_tolerance(
        loop_definition,
        None,
        Some(value(symbols.proportional)),
        process_noise,
        AllowanceTerms {
            integral_gain: symbols.integral.map(value),
            derivative_gain: symbols.derivative.map(value),
            cycle_seconds: Some(cycle_seconds),
            entry_error: None,
        },
    )
    .unwrap_or(f64::NAN);
    let response = allowance + integral_gain.abs() * cycle_seconds * error;
    if response.is_finite() {
        response
    } else {
        0.0
    }
}

/// The number `observed` reports for `symbol` (`Number(observed.values[symbol])`): a BOOL is 0 or 1, and a value it does not
/// report is not a number.
fn observed_number(observed: &TransferStatus, symbol: &str) -> f64 {
    match observed.values.get(symbol) {
        Some(TransferValue::Number(value)) => *value,
        Some(TransferValue::Boolean(flag)) => f64::from(u8::from(*flag)),
        None => f64::NAN,
    }
}

/// The transfer that writes the loop's references and commands back to their values when Identify found the loop, in Manual,
/// before the loop returns to Auto (`commandBinding` of `restore`, A6.2).
#[derive(Debug, Clone, PartialEq)]
pub struct CommandReturn {
    /// The binding: the mode binding on its own transfer loop (`:commands`), with the references and commands as its states.
    pub binding: TransferBinding,
    /// The value of each of those states when Identify found the loop.
    pub values: BTreeMap<String, f64>,
    /// The Manual command among those states, when it is written back.
    pub command: Option<String>,
}

impl CommandReturn {
    /// The allowed output change of the write-back (A6.2, annex E6.9): the larger of the mode change's own allowance `base`
    /// and the change the restored Manual command makes, from the output `output_now` that the runtime holds before the
    /// write. Returning the plant input to where Identify found it is the restoration itself, so it is the reviewed tolerance
    /// of that transfer; a write-back without the Manual command, or an output that is not read, allows `base`.
    #[must_use]
    pub fn allowance(&self, base: f64, output_now: Option<f64>) -> f64 {
        let moved = self
            .command
            .as_ref()
            .and_then(|symbol| self.values.get(symbol))
            .zip(output_now)
            .map_or(0.0, |(back, now)| (back - now).abs());
        base.max(moved)
    }
}

/// The references and commands a mode return writes back in Manual (`restore` of `ConnectedIdentificationMode`, A6.2): the
/// loop's reference, and its Manual command unless the loop returns to Auto and the program copies the output into the Manual
/// command there (§17.1), since that copy only holds the output of the moment the mode was changed, and writing it back would
/// move the output. Their binding is the mode binding on the transfer loop `:commands`, with these states and, as guards, the
/// parameters of the loop's record and the feedback guard of each tracked output, so that the binding names every guard its
/// tracked outputs need (annex E6.2). The values are those `original` reports, rounded as their members are typed.
///
/// The `TypeScript` keeps only the parameter guards, so a loop with a measured actuator would name a feedback guard the binding
/// does not hold, which the runtime refuses (E6.2); the feedback guard is kept here.
///
/// # Errors
/// The loop's channels do not fit (see `declared_channels`), or an original value does not fit its member.
pub fn command_return(
    loop_definition: &LoopDefinition,
    binding: &TransferBinding,
    original: &TransferStatus,
) -> Result<CommandReturn, Failure> {
    let channels = declared_channels(loop_definition)?;
    let follows = loop_definition.controller.manual_output_follows_output == Some(true);
    let mut states = vec![channels.reference.clone()];
    let writes_command = !(found_in_auto(binding, original) && follows);
    if writes_command {
        states.push(channels.command.clone());
    }
    let parameters: BTreeSet<&str> = loop_definition
        .parameter_binding
        .iter()
        .flat_map(|record| record.parameters.iter())
        .map(|declared| declared.symbol.as_str())
        .collect();
    let feedback: BTreeSet<&str> = binding
        .tracking_outputs
        .values()
        .map(String::as_str)
        .collect();
    let guards = binding
        .guards
        .iter()
        .filter(|guard| {
            parameters.contains(guard.symbol.as_str()) || feedback.contains(guard.symbol.as_str())
        })
        .cloned()
        .collect();
    let commands = TransferBinding {
        loop_id: format!("{}{COMMANDS_LOOP_SUFFIX}", binding.loop_id),
        states,
        guards,
        ..binding.clone()
    };
    let values = declared_states(&commands, &original.values)?;
    Ok(CommandReturn {
        binding: commands,
        values,
        command: writes_command.then(|| channels.command.symbol.clone()),
    })
}

/// The step that returns the loop's reference after a loop found in Auto has entered Auto again (specification 35, C1: the
/// references return to their values from before identification).
#[derive(Debug, Clone, PartialEq)]
pub struct ReferenceReturn {
    /// The states the transfer writes: those the loop holds, with the reference at its value when Identify found the loop.
    pub values: BTreeMap<String, f64>,
    /// The tracking of the mode transfer at the states the loop holds ([`mode_tracking`]).
    pub tracking: TransferTracking,
    /// The allowed change of the first output: a target change across the outputs, as Apply's target step allows (C13.6).
    pub tolerance: f64,
}

/// The reference step of a mode return, after the loop entered Auto again (C1, A6.2): a program that sets its reference from the
/// measurement in Manual overwrites the reference written back there at its next scan, so the loop enters Auto at the measurement.
/// A reference that `current` no longer holds at its value in `original` then returns in Auto, as a target change: every state at
/// the value `current` reports and the reference at its found value, rounded as its member is typed. `None` when the loop holds its
/// reference.
///
/// # Errors
/// The loop's channels do not fit (see `declared_channels`), a value does not fit its member, or the binding has no output basis.
pub fn reference_return(
    loop_definition: &LoopDefinition,
    binding: &TransferBinding,
    original: &TransferStatus,
    current: &TransferStatus,
) -> Result<Option<ReferenceReturn>, Failure> {
    let channels = declared_channels(loop_definition)?;
    let reference = TransferBinding {
        states: vec![channels.reference.clone()],
        ..binding.clone()
    };
    let found = declared_states(&reference, &original.values)?;
    let mut values = mode_states(binding, current)?;
    let symbol = &channels.reference.symbol;
    let Some(back) = found.get(symbol).copied() else {
        return Ok(None);
    };
    if values.get(symbol).copied() == Some(back) {
        return Ok(None);
    }
    values.insert(symbol.clone(), back);
    Ok(Some(ReferenceReturn {
        values,
        tracking: mode_tracking(loop_definition, binding, current)?,
        tolerance: transfer_target_allowance(binding).ok_or_else(binding_failure)?,
    }))
}

/// Checks that the loop is still the one Identify changed the mode of (`restore`, A6.2, A8.2): the checkpoint's loop revision is
/// the loop's.
///
/// # Errors
/// The loop's revision changed since the mode was changed: restore the original loop bindings first.
pub fn check_mode_loop(
    loop_definition: &LoopDefinition,
    checkpoint: &IdentificationModeCheckpoint,
) -> Result<(), Failure> {
    if checkpoint.loop_revision == loop_definition.revision {
        return Ok(());
    }
    Err(Failure::new(
        &texts::mode_loop_changed(&checkpoint.loop_revision, &loop_definition.revision),
        Cause::Settings,
        NextAction::OpenLoopSetup,
    ))
}

/// Checks that the runtime still runs the program Identify changed the mode on (`restore`, A6.7, E11.5): `current` reports the
/// program revision of the checkpoint's original status.
///
/// # Errors
/// The program changed: the mode change is unresolved, and nothing is written to the new program.
pub fn check_mode_program(
    checkpoint: &IdentificationModeCheckpoint,
    current: &TransferStatus,
) -> Result<(), Failure> {
    if current.program_revision == checkpoint.original.program_revision {
        return Ok(());
    }
    Err(Failure::new(
        &texts::mode_program_changed(
            &checkpoint.original.program_revision,
            &current.program_revision,
        ),
        Cause::Runtime,
        NextAction::None,
    ))
}

#[cfg(test)]
#[path = "mode_transfer_tests.rs"]
mod tests;
