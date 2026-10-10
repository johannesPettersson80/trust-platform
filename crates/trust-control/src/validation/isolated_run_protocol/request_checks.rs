//! The limits and rules of an isolated run's request, checked before the request is sent (E8.3 and E8.4, with E0.2 and
//! E0.3): the identifier, the timing, the samples and cells, the observed channels, the parameter overrides, the initial
//! values, the events, the feedback relay and the selected model. A refusal names the check and the values involved
//! (A11.1).

use std::collections::{BTreeMap, HashSet};

use crate::failure::{Cause, Failure, NextAction};
use crate::limits::{
    ISOLATED_SIMULATION_CELL_LIMIT, ISOLATED_SIMULATION_CHANNEL_LIMIT,
    ISOLATED_SIMULATION_DURATION_LIMIT_MS, ISOLATED_SIMULATION_SAMPLE_LIMIT,
    ISOLATED_SIMULATION_SCAN_LIMIT, ISOLATED_SIMULATION_WALL_TIME_LIMIT_MS,
};
use crate::model::SimulationOverride;

use super::{
    scan_count, IsolatedFeedbackRelay, IsolatedParameterOverride, IsolatedParameterType,
    IsolatedRunObservation, IsolatedRunRequest,
};

/// The largest integer a request carries (E0.3): 2^53 − 1.
const MAXIMUM_REQUEST_INTEGER: u64 = 9_007_199_254_740_991;
/// The longest identifier of a run, in bytes (E0.2).
const MAXIMUM_REQUEST_ID_BYTES: usize = 128;
/// The longest symbol or unit of a request, in bytes (the runtime's bound; E8.4 does not state one).
const MAXIMUM_TEXT_BYTES: usize = 256;
/// The longest step of a run, in milliseconds (E8.4).
const MAXIMUM_STEP_MS: u64 = 1_000;
/// The most executions one sample averages (E8.4).
const MAXIMUM_RECORD_EVERY: u64 = 100_000;
/// The most parameter overrides of a run (E8.4).
const MAXIMUM_PARAMETER_OVERRIDES: usize = 64;
/// The most initial values of a run (the runtime's bound; E8.4 does not state one).
const MAXIMUM_INITIAL_VALUES: usize = 64;
/// The most events of a run (the runtime's bound; E8.4 does not state one).
const MAXIMUM_EVENTS: usize = 1_024;
/// The longest revision of a selected model, in bytes (E8.4).
const MAXIMUM_MODEL_REVISION_BYTES: usize = 256;
/// The longest provenance of a selected model, in bytes (E8.4).
const MAXIMUM_MODEL_PROVENANCE_BYTES: usize = 4_096;
/// The longest simulation configuration of a selected model, in bytes (E8.4).
const MAXIMUM_SIMULATION_TOML_BYTES: usize = 262_144;

/// Checks `request` against the limits and rules of E8.4, with E0.2 and E0.3, before it is sent. The checks run in the
/// order of the request: its identifier and revision, its observations, its timing, its overrides, its initial values
/// and events, its feedback relay and its selected model.
///
/// # Errors
/// A limit or rule is broken; the failure names the field and the values involved.
pub(super) fn check_request(request: &IsolatedRunRequest) -> Result<(), Failure> {
    check_request_id(&request.request_id)?;
    if request.expected_project_revision.is_empty() {
        return Err(refuse_request(
            "The run names no project revision; review the loop again before a test run.",
        ));
    }
    check_observations(&request.observations)?;
    check_timing(request)?;
    check_parameter_overrides(&request.parameter_overrides)?;
    let parameters = parameter_symbols(&request.parameter_overrides);
    check_initial_values(&request.initial_values, &parameters)?;
    check_events(request, &parameters)?;
    if let Some(relay) = &request.feedback_relay {
        check_feedback_relay(request, relay, &parameters)?;
    }
    if let Some(selected) = &request.simulation_override {
        check_simulation_override(selected)?;
    }
    Ok(())
}

/// A request the engine refuses before it sends it: one of its settings is outside the protocol.
fn refuse_request(text: &str) -> Failure {
    Failure::new(text, Cause::Settings, NextAction::None)
}

fn check_request_id(id: &str) -> Result<(), Failure> {
    if id.is_empty()
        || id.len() > MAXIMUM_REQUEST_ID_BYTES
        || !id
            .bytes()
            .all(|byte| byte.is_ascii_alphanumeric() || byte == b'-')
    {
        return Err(refuse_request(&format!(
            "The run identifier {id} must be 1 to 128 ASCII letters, digits or hyphens."
        )));
    }
    Ok(())
}

/// A symbol or unit of 1 to 256 bytes.
fn check_text(role: &str, text: &str) -> Result<(), Failure> {
    if text.is_empty() || text.len() > MAXIMUM_TEXT_BYTES {
        return Err(refuse_request(&format!(
            "The {role} is {} bytes; it must be 1 to 256 bytes.",
            text.len()
        )));
    }
    Ok(())
}

fn check_observations(observations: &[IsolatedRunObservation]) -> Result<(), Failure> {
    let channels = u64::try_from(observations.len()).unwrap_or(u64::MAX);
    if channels == 0 {
        return Err(refuse_request(
            "The run observes no channel; observe 1 to 64 channels.",
        ));
    }
    if channels > ISOLATED_SIMULATION_CHANNEL_LIMIT {
        return Err(refuse_request(&format!(
            "Validation records {channels} channels; the runtime records at most {ISOLATED_SIMULATION_CHANNEL_LIMIT}. Declare fewer additional signals."
        )));
    }
    let mut symbols = HashSet::new();
    for observation in observations {
        check_text("observed symbol", &observation.symbol)?;
        check_text("observed unit", &observation.unit)?;
        if !symbols.insert(observation.symbol.as_str()) {
            return Err(refuse_request(&format!(
                "The observed symbol {} is listed twice.",
                observation.symbol
            )));
        }
    }
    Ok(())
}

/// The step, the duration, the seed, the wall budget, the cycles and the samples of the run (E8.3, E8.4, A7).
fn check_timing(request: &IsolatedRunRequest) -> Result<(), Failure> {
    let step = request.step_ms;
    let duration = request.duration_ms;
    if !(1..=MAXIMUM_STEP_MS).contains(&step) {
        return Err(refuse_request(&format!(
            "The step is {step} ms; a run steps 1 to 1,000 ms."
        )));
    }
    if duration == 0
        || duration > ISOLATED_SIMULATION_DURATION_LIMIT_MS
        || !duration.is_multiple_of(step)
    {
        return Err(refuse_request(&format!(
            "The duration is {duration} ms; it must be more than 0, at most 86,400,000 ms, and a multiple of the step of {step} ms."
        )));
    }
    if request.seed > MAXIMUM_REQUEST_INTEGER {
        return Err(refuse_request(&format!(
            "The seed {} is above 9,007,199,254,740,991.",
            request.seed
        )));
    }
    if !(1..=ISOLATED_SIMULATION_WALL_TIME_LIMIT_MS).contains(&request.wall_budget_ms) {
        return Err(refuse_request(&format!(
            "The wall budget is {} ms; a run may take 1 to 600,000 ms.",
            request.wall_budget_ms
        )));
    }
    if !(1..=MAXIMUM_RECORD_EVERY).contains(&request.record_every) {
        return Err(refuse_request(&format!(
            "The record period is {} executions; a sample averages 1 to 100,000 executions.",
            request.record_every
        )));
    }
    let points = scan_count(step, duration);
    if request.max_total_cycles < points
        || request.max_total_cycles > ISOLATED_SIMULATION_SCAN_LIMIT
    {
        return Err(refuse_request(&format!(
            "max_total_cycles is {}; the run needs {points} scans, and at most 10,000,000 are allowed.",
            request.max_total_cycles
        )));
    }
    let channels = u64::try_from(request.observations.len()).unwrap_or(u64::MAX);
    let samples = points.div_ceil(request.record_every);
    if samples > ISOLATED_SIMULATION_SAMPLE_LIMIT
        || samples.saturating_mul(channels) > ISOLATED_SIMULATION_CELL_LIMIT
    {
        return Err(refuse_request(&format!(
            "Validation would record {samples} samples × {channels} channels; the runtime keeps at most {ISOLATED_SIMULATION_SAMPLE_LIMIT} samples and {ISOLATED_SIMULATION_CELL_LIMIT} channel samples. Record fewer signals."
        )));
    }
    Ok(())
}

fn check_parameter_overrides(parameters: &[IsolatedParameterOverride]) -> Result<(), Failure> {
    if parameters.len() > MAXIMUM_PARAMETER_OVERRIDES {
        return Err(refuse_request(&format!(
            "The run overrides {} parameters; at most 64 are allowed.",
            parameters.len()
        )));
    }
    let mut symbols = HashSet::new();
    for parameter in parameters {
        check_text("parameter symbol", &parameter.symbol)?;
        check_parameter(parameter)?;
        if !symbols.insert(parameter.symbol.to_ascii_uppercase()) {
            return Err(refuse_request(&format!(
                "The parameter {} is overridden twice; symbols are compared without regard to case.",
                parameter.symbol
            )));
        }
    }
    Ok(())
}

/// One override: its unit, its finite numbers, its bounds, its value, and its type's rule (E8.4, E0.3).
fn check_parameter(parameter: &IsolatedParameterOverride) -> Result<(), Failure> {
    let symbol = parameter.symbol.as_str();
    if parameter.unit.is_empty() || parameter.convention.is_empty() {
        return Err(refuse_request(&format!(
            "The parameter {symbol} needs a unit and a convention."
        )));
    }
    if !(parameter.minimum.is_finite()
        && parameter.maximum.is_finite()
        && parameter.value.is_finite())
    {
        return Err(refuse_request(&format!(
            "The parameter {symbol} has a value or a bound that is not a finite number."
        )));
    }
    if parameter.minimum > parameter.maximum {
        return Err(refuse_request(&format!(
            "The parameter {symbol} has a minimum of {} above its maximum of {}.",
            parameter.minimum, parameter.maximum
        )));
    }
    if parameter.value < parameter.minimum || parameter.value > parameter.maximum {
        return Err(refuse_request(&format!(
            "The parameter {symbol} is set to {}, outside its bounds {} to {}.",
            parameter.value, parameter.minimum, parameter.maximum
        )));
    }
    match parameter.iec_type {
        IsolatedParameterType::Real => check_real_precision(parameter),
        IsolatedParameterType::Lreal => Ok(()),
        IsolatedParameterType::Bool => check_fixed_bool(parameter),
    }
}

/// A `REAL` parameter's value and bounds are exactly representable as single precision (E0.3).
fn check_real_precision(parameter: &IsolatedParameterOverride) -> Result<(), Failure> {
    let numbers = [parameter.value, parameter.minimum, parameter.maximum];
    if numbers.iter().all(|&number| exact_single(number)) {
        return Ok(());
    }
    Err(refuse_request(&format!(
        "The REAL parameter {} has a value or a bound that is not exactly representable as single precision; round it to REAL first.",
        parameter.symbol
    )))
}

/// A fixed configuration `BOOL` has unit `1`, bounds exactly 0 and 1, and a value of 0 or 1 (E8.4). The bounds are
/// checked against both the specification's containment of 0 and 1 and the runtime's bounds inside [0, 1].
#[allow(clippy::float_cmp)] // The bounds and the value of a BOOL are exactly 0 or 1 (E8.4).
fn check_fixed_bool(parameter: &IsolatedParameterOverride) -> Result<(), Failure> {
    let symbol = parameter.symbol.as_str();
    if parameter.unit != "1" {
        return Err(refuse_request(&format!(
            "The BOOL parameter {symbol} has unit {}; its unit is 1.",
            parameter.unit
        )));
    }
    if parameter.minimum != 0.0 || parameter.maximum != 1.0 {
        return Err(refuse_request(&format!(
            "The BOOL parameter {symbol} has bounds {} to {}; its bounds are 0 to 1.",
            parameter.minimum, parameter.maximum
        )));
    }
    if parameter.value != 0.0 && parameter.value != 1.0 {
        return Err(refuse_request(&format!(
            "The BOOL parameter {symbol} is set to {}; it is 0 or 1.",
            parameter.value
        )));
    }
    Ok(())
}

/// Whether `value` is finite and exactly representable as single precision (E0.3).
#[allow(clippy::cast_possible_truncation, clippy::float_cmp)] // The round trip through f32 is the test itself.
fn exact_single(value: f64) -> bool {
    value.is_finite() && f64::from(value as f32) == value
}

/// The upper-case symbols of the overrides; a run's other values may not name them (E8.4).
fn parameter_symbols(parameters: &[IsolatedParameterOverride]) -> HashSet<String> {
    parameters
        .iter()
        .map(|parameter| parameter.symbol.to_ascii_uppercase())
        .collect()
}

fn check_initial_values(
    values: &BTreeMap<String, f64>,
    parameters: &HashSet<String>,
) -> Result<(), Failure> {
    if values.len() > MAXIMUM_INITIAL_VALUES {
        return Err(refuse_request(&format!(
            "The run sets {} initial values; at most 64 are allowed.",
            values.len()
        )));
    }
    let mut symbols = HashSet::new();
    for (symbol, value) in values {
        check_text("initial-value symbol", symbol)?;
        let upper = symbol.to_ascii_uppercase();
        if parameters.contains(&upper) {
            return Err(refuse_request(&format!(
                "The initial value {symbol} would set a parameter that the run overrides."
            )));
        }
        if !symbols.insert(upper) {
            return Err(refuse_request(&format!(
                "The initial value {symbol} is listed twice; symbols are compared without regard to case."
            )));
        }
        if !value.is_finite() {
            return Err(refuse_request(&format!(
                "The initial value of {symbol} is {value}, not a finite number."
            )));
        }
    }
    Ok(())
}

fn check_events(request: &IsolatedRunRequest, parameters: &HashSet<String>) -> Result<(), Failure> {
    if request.events.len() > MAXIMUM_EVENTS {
        return Err(refuse_request(&format!(
            "The run has {} events; at most 1,024 are allowed.",
            request.events.len()
        )));
    }
    let mut previous_ms: u64 = 0;
    let mut targets = HashSet::new();
    for event in &request.events {
        check_text("event symbol", &event.symbol)?;
        let symbol = event.symbol.as_str();
        if event.at_ms < previous_ms {
            return Err(refuse_request(&format!(
                "The events are not in time order: an event at {} ms follows one at {previous_ms} ms.",
                event.at_ms
            )));
        }
        if event.at_ms > request.duration_ms {
            return Err(refuse_request(&format!(
                "The event on {symbol} at {} ms lies after the end of the run at {} ms.",
                event.at_ms, request.duration_ms
            )));
        }
        if !event.at_ms.is_multiple_of(request.step_ms) {
            return Err(refuse_request(&format!(
                "The event on {symbol} at {} ms is not on a scan of {} ms.",
                event.at_ms, request.step_ms
            )));
        }
        if !event.value.is_finite() {
            return Err(refuse_request(&format!(
                "The event on {symbol} sets {}, not a finite number.",
                event.value
            )));
        }
        let upper = symbol.to_ascii_uppercase();
        if parameters.contains(&upper) {
            return Err(refuse_request(&format!(
                "The event on {symbol} would set a parameter that the run overrides."
            )));
        }
        if !targets.insert((event.at_ms, upper)) {
            return Err(refuse_request(&format!(
                "The run has two events on {symbol} at {} ms.",
                event.at_ms
            )));
        }
        previous_ms = event.at_ms;
    }
    Ok(())
}

fn check_feedback_relay(
    request: &IsolatedRunRequest,
    relay: &IsolatedFeedbackRelay,
    parameters: &HashSet<String>,
) -> Result<(), Failure> {
    check_relay_levels(relay)?;
    check_relay_timing(request, relay)?;
    let observed = |symbol: &str| {
        request
            .observations
            .iter()
            .any(|channel| channel.symbol == symbol)
    };
    let command_is_free = !parameters.contains(&relay.command.to_ascii_uppercase())
        && !request
            .events
            .iter()
            .any(|event| event.symbol.eq_ignore_ascii_case(&relay.command));
    if !command_is_free
        || !observed(relay.process_variable.as_str())
        || !observed(relay.command.as_str())
    {
        return Err(refuse_request(
            "The feedback relay must observe its measurement and its command, and its command cannot be a parameter override or an event.",
        ));
    }
    Ok(())
}

/// The relay's signals and levels: finite, a band above 0 that lies inside the measurement's bounds, low below high
/// with restore between them (E8.4).
fn check_relay_levels(relay: &IsolatedFeedbackRelay) -> Result<(), Failure> {
    check_text("relay measurement", &relay.process_variable)?;
    check_text("relay command", &relay.command)?;
    if relay.command.eq_ignore_ascii_case(&relay.process_variable) {
        return Err(refuse_request(
            "The feedback relay's measurement and command must be different signals.",
        ));
    }
    let levels = [
        relay.center,
        relay.hysteresis,
        relay.low,
        relay.high,
        relay.restore,
        relay.process_minimum,
        relay.process_maximum,
    ];
    if !levels.iter().all(|level| level.is_finite()) {
        return Err(refuse_request(
            "The feedback relay has a level that is not a finite number.",
        ));
    }
    if relay.hysteresis <= 0.0
        || relay.low >= relay.high
        || relay.restore < relay.low
        || relay.restore > relay.high
    {
        return Err(refuse_request(
            "The feedback relay needs a hysteresis above 0 and low below high, with restore between them.",
        ));
    }
    if relay.center - relay.hysteresis <= relay.process_minimum
        || relay.center + relay.hysteresis >= relay.process_maximum
    {
        return Err(refuse_request(
            "The feedback relay's switching band must lie inside the measurement's bounds.",
        ));
    }
    Ok(())
}

/// The relay's times: multiples of the step, starting before it restores and restoring within the run, with dwell
/// times shorter than the active window (E8.4).
fn check_relay_timing(
    request: &IsolatedRunRequest,
    relay: &IsolatedFeedbackRelay,
) -> Result<(), Failure> {
    let step = request.step_ms;
    let minimum_high = relay.minimum_high_ms.unwrap_or(0);
    let minimum_low = relay.minimum_low_ms.unwrap_or(0);
    if !relay.start_ms.is_multiple_of(step)
        || !relay.restore_ms.is_multiple_of(step)
        || !minimum_high.is_multiple_of(step)
        || !minimum_low.is_multiple_of(step)
    {
        return Err(refuse_request(&format!(
            "The feedback relay's times must be multiples of the step of {step} ms."
        )));
    }
    if relay.start_ms >= relay.restore_ms || relay.restore_ms > request.duration_ms {
        return Err(refuse_request(&format!(
            "The feedback relay starts at {} ms and restores at {} ms of a {} ms run; it must start before it restores, within the run.",
            relay.start_ms, relay.restore_ms, request.duration_ms
        )));
    }
    if minimum_high.max(minimum_low) >= relay.restore_ms - relay.start_ms {
        return Err(refuse_request(
            "The feedback relay's minimum dwell times must be shorter than its active window.",
        ));
    }
    Ok(())
}

fn check_simulation_override(selected: &SimulationOverride) -> Result<(), Failure> {
    if selected.model_revision.is_empty()
        || selected.model_revision.len() > MAXIMUM_MODEL_REVISION_BYTES
    {
        return Err(refuse_request(&format!(
            "The selected model's revision is {} bytes; it must be 1 to 256 bytes.",
            selected.model_revision.len()
        )));
    }
    if selected.model_provenance.is_empty()
        || selected.model_provenance.len() > MAXIMUM_MODEL_PROVENANCE_BYTES
    {
        return Err(refuse_request(&format!(
            "The selected model's provenance is {} bytes; it must be 1 to 4,096 bytes.",
            selected.model_provenance.len()
        )));
    }
    if selected.simulation_toml.is_empty()
        || selected.simulation_toml.len() > MAXIMUM_SIMULATION_TOML_BYTES
    {
        return Err(refuse_request(&format!(
            "The selected model's simulation configuration is {} bytes; it must be 1 to 262,144 bytes.",
            selected.simulation_toml.len()
        )));
    }
    Ok(())
}
