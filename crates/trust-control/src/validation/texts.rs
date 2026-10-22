//! The sentences of the validation tree (specification 35, B5 and A11.1; numerical annex N8). A rule decides; these
//! functions say what it decided, naming the values involved, in the words the engineer reads. Each keeps the wording of
//! the `TypeScript` it was ported from. Numbers print as `ECMAScript` prints them, so the sentences match the extension's.

use trust_control_math::sampling::SampleGap;

use crate::experiment::number_text::js_number;
use crate::identification::texts::gap_phrase;

/// The task requirements are missing or not finite (`engineerRequirements`, `engineerVerificationPlan.ts`).
#[must_use]
pub fn task_requirements_missing() -> String {
    "Enter finite task requirements.".to_owned()
}

/// The Auto-entry tolerance is neither blank for automatic nor a finite nonnegative value.
#[must_use]
pub fn transfer_tolerance_invalid() -> String {
    "Auto-entry tolerance must be blank for automatic or a finite nonnegative value in output units.".to_owned()
}

/// The tuning method is not one of the supported methods.
#[must_use]
pub fn tuning_method_unsupported() -> String {
    "Choose a supported tuning method.".to_owned()
}

/// The tuning objective is not one of the supported objectives.
#[must_use]
pub fn tuning_objective_unsupported() -> String {
    "Choose a supported tuning objective.".to_owned()
}

/// A declared overshoot limit is blank-for-no-limit or out of range. `label` names the limit ("Maximum overshoot", or the
/// output's name); `maximum_percent` is the largest accepted limit in percent of the setpoint change.
#[must_use]
pub fn overshoot_limit_invalid(label: &str, maximum_percent: f64) -> String {
    format!(
        "{label} must be blank for no limit, or a finite value above 0 and at most {} % of the setpoint change.",
        js_number(maximum_percent)
    )
}

/// The disturbance, noise or gain-uncertainty bounds are not finite, or the gain uncertainty is outside zero to one.
#[must_use]
pub fn disturbance_noise_bounds_invalid() -> String {
    "Enter finite disturbance/noise bounds and gain uncertainty between zero and one.".to_owned()
}

/// The automatic test criteria cannot be decoded.
#[must_use]
pub fn automatic_criteria_invalid() -> String {
    "Invalid automatic test criteria.".to_owned()
}

/// The loop has no parameter binding or no reference binding, so no plan can be made.
#[must_use]
pub fn parameter_or_reference_binding_missing() -> String {
    "Declare the parameter and reference bindings first.".to_owned()
}

/// The task period is not a whole number of milliseconds from 1 to 1000.
#[must_use]
pub fn task_period_unsupported() -> String {
    "This runtime adapter requires a whole-millisecond task period from 1 to 1000.".to_owned()
}

/// The validation is longer than a day, or it does not run past the response deadline.
#[must_use]
pub fn duration_does_not_pass_deadline() -> String {
    "Choose a validation of at most 24 h that runs past the response deadline.".to_owned()
}

/// The validation would execute more scans than the runtime allows per run.
#[must_use]
pub fn too_many_scans(scans: u64, limit: u64) -> String {
    format!(
        "Validation would execute {scans} scans; the runtime executes at most {limit} per run. Shorten the test duration."
    )
}

/// The validation observes more channels, including parameters, than the runtime accepts.
#[must_use]
pub fn too_many_observed_channels(limit: u64) -> String {
    format!("Verification supports at most {limit} observed channels, including parameters.")
}

/// A parameter value is missing, outside its bounds, or not representable in its REAL type.
#[must_use]
pub fn parameter_not_representable(symbol: &str) -> String {
    format!("Parameter {symbol} is missing, out of bounds or not representable.")
}

/// The target or the deadline of a controlled output lies outside the declared scope of the signal or the model.
#[must_use]
pub fn target_outside_scope(symbol: &str) -> String {
    format!("Target or deadline is outside the declared scope for {symbol}.")
}

/// Scenario effects need a selected plant and distinct, bounded effect identities.
#[must_use]
pub fn scenario_effects_need_plant() -> String {
    "Scenario effects require a selected plant and distinct bounded effect identities.".to_owned()
}

/// A scenario effect names a channel, a window or an observation interval that does not fit the plant or the run.
#[must_use]
pub fn scenario_effect_coordinates_invalid() -> String {
    "Scenario effect coordinates or observation interval are invalid.".to_owned()
}

/// A constant or pulse scenario effect has no finite value.
#[must_use]
pub fn constant_effect_needs_value() -> String {
    "A constant/pulse scenario effect requires a finite value.".to_owned()
}

/// A pulse scenario effect has no positive duration.
#[must_use]
pub fn pulse_needs_duration() -> String {
    "A pulse scenario requires a positive duration.".to_owned()
}

/// The bounds of a noise scenario effect are not ordered finite numbers.
#[must_use]
pub fn noise_bounds_invalid() -> String {
    "Noise requires ordered finite bounds.".to_owned()
}

/// The scenario effect names a generator the native runtime does not provide.
#[must_use]
pub fn generator_unsupported() -> String {
    "Unsupported native scenario generator.".to_owned()
}

/// The configured verification scenarios are not a valid campaign.
#[must_use]
pub fn scenarios_invalid() -> String {
    "Invalid mandatory verification scenario campaign.".to_owned()
}

/// A configured scenario reuses the identity of an automatic target or interaction scenario.
#[must_use]
pub fn scenario_duplicates_automatic() -> String {
    "A configured scenario duplicates an automatic target/interaction scenario.".to_owned()
}

/// A scenario checks a requirement after the end of the validation run. The end and the run length are in seconds.
#[must_use]
pub fn scenario_check_after_run(
    scenario: &str,
    requirement: &str,
    end_seconds: f64,
    run_seconds: f64,
) -> String {
    let end = js_number(end_seconds);
    format!(
        "{scenario} checks {requirement} until {end} s, after the {} s validation. Allow a validation duration of at least {end} s.",
        js_number(run_seconds)
    )
}

/// A scenario supplies a model that cannot be decoded.
#[must_use]
pub fn scenario_model_invalid(id: &str) -> String {
    format!("Scenario {id} has an invalid supplied model.")
}

/// A scenario names an operating range but supplies no model validated there.
#[must_use]
pub fn scenario_operating_model_missing(id: &str) -> String {
    format!("Scenario {id} needs the validated model of its operating range.")
}

/// A scenario writes a frozen controller parameter.
#[must_use]
pub fn scenario_overwrites_parameters(id: &str) -> String {
    format!("Scenario {id} cannot overwrite frozen controller parameters.")
}

/// A scenario event does not fall on a task sample inside the run.
#[must_use]
pub fn scenario_events_off_grid(id: &str) -> String {
    format!("Scenario {id} events must fall on a declared task sample within the run.")
}

/// A scenario event is written between two task samples: its time is not a whole number of task periods. The engineer sees it as the
/// scenarios change, before a validation runs (B4.3).
#[must_use]
pub fn scenario_event_between_samples(id: &str, at_ms: f64, period_ms: f64) -> String {
    format!(
        "Scenario {id} writes an event at {} ms, which is not a multiple of the {} ms task period.",
        js_number(at_ms),
        js_number(period_ms)
    )
}

/// A scenario has duplicate requirement identities or more requirements than one run accepts.
#[must_use]
pub fn scenario_requirements_invalid(id: &str) -> String {
    format!("Scenario {id} has duplicate or excessive mandatory requirements.")
}

/// A declared measurement filter needs a nonzero reviewed measurement-noise band.
#[must_use]
pub fn measurement_filter_needs_noise() -> String {
    "A declared measurement filter requires a nonzero reviewed measurement-noise band so target and noise response are both verified."
        .to_owned()
}

/// Declared reference shaping needs a nonzero reviewed input disturbance.
#[must_use]
pub fn reference_shaping_needs_load() -> String {
    "Declared reference shaping requires a nonzero reviewed input disturbance so target and load response are both verified."
        .to_owned()
}

/// A reviewed load disturbance has no bound process-input channel to act on.
#[must_use]
pub fn load_case_needs_input() -> String {
    "A reviewed load disturbance requires a bound process-input channel.".to_owned()
}

/// A reviewed measurement-noise case has no bound process-output channel to act on.
#[must_use]
pub fn noise_case_needs_output() -> String {
    "A reviewed measurement-noise case requires a bound process-output channel.".to_owned()
}

/// Requirement identities of one evaluation are not unique.
#[must_use]
pub fn requirement_ids_not_unique() -> String {
    "Requirement IDs must be unique.".to_owned()
}

/// The scope sentence every evaluation carries: only the declared observed conditions were checked.
#[must_use]
pub fn evaluation_scope() -> String {
    "Only the declared observed conditions were evaluated; Pass does not establish general stability.".to_owned()
}

/// The current and the candidate run differ in evidence class, identities or immutable conditions.
#[must_use]
pub fn comparison_conditions_differ() -> String {
    "Evidence class, channel/parameter identities or immutable experiment conditions differ."
        .to_owned()
}

/// A comparison was made without a ranking objective.
#[must_use]
pub fn no_ranking_objective() -> String {
    "No ranking objective was declared.".to_owned()
}

/// The ranking objective does not name a declared requirement.
#[must_use]
pub fn ranking_objective_undeclared() -> String {
    "Ranking objective must name a declared requirement.".to_owned()
}

/// Neither run satisfies every requirement, so neither ranks.
#[must_use]
pub fn neither_run_qualifies() -> String {
    "Neither run satisfies all requirements; no ranking is available.".to_owned()
}

/// Only the candidate satisfies every requirement.
#[must_use]
pub fn only_candidate_qualifies() -> String {
    "Only candidate satisfies all requirements.".to_owned()
}

/// Only the current run satisfies every requirement.
#[must_use]
pub fn only_current_qualifies() -> String {
    "Only current satisfies all requirements.".to_owned()
}

/// The ranking measurement is missing for one of the runs.
#[must_use]
pub fn ranking_measurement_unavailable() -> String {
    "Ranking measurement is unavailable.".to_owned()
}

/// Both runs satisfy every requirement, so the declared measurement decides between them.
#[must_use]
pub fn both_runs_qualify(objective: &str) -> String {
    format!("Both runs qualify; minimizing {objective}.")
}

/// The total command movement of a run, measured.
#[must_use]
pub fn movement_measured() -> String {
    "Measured total command movement.".to_owned()
}

/// A calculated measurement is not finite.
#[must_use]
pub fn measurement_nonfinite() -> String {
    "The calculated measurement is nonfinite.".to_owned()
}

/// The run holds no complete dwell interval in the requested state. `state` is "high" or "low".
#[must_use]
pub fn no_dwell_observed(state: &str) -> String {
    format!("No complete {state} dwell interval was observed; extend the switching scenario.")
}

/// Every completed observed dwell meets the declared minimum.
#[must_use]
pub fn dwell_meets_minimum() -> String {
    "Every completed observed dwell meets the declared minimum.".to_owned()
}

/// An observed switch came before the declared minimum dwell.
#[must_use]
pub fn dwell_below_minimum() -> String {
    "An observed switch occurred before the declared minimum dwell.".to_owned()
}

/// The observed measurement is within its declared limit.
#[must_use]
pub fn measurement_within_limit() -> String {
    "Observed measurement satisfies the declared limit.".to_owned()
}

/// The observed measurement exceeds its declared limit.
#[must_use]
pub fn measurement_exceeds_limit() -> String {
    "Observed measurement exceeds the declared limit.".to_owned()
}

/// The refusal of a validation whose current settings could not be read from the running controller (B5.3): the setting that
/// is missing from the runtime's answer.
#[must_use]
pub fn current_setting_unread(symbol: &str) -> String {
    format!(
        "The current value of {symbol} could not be read from the running controller, so the current settings cannot be validated. Check the runtime connection and the loop's parameter binding."
    )
}

/// The reason of each requirement of an isolated run that did not complete (B5.4, B5.5): the job's end, and the runtime's own
/// reason when it gave one, so the requirement has no evidence and is Inconclusive.
#[must_use]
pub fn run_not_completed(state: &str, reason: Option<&str>) -> String {
    match reason.filter(|reason| !reason.trim().is_empty()) {
        Some(reason) => format!(
            "The isolated run ended {state} before it completed, so this requirement has no evidence. The runtime said: {reason}"
        ),
        None => format!(
            "The isolated run ended {state} before it completed, so this requirement has no evidence."
        ),
    }
}

/// The declared window lacks complete, ordered, good-quality sample coverage (N8.4).
#[must_use]
pub fn incomplete_coverage() -> String {
    "The declared window lacks complete, ordered, good-quality sample coverage.".to_owned()
}

/// The recording has a gap inside the window; the gap's start and length are named (A4.3).
#[must_use]
pub fn recording_gap(gap: SampleGap) -> String {
    format!("The recording has {}.", gap_phrase(gap))
}

/// The settling window never holds the target band for the required hold through its end (N8.3).
#[must_use]
pub fn no_arrival_in_band() -> String {
    "No observed arrival remained in band for the required hold through the end of the window."
        .to_owned()
}

/// A recorded run does not match the recorded-run contract.
#[must_use]
pub fn invalid_recorded_run() -> String {
    "Invalid recorded-run contract.".to_owned()
}

/// The immutable experiment conditions of a recorded run are invalid.
#[must_use]
pub fn invalid_experiment_conditions() -> String {
    "Invalid immutable experiment conditions.".to_owned()
}

/// The channels or the samples of a recorded run are invalid.
#[must_use]
pub fn invalid_run_channels() -> String {
    "Invalid run channels or sample structure.".to_owned()
}

/// A requirement names an unknown channel, or its window does not lie within the run.
#[must_use]
pub fn invalid_requirement_window() -> String {
    "Invalid requirement identity, channel or sample window.".to_owned()
}

/// A requirement's calculation or its limits are invalid.
#[must_use]
pub fn invalid_requirement_limits() -> String {
    "Invalid requirement calculation or limits.".to_owned()
}

/// The commissioning recording is incomplete or holds invalid samples.
#[must_use]
pub fn commissioning_recording_incomplete() -> String {
    "Commissioning recording is incomplete or has invalid samples.".to_owned()
}

/// The recording never shows the controller in Auto at the reviewed target.
#[must_use]
pub fn commissioning_no_auto_target() -> String {
    "The recorded controller never adopted the reviewed Auto target.".to_owned()
}

/// The recorded response is shorter than the reviewed duration.
#[must_use]
pub fn commissioning_duration_not_covered() -> String {
    "The recorded response does not cover the reviewed duration.".to_owned()
}

/// The commissioned loop left its mode, its reference or its signal bounds.
#[must_use]
pub fn commissioning_bounds_violated() -> String {
    "The commissioned loop violated mode, reference or signal bounds.".to_owned()
}

/// No sample lies at or after the response deadline.
#[must_use]
pub fn commissioning_deadline_unrecorded() -> String {
    "The response deadline was not recorded.".to_owned()
}

/// The commissioned target error is above its limit. Both values are in the output's unit.
#[must_use]
pub fn commissioning_error_exceeds(error: f64, limit: f64) -> String {
    format!(
        "Commissioning target error {} exceeds {}.",
        js_number(error),
        js_number(limit)
    )
}

/// A switching check of a commissioning recording failed; `id` names the check and `reason` is its own sentence.
#[must_use]
pub fn commissioning_check_failed(id: &str, reason: &str) -> String {
    format!("Commissioning {id}: {reason}")
}

/// The evidence name of a configured simulated plant (A9).
#[must_use]
pub fn configured_plant_name(plant_id: &str) -> String {
    format!("configured plant {plant_id}")
}

/// The evidence name of a model, with its revision (A9).
#[must_use]
pub fn model_name(revision: &str) -> String {
    format!("model {revision}")
}

/// The model's estimation recording is not among the recordings of the campaigns.
#[must_use]
pub fn estimation_recording_missing() -> String {
    "The provisional model's estimation recording is missing. Identify the plant again.".to_owned()
}

/// The project changed while the baseline and the candidate were predicted, so their runs do not compare.
#[must_use]
pub fn project_changed_during_prediction() -> String {
    "The project changed during model prediction.".to_owned()
}

/// The loop has no channel bindings and its model does not have exactly one input and one output, so the single-channel
/// path cannot map it (`connectedModelChannels` without `channels`, `loopChannels.ts`).
#[must_use]
pub fn channel_bindings_unmapped() -> String {
    "Map every model input and output in loop channel bindings.".to_owned()
}

/// The stored identification policy does not hold (`validIdentificationPolicy`, `identificationPolicy.ts`): its maximum
/// normalized error must be above 0 and below 1, and its other settings must be valid. The `TypeScript` rejects such a loop
/// at load with a code and no sentence, so this sentence is new text, not a copy.
#[must_use]
pub fn identification_policy_invalid() -> String {
    "The stored identification policy does not hold: its maximum normalized error must be above 0 and below 1, and its other settings must be valid."
        .to_owned()
}

// The words of the Validate table and plot (B5.4, B9.9). A requirement's label names its check; a role names a signal by its
// part in the loop, never by its symbol.

/// The name of a fixed parameter that the run keeps at its value.
pub const PARAMETER_UNCHANGED: &str = "Parameter unchanged";
/// The name of the settling requirement of a response (N8.3).
pub const MAXIMUM_SETTLING_TIME: &str = "Maximum settling time";
/// The name of the overshoot requirement of a step (N8.3).
pub const MAXIMUM_OVERSHOOT: &str = "Maximum overshoot";
/// The name of an error measured from the setpoint (N8.3).
pub const MAXIMUM_STEADY_STATE_ERROR: &str = "Maximum steady-state error";
/// The start of the name of an error measured from another reference; see [`maximum_error_from`].
pub const MAXIMUM_ERROR_FROM: &str = "Maximum error from";
/// The name of a signal's bounds over the window (N8.3).
pub const OPERATING_RANGE: &str = "Operating range";
/// The name of a state's bounds over the window (N8.3).
pub const STATE_RANGE: &str = "State range";
/// The name of the Manual-to-Auto entry case in the table (B5.3, B9.9): the case's rows are named under it.
pub const MANUAL_TO_AUTO_ENTRY: &str = "Manual-to-Auto entry";
/// The name of a levels requirement (N8.3).
pub const ALLOWED_OUTPUT_LEVELS: &str = "Allowed output levels";
/// The name of the shortest ON stay of a switching output.
pub const MINIMUM_ON_TIME: &str = "Minimum ON time";
/// The name of the shortest OFF stay of a switching output.
pub const MINIMUM_OFF_TIME: &str = "Minimum OFF time";
/// The name of the rate limit of an upward movement (N8.3).
pub const MAXIMUM_INCREASE_RATE: &str = "Maximum increase rate";
/// The name of the rate limit of a downward movement (N8.3).
pub const MAXIMUM_DECREASE_RATE: &str = "Maximum decrease rate";
/// The name of the rate limit of a movement in either direction (N8.3).
pub const MAXIMUM_RATE: &str = "Maximum rate";
/// The name of the saturation requirement (N8.3).
pub const MAXIMUM_SATURATION_TIME: &str = "Maximum saturation time";
/// The name of the switching requirement (N8.3).
pub const MAXIMUM_SWITCHING_COUNT: &str = "Maximum switching count";
/// The name of the command activity requirement.
pub const COMMAND_ACTIVITY: &str = "Command activity";
/// The name of the independent model check's row (B5.2).
pub const MODEL_AGREEMENT: &str = "Model agreement";
/// The name of the run of the current settings in a scenario (B5.3).
pub const CURRENT_SETTINGS: &str = "Current settings";
/// The name of the run of the candidate in a scenario (B5.3).
pub const CANDIDATE: &str = "Candidate";
/// The name of the candidate's overall row (N8.5).
pub const CANDIDATE_OVERALL: &str = "Candidate overall";
/// The mark of a limit that Automatic generated (B4.3).
pub const AUTOMATIC: &str = "Automatic";
/// The words of the row that counts the passing guards: the generated ranges, states and transfer (B9.9).
pub const GUARDS: &str = "Ranges, states and transfer";
/// The limit column of the row that counts the passing guards: they stayed within their declared limits (B9.9).
pub const GUARDS_LIMIT: &str = "Within declared limits";
/// The title of the Validate plot (B9.9).
pub const MEASURED_RESPONSE: &str = "Measured response";
/// The name of the candidate's trace on the Validate plot.
pub const TUNED: &str = "Tuned";
/// The name of the current settings' trace on the Validate plot.
pub const CURRENT: &str = "Current";
/// The role of the loop's process variable (B9.9).
pub const MEASUREMENT: &str = "Measurement";
/// The role of the loop's manipulated variable (B9.9).
pub const OUTPUT: &str = "Output";
/// The role of the loop's setpoint (B9.9).
pub const SETPOINT: &str = "Setpoint";
/// The role of the loop's actuator signal (B9.9).
pub const ACTUATOR: &str = "Actuator";

/// The name of an error measured from `reference`, which is named after that reference's role or name (B9.9), for example
/// "Maximum error from Trajectory".
#[must_use]
pub fn maximum_error_from(reference: &str) -> String {
    format!("{MAXIMUM_ERROR_FROM} {reference}")
}

/// A scenario's name in words (B9.9): its identity made readable, with its first letter capitalized.
#[must_use]
pub fn scenario_label(id: &str) -> String {
    let words = readable_words(id);
    let mut chars = words.chars();
    chars.next().map_or_else(String::new, |first| {
        first.to_uppercase().chain(chars).collect()
    })
}

/// A symbol's last part, made readable: `P1.Parameters.ProportionalGain` reads "Proportional Gain" (B9.2).
#[must_use]
pub fn readable_name(symbol: &str) -> String {
    readable_words(symbol.rsplit('.').next().unwrap_or(symbol))
}

/// Identifier-like words made readable: camelCase, hyphenated and underscored names read as separate words (B9.2).
fn readable_words(text: &str) -> String {
    let mut words = String::with_capacity(text.len() + 4);
    let mut after_lower = false;
    for ch in text.chars() {
        if ch == '-' || ch == '_' {
            words.push(' ');
            after_lower = false;
        } else {
            if after_lower && ch.is_ascii_uppercase() {
                words.push(' ');
            }
            words.push(ch);
            after_lower = ch.is_ascii_lowercase();
        }
    }
    words
}
