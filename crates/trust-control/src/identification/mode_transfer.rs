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
//!
//! A PID group (C7) changes mode as one transaction, through its own binding: that binding holds the setpoint and the Manual command
//! of every member as states, and the output of every member. Its tracking therefore names every other member's setpoint with its
//! process value and Manual command with the output it holds, which the runtime requires of a group (annex E6.4, `additional_references`,
//! `additional_commands` and `additional_measurements`); the return writes back every member's setpoint and Manual command, with
//! the change each makes to its own output as its allowance, and gives every setpoint its reference step.

use std::collections::{BTreeMap, BTreeSet};

use super::texts;
use crate::commissioning::actuator_feedback::{transfer_output_basis, transfer_target_allowance};
use crate::commissioning::adapters::controller_adapter;
use crate::commissioning::channel_mapping::{declared_channels, DeclaredChannels};
use crate::commissioning::checkpoint::IdentificationModeCheckpoint;
use crate::commissioning::commissioning_result::TransferStatus;
use crate::commissioning::declared_initialization::{
    declared_states, further_channels, DeclaredStateTracking, FurtherChannels,
};
use crate::commissioning::entry_allowance::{noise_aware_transfer_tolerance, AllowanceTerms};
use crate::commissioning::found_mode::{found_mode_of, FoundMode};
use crate::commissioning::project_law::gain_symbols;
use crate::commissioning::proportional_commissioning::binding_failure;
use crate::commissioning::transfer::{TransferBinding, TransferMember, TransferValue};
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

/// The headroom of the change an additional output is allowed, as a factor: the runtime scales the tolerance of an additional output
/// by the ratio of the output spans in floating point (annex E6.9), which can leave it a few units in the last place below the exact
/// change that the write-back must be allowed, and the runtime refuses an output change above its tolerance.
const ADDITIONAL_OUTPUT_HEADROOM: f64 = 1.0 + 4.0 * f64::EPSILON;

/// The binding of Identify's mode transfer (`binding` of `ConnectedIdentificationMode`): the transfer binding of the loop's
/// controller adapter (A6.3, C0), on a transfer loop of its own, the adapter's loop identifier followed by
/// `:identification-mode`. A PID group's adapter binds every member (C7).
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
/// A PID group's other members are further channels of the record (C7), as in the commissioning profile's tracking
/// (`further_channels`): each other member's setpoint with its process value, its Manual command with the output it holds, and
/// every member's process value. Those setpoints and Manual commands are states of the group's binding but not initialized ones,
/// since the runtime accepts the record only when the initialized states, the two leading states and the further setpoints and
/// commands are every state of the binding once, and only a loop's own Manual command may be bounded (`validate_declared_states`).
///
/// # Errors
/// The loop's channels do not fit (see `declared_channels` and `further_channels`), or a state's value does not fit its member
/// (see [`mode_states`]).
pub fn mode_tracking(
    loop_definition: &LoopDefinition,
    binding: &TransferBinding,
    status: &TransferStatus,
) -> Result<TransferTracking, Failure> {
    let channels = declared_channels(loop_definition)?;
    let further = further_channels(loop_definition)?;
    let observed_states = mode_states(binding, status)?;
    let dynamic: Vec<String> = binding
        .states
        .iter()
        .map(|member| member.symbol.as_str())
        .filter(|symbol| !is_control(&channels, &further, symbol))
        .map(str::to_owned)
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
        additional_references: further.references,
        additional_measurements: further.measurements,
        additional_commands: further.commands,
    }))
}

/// Whether `symbol` is a setpoint or a Manual command of the loop's transfer: the loop's own, or one of a PID group's other members
/// (C7). The transfer writes these as references and commands; every other state of its binding is the controller's own.
fn is_control(channels: &DeclaredChannels, further: &FurtherChannels, symbol: &str) -> bool {
    symbol == channels.reference.symbol
        || symbol == channels.command.symbol
        || further
            .references
            .iter()
            .any(|channel| channel.reference_symbol == symbol)
        || further
            .commands
            .iter()
            .any(|channel| channel.manual_command_symbol == symbol)
}

/// The state of `binding` named `symbol`: a PID group's further setpoint or Manual command (C7), which the binding holds beside the
/// loop's own.
///
/// # Errors
/// The binding has no such state, so it is not the binding of the loop (see `binding_failure`).
fn state_of(binding: &TransferBinding, symbol: &str) -> Result<TransferMember, Failure> {
    binding
        .states
        .iter()
        .find(|member| member.symbol == symbol)
        .cloned()
        .ok_or_else(binding_failure)
}

/// The output change a mode transfer allows at its first execution (`maximum_output_change`, annex E6.9; `transfer` of
/// `ConnectedIdentificationMode`): the larger of `1e-6`, the loop's faster rate limit over one execution of its task, and, for an
/// Auto entry reviewed on `observed`, the controller's own response in that execution ([`auto_entry_response`]). A transfer to
/// Manual, or an Auto entry without a reviewed status, allows no response. The integral step on the error the controller enters
/// with is the runtime's to add ([`mode_entry_error_gain`]).
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

/// The response of a PID in its first execution after an Auto entry to the measured noise (`autoEntryResponse`, A6.4): the
/// allowance of `noise_aware_transfer_tolerance` with the gains `observed` reports and the measured output noise `process_noise`,
/// that is the larger of the exact check and `2·|Kp|·n + |Ki|·Δt·n + 4·|Kd|·n/Δt`, Δt being the loop's task period. The gains are
/// the law's, or a program's seed variables (C2). A loop without an integral or derivative term (no law, or a proportional law),
/// without a setpoint, or with a response that is not a finite number (a gain that `observed` does not report), allows no
/// response: zero.
#[must_use]
pub fn auto_entry_response(
    loop_definition: &LoopDefinition,
    observed: &TransferStatus,
    process_noise: Option<f64>,
) -> f64 {
    entry_response_terms(loop_definition, observed, process_noise)
        .map_or(0.0, |(allowance, _)| allowance)
}

/// The factor of the error at entry in a PID's first execution after an Auto entry, `|Ki|·Δt` (A6.4), which the runtime
/// multiplies by the error it reads in the scan of the entry (`entry_error_gain`, annex E6.9): identification moved the
/// measurement away from the setpoint, and the plant still moves while the transfer waits for its boundary. None for a transfer
/// to Manual, an Auto entry without a reviewed status, and a loop whose response has no integral step.
#[must_use]
pub fn mode_entry_error_gain(
    loop_definition: &LoopDefinition,
    manual: bool,
    observed: Option<&TransferStatus>,
    process_noise: Option<f64>,
) -> Option<f64> {
    let observed = observed.filter(|_| !manual)?;
    let (_, integral_gain) = entry_response_terms(loop_definition, observed, process_noise)?;
    let gain = integral_gain.abs() * loop_definition.controller.task_period_ms / 1000.0;
    (gain > 0.0 && gain.is_finite()).then_some(gain)
}

/// The noise response of [`auto_entry_response`] and the integral gain `Ki` that `observed` reports (zero for a law without an
/// integral term), when the loop has an integral or derivative term and a setpoint and both are finite numbers.
fn entry_response_terms(
    loop_definition: &LoopDefinition,
    observed: &TransferStatus,
    process_noise: Option<f64>,
) -> Option<(f64, f64)> {
    let symbols = gain_symbols(loop_definition)
        .filter(|symbols| symbols.integral.is_some() || symbols.derivative.is_some())?;
    loop_definition.signals.setpoint.as_ref()?;
    let value = |symbol: &str| observed_number(observed, symbol);
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
            cycle_seconds: Some(loop_definition.controller.task_period_ms / 1000.0),
            entry_error: None,
        },
    )
    .ok()?;
    (allowance.is_finite() && integral_gain.is_finite()).then_some((allowance, integral_gain))
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

/// A Manual command that a mode return writes back, with the output it holds in Manual.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct WrittenCommand {
    /// The Manual command, one of the states of [`CommandReturn::binding`].
    pub command: String,
    /// The output the command holds: the transfer's own output for the loop's Manual command, a PID group's further output for
    /// another member's (C7).
    pub output: String,
}

/// The transfer that writes the loop's references and commands back to their values when Identify found the loop, in Manual,
/// before the loop returns to Auto (`commandBinding` of `restore`, A6.2).
#[derive(Debug, Clone, PartialEq)]
pub struct CommandReturn {
    /// The binding: the mode binding on its own transfer loop (`:commands`), with the references and commands as its states.
    pub binding: TransferBinding,
    /// The value of each of those states when Identify found the loop.
    pub values: BTreeMap<String, f64>,
    /// The Manual commands among those states that are written back, each with the output it holds: the loop's own first, when it
    /// is written back, then a PID group's other members' (C7).
    pub commands: Vec<WrittenCommand>,
}

impl CommandReturn {
    /// The allowed output change of the write-back (A6.2, annex E6.9): the larger of the mode change's own allowance `base`
    /// and the change the restored Manual commands make, from the outputs that `now` reports before the write. Returning the plant
    /// inputs to where Identify found them is the restoration itself, so it is the reviewed tolerance of that transfer. The runtime
    /// scales the tolerance of an additional output by the ratio of the output spans (E6.9), so the change of a PID group's other
    /// member counts in the span of the transfer's own output (C7). A write-back without Manual commands, or an output that `now`
    /// does not report, allows `base`.
    #[must_use]
    pub fn allowance(&self, base: f64, now: &TransferStatus) -> f64 {
        self.commands
            .iter()
            .filter_map(|written| self.change_of(written, now))
            .fold(base, f64::max)
    }

    /// The change that writing back `written` makes to the output it holds: the distance from the output `now` reports to the
    /// command's value. For an additional output it is taken in the span of the transfer's own output, times the ratio of the
    /// spans and [`ADDITIONAL_OUTPUT_HEADROOM`]. `None` when the command has no value, the output is not reported as a number, or an
    /// additional output has no span.
    fn change_of(&self, written: &WrittenCommand, now: &TransferStatus) -> Option<f64> {
        let back = *self.values.get(&written.command)?;
        let held = now.values.get(&written.output).copied()?.as_number()?;
        let change = (back - held).abs();
        if written.output == self.binding.output.symbol {
            // The runtime compares the output of the transfer with its tolerance as it is.
            return Some(change);
        }
        let output = self
            .binding
            .additional_outputs
            .iter()
            .find(|member| member.symbol == written.output)?;
        let span = self.span_of(output)?;
        let primary_span = self.span_of(&self.binding.output)?;
        (span > 0.0).then(|| change * (primary_span / span) * ADDITIONAL_OUTPUT_HEADROOM)
    }

    /// The width of the range that an output's value is checked against: its feedback guard's when it is tracked, its own otherwise
    /// (`transfer_output_basis`). `None` when the binding lacks the feedback guard.
    fn span_of(&self, output: &TransferMember) -> Option<f64> {
        let basis = transfer_output_basis(&self.binding, output)?;
        Some(basis.maximum - basis.minimum)
    }
}

/// The references and commands a mode return writes back in Manual (`restore` of `ConnectedIdentificationMode`, A6.2): the
/// loop's reference, and its Manual command unless the loop returns to Auto and the program copies the output into the Manual
/// command there (§17.1), since that copy only holds the output of the moment the mode was changed, and writing it back would
/// move the output. A PID group writes back the setpoint and the Manual command of every other member as well (C7), after the
/// loop's own, all of them in the group's binding. Their binding is the mode binding on the transfer loop `:commands`, with these
/// states and, as guards, the parameters of the loop's record and the feedback guard of each tracked output, so that the binding
/// names every guard its tracked outputs need (annex E6.2). The values are those `original` reports, rounded as their members are
/// typed.
///
/// The `TypeScript` keeps only the parameter guards, so a loop with a measured actuator would name a feedback guard the binding
/// does not hold, which the runtime refuses (E6.2); the feedback guard is kept here.
///
/// # Errors
/// The loop's channels do not fit (see `declared_channels` and `further_channels`), the binding lacks a further member's setpoint
/// or Manual command, or an original value does not fit its member.
pub fn command_return(
    loop_definition: &LoopDefinition,
    binding: &TransferBinding,
    original: &TransferStatus,
) -> Result<CommandReturn, Failure> {
    let channels = declared_channels(loop_definition)?;
    let further = further_channels(loop_definition)?;
    let follows = loop_definition.controller.manual_output_follows_output == Some(true);
    let mut states = vec![channels.reference.clone()];
    for channel in &further.references {
        states.push(state_of(binding, &channel.reference_symbol)?);
    }
    let mut written = Vec::new();
    // The loop declares that its program copies the output into its own Manual command, and no other: a PID group's other
    // members' commands are always written back.
    if !(found_in_auto(binding, original) && follows) {
        states.push(channels.command.clone());
        written.push(WrittenCommand {
            command: channels.command.symbol.clone(),
            output: binding.output.symbol.clone(),
        });
    }
    for channel in &further.commands {
        states.push(state_of(binding, &channel.manual_command_symbol)?);
        written.push(WrittenCommand {
            command: channel.manual_command_symbol.clone(),
            output: channel.output_symbol.clone(),
        });
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
    let returned = TransferBinding {
        loop_id: format!("{}{COMMANDS_LOOP_SUFFIX}", binding.loop_id),
        states,
        guards,
        ..binding.clone()
    };
    let values = declared_states(&returned, &original.values)?;
    Ok(CommandReturn {
        binding: returned,
        values,
        commands: written,
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
/// the value `current` reports and each such reference at its found value, rounded as its member is typed. A PID group has a
/// reference for every member (C7), and one step returns every reference the group no longer holds. `None` when the loop holds
/// every reference.
///
/// # Errors
/// The loop's channels do not fit (see `declared_channels` and `further_channels`), the binding lacks a further member's setpoint,
/// a value does not fit its member, or the binding has no output basis.
pub fn reference_return(
    loop_definition: &LoopDefinition,
    binding: &TransferBinding,
    original: &TransferStatus,
    current: &TransferStatus,
) -> Result<Option<ReferenceReturn>, Failure> {
    let channels = declared_channels(loop_definition)?;
    let further = further_channels(loop_definition)?;
    let mut references = vec![channels.reference.clone()];
    for channel in &further.references {
        references.push(state_of(binding, &channel.reference_symbol)?);
    }
    let found = declared_states(
        &TransferBinding {
            states: references.clone(),
            ..binding.clone()
        },
        &original.values,
    )?;
    let mut values = mode_states(binding, current)?;
    let mut moved = false;
    for reference in &references {
        let Some(back) = found.get(&reference.symbol).copied() else {
            continue;
        };
        if values.get(&reference.symbol).copied() != Some(back) {
            values.insert(reference.symbol.clone(), back);
            moved = true;
        }
    }
    if !moved {
        return Ok(None);
    }
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

#[cfg(test)]
#[path = "mode_transfer_group_tests.rs"]
mod group_tests;
