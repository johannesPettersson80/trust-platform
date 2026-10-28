//! The sentences of the runtime transactions (protocol annex E6, E7) and the commissioning checkpoint (A8.5), re-exported by
//! `commissioning::texts`.

/// A transfer status the runtime answered that does not fit its binding (E6.3).
#[must_use]
pub fn transfer_status_invalid() -> String {
    "The runtime returned invalid controller-transfer status.".to_owned()
}

/// A transfer outcome whose evidence does not fit its binding (E6.8).
#[must_use]
pub fn transfer_outcome_invalid() -> String {
    "The runtime returned invalid transfer outcome evidence.".to_owned()
}

/// A transfer that did not apply and gave no reason, by its terminal status (E6.8).
#[must_use]
pub fn transfer_rejected_without_reason(status: &str) -> String {
    format!("Transfer {status}.")
}

/// A failed transfer the runtime reverted in the same scan (A6.3, E6.7), after the runtime's own clause.
#[must_use]
pub fn transfer_reverted(clause: &str) -> String {
    format!("{clause} The runtime reverted the transfer in the same scan, so the loop kept its mode and values.")
}

/// The runtime reports another transaction than the one this engine owns (E11.4).
#[must_use]
pub fn transfer_identity_changed() -> String {
    "The owned controller-transfer identity changed; confirmation is unresolved.".to_owned()
}

/// A `parallel-pid-v1` transfer whose applied states are incomplete (E6.4).
#[must_use]
pub fn transfer_pid_applied_incomplete() -> String {
    "Applied PID initialization states are incomplete or nonfinite.".to_owned()
}

/// A `declared-state-v1` transfer that kept no applied states (E6.4).
#[must_use]
pub fn transfer_declared_states_missing() -> String {
    "Declared initialization states were not retained at the task boundary.".to_owned()
}

/// A transfer whose boundary snapshot is missing (E6.6).
#[must_use]
pub fn transfer_snapshot_unavailable() -> String {
    "The initialization snapshot is unavailable.".to_owned()
}

/// An Auto entry whose reference does not match the boundary measurement (E6.4).
#[must_use]
pub fn transfer_auto_reference_mismatch() -> String {
    "Auto-entry reference did not match the task-boundary measurement.".to_owned()
}

/// A Manual command that did not take the committed output (E6.4).
#[must_use]
pub fn transfer_manual_not_tracking() -> String {
    "Manual output did not track the committed output.".to_owned()
}

/// A further reference that did not align with its measurement (E6.4).
#[must_use]
pub fn transfer_additional_reference_mismatch() -> String {
    "An additional reference did not align with its measured process value.".to_owned()
}

/// A further Manual command that did not take its committed output (E6.4).
#[must_use]
pub fn transfer_additional_command_mismatch() -> String {
    "An additional Manual command did not track its committed output.".to_owned()
}

/// A state observed after the transfer that is missing or outside its bounds, by its symbol (E6.7).
#[must_use]
pub fn transfer_observed_state_invalid(symbol: &str) -> String {
    format!("Observed initialization state {symbol} is invalid.")
}

/// A requested initialization the runtime did not adopt, by its symbol (E6.4).
#[must_use]
pub fn transfer_initial_not_adopted(symbol: &str) -> String {
    format!("Initialization state {symbol} was not adopted coherently.")
}

/// A proportional transfer whose boundary inputs were not kept (E6.4).
#[must_use]
pub fn transfer_boundary_inputs_missing() -> String {
    "Boundary tracking inputs were not retained.".to_owned()
}

/// A proportional transfer whose applied values disagree with its boundary inputs (E6.4).
#[must_use]
pub fn transfer_applied_tracking_disagrees() -> String {
    "Applied tracking values disagree with the retained boundary inputs.".to_owned()
}

/// The mode or states observed after the transfer differ from the reviewed ones (E6.7).
#[must_use]
pub fn transfer_observed_mismatch() -> String {
    "The observed transfer does not match the reviewed mode and state.".to_owned()
}

/// A journaled transfer owner that belongs to another program or meets an owned transaction (E11.4).
#[must_use]
pub fn transfer_owner_restore_refused() -> String {
    "Cannot restore a transfer owned by another program or active transaction.".to_owned()
}

/// A new transfer while this engine still owns one (A6.6).
#[must_use]
pub fn transfer_owned_pending() -> String {
    "Resolve the owned transfer before starting another.".to_owned()
}

/// The program changed, or another transfer is pending, before a transfer (E6.5).
#[must_use]
pub fn transfer_program_changed() -> String {
    "The program changed or another transfer is pending.".to_owned()
}

/// A transfer that stayed pending past its confirmation window (E6.7).
#[must_use]
pub fn transfer_unresolved() -> String {
    "Controller transfer confirmation is unresolved. Cancel retries the same owned transaction."
        .to_owned()
}

/// A journaled commissioning checkpoint of another loop or loop revision (A8.5).
#[must_use]
pub fn checkpoint_does_not_match_loop() -> String {
    "The retained commissioning checkpoint does not match this loop. Reopen the original loop to review recovery.".to_owned()
}

/// A retained parameter operation that could not be read, with the reason (E11.4).
#[must_use]
pub fn parameter_reconcile_inspect(operation: &str, reason: &str) -> String {
    format!("Could not inspect the retained {operation}: {reason} Make a fresh review after reconnecting.")
}

/// A retained parameter operation whose program or binding changed (E11.4).
#[must_use]
pub fn parameter_reconcile_program_changed(operation: &str) -> String {
    format!(
        "The program or parameter binding changed since the operation was issued. The {operation} result is unresolved; make a fresh review."
    )
}

/// A retained parameter operation that could not be reconciled, with the reason (E11.4).
#[must_use]
pub fn parameter_reconcile_unreconciled(reason: &str) -> String {
    format!("The retained parameter operation could not be reconciled: {reason}")
}

/// A retained parameter operation without a terminal status (E7.2).
#[must_use]
pub fn parameter_reconcile_no_terminal(operation: &str) -> String {
    format!(
        "The current runtime did not provide a terminal {operation} status. Make a fresh review before dependent work."
    )
}

/// A retained parameter operation whose status changed after it was retained (E11.4).
#[must_use]
pub fn parameter_reconcile_status_changed(operation: &str) -> String {
    format!(
        "The current runtime status changed after the retained {operation}. Make a fresh review before dependent work."
    )
}

/// A runtime result that does not match the record retained before dispatch (E11.4).
#[must_use]
pub fn parameter_reconcile_mismatch() -> String {
    "The runtime result does not match the exact parameter record retained before dispatch. Review current and requested values before further application; the operation was not repeated.".to_owned()
}

/// A retained parameter operation the runtime confirms (E7.4).
#[must_use]
pub fn parameter_reconcile_confirmed(operation: &str) -> String {
    format!("Retained {operation} is confirmed by the current runtime status.")
}

/// A retained parameter operation the runtime rejected, with its reason when it gave one (E7.4).
#[must_use]
pub fn parameter_reconcile_rejected(operation: &str, reason: Option<&str>) -> String {
    format!(
        "Retained {operation} was rejected by the current runtime: {}",
        reason.unwrap_or("no reason supplied")
    )
}

/// A prepared parameter operation cancelled before dispatch (E7.6).
#[must_use]
pub fn parameter_reconcile_cancelled(operation: &str) -> String {
    format!(
        "The prepared {operation} was cancelled before dispatch. No operation was replayed; make a fresh review to apply settings."
    )
}
