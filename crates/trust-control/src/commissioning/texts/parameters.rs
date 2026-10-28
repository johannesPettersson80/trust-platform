//! The sentences of the parameter transactions and the parameter review (protocol annex E7, A6.8), re-exported by
//! `commissioning::texts`.

/// Parameter record (`controllerLaw.ts`, `materializePidGainRecord`; C0, C1): the PID record needs its typed adapter first.
#[must_use]
pub fn parameter_pid_record_required() -> String {
    "A typed PID record adapter is required before materializing controller gains.".to_owned()
}

/// Parameter record (`controllerLaw.ts`, `materializePidGainRecord`; C0 "Back-calculation anti-windup"): the tracking gain has no declared term.
#[must_use]
pub fn parameter_anti_windup_required() -> String {
    "A typed PID anti-windup adapter is required before materializing a back-calculation gain."
        .to_owned()
}

/// Parameter record (`controllerLaw.ts`, `materializePidGainRecord`; C0 "Setpoint weight"): a weight other than one has no declared term.
#[must_use]
pub fn parameter_setpoint_weight_required() -> String {
    "A declared proportional setpoint weight is required before materializing a weighted controller.".to_owned()
}

/// Parameter record (`controllerLaw.ts`, `materializePidGainRecord`; C0 "Setpoint weight"): a derivative weight other than zero has no term.
#[must_use]
pub fn parameter_derivative_weight_required() -> String {
    "A declared derivative setpoint weight is required before materializing a controller that differentiates the setpoint.".to_owned()
}

/// Parameter record (`controllerLaw.ts`, `materializePidGainRecord`; C0 "Measurement filter Tf"): a filter time other than zero has no term.
#[must_use]
pub fn parameter_filter_time_required() -> String {
    "A declared measurement filter time is required before materializing a filtered controller."
        .to_owned()
}

/// Parameter record (`controllerLaw.ts`, `materializePidGainRecord`; C0, A11.1): a term the law does not bind, named by its symbol.
#[must_use]
pub fn parameter_not_declared(symbol: &str) -> String {
    format!("PID gain for {symbol} is outside its declared binding.")
}

/// Parameter record (`controllerLaw.ts`, `materializePidGainRecord`; C0, N6.4, A11.1): a value outside its declared binding, with the
/// bounds it is checked against (the magnitudes of a gain).
#[must_use]
pub fn parameter_outside_binding(
    symbol: &str,
    value: f64,
    lowest: f64,
    highest: f64,
    unit: &str,
    gain: bool,
) -> String {
    let suffix = crate::tuning::texts::unit_suffix(unit);
    let shown = crate::tuning::texts::engineering(value, None);
    let low = crate::tuning::texts::engineering(lowest, None);
    let high = crate::tuning::texts::engineering(highest, None);
    let scope = if gain { "the magnitudes" } else { "the bounds" };
    format!(
        "PID gain for {symbol} is outside its declared binding: {shown}{suffix} is not within {scope} {low}{suffix} to {high}{suffix}."
    )
}

/// Parameter record (`controllerLaw.ts`, `materializePidGainRecord`; C0 "REAL rounding", A11.1): a `REAL` value that single precision
/// moves outside its binding.
#[must_use]
pub fn parameter_realized_outside_binding(
    symbol: &str,
    value: f64,
    lowest: f64,
    highest: f64,
    unit: &str,
    gain: bool,
) -> String {
    let suffix = crate::tuning::texts::unit_suffix(unit);
    let shown = crate::tuning::texts::engineering(value, None);
    let low = crate::tuning::texts::engineering(lowest, None);
    let high = crate::tuning::texts::engineering(highest, None);
    let scope = if gain { "the magnitudes" } else { "the bounds" };
    format!(
        "Realized PID gain for {symbol} is outside its declared binding: {shown}{suffix} is not within {scope} {low}{suffix} to {high}{suffix}."
    )
}

/// Binding (`decodeParameterBinding`, parameterBinding.ts; E7.1, C0): a label is blank, too long or holds a control character.
#[must_use]
pub fn parameter_binding_label(field: &str) -> String {
    format!("Invalid parameter binding. The {field} is blank, longer than 1,024 characters, or holds a control character.")
}

/// Binding (`decodeParameterBinding`, parameterBinding.ts; E7.1, C0): a parameter's unit or convention is blank or invalid.
#[must_use]
pub fn parameter_binding_member(field: &str, symbol: &str) -> String {
    format!("Invalid parameter binding. The {field} of {symbol} is blank, longer than 1,024 characters, or holds a control character.")
}

/// Binding (`decodeParameterBinding`, parameterBinding.ts; E7.1): the binding names another loop than the one it is checked for.
#[must_use]
pub fn parameter_binding_loop(loop_id: &str, expected: &str) -> String {
    format!("Invalid parameter binding. It names the loop {loop_id}, not {expected}.")
}

/// Binding (`decodeParameterBinding`, parameterBinding.ts; E7.1, C0): a binding holds one to 64 parameters, and `count` is not in that range.
#[must_use]
pub fn parameter_binding_count(count: usize) -> String {
    format!("Invalid parameter binding. A binding holds one to 64 parameters, not {count}.")
}

/// Binding (`decodeParameterBinding`, parameterBinding.ts; E7.1, C0): a symbol is repeated, or it is the mode symbol.
#[must_use]
pub fn parameter_binding_symbol(symbol: &str) -> String {
    format!("Invalid parameter binding. The symbol {symbol} is repeated, or it is the mode symbol; symbols are compared without regard to case.")
}

/// Binding (`decodeParameterBinding`, parameterBinding.ts; E7.1, C0): the bounds of `symbol` are not finite, or the minimum is above the maximum.
#[must_use]
pub fn parameter_binding_bounds(symbol: &str, minimum: f64, maximum: f64) -> String {
    let minimum_text = crate::tuning::texts::engineering(minimum, None);
    let maximum_text = crate::tuning::texts::engineering(maximum, None);
    format!("Invalid parameter binding. The bounds of {symbol} must be finite, and the minimum may not exceed the maximum; they are {minimum_text} to {maximum_text}.")
}

/// Status (`ParameterApplicationClient.status`, parameterApplication.ts; E7.2, E11.8): the runtime's record does not have the form of annex E7.2.
#[must_use]
pub fn parameter_status_invalid() -> String {
    "Runtime returned invalid parameter status.".to_owned()
}

/// Review (`ParameterApplicationClient.review`, parameterApplication.ts; B6.2, E7.3): the values are not exactly the declared finite parameters.
#[must_use]
pub fn parameter_review_values_invalid() -> String {
    "Provide exactly the declared finite parameter values.".to_owned()
}

/// Review (`ParameterApplicationClient.review`, parameterApplication.ts; B6.2, C0): a value is outside its bounds after IEC rounding, with the value and the bounds.
#[must_use]
pub fn parameter_outside_bounds(
    symbol: &str,
    value: f64,
    minimum: f64,
    maximum: f64,
    unit: &str,
) -> String {
    let value_text = crate::tuning::texts::engineering(value, None);
    let minimum_text = crate::tuning::texts::engineering(minimum, None);
    let maximum_text = crate::tuning::texts::engineering(maximum, None);
    let unit_text = crate::tuning::texts::unit_suffix(unit);
    format!("{symbol} is outside its declared bounds after IEC rounding. The value {value_text}{unit_text} must lie between {minimum_text}{unit_text} and {maximum_text}{unit_text}.")
}

/// Review (`ParameterApplicationClient.review`, parameterApplication.ts; B6.2): the record holds a pending transaction, which is waited for first.
#[must_use]
pub fn parameter_review_pending() -> String {
    "Wait for the pending parameter transaction before review.".to_owned()
}

/// Rollback review (`ParameterApplicationClient.reviewRollback`, parameterApplication.ts; E7.5): no confirmed previous set is retained.
#[must_use]
pub fn parameter_rollback_unavailable() -> String {
    "No confirmed previous parameter set is available for rollback.".to_owned()
}

/// Apply (`ParameterApplicationClient.apply`, parameterApplication.ts; E7.3): the apply has no review of this parameter set.
#[must_use]
pub fn parameter_apply_review_first() -> String {
    "Review this parameter set on the bound runtime first.".to_owned()
}

/// Rollback (`ParameterApplicationClient.rollback`, parameterApplication.ts; E7.5): the rollback has no retained rollback review.
#[must_use]
pub fn parameter_rollback_review_first() -> String {
    "Review the retained rollback values on this runtime first.".to_owned()
}

/// Apply (`ParameterApplicationClient.apply`, parameterApplication.ts; E7.2, E7.3): the runtime does not keep request identities.
#[must_use]
pub fn parameter_apply_identity() -> String {
    "Reload the runtime with parameter-operation identity support before application.".to_owned()
}

/// Rollback (`ParameterApplicationClient.rollback`, parameterApplication.ts; E7.2, E7.5): the runtime does not keep request identities.
#[must_use]
pub fn parameter_rollback_identity() -> String {
    "Reload the runtime with parameter-operation identity support before rollback.".to_owned()
}

/// Physical approval (`PhysicalApprovalRequired`, runtimeTarget.ts, used by parameterApplication.ts; A2.5, E10.2): `operation` names the write.
#[must_use]
pub fn parameter_physical_approval(operation: &str) -> String {
    format!("Physical approval is required for {operation} on a physical target. Review it on the page and approve it immediately before it starts.")
}

/// Confirmation (`ParameterApplicationClient.confirm`, parameterApplication.ts; E7.4): the program or the binding changed during the write.
#[must_use]
pub fn parameter_program_changed() -> String {
    "Runtime program or parameter binding changed during application.".to_owned()
}

/// Confirmation (`ParameterApplicationClient.confirm`, parameterApplication.ts; E7.4): the transaction ended without the reviewed values, and the runtime gave no reason.
#[must_use]
pub fn parameter_not_confirmed() -> String {
    "The reviewed values were not confirmed after controller execution.".to_owned()
}

/// Confirmation (`ParameterApplicationClient.confirm`, parameterApplication.ts; E11.8): the confirmation window ran out; the status is read again.
#[must_use]
pub fn parameter_confirmation_timeout() -> String {
    "Timed out waiting for parameter confirmation; inspect runtime status before retrying."
        .to_owned()
}

/// Cancel (`ParameterApplicationClient.cancelOwner`, parameterApplication.ts; E7.6): the cancel stays unconfirmed for its window.
#[must_use]
pub fn parameter_cancel_unconfirmed() -> String {
    "Parameter cancellation remains unconfirmed; retry the retained operation after reconnecting."
        .to_owned()
}

/// Cancel (`ParameterApplicationClient.cancelOwner`, parameterApplication.ts; E7.6, E11.8): the identity of the operation no longer belongs to its owner.
#[must_use]
pub fn parameter_identity_unresolved() -> String {
    "The parameter-operation identity remains unresolved.".to_owned()
}

/// Review text (`values_of_text`, `controlStudio/parameters`; C12): the review text is missing or longer than 64 kB.
#[must_use]
pub fn parameter_json_too_long() -> String {
    "Enter a parameter JSON object of at most 64 kB.".to_owned()
}

/// Review text (`values_of_text`, `controlStudio/parameters`; C12): the review text is not JSON (a text that the TypeScript reports by the JSON parser's own message).
#[must_use]
pub fn parameter_json_invalid() -> String {
    "The parameter values are not valid JSON.".to_owned()
}

/// Selected analysis (`controlStudio/parameters`, `draft`; C12): the selected analysis values are copied for review; the runtime is not changed.
#[must_use]
pub fn parameter_selected_analysis_draft() -> String {
    "Selected analysis values copied for review. The runtime has not changed.".to_owned()
}

/// The values changed after a review, so the review is dropped: the answer of `controlStudio/parameters` with `invalidate` (C12).
#[must_use]
pub fn parameter_edited() -> String {
    "Values changed. Review again before applying.".to_owned()
}

/// The progress of a parameter write of `controlStudio/parameters` (C12, E7.4): the write waits for its task boundary and the values the
/// runtime then reports.
#[must_use]
pub fn parameter_applying_reason() -> String {
    "Waiting for the owning task and observed values…".to_owned()
}

/// Confirmed write (`applied_reason`; C12, B6.4): a rollback is applied and its values were observed.
#[must_use]
pub fn parameter_applied_rollback() -> String {
    "Previous values restored and observed after ST execution.".to_owned()
}

/// Confirmed write (`applied_reason`; C12, B6.4): an Apply is applied and its values were observed.
#[must_use]
pub fn parameter_applied_apply() -> String {
    "Reviewed values observed after ST execution.".to_owned()
}
