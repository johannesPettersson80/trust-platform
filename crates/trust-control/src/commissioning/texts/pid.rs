//! The sentences of the PID law's commissioning and recovery (C1, C2, B6), re-exported by `commissioning::texts`.

/// PID operating state (`pidOperatingState`, pidOperatingState.ts; C1): the proportional gain is needed to start the history at the
/// operating point, and it is not finite.
#[must_use]
pub fn pid_state_gain_required() -> String {
    "A finite PID gain is required to initialize operating-point history.".to_owned()
}

/// PID initialization plan (`pidInitializationPlan`, pidInitializationPlan.ts; C1): the candidate's proportional gain is not
/// available for the state initialization.
#[must_use]
pub fn pid_state_gain_unavailable() -> String {
    "The candidate proportional gain is unavailable for PID state initialization.".to_owned()
}

/// PID initialization plan (`pidInitializationPlan`, pidInitializationPlan.ts; C0, C1): the initialization adapter does not fit the
/// compiled reference, mode or Manual command.
#[must_use]
pub fn pid_state_incompatible() -> String {
    "The declared PI/PID initialization adapter is incompatible with the compiled reference, mode or Manual command.".to_owned()
}

/// PID initialization plan (`pidInitializationPlan`, pidInitializationPlan.ts; A6.3): a value of the PI/PID start is not inside its
/// IEC range after rounding, which the Manual command and the states must be. The value and the range are named.
#[must_use]
pub fn pid_state_outside_range(
    symbol: &str,
    value: f64,
    minimum: f64,
    maximum: f64,
    unit: &str,
) -> String {
    let suffix = crate::tuning::texts::unit_suffix(unit);
    format!(
        "The declared PI/PID initialization value of {symbol}, {}{suffix}, is outside its range of {} to {}{suffix} after rounding.",
        crate::tuning::texts::engineering(value, None),
        crate::tuning::texts::engineering(minimum, None),
        crate::tuning::texts::engineering(maximum, None),
    )
}

/// The description of a Control.PID run before it starts (`description`, `pidCommissioning.ts`; B6.2, B6.3): the structure's label,
/// the target it applies in the process unit, and the duration it observes.
#[must_use]
pub fn pid_commission_description(
    label: &str,
    target: f64,
    process_unit: &str,
    duration_seconds: f64,
) -> String {
    let target = crate::tuning::texts::engineering(target, None);
    let unit = crate::tuning::texts::unit_suffix(process_unit);
    let duration = crate::tuning::texts::engineering(duration_seconds, Some(6));
    format!(
        "Apply the reviewed {label} settings and start Auto at {target}{unit}. Monitor for {duration} s."
    )
}

/// The description of the rollback of a Control.PID run (`rollbackDescription`, `pidCommissioning.ts`; B6.4, A6.2): the reference it
/// restores, the structure's record it restores, and whether the loop returns to Auto.
#[must_use]
pub fn pid_commission_rollback_description(
    label: &str,
    reference: f64,
    process_unit: &str,
    found_in_auto: bool,
) -> String {
    let reference = crate::tuning::texts::engineering(reference, None);
    let unit = crate::tuning::texts::unit_suffix(process_unit);
    let return_to_auto = if found_in_auto {
        " and return to Auto"
    } else {
        ""
    };
    format!(
        "Return to Manual at the current applied output, restore reference {reference}{unit}, then restore the previous {label} record{return_to_auto}. Confirm recovery on the reviewed runtime."
    )
}

/// The reason of a Control.PID run that passed (`execute`, `pidCommissioning.ts`; B6.3, C1): the structure's label, the recorded target
/// error to five fraction digits in the process unit, and that the controller stays in Auto.
#[must_use]
pub fn pid_commission_passed(label: &str, maximum_error: f64, process_unit: &str) -> String {
    let error = crate::tuning::texts::engineering(maximum_error, Some(5));
    let unit = crate::tuning::texts::unit_suffix(process_unit);
    format!("{label} Auto commissioning passed: recorded target error {error}{unit}; controller remains in Auto.")
}

/// The reason of a Control.PID run the engineer cancelled (`checkCancelled`, `pidCommissioning.ts`; A10.4).
#[must_use]
pub fn pid_commission_cancelled() -> String {
    "Controller commissioning cancelled.".to_owned()
}

/// The refusal of a second start of a reviewed Control.PID run (`run`, `pidCommissioning.ts`; A6.6): a campaign starts once.
#[must_use]
pub fn pid_commission_already_started() -> String {
    "The reviewed controller commissioning campaign has already started.".to_owned()
}

/// The refusal of a runtime that does not support the tracking a Control.PID run needs (`review`, `pidCommissioning.ts`; E6.3). The
/// sentence names the structure's tracking kind, as the TypeScript does, whichever of its required features is missing.
#[must_use]
pub fn pid_commission_tracking_required(tracking_kind: &str) -> String {
    format!("The runtime needs {tracking_kind} state tracking before Auto commissioning.")
}

/// The refusal of a Control.PID run whose program changed after the review (`review`, `pidCommissioning.ts`; B6.7: Validate).
#[must_use]
pub fn pid_commission_program_changed() -> String {
    "The program changed during PID commissioning review.".to_owned()
}

/// The refusal of a Control.PID run whose parameter record, mode or reference changed after the review (`execute`,
/// `pidCommissioning.ts`; B6.2, B6.7: make a fresh review).
#[must_use]
pub fn pid_commission_reviewed_changed() -> String {
    "Reviewed parameters, mode or reference changed before commissioning; make a fresh review."
        .to_owned()
}

/// The refusal of a saved Control.PID campaign that already recovered in Manual (`assertRecoveryNeeded`, `pidCommissioning.ts`; B6.4).
#[must_use]
pub fn pid_commission_already_recovered() -> String {
    "This controller commissioning campaign is already recovered in Manual; no further rollback is needed.".to_owned()
}

/// The refusal of a Control.PID recovery whose program changed (`recover`, `pidCommissioning.ts`; A6.7): no write goes to the new program.
#[must_use]
pub fn pid_commission_program_identity() -> String {
    "Program identity changed; PID recovery cannot write to the new program.".to_owned()
}

/// The refusal of a Control.PID recovery whose transfer identity changed, or whose transfer result is still pending (`reviewRecovery`,
/// `pidCommissioning.ts`; B6.4).
#[must_use]
pub fn pid_commission_identity_changed() -> String {
    "Controller transfer identity changed or its result remains pending.".to_owned()
}

/// The refusal of a saved Control.PID campaign whose original reference `symbol` its first transaction does not hold (`reviewRecovery`,
/// `pidCommissioning.ts`; B6.4).
#[must_use]
pub fn pid_commission_reference_unavailable(symbol: &str) -> String {
    format!("Original commissioning reference {symbol} is unavailable.")
}

/// The refusal of a saved Control.PID campaign that the reviewed runtime does not match (`reviewRecovery`, `pidCommissioning.ts`; B6.4):
/// the program, binding, active revision or values, or the model, changed since the campaign.
#[must_use]
pub fn pid_commission_saved_mismatch() -> String {
    "The saved controller campaign does not match the reviewed runtime and rollback values."
        .to_owned()
}

/// The refusal of a Control.PID recovery whose parameter record another operation owns (`recover`, `pidCommissioning.ts`; A6.8, B6.4).
#[must_use]
pub fn pid_commission_ownership_changed() -> String {
    "Parameter ownership changed; recovery will not overwrite another operation's values."
        .to_owned()
}

/// The refusal of a Control.PID parameter recovery that is unresolved, or whose identity changed (`recover`, `pidCommissioning.ts`;
/// A6.8).
#[must_use]
pub fn pid_commission_parameter_unresolved() -> String {
    "Controller parameter recovery is unresolved or its identity changed.".to_owned()
}

/// The refusal of a retained Control.PID rollback that no longer matches the campaign (`recover`, `pidCommissioning.ts`; B6.4).
#[must_use]
pub fn pid_commission_rollback_mismatch() -> String {
    "The retained controller rollback no longer matches this campaign.".to_owned()
}

/// The outcome of a failed requirement whose applied settings could not be read back (`execute`, `pidCommissioning.ts`; B6.3). `error`
/// is the read's own error.
#[must_use]
pub fn pid_commission_settings_unread(error: &str) -> String {
    format!("The applied settings could not be read: {error}")
}

/// The outcome of a Control.PID recovery that reached Manual and the previous record, when a Restore is confirmed (`rollback`,
/// `pidCommissioning.ts`; B6.4). The mode is the one the loop was found in.
#[must_use]
pub fn pid_commission_rollback_confirmed(found_in_auto: bool) -> String {
    let mode = if found_in_auto { "Auto" } else { "Manual" };
    format!("Controller recovery confirmed in {mode}; previous parameters restored.")
}

/// The outcome of a cancelled Control.PID run whose recovery and recording stop are both confirmed (`cancel`, `pidCommissioning.ts`;
/// B6.4). The mode is the one the loop was found in.
#[must_use]
pub fn pid_commission_cancel_confirmed(found_in_auto: bool) -> String {
    let mode = if found_in_auto { "Auto" } else { "Manual" };
    format!("Controller recovery and recording stop confirmed in {mode}.")
}

/// The outcome of a cancel whose recovery stays unresolved (`cancel`, `pidCommissioning.ts`; B6.7). `error` is the failure of the
/// recovery; Restore is offered again.
#[must_use]
pub fn pid_commission_cancel_unresolved(error: &str) -> String {
    format!("Controller recovery remains unresolved: {error}")
}
