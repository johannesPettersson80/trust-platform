//! The sentences of the commissioning rules (A11.1: each names the values involved). Each rule decides in its own
//! module; these functions say what it decided, in the words of the TypeScript sentences they replace.

mod parameters;
mod pid;
mod proportional;
mod transactions;
pub use parameters::*;
pub use pid::*;
pub use proportional::*;
pub use transactions::*;

/// Transfer binding (`proportionalTransferBinding`, `pidTransferBinding`, proportionalTransfer.ts and pidTransfer.ts; A6.3, A11.1):
/// the loop does not name the references, mode or REAL/LREAL manual command that an Auto commissioning needs.
#[must_use]
pub fn transfer_configure_binding() -> String {
    "Configure the compiled reference, mode and manual-command type before Auto commissioning."
        .to_owned()
}

/// Transfer binding (`proportionalTransferBinding`, proportionalTransfer.ts; A6.3): a primary signal is not REAL or LREAL.
#[must_use]
pub fn transfer_primary_not_real() -> String {
    "Auto commissioning requires compiled REAL or LREAL primary signals.".to_owned()
}

/// Transfer binding (`pidTransferBinding`, pidTransfer.ts; A6.3): the PI/PID initialization does not fit the loop's bindings.
#[must_use]
pub fn transfer_pid_incompatible() -> String {
    "The declared PI/PID initialization adapter is incompatible with the compiled parameter, reference, mode or Manual command binding.".to_owned()
}

/// Transfer binding (`pidTransferBinding`, pidTransfer.ts; A6.3): a PID state or signal is not REAL or LREAL.
#[must_use]
pub fn transfer_pid_states_not_real() -> String {
    "PID Auto commissioning requires REAL or LREAL state bindings.".to_owned()
}

/// Transfer binding (`pidInitializationAdapter`, pidInitialization.ts; A6.3, C0): the loop has no explicit `parallel-pid-v1`
/// contract, so the PID states of an Auto entry are not known. The next action is the choice of a method.
#[must_use]
pub fn transfer_pid_adapter_required() -> String {
    "Typed PI/PID initialization adapter is required for Auto commissioning; choose Manual staging after Verify.".to_owned()
}

/// Transfer binding (Rust port, A7 and A6.4): the loop does not set both output limits, which the Auto entry needs as its range.
#[must_use]
pub fn transfer_output_limits_missing() -> String {
    "Auto commissioning needs the output minimum and maximum, and one of them is not set. Set both under Limits.".to_owned()
}

/// Proportional gain (`proportionalParameter`, engineerVerificationPlan.ts): the proportional law, its gain or the setpoint is
/// not set.
#[must_use]
pub fn transfer_proportional_unset() -> String {
    "Confirm the proportional law, gain binding and setpoint before tuning.".to_owned()
}

/// Rounding (`roundedTransferValue`, proportionalTransfer.ts; A6.4, E6.2): a value after IEC rounding is outside the member's
/// declared range. `symbol` names the member.
#[must_use]
pub fn transfer_outside_range(symbol: &str) -> String {
    format!("{symbol} is outside its declared range after IEC rounding.")
}

/// Rounding (`roundedTransferValue`, proportionalTransfer.ts; E6.2): a BOOL transfer state is neither 0 nor 1.
#[must_use]
pub fn transfer_bool_not_binary() -> String {
    "BOOL transfer state must be 0 or 1.".to_owned()
}

/// Reference alignment (`proportionalTrackingReference`, proportionalTransfer.ts; A6.3, E6.4): the gain cannot align the
/// reference, because it is zero or not finite.
#[must_use]
pub fn transfer_gain_cannot_align() -> String {
    "A zero or invalid P gain cannot align the reference for Auto entry.".to_owned()
}

/// Parallel PID tracking (`parallelPidTracking`, pidTransfer.ts; E6.4): the loop has no PID law to track.
#[must_use]
pub fn transfer_pid_law_unavailable() -> String {
    "The declared PID law is unavailable for Auto initialization.".to_owned()
}

/// PID state reading (`pidTransferStates`, pidTransfer.ts; E6.3): the runtime did not report a number for a declared PID state.
/// `symbol` names the state.
#[must_use]
pub fn transfer_pid_state_missing(symbol: &str) -> String {
    format!("The runtime did not return the declared PID state {symbol}.")
}

/// Actuator feedback (`withActuatorFeedback`, transferActuatorFeedback.ts; A5.3, A6.3): a measured actuator is not a typed
/// input in the command's unit.
#[must_use]
pub fn feedback_needs_command_input() -> String {
    "Actuator tracking needs a separate typed input in command coordinates.".to_owned()
}

/// Actuator feedback (`withActuatorFeedback`, transferActuatorFeedback.ts; A6.3): the applied feedback disagrees with a bound
/// measurement of the same symbol.
#[must_use]
pub fn feedback_disagrees() -> String {
    "Applied feedback disagrees with another bound measurement.".to_owned()
}

/// Auto-entry tolerance (`transferTolerance`, proportionalTransfer.ts; A6.4): the reviewed tolerance is not between zero and the
/// output span.
#[must_use]
pub fn entry_tolerance_outside_span() -> String {
    "Auto-entry tolerance must be between zero and the declared output span.".to_owned()
}

/// Auto-entry tolerance (Rust port, A6.4): the loop declares state feedback, whose allowance the Rust engine does not run yet
/// (checklist V7).
#[must_use]
pub fn entry_state_feedback_parked() -> String {
    "This loop uses state feedback, which Control Studio's Rust engine does not run yet.".to_owned()
}

/// Reference hold (`assertReferenceHeld`, commissioningStartingPoint.ts; the cascade rule). `mode_symbol` names the outer
/// controller's mode.
#[must_use]
pub fn start_reference_held(mode_symbol: &str) -> String {
    format!("Put the outer controller in Manual before applying ({mode_symbol}): its output is this loop's setpoint.")
}

/// Starting point (`startingPointRows`, commissioningStartingPoint.ts; A11.1): the reading holds no finite value for `symbol`.
#[must_use]
pub fn start_value_missing(symbol: &str) -> String {
    format!("The runtime did not return a finite {symbol} value.")
}

/// Starting point (`unsettledError`, commissioningStartingPoint.ts; B6.2, B6.7): a plant input is away from its validated point,
/// with the values involved (A11.1). `unit` is the signal's display unit, without a leading space.
#[must_use]
pub fn start_input_away(
    symbol: &str,
    observed: f64,
    expected: f64,
    tolerance: f64,
    unit: &str,
) -> String {
    let unit = crate::tuning::texts::unit_suffix(unit);
    let observed = crate::tuning::texts::engineering(observed, None);
    let expected = crate::tuning::texts::engineering(expected, None);
    let tolerance = crate::tuning::texts::engineering(tolerance, None);
    format!(
        "{symbol} is {observed}{unit}, but the settings were validated from {expected}{unit} ± {tolerance}{unit}. Return the manual output to {expected}{unit} and let the process settle before applying."
    )
}

/// Starting point (`unsettledError`, commissioningStartingPoint.ts; B6.2): a measured output is not yet inside its settling band
/// of the validated point. `unit` is the signal's display unit, without a leading space.
#[must_use]
pub fn start_output_unsettled(
    symbol: &str,
    observed: f64,
    expected: f64,
    tolerance: f64,
    unit: &str,
) -> String {
    let unit = crate::tuning::texts::unit_suffix(unit);
    let observed = crate::tuning::texts::engineering(observed, None);
    let expected = crate::tuning::texts::engineering(expected, None);
    let tolerance = crate::tuning::texts::engineering(tolerance, None);
    format!(
        "{symbol} is {observed}{unit}, but the settings were validated from {expected}{unit} ± {tolerance}{unit}. Wait for the process to settle there before applying."
    )
}

/// Starting point (`settleTimeoutError`, commissioningStartingPoint.ts; B6.2, B6.7): an output did not settle within the
/// process-time bound, in seconds. `unit` is the signal's display unit, without a leading space.
#[must_use]
pub fn start_settle_timeout(
    symbol: &str,
    expected: f64,
    tolerance: f64,
    observed: f64,
    bound_seconds: f64,
    unit: &str,
) -> String {
    let unit = crate::tuning::texts::unit_suffix(unit);
    let expected = crate::tuning::texts::engineering(expected, None);
    let tolerance = crate::tuning::texts::engineering(tolerance, None);
    let observed = crate::tuning::texts::engineering(observed, None);
    let bound_seconds = crate::tuning::texts::engineering(bound_seconds, None);
    format!(
        "{symbol} did not settle at {expected}{unit} ± {tolerance}{unit} within {bound_seconds} s of process time; it is {observed}{unit}. Apply again once it has settled there, or validate again from its current operating point."
    )
}

/// Starting point progress (`progressText`, commissioningStartingPoint.ts): a measured output that has settled, with its
/// validated and its current value. `unit` is the signal's display unit, without a leading space.
#[must_use]
pub fn start_progress_settled(
    symbol: &str,
    expected: f64,
    tolerance: f64,
    observed: f64,
    unit: &str,
) -> String {
    let unit = crate::tuning::texts::unit_suffix(unit);
    let expected = crate::tuning::texts::engineering(expected, None);
    let tolerance = crate::tuning::texts::engineering(tolerance, None);
    let observed = crate::tuning::texts::engineering(observed, None);
    format!("{symbol} settled at {expected}{unit} ± {tolerance}{unit} (now {observed}{unit})")
}

/// Starting point progress (`progressText`, commissioningStartingPoint.ts): a measured output that is still waited for, with its
/// validated and its current value. `unit` is the signal's display unit, without a leading space.
#[must_use]
pub fn start_progress_waiting(
    symbol: &str,
    expected: f64,
    tolerance: f64,
    observed: f64,
    unit: &str,
) -> String {
    let unit = crate::tuning::texts::unit_suffix(unit);
    let expected = crate::tuning::texts::engineering(expected, None);
    let tolerance = crate::tuning::texts::engineering(tolerance, None);
    let observed = crate::tuning::texts::engineering(observed, None);
    format!("Waiting for {symbol} to settle at {expected}{unit} ± {tolerance}{unit} (now {observed}{unit})")
}

/// Starting point progress (`progressText`, commissioningStartingPoint.ts): the outputs' progress lines, already joined with
/// `; `, ended with an ellipsis.
#[must_use]
pub fn start_progress(lines: &str) -> String {
    format!("{lines}…")
}

/// Starting point (`StartingPointWaitCancelled`, commissioningStartingPoint.ts; B6.2): the user ended the wait; nothing was
/// written.
#[must_use]
pub fn start_wait_cancelled() -> String {
    "Apply cancelled while waiting for the process to settle; nothing was written.".to_owned()
}

/// Restore (`restoreUnavailableReason`, restoreAvailability.ts; B6.4): the settings or the program changed after Apply, so
/// Restore would overwrite another change.
#[must_use]
pub fn restore_unavailable() -> String {
    "Restore is unavailable: the controller settings or program changed after Apply, so restoring would overwrite another change. Nothing was written.".to_owned()
}

/// The sentence that names a permitted-range departure, as B6.7 words it, with the value and the range that it left
/// (`permittedRangeStatement`, `applyObservation.ts`; B6.3, B6.7, A11.1). The unit is shown in its engineering notation.
#[must_use]
pub fn permitted_range_statement(
    symbol: &str,
    unit: &str,
    value: f64,
    minimum: f64,
    maximum: f64,
) -> String {
    let suffix = crate::tuning::texts::unit_suffix(
        &crate::identification::recording_import::display_engineering_unit(unit),
    );
    format!(
        "{symbol} left its permitted range ({value_text}{suffix} outside {minimum_text}{suffix}..{maximum_text}{suffix})",
        value_text = crate::tuning::texts::engineering(value, None),
        minimum_text = crate::tuning::texts::engineering(minimum, None),
        maximum_text = crate::tuning::texts::engineering(maximum, None),
    )
}

/// The message of an Apply observation whose recording the runtime no longer holds, with the runtime's own sentence
/// (`ObservationNotHeld`, `applyObservation.ts`; A8.6).
#[must_use]
pub fn apply_observation_not_held(runtime_reason: &str) -> String {
    format!(
        "The runtime no longer holds the recording of this Apply observation, so the observation is incomplete. {runtime_reason}"
    )
}

/// The message of an Apply observation whose panel closed while it was reattached (`ObservationDetached`,
/// `applyObservation.ts`; A8.5, A10.4).
#[must_use]
pub fn apply_observation_detached() -> String {
    "The panel closed while the observation was reattached; the runtime keeps recording it."
        .to_string()
}

/// The message of a commissioning requirement that the observed response does not pass, with its identifier and its reason
/// (`assessCommissioningObservation`, `commissioningObservation.ts`; B6.3).
#[must_use]
pub fn observe_requirement_not_passed(id: &str, reason: &str) -> String {
    format!("Commissioning {id}: {reason}")
}

/// The refusal of an observation whose response holds no scans (`assessCommissioningObservation`,
/// `commissioningObservation.ts`; B6.3).
#[must_use]
pub fn observe_channels_missing() -> String {
    "Commissioning channel observations are missing.".to_string()
}

/// The refusal of a loop whose model does not have one input and one output, so its reference targets cannot be formed
/// (`connectedModelChannels`, `commissioningObservation.ts`; B6.2).
#[must_use]
pub fn observe_model_channels_unmapped() -> String {
    "Map every model input and output in loop channel bindings.".to_string()
}

/// The refusal of a loop with channel bindings, which the Rust engine does not run yet (`commissioningObservation.ts`
/// multichannel branch; checklist V7).
#[must_use]
pub fn observe_channels_parked() -> String {
    "This loop uses channels, which Control Studio's Rust engine does not run yet.".to_string()
}

/// Declared state (`decodeControllerInitialization`, controllerInitialState.ts; C0): the state has no unit, bounds that are not
/// finite and ordered, a BOOL that is not dimensionless in 0 to 1, or an initial value that does not fit its source.
#[must_use]
pub fn init_state_declaration_invalid(symbol: &str) -> String {
    format!("Controller state {symbol} is declared incorrectly: its unit, bounds, type or initial value does not fit.")
}

/// Declared state (`decodeControllerInitialization`, controllerInitialState.ts; C0): two states share a symbol.
#[must_use]
pub fn init_state_duplicate(symbol: &str) -> String {
    format!("Controller state {symbol} is declared twice.")
}

/// Declared state list (`decodeControllerInitialization`, controllerInitialState.ts; C0): more states than one initialization holds.
#[must_use]
pub fn init_states_too_many(count: usize, limit: usize) -> String {
    format!("A controller declares at most {limit} states; this one declares {count}.")
}

/// Declared state source (controllerInitialState.ts; C0, specification 35 §17): the initial value comes from a formula kind that
/// the Rust engine does not run yet (a parked feature, checklist V7).
#[must_use]
pub fn init_parked_source(symbol: &str, source: &str) -> String {
    format!("The initial value of {symbol} comes from {source}, which Control Studio's Rust engine does not run yet.")
}

/// Initialization output (`initializationValues`, controllerInitialState.ts; C0): the model has no output at the state's index.
#[must_use]
pub fn init_output_unavailable(symbol: &str) -> String {
    format!("Initialization output for {symbol} is unavailable.")
}

/// Initialization parameter (`parameter`, controllerInitialState.ts; C0): the gain or weight a value needs is not a bound
/// parameter of the record, or its value is not finite.
#[must_use]
pub fn init_parameter_unavailable(symbol: &str) -> String {
    format!("Initialization parameter {symbol} is not available.")
}

/// Integral initialization (`initializationValues`, controllerInitialState.ts; C0): the integral state must use the actuator's
/// coordinate unit.
#[must_use]
pub fn init_integral_unit(symbol: &str) -> String {
    format!("Integral initialization of {symbol} must use the actuator coordinate unit.")
}

/// Filter or error history (`initializationValues`, controllerInitialState.ts; C0): the state must use the process
/// coordinate unit.
#[must_use]
pub fn init_filter_unit(symbol: &str) -> String {
    format!("Filter or error history {symbol} must use the process coordinate unit.")
}

/// Initialization coordinates (`initializationValues`, controllerInitialState.ts; C0): the model channel does not exist, or its
/// unit is not the state's unit.
#[must_use]
pub fn init_coordinates_units(symbol: &str) -> String {
    format!("Initialization coordinates or units do not match {symbol}.")
}

/// Boolean operating-point initialization (`model-input-is`, controllerInitialState.ts; C0): the state is not dimensionless, the
/// predicate is not a finite level of the actuator, or the model's input is not the one the loop drives.
#[must_use]
pub fn init_boolean_incompatible(symbol: &str) -> String {
    format!("Boolean operating-point initialization is incompatible with {symbol}.")
}

/// Loop channels (`connectedModelChannels`, loopChannels.ts; C0): a loop without channel bindings connects one model input and one
/// output, and the model has another number of them.
#[must_use]
pub fn init_model_channels_unmapped() -> String {
    "Map every model input and output in loop channel bindings.".to_owned()
}

/// Loop channels (specification 35 §17.1, checklist V7): the loop has channel bindings, which the Rust engine does not run yet.
#[must_use]
pub fn init_channels_parked() -> String {
    "This loop uses channels, which Control Studio's Rust engine does not run yet.".to_owned()
}

/// Declared channels (`declaredControllerChannels`, declaredControllerChannels.ts; C0): the loop has no setpoint, so the
/// primary reference and output do not belong to the model channel bindings.
#[must_use]
pub fn init_reference_missing() -> String {
    "The loop's primary reference and output must belong to the declared model channel bindings."
        .to_owned()
}

/// Declared channels (`declaredControllerChannels`, declaredControllerChannels.ts; C0): the Manual command or its IEC type is
/// not declared.
#[must_use]
pub fn init_manual_command_missing() -> String {
    "Declare the compiled Manual command and type.".to_owned()
}

/// Declared channels (`transferSignalMember`, declaredControllerChannels.ts; A6.3): a reference, process value or output is not REAL
/// or LREAL (a BOOL output is accepted only as a dimensionless level).
#[must_use]
pub fn init_primary_coordinates() -> String {
    "Controller reference and output transfer bindings require REAL/LREAL coordinates.".to_owned()
}

/// Initialization formula (`initializationFormulas`, controllerInitializationFormulas.ts; C0): the state's actuator has no bound
/// plant input at the state's index.
#[must_use]
pub fn formula_no_bound_actuator(symbol: &str) -> String {
    format!("The initialization formula of {symbol} has no bound actuator.")
}

/// Initialization formula (`initializationFormulas`, controllerInitializationFormulas.ts; C0): the state's measurement has no
/// bound process value at the state's index.
#[must_use]
pub fn formula_no_bound_process(symbol: &str) -> String {
    format!("The initialization formula of {symbol} has no bound process measurement.")
}

/// Initialization formula (`model-input-is`, controllerInitializationFormulas.ts; C0): the Boolean predicate is not a declared
/// level of the actuator.
#[must_use]
pub fn formula_predicate_not_level(symbol: &str) -> String {
    format!("The Boolean input-state predicate of {symbol} must be a declared actuator level.")
}

/// Initialization snapshot (`evaluateInitializationFormula`, controllerInitializationFormulas.ts; C0, E6.4): the runtime's fresh
/// value that a formula needs is missing or not finite.
#[must_use]
pub fn formula_snapshot_missing(symbol: &str) -> String {
    format!("Initialization snapshot is missing {symbol}.")
}

/// Library PID (`libraryPidInitialization`, libraryPidInitialization.ts; C1): the `Control.PID` does not declare the seven retained
/// states with a setpoint, a PID law and a snapshot age that fits its task period.
#[must_use]
pub fn library_pid_declare_seven() -> String {
    "Declare all seven Control.PID states and compatible timing before Auto commissioning."
        .to_owned()
}

/// Library PID (`assertLibraryPidStates`, libraryPidInitialization.ts; C1): the instance does not declare exactly seven retained
/// states.
#[must_use]
pub fn library_pid_seven_states(instance: &str) -> String {
    format!("Declare all seven retained states for {instance}.")
}

/// Library PID (`assertLibraryPidStates`, libraryPidInitialization.ts; C1): a retained state is not bound to its member, its LREAL
/// type, its unit, its bounded post-scan rule or its declared operating-point coordinate.
#[must_use]
pub fn library_pid_state_contract(instance: &str, member: &str) -> String {
    format!("Control.PID {instance}.{member} must bind its actual retained state and declared operating-point coordinate.")
}

/// Library PID (`libraryPidInitialization`, libraryPidInitialization.ts; C1): the application states are not unique, or they do not
/// belong to the controller's owning program.
#[must_use]
pub fn library_pid_application_states() -> String {
    "Declare unique application startup states in the controller's owning program.".to_owned()
}

/// Declared initialization (`declaredTransferBinding`, controllerInitialization.ts; C0): the reference, the Manual command or the
/// mode is not declared before the controller initialization.
#[must_use]
pub fn declared_references_required() -> String {
    "Declare the reference, Manual command and mode before controller initialization.".to_owned()
}

/// Declared initialization (`declaredTransferBinding`, controllerInitialization.ts; A6.3): two roles of the binding share a symbol,
/// without regard to case, so one member would be written twice.
#[must_use]
pub fn declared_roles_overlap() -> String {
    "Controller initialization roles overlap parameters, measurements or output ownership."
        .to_owned()
}

/// Declared initialization (`states`, controllerInitialization.ts; E6.3): the runtime reports no value for a state of the binding.
#[must_use]
pub fn declared_state_unavailable(symbol: &str) -> String {
    format!("Controller state {symbol} is not available.")
}

/// The refusal of a historical record that lacks the original reference, the gain set or the compatible model that a recovery needs
/// (`reviewRecovery`, `gainRecoveryContext.ts` and `proportionalCommissioning.ts`; B6.4).
#[must_use]
pub fn gain_recovery_incomplete() -> String {
    "The historical study lacks the original reference, gain set or compatible model needed for recovery.".to_owned()
}

/// The refusal of a recovery whose running record changed since the commissioning record (`reviewRecovery`,
/// `proportionalCommissioning.ts`; B6.4: Restore explains that it is unavailable).
#[must_use]
pub fn gain_recovery_changed() -> String {
    "The running controller or retained gain history changed since this commissioning record."
        .to_owned()
}

/// The reason of a failed commissioning run with its recovery outcome as its own sentence (`withRecoveryOutcome`, `pidCommissioning.ts`;
/// A6.3, B6.7): the reason, with a full stop when it has none, and then the outcome.
#[must_use]
pub fn recovery_with_outcome(reason: &str, outcome: &str) -> String {
    let reason = reason.trim();
    let sentence = if reason.ends_with(['.', '!', '?']) {
        reason.to_owned()
    } else {
        format!("{reason}.")
    };
    format!("{sentence} {outcome}")
}

/// The recovery outcome of a PID campaign (`recoveryConfirmedReason`, `pidCommissioning.ts`; A6.2, B6.4): the mode the loop is
/// confirmed in. Only `auto` names Auto; any other value confirms Manual, as the `TypeScript` default does.
#[must_use]
pub fn recovery_confirmed_reason(mode: &str) -> String {
    if mode == "auto" {
        "Recovery confirmed in Auto; previous controller parameters restored.".to_owned()
    } else {
        "Recovery confirmed in Manual; previous controller parameters restored.".to_owned()
    }
}

/// Application (`controllerApplication.ts`; B6.1, C1 "Atomic write", E7.3 and E7.7; A11.1): a physical write without its own approval.
#[must_use]
pub fn application_physical_approval_required() -> String {
    "Physical controller application requires separate immediate approval.".to_owned()
}

/// Application (`controllerApplication.ts`, `required`; A11.1): a request field that is blank, named by the field.
#[must_use]
pub fn application_request_blank(field: &str) -> String {
    format!("The controller request needs a non-blank {field}.")
}

/// Application (`controllerApplication.ts`, `status`; E7.7): the status answer's parameters are not an object.
#[must_use]
pub fn application_status_parameters_invalid() -> String {
    "Runtime returned invalid controller parameters.".to_owned()
}

/// Application (`controllerApplication.ts`, `required`; E7.7): an answer names no usable revision.
#[must_use]
pub fn application_revision_invalid() -> String {
    "Runtime returned an invalid controller revision.".to_owned()
}

/// Application (`waitForConfirmedRevision`; C1 "Atomic write", A11.1): another write is pending in place of the reviewed candidate.
#[must_use]
pub fn application_candidate_replaced(pending: &str, reviewed: &str) -> String {
    format!(
        "A different controller write replaced the reviewed candidate: revision {pending} is pending, and the reviewed candidate is {reviewed}."
    )
}

/// Application (`waitForConfirmedRevision`; C1, A11.1): the record changed with no write pending, so the write was not confirmed.
#[must_use]
pub fn application_not_confirmed(active: &str, reviewed: &str) -> String {
    format!(
        "The controller write was not confirmed by the runtime: the active revision is {active}, and the reviewed candidate is {reviewed}."
    )
}

/// Application (`waitForConfirmedRevision`; A7 "Confirmation of a transfer or parameter change", A11.1): the window ran out, in seconds.
#[must_use]
pub fn application_confirmation_timed_out(window_ms: f64) -> String {
    format!(
        "Timed out waiting for scan-boundary controller confirmation after {} s.",
        crate::tuning::texts::engineering(window_ms / 1000.0, None)
    )
}

/// Adapters (`controllerAdapters.ts`, `controllerAdapter`; C0 "Missing adapters", A11.1): the loop uses a feature no adapter realizes.
#[must_use]
pub fn adapter_parked_feature(feature: &str) -> String {
    format!("This loop uses {feature}, which Control Studio's Rust adapter does not run yet.")
}

/// Adapters (`controllerAdapters.ts`, `controllerAdapter`; C0, A11.1): no adapter recognises the controller's type, with the reason.
#[must_use]
pub fn adapter_unrecognised_type(controller_type: &str, reason: &str) -> String {
    format!("Controller type {controller_type} is not a recognised ST adapter. {reason}")
}

/// Adapters (`controllerAdapters.ts`, `materialize` of `Control.PID`; C1): the shipped block needs its proportional gain.
#[must_use]
pub fn adapter_control_pid_gain_required() -> String {
    "A proportional gain is required by Control.PID.".to_owned()
}

/// Adapters (`controllerAdapters.ts`, `materialize` of the declared PID; C2): the parallel PID needs its proportional gain.
#[must_use]
pub fn adapter_pid_record_gain_required() -> String {
    "A proportional gain is required by the declared PID record.".to_owned()
}

/// Adapters (`controllerAdapters.ts`, `materialize` of the proportional law; C13): the connected P needs its proportional gain.
#[must_use]
pub fn adapter_proportional_gain_required() -> String {
    "The proportional ST law needs a proportional gain.".to_owned()
}

/// Adapters (`controllerAdapters.ts`, `materialize` of the proportional law; C13): the connected P realizes no integral or derivative term.
#[must_use]
pub fn adapter_proportional_only() -> String {
    "The bound proportional ST law cannot realize integral or derivative gains.".to_owned()
}

/// Adapters (`controllerAdapters.ts`, `materialize` of the proportional law; C13, N6.4, A11.1): the realized gain is outside its bounds,
/// which bound its magnitude.
#[must_use]
pub fn adapter_proportional_outside_bounds(
    value: f64,
    lowest: f64,
    highest: f64,
    unit: &str,
) -> String {
    let suffix = crate::tuning::texts::unit_suffix(unit);
    let shown = crate::tuning::texts::engineering(value, None);
    let low = crate::tuning::texts::engineering(lowest, None);
    let high = crate::tuning::texts::engineering(highest, None);
    format!(
        "Realized proportional gain {shown}{suffix} is outside its declared bounds: its magnitude must lie from {low}{suffix} to {high}{suffix}."
    )
}
