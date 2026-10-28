//! The sentences of the proportional law's commissioning and recovery (C13, B6), re-exported by `commissioning::texts`.

/// The description of a proportional campaign before it starts (`description`, `proportionalCommissioning.ts`; B6.3, C13.4): the
/// target the run applies and the duration it observes, and the largest Auto-entry change its allowance accepts, in the output's unit.
#[must_use]
pub fn prop_commission_description(
    target: f64,
    process_unit: &str,
    duration_seconds: f64,
    tolerance: f64,
    output_unit: &str,
) -> String {
    let target = crate::tuning::texts::engineering(target, None);
    let process_unit = crate::tuning::texts::unit_suffix(process_unit);
    let duration = crate::tuning::texts::engineering(duration_seconds, Some(6));
    let tolerance = crate::tuning::texts::engineering(tolerance, Some(8));
    let output_unit = crate::tuning::texts::unit_suffix(output_unit);
    format!(
        "Stage the verified gain in Manual, align the reference, enter Auto, then apply target {target}{process_unit} and observe for {duration} s. Maximum Auto-entry change: {tolerance}{output_unit}. Recovery tracks the observed output in Manual and restores the previous reference and gain."
    )
}

/// The description of the rollback of a proportional campaign (`rollbackDescription`, `proportionalCommissioning.ts`; B6.4, A6.2): the
/// reference it restores, whether the loop returns to Auto, and the largest tracking change its tolerance accepts.
#[must_use]
pub fn prop_commission_rollback_description(
    reference: f64,
    process_unit: &str,
    found_in_auto: bool,
    tolerance: f64,
    output_unit: &str,
) -> String {
    let reference = crate::tuning::texts::engineering(reference, None);
    let process_unit = crate::tuning::texts::unit_suffix(process_unit);
    let return_to_auto = if found_in_auto {
        " and return to Auto"
    } else {
        ""
    };
    let tolerance = crate::tuning::texts::engineering(tolerance, Some(8));
    let output_unit = crate::tuning::texts::unit_suffix(output_unit);
    format!(
        "Return to Manual tracking the current output, restore reference {reference}{process_unit}, then restore the previous gain set{return_to_auto}. Maximum tracking change: {tolerance}{output_unit}. Confirm this recovery on the currently reviewed runtime."
    )
}

/// The reason of a run that passed (`execute`, `proportionalCommissioning.ts`; B6.3, C13.7): the recorded target error, to five
/// fraction digits, in the process unit; the controller stays in Auto.
#[must_use]
pub fn prop_commission_passed(maximum_error: f64, process_unit: &str) -> String {
    let error = crate::tuning::texts::engineering(maximum_error, Some(5));
    let unit = crate::tuning::texts::unit_suffix(process_unit);
    format!("Auto commissioning passed: recorded target error {error}{unit}; controller remains in Auto.")
}

/// The reason of a run the engineer cancelled (`checkCancelled`, `proportionalCommissioning.ts`; A10.4).
#[must_use]
pub fn prop_commission_cancelled() -> String {
    "Commissioning cancelled.".to_owned()
}

/// The sentence of a cancelled campaign whose previous settings are back (`cancel`, `proportionalCommissioning.ts`; B6.4, A6.2): the
/// mode the loop is confirmed in.
#[must_use]
pub fn prop_commission_cancelled_recovered(found_in_auto: bool) -> String {
    let mode = if found_in_auto { "Auto" } else { "Manual" };
    format!("Commissioning cancelled. Recovery confirmed in {mode} with the previous gain set.")
}

/// The outcome of a recovery that reached Manual and the previous gains (`recoveredText`, `proportionalCommissioning.ts`; B6.4, A6.2).
#[must_use]
pub fn prop_commission_recovered(found_in_auto: bool) -> String {
    let mode = if found_in_auto { "Auto" } else { "Manual" };
    format!("Recovery confirmed in {mode}; previous gains restored.")
}

/// The outcome of a recovery that could not be confirmed, with the runtime's or the read's error (`execute`, `cancel`, `rollback`,
/// `proportionalCommissioning.ts`; B6.7: Restore again).
#[must_use]
pub fn prop_commission_recovery_unconfirmed(error: &str) -> String {
    format!("Recovery unconfirmed: {error}")
}

/// The outcome of a failed requirement whose applied gains could not be read back (`execute`, `proportionalCommissioning.ts`; B6.3).
#[must_use]
pub fn prop_commission_settings_unread(error: &str) -> String {
    format!("The applied gains could not be read: {error}")
}

/// The reason of a permitted-range departure once the previous settings are restored (`execute`, `proportionalCommissioning.ts`;
/// B6.3, B6.7). `statement` is the departure's sentence, without its full stop.
#[must_use]
pub fn prop_commission_range_restored(statement: &str) -> String {
    format!("{statement}; the previous settings were restored.")
}

/// The sentence added when the recording could not be stopped (`execute`, `proportionalCommissioning.ts`; A8.5): the cancel retries the
/// owned stop.
#[must_use]
pub fn prop_commission_stop_unconfirmed(error: &str) -> String {
    format!("Recording stop unconfirmed: {error}. Cancel retries the owned Stop.")
}

/// The reason of a campaign whose recovery is unconfirmed when the panel closes (`settleForClose`, `proportionalCommissioning.ts`;
/// B6.4). `reason` is the campaign's reason so far.
#[must_use]
pub fn prop_commission_close_unconfirmed(reason: &str, error: &str) -> String {
    format!("{reason} Close recovery unconfirmed: {error}")
}

/// The refusal of a recovery review that the campaign does not permit (`run`, `proportionalCommissioning.ts`; B6.4).
#[must_use]
pub fn prop_commission_recovery_only() -> String {
    "This fresh review permits recovery only, not Auto commissioning.".to_owned()
}

/// The refusal of a second start of a reviewed campaign (`run`, `proportionalCommissioning.ts`; A6.6).
#[must_use]
pub fn prop_commission_already_started() -> String {
    "The reviewed commissioning campaign has already started.".to_owned()
}

/// The refusal of a runtime that cannot track a proportional reference at a task boundary (`review`, `proportionalCommissioning.ts`;
/// C13.2, E6.3).
#[must_use]
pub fn prop_commission_tracking_required() -> String {
    "The runtime needs task-boundary P tracking support before Auto commissioning.".to_owned()
}

/// The refusal of a campaign whose parameter operations cannot be recovered after a reload (`setCheckpointWriter`,
/// `proportionalCommissioning.ts`; E7.2).
#[must_use]
pub fn prop_commission_recoverable_required() -> String {
    "Reload the runtime with recoverable parameter operations before commissioning.".to_owned()
}

/// The refusal of a run whose program changed after the review (`review`, `proportionalCommissioning.ts`; B6.7: Validate).
#[must_use]
pub fn prop_commission_program_changed() -> String {
    "The program changed during commissioning review.".to_owned()
}

/// The refusal of a run whose parameter record, mode or reference changed after the review (`execute`, `proportionalCommissioning.ts`;
/// B6.2, B6.7: Validate again).
#[must_use]
pub fn prop_commission_reviewed_changed() -> String {
    "Parameters, mode or reference changed since commissioning review.".to_owned()
}

/// The refusal of staged gains that the program changed before the Auto entry (`execute`, `proportionalCommissioning.ts`; A6.8, B6.7).
/// `names` lists the parameters, comma-separated.
#[must_use]
pub fn prop_commission_staged_changed(names: &str) -> String {
    format!("The staged controller parameters changed before Auto entry: {names}. The program writes them; change the program.")
}

/// The refusal of a parameter record that the program changed after the Auto entry (`execute`, `proportionalCommissioning.ts`; A6.4,
/// A6.8). `names` lists the parameters, comma-separated.
#[must_use]
pub fn prop_commission_held_changed(names: &str) -> String {
    format!("Controller parameters changed during commissioning: {names}. The program writes them; change the program.")
}

/// The refusal of a parameter record whose program revision changed during the run (`execute`, `proportionalCommissioning.ts`; B6.7).
#[must_use]
pub fn prop_commission_held_program() -> String {
    "Controller parameters changed during commissioning: the program changed.".to_owned()
}

/// The refusal of a recovery that the campaign already completed in Manual (`assertRecoveryNeeded`, `proportionalCommissioning.ts`; B6.4).
#[must_use]
pub fn prop_commission_already_recovered() -> String {
    "This commissioning campaign is already recovered in Manual; no further rollback is needed."
        .to_owned()
}

/// The refusal of a historical record without its original reference (`reviewRecovery`, `proportionalCommissioning.ts`; B6.4).
#[must_use]
pub fn prop_commission_reference_unavailable() -> String {
    "The recorded original reference is unavailable.".to_owned()
}

/// The refusal of a runtime whose program or transfer binding differs from the campaign's evidence (`reviewRecovery`,
/// `proportionalCommissioning.ts`; B6.4).
#[must_use]
pub fn prop_commission_binding_differs() -> String {
    "The running program or transfer binding differs from the commissioning evidence.".to_owned()
}

/// The refusal of a gain transaction that no longer matches the review it was made on (`recover`, `proportionalCommissioning.ts`;
/// B6.4: Restore again).
#[must_use]
pub fn prop_commission_gain_transaction() -> String {
    "The running gain transaction no longer matches this recovery review.".to_owned()
}

/// The refusal of a gain that changed while the Manual transfer was reviewed (`recover`, `proportionalCommissioning.ts`; B6.4: Restore
/// again).
#[must_use]
pub fn prop_commission_gain_changed_review() -> String {
    "The gain changed while reviewing the Manual transfer.".to_owned()
}

/// The refusal of a recovery whose program changed (`recover`, `proportionalCommissioning.ts`; A6.7).
#[must_use]
pub fn prop_commission_program_identity() -> String {
    "Program identity changed; recovery cannot write to the new program.".to_owned()
}

/// The refusal of a parameter application that is unresolved, or whose identity changed (`recover`, `proportionalCommissioning.ts`;
/// A6.8: inspect before recovery).
#[must_use]
pub fn prop_commission_parameter_unresolved() -> String {
    "Parameter application is unresolved or its identity changed; inspect before recovery."
        .to_owned()
}

/// The refusal of a retained gain rollback that no longer matches the campaign (`recover`, `proportionalCommissioning.ts`; B6.4: Restore
/// again).
#[must_use]
pub fn prop_commission_rollback_mismatch() -> String {
    "The retained gain rollback no longer matches this campaign.".to_owned()
}
