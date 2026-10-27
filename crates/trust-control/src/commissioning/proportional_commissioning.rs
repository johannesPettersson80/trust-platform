//! The decisions of a proportional commissioning run (`proportionalCommissioning.ts`; specification 35, C13 and B6): the reviewed
//! target, the criteria and channels the result records, the checks a run makes at each task boundary, the transfers it writes,
//! how an attempt ends and what its result says. The caller reads the runtime, writes the transfers and stops the recording, and
//! passes each outcome here (B6.1, A6.3).
//!
//! Left out, as transport: the review's connection and live-write precondition, the parameter and transfer clients, the wait for
//! the starting point (its decision is `StartingPoint::wait_step`), the recording session and its persistence, the follow loop and
//! target change of the observation (`observeTargetChange`), the detach of a closed panel, the journal and its checkpoint writer
//! (`CommissioningJournal`), the promises and the clocks. The recovery that a run ends with is in `proportional_recovery`.

use std::collections::BTreeMap;

use crate::failure::{Cause, Failure, NextAction};
use crate::loop_definition::{LoopDefinition, Range};
use crate::model::GeneralModel;
use crate::validation::commissioning_assessment::CommissioningCriteria;
use crate::validation::engineer_verification_plan::EngineerRequirements;

use super::actuator_feedback::{transfer_output_basis, transfer_target_allowance};
use super::commissioning_result::{CommissioningChannel, CommissioningStatus};
use super::entry_allowance::{auto_entry_error_gain, auto_entry_tolerance, transfer_tolerance};
use super::found_mode::FoundMode;
use super::observation::CommissioningObservation;
use super::observation_assessment::{
    assess_observation, JournalStage, ObservationRefusal, ObservedRecording,
};
use super::parameter_status::{same_parameter_values, ParameterStatus};
use super::project_law::{entry_application_values, live_application_values};
use super::starting_point::{compared_plant_inputs, StartingPoint};
use super::texts;
use super::transfer::{
    proportional_parameter, rounded_transfer_value, TransferBinding, TransferMember, TransferValue,
};
use super::transfer_tracking::{
    proportional_tracking, proportional_tracking_reference, TransferTracking,
};

/// One transfer a run writes at a task boundary: the states it writes, the allowance of its first output (A6.4), and its tracking
/// (annex E6.4). The mode is written with it: Manual for a staging or recovery, Auto for an entry.
#[derive(Debug, Clone, PartialEq)]
pub struct TransferStep {
    /// The state values the transfer writes, by symbol: the reference, the Manual command and the application states of the law (C1).
    pub values: BTreeMap<String, f64>,
    /// The largest change of the first output the transfer accepts (A6.4).
    pub tolerance: f64,
    /// The factor of the error at entry that the runtime adds to `tolerance` (`entry_error_gain`, annex E6.9), for an Auto
    /// entry whose allowance has the integral step on that error.
    pub entry_error_gain: Option<f64>,
    /// The tracking the transfer carries.
    pub tracking: TransferTracking,
}

/// What an attempt of a run ended with, as the caller observed it (B6.3, B6.7, A6.3). `Detached` is the panel closing during a
/// recorded observation: nothing is restored, stored or reported (A8.5, A10.4).
#[derive(Debug, Clone, PartialEq)]
pub enum RunAttempt {
    /// The response met every requirement (B6.3): the largest error of its settled samples, in the output's unit.
    Passed {
        /// The largest error of the settled samples.
        maximum_error: f64,
    },
    /// The engineer cancelled the run; the caller's check found it (`checkCancelled`, A10.4).
    Cancelled,
    /// A step was refused or failed before the response passed: the previous settings are restored (A6.3, B6.7).
    Refused(Failure),
    /// A measured output left its permitted range: the previous settings are restored at once (B6.3).
    PermittedRangeLeft(Failure),
    /// A requirement of the recorded response failed: the reviewed gain stays applied, and Restore previous settings is offered
    /// (B6.3).
    RequirementFailed(Failure),
    /// The panel closed during the recorded observation: the runtime keeps recording it (A8.5, A10.4).
    Detached,
}

/// How a recovery of the previous settings ended (B6.3, B6.7, A6.3).
#[derive(Debug, Clone, PartialEq)]
pub enum Recovery {
    /// No recovery was attempted: the response passed, or its settings stay applied for Restore.
    NotAttempted,
    /// The runtime confirmed the previous settings.
    Confirmed,
    /// The recovery, or the read of the applied settings, failed with this error.
    Unconfirmed(String),
}

/// How a finished run reports itself (`execute`, `proportionalCommissioning.ts`; B6.3, B6.7): the status, the reason, the failure with
/// its next action, whether the previous settings are confirmed back, whether a Restore is awaited, and the journal stage the
/// outcome is written at.
#[derive(Debug, Clone, PartialEq)]
pub struct RunEnding {
    /// How the run ended.
    pub status: CommissioningStatus,
    /// The sentence the engineer reads (A11.1).
    pub reason: String,
    /// The typed failure with its next action (A11.2), when the run failed.
    pub failure: Option<Failure>,
    /// Whether the previous settings are confirmed back.
    pub recovery_confirmed: bool,
    /// Whether the reviewed gain stays applied until the engineer chooses Restore previous settings (B6.3).
    pub awaiting_restore: bool,
    /// The journal stage the outcome is written at.
    pub stage: JournalStage,
}

/// The refusal of a transfer binding without its reference, its Manual command or its output (`transferConfigure`, A6.3).
pub(crate) fn binding_failure() -> Failure {
    Failure::new(
        &texts::transfer_configure_binding(),
        Cause::Implementation,
        NextAction::OpenLoopSetup,
    )
}

/// The refusal that makes a failure's recovery Restore again (B6.7): the failure keeps its cause and its text, and offers Restore
/// previous settings. A failure without one is made from the reason.
#[must_use]
pub fn restore_again(failure: Option<Failure>, reason: &str) -> Failure {
    match failure {
        Some(failure) => Failure::from_parts(
            failure.message,
            failure.detail,
            failure.cause,
            NextAction::Restore,
        ),
        None => Failure::new(reason, Cause::Runtime, NextAction::Restore),
    }
}

/// The reviewed target, rounded to the reference's IEC type as the runtime holds it, so that the transfer, the report and the
/// assessment agree (`target`, C13.4, B6.2): the operating point's first output plus the reviewed reference change.
///
/// # Errors
/// The binding has no reference, or the target does not fit the reference's type and bounds.
pub fn reviewed_target(
    binding: &TransferBinding,
    model: &GeneralModel,
    task: &EngineerRequirements,
) -> Result<f64, Failure> {
    let (reference, _) = state_pair(binding)?;
    let operating_output = model
        .identity()
        .operating_point
        .as_ref()
        .and_then(|point| point.outputs.first())
        .copied()
        .unwrap_or(0.0);
    rounded_transfer_value(reference, operating_output + task.reference_change)
}

/// The criteria a run judges its response by (`criteria`, B6.3, C13.7): the reviewed target and duration, the settling time and the
/// error limit, the task period, and the ranges of the process and of the output. The `TypeScript` criteria also carry the observed
/// channels; here the observation is judged with its own type (`assess_run`).
#[must_use]
pub fn run_criteria(
    loop_definition: &LoopDefinition,
    binding: &TransferBinding,
    task: &EngineerRequirements,
    target: f64,
) -> CommissioningCriteria {
    CommissioningCriteria {
        target,
        duration_seconds: task.duration_seconds,
        settling_time_seconds: task.settling_time_seconds,
        maximum_error: task.maximum_error,
        task_period_ms: loop_definition.controller.task_period_ms,
        process_range: loop_definition.signals.process_variable.range,
        output_limits: Range {
            minimum: binding.output.minimum,
            maximum: binding.output.maximum,
        },
        switching: None,
    }
}

/// The channels a run records, in the order the result names them: the setpoint, the process variable and the manipulated variable
/// (`channels`, C13.8).
///
/// # Errors
/// The loop has no setpoint, so the reference the run tracks has no channel.
pub fn run_channels(
    loop_definition: &LoopDefinition,
) -> Result<Vec<CommissioningChannel>, Failure> {
    let signals = &loop_definition.signals;
    let setpoint = signals.setpoint.as_ref().ok_or_else(binding_failure)?;
    Ok([
        setpoint,
        &signals.process_variable,
        &signals.manipulated_variable,
    ]
    .into_iter()
    .map(|signal| CommissioningChannel {
        id: signal.symbol.clone(),
        unit: signal.unit.clone(),
    })
    .collect())
}

/// The settling time the observation is expected to reach, or none for an on/off controller (`expectedSettlingSeconds`, B6.3).
#[must_use]
pub fn expected_settling_seconds(
    loop_definition: &LoopDefinition,
    task: &EngineerRequirements,
) -> Option<f64> {
    loop_definition
        .controller
        .on_off
        .is_none()
        .then_some(task.settling_time_seconds)
}

/// Whether a campaign may start (`run`, `proportionalCommissioning.ts`): a review that permits recovery only never starts Auto
/// commissioning, and a campaign starts once.
///
/// # Errors
/// The review permits recovery only, or the campaign has already started.
pub fn check_may_run(historical: bool, started: bool) -> Result<(), Failure> {
    if historical {
        return Err(Failure::new(
            &texts::prop_commission_recovery_only(),
            Cause::Unclassified,
            NextAction::None,
        ));
    }
    if started {
        return Err(Failure::new(
            &texts::prop_commission_already_started(),
            Cause::Unclassified,
            NextAction::None,
        ));
    }
    Ok(())
}

/// Whether the runtime keeps what a campaign needs to recover after a reload (`setCheckpointWriter`, `proportionalCommissioning.ts`;
/// E7.2): a transaction identifier, and request identities.
///
/// # Errors
/// The review has no transaction identifier, or the runtime keeps no request identities.
pub fn check_recoverable(
    transaction_id: Option<&str>,
    supports_request_id: Option<bool>,
) -> Result<(), Failure> {
    if transaction_id.is_some_and(|id| !id.is_empty()) && supports_request_id == Some(true) {
        return Ok(());
    }
    Err(Failure::new(
        &texts::prop_commission_recoverable_required(),
        Cause::Runtime,
        NextAction::None,
    ))
}

/// Whether the runtime supports the proportional tracking that a P Auto entry needs (`review`, `proportionalCommissioning.ts`;
/// E6.3).
///
/// # Errors
/// The runtime does not list `proportional` among its tracking kinds.
pub fn check_proportional_tracking(supported: Option<&[String]>) -> Result<(), Failure> {
    if supported.is_some_and(|kinds| kinds.iter().any(|kind| kind.as_str() == "proportional")) {
        return Ok(());
    }
    Err(Failure::new(
        &texts::prop_commission_tracking_required(),
        Cause::ControllerCapability,
        NextAction::None,
    ))
}

/// Whether the program is the one the review read (`review`, `proportionalCommissioning.ts`; B6.7: Validate again).
///
/// # Errors
/// The program revision differs from the reviewed one.
pub fn check_program_unchanged(reviewed: &str, observed: &str) -> Result<(), Failure> {
    if reviewed == observed {
        return Ok(());
    }
    Err(Failure::new(
        &texts::prop_commission_program_changed(),
        Cause::Runtime,
        NextAction::Validate,
    ))
}

/// The starting point of the run, checked against the validated point (`assertCommissioningStartingPoint`, B6.2): a Manual loop's
/// plant input must lie within the Auto-entry allowance `tolerance`, and the measured output must rest in its band. A loop found in
/// Auto does not compare its plant input (`compared_plant_inputs`).
///
/// # Errors
/// A plant input is away from its point, an output has not settled, or a value is not a finite number.
pub fn check_starting_point(
    loop_definition: &LoopDefinition,
    model: &GeneralModel,
    task: &EngineerRequirements,
    binding: &TransferBinding,
    found: Option<FoundMode>,
    observed: &BTreeMap<String, TransferValue>,
    tolerance: f64,
) -> Result<(), Failure> {
    let inputs = [output_symbol(binding)?.to_owned()];
    let compared = compared_plant_inputs(&inputs, found);
    let outputs = [loop_definition.signals.process_variable.symbol.clone()];
    StartingPoint {
        loop_definition,
        model,
        task,
        input_symbols: compared,
        output_symbols: &outputs,
        input_tolerance: tolerance,
    }
    .assert_settled(observed)
}

/// The parameter record, the mode and the reference at the start of a run are the ones the review read (`execute`,
/// `proportionalCommissioning.ts`; B6.2, B6.7: Validate again).
///
/// # Errors
/// The parameter record changed since the review, the mode is not the reviewed one, or the reference is not the reviewed one.
#[allow(clippy::float_cmp)] // The reference is compared exactly, as the TypeScript compares it.
pub fn check_reviewed_state(
    reviewed: &ParameterStatus,
    current: &ParameterStatus,
    reviewed_mode: Option<bool>,
    binding: &TransferBinding,
    boundary: &BTreeMap<String, TransferValue>,
    previous_reference: f64,
) -> Result<(), Failure> {
    let record_held = current.program_revision == reviewed.program_revision
        && current.binding_revision == reviewed.binding_revision
        && current.active_revision == reviewed.active_revision
        && same_parameter_values(&current.values, &reviewed.values);
    let mode_held = reviewed_mode.is_some_and(|mode| {
        boundary.get(&binding.mode.symbol) == Some(&TransferValue::Boolean(mode))
    });
    let reference_held = binding.states.first().is_some_and(|reference| {
        boundary
            .get(&reference.symbol)
            .copied()
            .and_then(TransferValue::as_number)
            == Some(previous_reference)
    });
    if record_held && mode_held && reference_held {
        return Ok(());
    }
    Err(Failure::new(
        &texts::prop_commission_reviewed_changed(),
        Cause::Runtime,
        NextAction::Validate,
    ))
}

/// The staged gains still hold the reviewed values before the Auto entry (`execute`, `proportionalCommissioning.ts`; A6.8, B6.7). The
/// refusal names each parameter the program changed.
///
/// # Errors
/// A staged parameter does not hold its reviewed value.
pub fn check_staged_values(
    reviewed: &BTreeMap<String, f64>,
    observed: &BTreeMap<String, TransferValue>,
) -> Result<(), Failure> {
    let changed = changed_symbols(reviewed, |symbol| {
        observed
            .get(symbol)
            .copied()
            .and_then(TransferValue::as_number)
    });
    if changed.is_empty() {
        return Ok(());
    }
    Err(Failure::new(
        &texts::prop_commission_staged_changed(&changed.join(", ")),
        Cause::Implementation,
        NextAction::OpenLoopSetup,
    ))
}

/// The parameter record still holds the reviewed program and values after the observation (`execute`, `proportionalCommissioning.ts`;
/// A6.4, A6.8). The refusal names each parameter the program changed.
///
/// # Errors
/// The program revision changed, or a parameter does not hold its reviewed value.
pub fn check_held_values(
    reviewed_program: &str,
    reviewed: &BTreeMap<String, f64>,
    parameters: &ParameterStatus,
) -> Result<(), Failure> {
    if parameters.program_revision != reviewed_program {
        return Err(Failure::new(
            &texts::prop_commission_held_program(),
            Cause::Implementation,
            NextAction::OpenLoopSetup,
        ));
    }
    let changed = changed_symbols(reviewed, |symbol| parameters.values.get(symbol).copied());
    if changed.is_empty() {
        return Ok(());
    }
    Err(Failure::new(
        &texts::prop_commission_held_changed(&changed.join(", ")),
        Cause::Implementation,
        NextAction::OpenLoopSetup,
    ))
}

/// Whether the loop is not in its Manual mode at the boundary, so the run stages Manual before it writes anything (`execute`, B6.2).
#[must_use]
pub fn stages_manual(
    binding: &TransferBinding,
    observed: &BTreeMap<String, TransferValue>,
) -> bool {
    observed.get(&binding.mode.symbol) != Some(&TransferValue::Boolean(binding.mode.manual_value))
}

/// The transfer that sets the loop to Manual and tracks its held output with the previous reference (`recover` and the first
/// staging of `execute`, A6.2, C13.2): the states, the reviewed Auto-entry tolerance or the exact check, and the unaligned tracking.
///
/// # Errors
/// The binding is incomplete, a value is missing or not finite, or a state does not fit its type and bounds.
pub fn manual_step(
    loop_definition: &LoopDefinition,
    binding: &TransferBinding,
    task: &EngineerRequirements,
    observed: &BTreeMap<String, TransferValue>,
    previous_reference: f64,
) -> Result<TransferStep, Failure> {
    Ok(TransferStep {
        values: state_values(
            binding,
            observed,
            previous_reference,
            live_application_values(loop_definition, binding, observed)?,
        )?,
        tolerance: transfer_tolerance(loop_definition, task.transfer_tolerance)?,
        entry_error_gain: None,
        tracking: TransferTracking::Proportional(proportional_tracking(
            loop_definition,
            binding,
            false,
        )?),
    })
}

/// The Auto entry of a proportional loop (`execute` and the return to Auto of a recovery, C13.3, C13.4): the reference aligned to the
/// held output by the gain in `gains` (`r = PV + u / Kp`), the states it writes, the allowance of the first output for those gains
/// (A6.4), and the aligned tracking.
///
/// # Errors
/// The gain is zero or not finite, a value is missing, a state does not fit its bounds, or the loop declares state feedback.
pub fn auto_entry_step(
    loop_definition: &LoopDefinition,
    binding: &TransferBinding,
    task: &EngineerRequirements,
    gains: &BTreeMap<String, f64>,
    process_noise: Option<f64>,
    observed: &BTreeMap<String, TransferValue>,
) -> Result<TransferStep, Failure> {
    let gain_symbol = &proportional_parameter(loop_definition)?.symbol;
    let gain = gains.get(gain_symbol).copied().unwrap_or(f64::NAN);
    let reference = aligned_reference(loop_definition, binding, observed, gain)?;
    Ok(TransferStep {
        values: state_values(
            binding,
            observed,
            reference,
            entry_application_values(loop_definition, binding)?,
        )?,
        tolerance: auto_entry_tolerance(loop_definition, task, gains, process_noise, None)?,
        entry_error_gain: auto_entry_error_gain(loop_definition, task, gains, process_noise),
        tracking: TransferTracking::Proportional(proportional_tracking(
            loop_definition,
            binding,
            true,
        )?),
    })
}

/// The step to the reviewed target while the loop is in Auto (`applyTarget`, C13.6): the states at the target, the allowance of the
/// target change across the outputs, and the unaligned tracking.
///
/// # Errors
/// The binding is incomplete or a value is missing or not finite.
pub fn target_step(
    loop_definition: &LoopDefinition,
    binding: &TransferBinding,
    observed: &BTreeMap<String, TransferValue>,
    target: f64,
) -> Result<TransferStep, Failure> {
    Ok(TransferStep {
        values: state_values(
            binding,
            observed,
            target,
            live_application_values(loop_definition, binding, observed)?,
        )?,
        tolerance: transfer_target_allowance(binding).ok_or_else(binding_failure)?,
        entry_error_gain: None,
        tracking: TransferTracking::Proportional(proportional_tracking(
            loop_definition,
            binding,
            false,
        )?),
    })
}

/// Judges the recorded response of an attempt (`assessObservation`, B6.3): the response passes, or its refusal says how it failed.
#[must_use]
pub fn assess_run(
    recording: &ObservedRecording,
    criteria: &CommissioningCriteria,
    observation: &CommissioningObservation,
) -> RunAttempt {
    match assess_observation(recording, criteria, Some(observation)) {
        Ok(maximum_error) => RunAttempt::Passed { maximum_error },
        Err(refusal) => attempt_of_refusal(refusal),
    }
}

/// The attempt an observation refusal ends with (B6.3, A8.5): a permitted-range departure, a failed requirement, a detached panel, or
/// a recording the runtime no longer holds, which is a refusal of the run.
#[must_use]
pub fn attempt_of_refusal(refusal: ObservationRefusal) -> RunAttempt {
    match refusal {
        ObservationRefusal::PermittedRangeLeft(failure) => RunAttempt::PermittedRangeLeft(failure),
        ObservationRefusal::RequirementFailed(failure) => RunAttempt::RequirementFailed(failure),
        ObservationRefusal::Detached(_) => RunAttempt::Detached,
        ObservationRefusal::NotHeld(failure) => RunAttempt::Refused(failure),
    }
}

/// Ends a run (`execute`, `proportionalCommissioning.ts`; B6.3, B6.7, A6.3, A8.5): the status, the reason, the failure, the recovery and
/// the journal stage of its result. `None` when the panel closed during a recorded observation: that outcome belongs to the runtime
/// and to the reopened panel. `process_unit` is the unit of the measured output, and `recording_stop` is the error of a stop that was
/// not confirmed.
#[must_use]
pub fn settle_run(
    attempt: RunAttempt,
    cancelled: bool,
    recovery: Recovery,
    found_in_auto: bool,
    process_unit: &str,
    recording_stop: Option<&str>,
) -> Option<RunEnding> {
    let mut ending = match attempt {
        RunAttempt::Detached => return None,
        RunAttempt::Passed { maximum_error } => RunEnding {
            status: CommissioningStatus::Passed,
            reason: texts::prop_commission_passed(maximum_error, process_unit),
            failure: None,
            recovery_confirmed: true,
            awaiting_restore: false,
            stage: JournalStage::Completed,
        },
        RunAttempt::RequirementFailed(failure) => {
            // B6.3: the reviewed gain stays applied, the failed requirement is reported, and Restore is offered.
            let mut ending = failed_ending(cancelled, full_text(&failure), Some(failure), true);
            if let Recovery::Unconfirmed(error) = recovery {
                unconfirmed(&mut ending, &texts::prop_commission_settings_unread(&error));
            }
            ending
        }
        RunAttempt::PermittedRangeLeft(failure) => {
            // B6.3, B6.7: the previous settings are restored at once, and the departure is the reason.
            let statement = full_text(&failure);
            let mut ending = failed_ending(cancelled, statement.clone(), Some(failure), false);
            match recovery {
                Recovery::Unconfirmed(error) => unconfirmed(
                    &mut ending,
                    &texts::prop_commission_recovery_unconfirmed(&error),
                ),
                Recovery::Confirmed => {
                    ending.reason = texts::prop_commission_range_restored(&statement);
                }
                Recovery::NotAttempted => {}
            }
            ending
        }
        RunAttempt::Refused(failure) => {
            let mut ending = failed_ending(cancelled, full_text(&failure), Some(failure), false);
            recovered(&mut ending, recovery, found_in_auto);
            ending
        }
        RunAttempt::Cancelled => {
            let mut ending = failed_ending(true, texts::prop_commission_cancelled(), None, false);
            recovered(&mut ending, recovery, found_in_auto);
            ending
        }
    };
    if let Some(error) = recording_stop {
        ending.recovery_confirmed = false;
        ending.reason = format!(
            "{} {}",
            ending.reason,
            texts::prop_commission_stop_unconfirmed(error)
        );
    }
    ending.stage = if ending.status == CommissioningStatus::Passed || ending.awaiting_restore {
        JournalStage::Completed
    } else if ending.recovery_confirmed {
        JournalStage::Recovered
    } else {
        JournalStage::Transfer
    };
    Some(ending)
}

/// The ending of a run that did not pass: cancelled when the engineer cancelled it, failed otherwise.
fn failed_ending(
    cancelled: bool,
    reason: String,
    failure: Option<Failure>,
    awaiting_restore: bool,
) -> RunEnding {
    RunEnding {
        status: if cancelled {
            CommissioningStatus::Cancelled
        } else {
            CommissioningStatus::Failed
        },
        reason,
        failure,
        recovery_confirmed: true,
        awaiting_restore,
        stage: JournalStage::Transfer,
    }
}

/// Adds a restored or unconfirmed recovery to the reason of a refusal (`execute`'s catch, A6.3).
fn recovered(ending: &mut RunEnding, recovery: Recovery, found_in_auto: bool) {
    match recovery {
        Recovery::NotAttempted => {}
        Recovery::Confirmed => {
            ending.reason = texts::recovery_with_outcome(
                &ending.reason,
                &texts::prop_commission_recovered(found_in_auto),
            );
        }
        Recovery::Unconfirmed(error) => {
            unconfirmed(ending, &texts::prop_commission_recovery_unconfirmed(&error));
        }
    }
}

/// Records that the previous settings are not confirmed: the reason carries the outcome, and the failure offers Restore again
/// (B6.7).
fn unconfirmed(ending: &mut RunEnding, outcome: &str) {
    ending.recovery_confirmed = false;
    ending.reason = texts::recovery_with_outcome(&ending.reason, outcome);
    let failure = ending.failure.take();
    ending.failure = Some(restore_again(failure, &ending.reason));
}

/// The whole text of a failure: its detail when it says more, otherwise its message (`MainWorkflowError.message`).
fn full_text(failure: &Failure) -> String {
    failure
        .detail
        .clone()
        .unwrap_or_else(|| failure.message.clone())
}

/// The state values of a transfer: the reference, and the Manual command set to the output the loop holds at the boundary, each
/// rounded to its IEC type (`stateValues`, E6.2).
/// The values a transfer writes for the states of its binding: the reference and the Manual command, and the application states of a
/// program's own law (C1), each at the value the caller gives for it. The transfer protocol takes a value for every state.
fn state_values(
    binding: &TransferBinding,
    observed: &BTreeMap<String, TransferValue>,
    reference: f64,
    application: BTreeMap<String, f64>,
) -> Result<BTreeMap<String, f64>, Failure> {
    let (reference_member, manual_member) = state_pair(binding)?;
    let held = number_at(observed, output_symbol(binding)?)?;
    let mut values = BTreeMap::from([
        (
            reference_member.symbol.clone(),
            rounded_transfer_value(reference_member, reference)?,
        ),
        (
            manual_member.symbol.clone(),
            rounded_transfer_value(manual_member, held)?,
        ),
    ]);
    values.extend(application);
    Ok(values)
}

/// The reference that an aligned Auto entry starts from (`trackingReference`, C13.3): `r = PV + u / Kp`, from the process value and
/// the held output that the runtime reports. The review computes it too, so that a zero gain is refused before any write.
///
/// # Errors
/// The gain is zero or not finite, a value is missing, or the reference does not fit its type and bounds.
pub fn aligned_reference(
    loop_definition: &LoopDefinition,
    binding: &TransferBinding,
    observed: &BTreeMap<String, TransferValue>,
    gain: f64,
) -> Result<f64, Failure> {
    let process = number_at(observed, &loop_definition.signals.process_variable.symbol)?;
    let held = number_at(observed, output_symbol(binding)?)?;
    proportional_tracking_reference(binding, process, held, gain)
}

/// The reference and the Manual command of a binding: the first two states, which every commissioning binding declares in that
/// order (A6.3).
fn state_pair(binding: &TransferBinding) -> Result<(&TransferMember, &TransferMember), Failure> {
    let (Some(reference), Some(manual)) = (binding.states.first(), binding.states.get(1)) else {
        return Err(binding_failure());
    };
    Ok((reference, manual))
}

/// The symbol whose bounds the output is checked against: the feedback of an actuator-tracked output, or the output (A5.3).
fn output_symbol(binding: &TransferBinding) -> Result<&str, Failure> {
    transfer_output_basis(binding, &binding.output)
        .map(|member| member.symbol.as_str())
        .ok_or_else(binding_failure)
}

/// A finite number the runtime reported for `symbol` (the runtime answer, E6.3).
fn number_at(observed: &BTreeMap<String, TransferValue>, symbol: &str) -> Result<f64, Failure> {
    observed
        .get(symbol)
        .copied()
        .and_then(TransferValue::as_number)
        .filter(|value| value.is_finite())
        .ok_or_else(|| {
            Failure::new(
                &texts::start_value_missing(symbol),
                Cause::Runtime,
                NextAction::None,
            )
        })
}

/// The symbols of the reviewed parameters whose values differ from `read` (exact comparison, as the `TypeScript` compares them).
#[allow(clippy::float_cmp)] // The gains are compared exactly: a program that rewrites one changed it.
fn changed_symbols(
    reviewed: &BTreeMap<String, f64>,
    read: impl Fn(&str) -> Option<f64>,
) -> Vec<String> {
    reviewed
        .iter()
        .filter(|(symbol, value)| read(symbol.as_str()) != Some(**value))
        .map(|(symbol, _)| symbol.clone())
        .collect()
}

#[cfg(test)]
#[path = "proportional_commissioning_tests.rs"]
mod tests;
