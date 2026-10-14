//! The protocol of a controller transfer (annex E6; specification 35, A6.3, B6.1 and B6.4): the request an apply sends, the
//! checks of a transfer status and of its outcome, the decision of the confirmation loop, and the check of a cancel's answer.
//! The runtime calls, the transaction identifier and the timers are transport (`runtime_client/transfers.rs`); each rule here
//! takes the runtime's answer as data that has already been read. From `runtimeTransfer.ts`.

use std::collections::BTreeMap;

use serde::{Deserialize, Serialize, Serializer};

use super::actuator_feedback::transfer_output_basis;
use super::application::ApplySource;
use super::commissioning_result::{
    TransferConfirmation, TransferOwner, TransferState, TransferStatus, TransferTransaction,
};
use super::declared_initialization::DeclaredStateTracking;
use super::initialization_formulas::evaluate_initialization_formula;
use super::texts;
use super::transfer::{rounded_transfer_value, TransferBinding, TransferMember, TransferValue};
use super::transfer_tracking::{ParallelPidTracking, ProportionalTracking, TransferTracking};
use crate::failure::{Cause, Failure, NextAction};

/// The most tracking kinds one status may report (`supported_tracking`, annex E6.3).
const MAXIMUM_SUPPORTED_TRACKING: usize = 16;

/// The request of `control.transfer.apply` (annex E6.5). Its fields are the runtime's, in the runtime's `snake_case` names.
#[derive(Debug, Clone, PartialEq, Serialize)]
pub struct TransferApplyRequest {
    /// The transaction identifier, journaled before the request is sent (E0.2, E11.2).
    pub transaction_id: String,
    /// The binding the transfer writes and checks.
    pub binding: TransferBinding,
    /// The program revision the apply was reviewed on.
    pub expected_program_revision: String,
    /// The binding revision the apply was reviewed on.
    pub expected_binding_revision: String,
    /// The values of the status the apply was reviewed on: one per member and per mode.
    pub expected_values: BTreeMap<String, TransferValue>,
    /// The states the transfer writes: exactly the states that its tracking does not compute.
    pub values: BTreeMap<String, f64>,
    /// `true` leaves the loop in Manual, `false` leaves it in Auto.
    pub manual: bool,
    /// The reviewed output change allowed at the first execution (E6.9).
    pub maximum_output_change: f64,
    /// The target the transfer goes to: `simulation` or `physical`.
    #[serde(serialize_with = "serialize_source")]
    pub source: ApplySource,
    /// Whether the engineer approved the physical write (E10.2).
    pub physical_approved: bool,
    /// The tracking that an Auto entry carries (E6.4), when the transfer has one.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub tracking: Option<TransferTracking>,
}

/// Writes an apply's source as the word the runtime reads (E6.5).
#[allow(clippy::trivially_copy_pass_by_ref)] // Serde's `serialize_with` passes the field by reference.
fn serialize_source<S: Serializer>(source: &ApplySource, serializer: S) -> Result<S::Ok, S::Error> {
    serializer.serialize_str(source.wire())
}

/// The parts of one apply that the caller chooses, grouped so that [`transfer_apply_request`] takes at most seven arguments.
#[derive(Debug, Clone, Copy)]
pub struct TransferApplyInput<'a> {
    /// The transaction identifier that the caller minted and retained.
    pub transaction_id: &'a str,
    /// The states the transfer writes.
    pub values: &'a BTreeMap<String, f64>,
    /// Whether the apply leaves the loop in Manual.
    pub manual: bool,
    /// The reviewed output change allowed at the first execution (E6.9).
    pub maximum_output_change: f64,
    /// The tracking of the apply, when it has one (E6.4).
    pub tracking: Option<&'a TransferTracking>,
    /// The target the transfer goes to.
    pub source: ApplySource,
    /// Whether the engineer approved the physical write (E10.2).
    pub physical_approved: bool,
}

/// The request of `control.transfer.cancel` (annex E6.10).
#[derive(Debug, Clone, PartialEq, Serialize)]
pub struct TransferCancelRequest {
    /// The loop the transfer belongs to.
    pub loop_id: String,
    /// The transaction identifier of the owned transfer.
    pub transaction_id: String,
    /// The program revision the owner was made on; a cancel for another revision is refused.
    pub expected_program_revision: String,
}

/// The answer of `control.transfer.cancel` (annex E6.10). A field the answer omits is `None` or `false`; only a confirmed answer that
/// names the owner settles a cancel (see [`transfer_cancel_acknowledged`]).
#[derive(Debug, Clone, PartialEq, Deserialize)]
pub struct TransferCancelAnswer {
    /// The status of the transaction after the cancel.
    #[serde(default)]
    pub status: Option<TransferState>,
    /// Whether the runtime confirmed the cancel.
    #[serde(default)]
    pub confirmed: bool,
    /// Why the transaction did not end as cancelled, when it did not.
    #[serde(default)]
    pub reason: Option<String>,
    /// The transaction identifier that the answer is for.
    #[serde(default)]
    pub transaction_id: Option<String>,
    /// The program revision that the answer is for.
    #[serde(default)]
    pub program_revision: Option<String>,
    /// The binding revision that the answer is for.
    #[serde(default)]
    pub binding_revision: Option<String>,
    /// Whether the transaction was cancelled before its task-boundary commit, so that nothing was written.
    #[serde(default)]
    pub before_dispatch: bool,
}

/// Checks the transfer status of the runtime (`status` of `runtimeTransfer.ts`, annex E6.3): the revisions are set, the values hold
/// exactly the binding's modes and members, the supported tracking is a short list of names, and the last transaction, when there is
/// one, belongs to the same status and holds coherent evidence (E6.8). The values are observations and never write authority.
///
/// # Errors
/// The status does not have the form of annex E6.3, or its transaction evidence is incoherent. Such evidence is never used.
pub fn check_transfer_status(
    binding: &TransferBinding,
    status: &TransferStatus,
) -> Result<(), Failure> {
    let revisions_set = !status.program_revision.is_empty() && !status.binding_revision.is_empty();
    let tracking_set = status.supported_tracking.as_ref().is_none_or(|kinds| {
        kinds.len() <= MAXIMUM_SUPPORTED_TRACKING && kinds.iter().all(|kind| !kind.is_empty())
    });
    if !(revisions_set && tracking_set && holds_declared_values(binding, &status.values)) {
        return Err(Failure::new(
            &texts::transfer_status_invalid(),
            Cause::Runtime,
            NextAction::None,
        ));
    }
    match &status.transaction {
        None => Ok(()),
        Some(transaction) => check_transaction(binding, status.status, transaction),
    }
}

/// The evidence of the last transaction: its identity is set, it has the status of the transfer, and every value it kept holds the
/// binding's declared members (annex E6.8).
fn check_transaction(
    binding: &TransferBinding,
    status: TransferState,
    transaction: &TransferTransaction,
) -> Result<(), Failure> {
    let coherent = !transaction.id.is_empty()
        && !transaction.program_revision.is_empty()
        && !transaction.binding_revision.is_empty()
        && transaction.status == status
        && transaction
            .observed_values
            .as_ref()
            .is_none_or(|values| holds_declared_values(binding, values))
        && transaction
            .initial_values
            .as_ref()
            .is_none_or(|values| holds_declared_values(binding, values))
        && transaction
            .applied_values
            .as_ref()
            .is_none_or(|values| holds_states(binding, values));
    if coherent {
        Ok(())
    } else {
        Err(Failure::new(
            &texts::transfer_outcome_invalid(),
            Cause::Runtime,
            NextAction::None,
        ))
    }
}

/// Whether `values` holds exactly the binding's modes and members: each mode is a BOOL, and each member is a finite number.
fn holds_declared_values(
    binding: &TransferBinding,
    values: &BTreeMap<String, TransferValue>,
) -> bool {
    let modes: Vec<&String> = std::iter::once(&binding.mode.symbol)
        .chain(
            binding
                .additional_modes
                .iter()
                .map(|further| &further.symbol),
        )
        .collect();
    let members: Vec<&TransferMember> = binding
        .states
        .iter()
        .chain(&binding.guards)
        .chain(std::iter::once(&binding.output))
        .chain(&binding.additional_outputs)
        .collect();
    values.len() == modes.len() + members.len()
        && modes
            .iter()
            .all(|symbol| matches!(values.get(*symbol), Some(TransferValue::Boolean(_))))
        && members.iter().all(|member| {
            values
                .get(&member.symbol)
                .and_then(|reading| reading.as_number())
                .is_some_and(f64::is_finite)
        })
}

/// Whether `values` holds exactly the binding's states, each a finite number (`validAppliedValues`).
fn holds_states(binding: &TransferBinding, values: &BTreeMap<String, f64>) -> bool {
    values.len() == binding.states.len()
        && binding.states.iter().all(|member| {
            values
                .get(&member.symbol)
                .is_some_and(|reading| reading.is_finite())
        })
}

/// Refuses a physical transfer that the engineer has not approved (annex E6.5, check 5; E10.2, A2.5).
///
/// # Errors
/// The source is physical and the approval is missing.
pub fn require_transfer_approval(
    source: ApplySource,
    physical_approved: bool,
) -> Result<(), Failure> {
    if source == ApplySource::Physical && !physical_approved {
        return Err(Failure::new(
            &texts::parameter_physical_approval("controller transfer"),
            Cause::Approval,
            NextAction::Approve,
        ));
    }
    Ok(())
}

/// The request of an apply (`apply` of `runtimeTransfer.ts`, annex E6.5). The expected revisions and values are those of the status
/// the apply was reviewed on, so that the runtime checks the same binding the engineer saw.
///
/// # Errors
/// The source is physical and the approval is missing.
pub fn transfer_apply_request(
    binding: &TransferBinding,
    reviewed: &TransferStatus,
    input: &TransferApplyInput<'_>,
) -> Result<TransferApplyRequest, Failure> {
    require_transfer_approval(input.source, input.physical_approved)?;
    Ok(TransferApplyRequest {
        transaction_id: input.transaction_id.to_owned(),
        binding: binding.clone(),
        expected_program_revision: reviewed.program_revision.clone(),
        expected_binding_revision: reviewed.binding_revision.clone(),
        expected_values: reviewed.values.clone(),
        values: input.values.clone(),
        manual: input.manual,
        maximum_output_change: input.maximum_output_change,
        source: input.source,
        physical_approved: input.physical_approved,
        tracking: input.tracking.cloned(),
    })
}

/// Whether a cancel's answer settles the owned transfer as cancelled before its dispatch (`cancel` of `runtimeTransfer.ts`,
/// annex E6.10): the answer must be confirmed and cancelled before dispatch, and name the owner's identity exactly.
#[must_use]
pub fn transfer_cancel_acknowledged(owner: &TransferOwner, answer: &TransferCancelAnswer) -> bool {
    answer.confirmed
        && answer.before_dispatch
        && answer.status == Some(TransferState::Cancelled)
        && answer.transaction_id.as_deref() == Some(owner.id.as_str())
        && answer.program_revision.as_deref() == Some(owner.program.as_str())
        && answer.binding_revision.as_deref() == Some(owner.binding.as_str())
}

/// The failure of a transfer that did not apply, with the runtime's reason kept whole (`transferRejection` of `runtimeTransfer.ts`).
/// A failure that the runtime reverted in the same scan left the loop as it was (A6.3): its next action is to validate again, and
/// nothing is offered to restore.
#[must_use]
pub fn transfer_rejection(outcome: &TransferStatus) -> Failure {
    let reason = outcome
        .reason
        .clone()
        .unwrap_or_else(|| texts::transfer_rejected_without_reason(state_word(outcome.status)));
    let restored = outcome
        .transaction
        .as_ref()
        .and_then(|transaction| transaction.restored)
        == Some(true);
    if !restored {
        return Failure::new(&reason, Cause::Runtime, NextAction::None);
    }
    // The runtime's reason is one clause without its full stop (annex E6.7); the revert is a sentence of its own.
    let clause = if reason.ends_with(['.', '!', '?']) {
        reason
    } else {
        format!("{reason}.")
    };
    Failure::new(
        &texts::transfer_reverted(&clause),
        Cause::Settings,
        NextAction::Validate,
    )
}

/// The word of a transfer state, as the runtime writes it.
fn state_word(state: TransferState) -> &'static str {
    match state {
        TransferState::Idle => "idle",
        TransferState::Pending => "pending",
        TransferState::Applied => "applied",
        TransferState::Rejected => "rejected",
        TransferState::Failed => "failed",
        TransferState::Cancelled => "cancelled",
    }
}

/// What the confirmation loop does after one status read of its owned transfer (`finish` of `runtimeTransfer.ts`).
#[derive(Debug, Clone, PartialEq)]
#[allow(clippy::large_enum_variant)] // One step per poll, matched at once: boxing the status would buy nothing.
pub enum TransferStep {
    /// The transaction is still pending: the status is read again after the poll interval.
    Wait,
    /// The transaction has ended with this status.
    Terminal(TransferStatus),
}

/// The decision of one status read while an owned transfer is confirmed (`finish` of `runtimeTransfer.ts`, A8.5, E11.4): the last
/// transaction must still be the owned one, and a pending transaction is waited for.
///
/// # Errors
/// The last transaction is not the owned one: its identity changed, so the confirmation is unresolved.
pub fn transfer_finish_step(
    owner: &TransferOwner,
    status: &TransferStatus,
) -> Result<TransferStep, Failure> {
    let held_by_owner = status.transaction.as_ref().is_some_and(|transaction| {
        transaction.id == owner.id
            && transaction.program_revision == owner.program
            && transaction.binding_revision == owner.binding
    });
    if !held_by_owner {
        return Err(Failure::new(
            &texts::transfer_identity_changed(),
            Cause::Runtime,
            NextAction::None,
        ));
    }
    Ok(if status.status == TransferState::Pending {
        TransferStep::Wait
    } else {
        TransferStep::Terminal(status.clone())
    })
}

/// Verifies an applied transfer's outcome against the apply that asked for it (`apply` of `runtimeTransfer.ts`, after the outcome):
/// the transaction is applied under the output image, the values the task boundary held agree with the tracking (annex E6.4, E6.7),
/// and the observed mode and states are the reviewed ones (A6.3).
///
/// # Errors
/// The outcome is not applied under the output image, or a tracking value or an observed value does not match the reviewed one.
pub fn verify_transfer_outcome(
    binding: &TransferBinding,
    values: &BTreeMap<String, f64>,
    manual: bool,
    tracking: Option<&TransferTracking>,
    outcome: &TransferStatus,
) -> Result<(), Failure> {
    let transaction = match &outcome.transaction {
        Some(transaction)
            if outcome.status == TransferState::Applied
                && transaction.confirmation == Some(TransferConfirmation::OutputImage) =>
        {
            transaction
        }
        _ => return Err(transfer_rejection(outcome)),
    };
    let mut expected = values.clone();
    match tracking {
        None => {}
        Some(TransferTracking::Proportional(tracking)) => {
            proportional_values(binding, tracking, transaction, &mut expected)?;
        }
        Some(TransferTracking::ParallelPid(tracking)) => {
            parallel_pid_values(binding, tracking, transaction, &mut expected)?;
        }
        Some(TransferTracking::DeclaredState(tracking)) => {
            declared_values(binding, tracking, transaction, &mut expected)?;
        }
    }
    check_observed(
        binding,
        manual,
        tracking,
        &expected,
        transaction.observed_values.as_ref(),
    )
}

/// The proportional tracking's expected values: the Manual command is the output that the boundary held, and the reference is aligned
/// to the process value plus the held output over the gain when the tracking aligns it (`apply` of `runtimeTransfer.ts`, E6.4).
fn proportional_values(
    binding: &TransferBinding,
    tracking: &ProportionalTracking,
    transaction: &TransferTransaction,
    expected: &mut BTreeMap<String, f64>,
) -> Result<(), Failure> {
    let initial = retained_inputs(binding, transaction)?;
    let held = number_of(initial, output_basis(binding)?);
    let manual = state_member(binding, &tracking.manual_command_symbol)?;
    expected.insert(
        tracking.manual_command_symbol.clone(),
        rounded_transfer_value(manual, held)?,
    );
    if tracking.align_reference {
        let reference = state_member(binding, &tracking.reference_symbol)?;
        let aligned = number_of(initial, &tracking.process_variable)
            + held / number_of(initial, &tracking.gain_symbol);
        expected.insert(
            tracking.reference_symbol.clone(),
            rounded_transfer_value(reference, aligned)?,
        );
    }
    require_applied(
        transaction.applied_values.as_ref(),
        expected,
        &texts::transfer_applied_tracking_disagrees(),
    )
}

/// The parallel PID tracking's expected values: the reference and the Manual command are the states that the boundary applied; the
/// other states are checked by their bounds afterwards (`apply` of `runtimeTransfer.ts`, E6.4).
fn parallel_pid_values(
    binding: &TransferBinding,
    tracking: &ParallelPidTracking,
    transaction: &TransferTransaction,
    expected: &mut BTreeMap<String, f64>,
) -> Result<(), Failure> {
    let applied = transaction
        .applied_values
        .as_ref()
        .filter(|values| holds_states(binding, values))
        .ok_or_else(|| {
            Failure::new(
                &texts::transfer_pid_applied_incomplete(),
                Cause::Runtime,
                NextAction::None,
            )
        })?;
    for symbol in [&tracking.reference_symbol, &tracking.manual_command_symbol] {
        let value = applied_value(applied, symbol)?;
        expected.insert(symbol.clone(), value);
    }
    Ok(())
}

/// The declared-state tracking's expected values: the initialization that the boundary adopted, the Manual command that tracks the
/// committed output, the aligned references and commands, and the states as the boundary applied them, or as the scan observed them
/// when the scan may move them (`apply` of `runtimeTransfer.ts`, E6.4, A6.4).
fn declared_values(
    binding: &TransferBinding,
    tracking: &DeclaredStateTracking,
    transaction: &TransferTransaction,
    expected: &mut BTreeMap<String, f64>,
) -> Result<(), Failure> {
    let applied = transaction
        .applied_values
        .as_ref()
        .filter(|values| holds_states(binding, values))
        .ok_or_else(|| {
            Failure::new(
                &texts::transfer_declared_states_missing(),
                Cause::Runtime,
                NextAction::None,
            )
        })?;
    let initial = transaction.initial_values.as_ref().ok_or_else(|| {
        Failure::new(
            &texts::transfer_snapshot_unavailable(),
            Cause::Runtime,
            NextAction::None,
        )
    })?;
    adopted_initialization(binding, tracking, applied, initial)?;
    let basis = output_basis(binding)?;
    if tracking.align_output {
        let reference = state_member(binding, &tracking.reference_symbol)?;
        let measured =
            rounded_transfer_value(reference, number_of(initial, &tracking.process_variable))?;
        if !applied_is(applied, &tracking.reference_symbol, measured) {
            return Err(Failure::new(
                &texts::transfer_auto_reference_mismatch(),
                Cause::Runtime,
                NextAction::None,
            ));
        }
    }
    let manual = state_member(binding, &tracking.manual_command_symbol)?;
    let held = rounded_transfer_value(manual, number_of(initial, basis))?;
    if !applied_is(applied, &tracking.manual_command_symbol, held) {
        return Err(Failure::new(
            &texts::transfer_manual_not_tracking(),
            Cause::Runtime,
            NextAction::None,
        ));
    }
    for channel in &tracking.additional_references {
        if tracking.align_output {
            let member = state_member(binding, &channel.reference_symbol)?;
            let measured =
                rounded_transfer_value(member, number_of(initial, &channel.process_variable))?;
            if !applied_is(applied, &channel.reference_symbol, measured) {
                return Err(Failure::new(
                    &texts::transfer_additional_reference_mismatch(),
                    Cause::Runtime,
                    NextAction::None,
                ));
            }
        }
    }
    for channel in &tracking.additional_commands {
        let member = state_member(binding, &channel.manual_command_symbol)?;
        let output = binding
            .tracking_outputs
            .get(&channel.output_symbol)
            .unwrap_or(&channel.output_symbol);
        let command = rounded_transfer_value(member, number_of(initial, output))?;
        if !applied_is(applied, &channel.manual_command_symbol, command) {
            return Err(Failure::new(
                &texts::transfer_additional_command_mismatch(),
                Cause::Runtime,
                NextAction::None,
            ));
        }
    }
    let observed = transaction.observed_values.as_ref();
    for member in &binding.states {
        let actual = observed
            .and_then(|readings| readings.get(&member.symbol))
            .and_then(|reading| reading.as_number())
            .filter(|actual| *actual >= member.minimum && *actual <= member.maximum);
        let Some(actual) = actual else {
            return Err(Failure::new(
                &texts::transfer_observed_state_invalid(&member.symbol),
                Cause::Runtime,
                NextAction::None,
            ));
        };
        let value = if tracking.bounded_state_symbols.contains(&member.symbol) {
            actual
        } else {
            applied_value(applied, &member.symbol)?
        };
        expected.insert(member.symbol.clone(), value);
    }
    Ok(())
}

/// The initialization that the boundary adopted for each declared state (`apply` of `runtimeTransfer.ts`, C0): a formula's value when
/// the tracking aligns the output, the requested value when it aligns without a formula, and the value the boundary held otherwise.
fn adopted_initialization(
    binding: &TransferBinding,
    tracking: &DeclaredStateTracking,
    applied: &BTreeMap<String, f64>,
    initial: &BTreeMap<String, TransferValue>,
) -> Result<(), Failure> {
    for (symbol, requested) in &tracking.initialization_values {
        let formula = tracking
            .initialization_formulas
            .as_ref()
            .and_then(|table| table.get(symbol));
        let adopted = if tracking.align_output {
            match formula {
                Some(formula) => {
                    let member = state_member(binding, symbol)?;
                    rounded_transfer_value(
                        member,
                        evaluate_initialization_formula(binding, formula, initial)?,
                    )?
                }
                None => *requested,
            }
        } else {
            number_of(initial, symbol)
        };
        if !applied_is(applied, symbol, adopted) {
            return Err(Failure::new(
                &texts::transfer_initial_not_adopted(symbol),
                Cause::Runtime,
                NextAction::None,
            ));
        }
    }
    Ok(())
}

/// The values the boundary kept for a tracking's inputs: the transaction's initial values, which must hold the binding's members.
fn retained_inputs<'a>(
    binding: &TransferBinding,
    transaction: &'a TransferTransaction,
) -> Result<&'a BTreeMap<String, TransferValue>, Failure> {
    transaction
        .initial_values
        .as_ref()
        .filter(|values| holds_declared_values(binding, values))
        .ok_or_else(|| {
            Failure::new(
                &texts::transfer_boundary_inputs_missing(),
                Cause::Runtime,
                NextAction::None,
            )
        })
}

/// The symbol whose value the output is held at: its feedback guard when the binding tracks one, the output otherwise.
fn output_basis(binding: &TransferBinding) -> Result<&str, Failure> {
    transfer_output_basis(binding, &binding.output)
        .map(|member| member.symbol.as_str())
        .ok_or_else(configure_binding_failure)
}

/// The state of `binding` named `symbol`.
fn state_member<'a>(
    binding: &'a TransferBinding,
    symbol: &str,
) -> Result<&'a TransferMember, Failure> {
    binding
        .states
        .iter()
        .find(|member| member.symbol == symbol)
        .ok_or_else(configure_binding_failure)
}

/// The failure of a binding that does not name the members a tracking needs.
fn configure_binding_failure() -> Failure {
    Failure::new(
        &texts::transfer_configure_binding(),
        Cause::Implementation,
        NextAction::OpenLoopSetup,
    )
}

/// The number that `values` holds for `symbol`; NaN when it holds none, so that the comparisons that follow fail.
fn number_of(values: &BTreeMap<String, TransferValue>, symbol: &str) -> f64 {
    values
        .get(symbol)
        .and_then(|reading| reading.as_number())
        .unwrap_or(f64::NAN)
}

/// The value the transfer applied for `symbol`.
fn applied_value(applied: &BTreeMap<String, f64>, symbol: &str) -> Result<f64, Failure> {
    applied.get(symbol).copied().ok_or_else(|| {
        Failure::new(
            &texts::transfer_pid_applied_incomplete(),
            Cause::Runtime,
            NextAction::None,
        )
    })
}

/// Whether the transfer applied `value` for `symbol`, exactly.
fn applied_is(applied: &BTreeMap<String, f64>, symbol: &str, value: f64) -> bool {
    applied
        .get(symbol)
        .is_some_and(|actual| same_number(*actual, value))
}

/// Requires that the applied values are the expected ones: the same symbols, each with the expected value (`Object.keys` and `!==` of
/// `apply`).
fn require_applied(
    applied: Option<&BTreeMap<String, f64>>,
    expected: &BTreeMap<String, f64>,
    text: &str,
) -> Result<(), Failure> {
    let agrees = applied.is_some_and(|applied| {
        applied.len() == expected.len()
            && expected
                .iter()
                .all(|(symbol, value)| applied_is(applied, symbol, *value))
    });
    if agrees {
        Ok(())
    } else {
        Err(Failure::new(text, Cause::Runtime, NextAction::None))
    }
}

/// The observed mode and states after the boundary and the scan: the mode is the reviewed one, every further controller's mode follows
/// it, a parallel PID's states stay within their bounds, and each expected value is the value the scan observed (`apply` of
/// `runtimeTransfer.ts`, E6.7).
fn check_observed(
    binding: &TransferBinding,
    manual: bool,
    tracking: Option<&TransferTracking>,
    expected: &BTreeMap<String, f64>,
    observed: Option<&BTreeMap<String, TransferValue>>,
) -> Result<(), Failure> {
    let mismatch = || {
        Failure::new(
            &texts::transfer_observed_mismatch(),
            Cause::Runtime,
            NextAction::None,
        )
    };
    let observed = observed.ok_or_else(mismatch)?;
    let mode = if manual {
        binding.mode.manual_value
    } else {
        !binding.mode.manual_value
    };
    let mode_matches = observed.get(&binding.mode.symbol) == Some(&TransferValue::Boolean(mode));
    let further_modes_match = binding.additional_modes.iter().all(|further| {
        let expected_mode = if manual {
            further.manual_value
        } else {
            !further.manual_value
        };
        observed.get(&further.symbol) == Some(&TransferValue::Boolean(expected_mode))
    });
    let pid_states_settled = match tracking {
        Some(TransferTracking::ParallelPid(_)) => binding.states.iter().all(|member| {
            observed
                .get(&member.symbol)
                .and_then(|value| value.as_number())
                .is_some_and(|value| {
                    value.is_finite() && value >= member.minimum && value <= member.maximum
                })
        }),
        _ => true,
    };
    if !mode_matches || !further_modes_match || !pid_states_settled {
        return Err(mismatch());
    }
    // A parallel PID checks only the reference and the Manual command exactly; the other states were checked by their bounds.
    let watched = |symbol: &str| match tracking {
        Some(TransferTracking::ParallelPid(pid)) => {
            symbol == pid.reference_symbol || symbol == pid.manual_command_symbol
        }
        _ => true,
    };
    for (symbol, value) in expected
        .iter()
        .filter(|(symbol, _)| watched(symbol.as_str()))
    {
        let same = observed
            .get(symbol)
            .and_then(|actual| actual.as_number())
            .is_some_and(|actual| same_number(actual, *value));
        if !same {
            return Err(mismatch());
        }
    }
    Ok(())
}

/// Whether two values are the same number. The comparison is exact on purpose: the runtime reports the values it applied, and a changed
/// bit is a changed value (annex E0.3).
#[allow(clippy::float_cmp)] // The exact comparison is the rule: the applied value must be the reviewed one.
fn same_number(left: f64, right: f64) -> bool {
    left == right
}

#[cfg(test)]
#[path = "transfer_protocol_tests.rs"]
mod tests;
