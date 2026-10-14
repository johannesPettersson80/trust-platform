//! The sentences of Restore previous settings from a journaled commissioning checkpoint (specification 35, B6.4, A6.2, A11.1):
//! the review's description and its mode clause, the statement of the mode a Restore left the loop in, the confirmed Restore's
//! reason, the evidence of a recording the runtime no longer holds, and the refusals of the checkpoint rules. The wording is that
//! of `interruptedCommissioningRecovery.ts` (`rollbackDescription`, `modeClause`, `modeStatement` and its refusals),
//! `engineerActions/recovery.ts` (the restarted runtime), `engineerActions/application.ts` (the restored reason) and
//! `applyObservation.ts` (`stopRecovered`), with the values involved named (A11.1).

use std::collections::BTreeMap;

use crate::commissioning::checkpoint::CheckpointStage;
use crate::commissioning::found_mode::FoundMode;
use crate::tuning::texts::engineering;

/// There is nothing to restore: the loop's operation journal holds no checkpoint of an Apply (B6.4, A8.5).
#[must_use]
pub fn no_applied_settings(loop_id: &str) -> String {
    format!(
        "There are no applied settings to restore: the operation journal of loop {loop_id} holds no commissioning checkpoint of an Apply. Nothing was written."
    )
}

/// There is nothing to restore: the checkpoint's previous settings are back already, or the checkpoint is kept as history because
/// the runtime was replaced (B6.4, A8.5).
#[must_use]
pub fn checkpoint_closed(checkpoint_id: &str, retired: bool) -> String {
    if retired {
        format!(
            "There are no applied settings to restore: checkpoint {checkpoint_id} is kept as history, because the runtime was replaced after its Apply. Nothing was written."
        )
    } else {
        format!(
            "There are no applied settings to restore: the previous settings of checkpoint {checkpoint_id} are restored already. Nothing was written."
        )
    }
}

/// Restore is unavailable because the runtime was restarted or its program reloaded since Apply (B6.4, A6.7; the restarted runtime
/// of `reviewInterruptedRollback`, engineerActions/recovery.ts): the program revision Apply wrote to and the one the runtime runs now.
#[must_use]
pub fn runtime_replaced(applied_program: &str, current_program: &str) -> String {
    format!(
        "Restore is unavailable: the runtime was restarted or its program reloaded since Apply (program revision {applied_program} at Apply, {current_program} now), so nothing of the checkpoint can be restored. The checkpoint is kept as history, and the loop continues from a new validation."
    )
}

/// The checkpoint is not the current loop's (`review`, interruptedCommissioningRecovery.ts; B6.4): the loop revision it was written
/// for and the loop's revision now, or, when they agree, that its transfer binding or recovery tracking differs.
#[must_use]
pub fn checkpoint_mismatch(checkpoint_revision: &str, loop_revision: &str) -> String {
    if checkpoint_revision == loop_revision {
        format!(
            "The commissioning checkpoint does not match the current controller binding: its transfer binding or recovery tracking differs from that of the loop at revision {loop_revision}. Restore is unavailable; nothing was written."
        )
    } else {
        format!(
            "The commissioning checkpoint does not match the current controller binding: it was written for loop revision {checkpoint_revision}, and the loop is at revision {loop_revision}. Restore is unavailable; nothing was written."
        )
    }
}

/// Another program or transfer owns the runtime's transfer (`review`, interruptedCommissioningRecovery.ts; B6.4, E6.3): the program
/// and binding revisions the runtime reports, and its last transaction.
#[must_use]
pub fn transfer_owned_elsewhere(program: &str, binding: &str, transaction: Option<&str>) -> String {
    let transaction = transaction.map_or_else(
        || "no transaction".to_owned(),
        |id| format!("transaction {id}"),
    );
    format!(
        "Another program or transfer owns the current runtime; recovery cannot overwrite it. The runtime's transfer reports program revision {program}, binding revision {binding} and {transaction}. Nothing was written."
    )
}

/// After the cancels, a transfer transaction is still pending (`rollback`, interruptedCommissioningRecovery.ts; E6.10, E11.7): the
/// transaction, when the runtime names it.
#[must_use]
pub fn transfer_pending(transaction: Option<&str>) -> String {
    let transaction = transaction.map_or_else(
        || "the last transfer transaction".to_owned(),
        |id| format!("transfer transaction {id}"),
    );
    format!(
        "Controller transfer ownership remains unresolved: {transaction} is still pending. Restore again once the runtime has ended it."
    )
}

/// After the cancels, the transfer belongs to another program (`rollback`, interruptedCommissioningRecovery.ts; A6.7, E11.7): the program
/// revision Apply found and the one the transfer runs on.
#[must_use]
pub fn transfer_program_changed(origin_program: &str, current_program: &str) -> String {
    format!(
        "Controller transfer ownership remains unresolved: the transfer runs on program revision {current_program}, not {origin_program}. Nothing further was written."
    )
}

/// A state that the Manual transfer holds is not reported as a finite number (`recoveryStates`, interruptedCommissioningRecovery.ts;
/// E6.3).
#[must_use]
pub fn recovery_state_unavailable(symbol: &str) -> String {
    format!("Recovery state {symbol} is unavailable.")
}

/// The runtime's retained rollback no longer returns to the record Apply found (`rollback`, interruptedCommissioningRecovery.ts; E7.5):
/// the values the runtime would roll back to, and the values Apply found.
#[must_use]
pub fn rollback_record_changed(
    rollback: &BTreeMap<String, f64>,
    origin: &BTreeMap<String, f64>,
) -> String {
    format!(
        "The original parameter record is no longer the runtime's rollback record: the runtime would roll back to {}, and Apply found {}. Nothing further was written.",
        values_text(rollback),
        values_text(origin)
    )
}

/// The description of a Restore's review (`rollbackDescription`, interruptedCommissioningRecovery.ts; B6.4, A6.2): a completed Apply
/// returns to Manual at the current output, an interrupted one is recovered in Manual, and the clause says which mode the loop ends
/// in.
#[must_use]
pub fn restore_description(
    stage: CheckpointStage,
    found: Option<FoundMode>,
    reenterable: bool,
) -> String {
    let base = if stage == CheckpointStage::Completed {
        "Return to Manual at the current output and restore the previous settings."
    } else {
        "Recover the interrupted operation in Manual and restore the previous settings."
    };
    format!("{base}{}", mode_clause(found, reenterable))
}

/// What a Restore's review says about the mode it leaves the loop in (`modeClause`, interruptedCommissioningRecovery.ts; A6.2): nothing
/// for a loop found in Manual, the return to Auto for a loop found in Auto whose controller has an Auto entry, and why the loop stays
/// in Manual otherwise.
#[must_use]
pub fn mode_clause(found: Option<FoundMode>, reenterable: bool) -> &'static str {
    match found {
        None => " The mode the loop was found in is unknown, so it stays in Manual.",
        Some(FoundMode::Manual) => "",
        Some(FoundMode::Auto) if reenterable => {
            " Then the loop returns to Auto, the mode it was found in."
        }
        Some(FoundMode::Auto) => {
            " Then the loop stays in Manual: its Auto entry needs the controller initialization, which this recovery does not hold."
        }
    }
}

/// The mode a confirmed Restore left the loop in, as its reason states it (`modeStatement`, interruptedCommissioningRecovery.ts; A6.2).
#[must_use]
pub fn mode_statement(
    found: Option<FoundMode>,
    reenterable: bool,
    returned_to_auto: bool,
) -> &'static str {
    if returned_to_auto {
        return "the loop returned to Auto";
    }
    match found {
        None => "the loop stays in Manual, because the mode it was found in is unknown",
        Some(FoundMode::Auto) if !reenterable => {
            "the loop stays in Manual, because its Auto entry needs the controller initialization, which this recovery does not hold"
        }
        Some(_) => "the loop stays in Manual",
    }
}

/// The reason of a confirmed Restore (the restore's `confirm`, engineerActions/application.ts; B6.4): the previous values are back and
/// observed after the controller executed, and `statement` names the mode the loop is in.
#[must_use]
pub fn restored_reason(statement: &str) -> String {
    format!("Previous values restored and observed after ST execution; {statement}.")
}

/// Why the evidence of a recording the runtime no longer holds is incomplete, with the runtime's own sentence (`stopRecovered`,
/// applyObservation.ts; A8.6).
#[must_use]
pub fn recording_not_held(runtime_reason: &str) -> String {
    format!("The runtime no longer holds this recording. {runtime_reason}")
}

/// Parameter values as a sentence names them: each symbol with its value, comma-separated.
fn values_text(values: &BTreeMap<String, f64>) -> String {
    values
        .iter()
        .map(|(symbol, value)| format!("{symbol} {}", engineering(*value, None)))
        .collect::<Vec<_>>()
        .join(", ")
}
