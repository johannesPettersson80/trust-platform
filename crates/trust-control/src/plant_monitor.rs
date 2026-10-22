//! The plant summary of a plant handoff (specification 35, §0.1a; specification 34, §20.4): the engine reads the active runtime's
//! `plant.status` and answers the plant's status, or the reason it is unavailable. This module decides it: which reads are not
//! made (a stopped runtime, a blank endpoint), the decode of the runtime's answer as specification 34 names it, the stale-revision
//! and missing-plant refusals, and the reason of a failed read. The read itself is the engine's runtime client, passed in.

#![allow(clippy::module_name_repetitions)] // The types name their concept: the plant summary.

use schemars::JsonSchema;
use serde::{Deserialize, Serialize};
use serde_json::{Map, Value};

use crate::loop_monitor::MonitorSource;
use crate::runtime::RuntimeConnection;

/// The sentences that say why a plant summary is unavailable (specification 35, §0.1a).
pub const STOPPED: &str = "Runtime is stopped";
/// The runtime cannot be reached, or no endpoint is set.
pub const UNREACHABLE: &str = "Runtime is unreachable";
/// The runtime refused the control token.
pub const AUTHORIZATION_REJECTED: &str = "Runtime authorization was rejected";
/// The runtime does not answer `plant.status`.
pub const UNSUPPORTED: &str = "Plant status is unsupported";
/// The runtime's answer is not the `plant.status` answer of specification 34, §20.4.
pub const MALFORMED: &str = "Plant status is malformed";
/// The running project has no plant with the handoff's identifier.
pub const NO_PLANT: &str = "Plant no longer exists";
/// The runtime runs another active configuration revision than the handoff names.
pub const STALE: &str = "Running plant revision is stale";

const CONTRACT: &str = "trust.plant";
const OPERATION: &str = "plant.status";
const REVISION_PREFIX: &str = "sha256:";
const PROJECT_PREFIX: &str = "project:sha256:";
const SAFE_INTEGER_MAX: u64 = 9_007_199_254_740_991;

/// The top-level fields of a `plant.status` answer (specification 34, §20.4), exactly as the runtime writes them.
const STATUS_FIELDS: [&str; 14] = [
    "contract",
    "schema_version",
    "operation",
    "project_id",
    "available",
    "active_configuration_revision",
    "simulation_time_ns",
    "effective_seed",
    "random_algorithm",
    "random_algorithm_version",
    "runtime_version",
    "time_scale",
    "restart_policy",
    "plants",
];

/// The fields of one plant's status in a `plant.status` answer.
const PLANT_FIELDS: [&str; 20] = [
    "id",
    "model",
    "operating_state",
    "enabled",
    "step_ns",
    "simulation_time_ns",
    "state",
    "command",
    "applied_input",
    "ideal_output",
    "measured_feedback",
    "published_feedback",
    "actuators",
    "active_uncertainties",
    "active_dropouts",
    "pending_event_count",
    "delay_buffer_depth",
    "accumulated_remainder_ns",
    "last_substeps",
    "current_fault",
];

/// The fields of one actuator's flags in a `plant.status` answer.
const ACTUATOR_FIELDS: [&str; 7] = [
    "position",
    "applied",
    "saturation_active",
    "slew_limit_active",
    "deadband_active",
    "stiction_active",
    "stuck",
];

/// The arguments of `controlStudio/plantMonitor`: the handoff's plant, the state of the runtime the panel saw, and the endpoint
/// to read. The connection's token is a credential: it is never journaled (E11.2).
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct PlantMonitorRequest {
    /// The project of the plant handoff; the runtime's answer must name it.
    pub project_id: String,
    /// The plant the handoff names.
    pub plant_id: String,
    /// The active configuration revision the handoff names; a runtime that runs another one is stale. Absent when it names none.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub active_configuration_digest: Option<String>,
    /// Whether the runtime's state is stopped.
    pub runtime_stopped: bool,
    /// Whether the endpoint answered the probe the panel made.
    pub endpoint_reachable: bool,
    /// The endpoint to read and its control token.
    pub connection: RuntimeConnection,
}

/// Whether the plant's status is available.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, JsonSchema)]
#[serde(rename_all = "lowercase")]
pub enum PlantAvailability {
    /// The runtime answered the plant's status.
    Available,
    /// The status cannot be shown; the reason says why.
    Unavailable,
}

/// The plant's operating state, as the runtime reports it.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, JsonSchema)]
#[serde(rename_all = "lowercase")]
pub enum OperatingState {
    /// The plant runs.
    Running,
    /// The plant has a fault.
    Faulted,
}

/// One actuator's position and flags (specification 34, §20.4).
#[allow(clippy::struct_excessive_bools)] // The runtime reports each actuator flag as its own boolean.
#[derive(Debug, Clone, PartialEq, Serialize, JsonSchema)]
#[serde(rename_all = "camelCase")]
pub struct PlantActuatorStatus {
    /// The actuator's position.
    pub position: f64,
    /// The value the actuator applied.
    pub applied: f64,
    /// Whether the actuator is saturated.
    pub saturation_active: bool,
    /// Whether the slew limit is active.
    pub slew_limit_active: bool,
    /// Whether the deadband is active.
    pub deadband_active: bool,
    /// Whether stiction is active.
    pub stiction_active: bool,
    /// Whether the actuator is stuck.
    pub stuck: bool,
}

/// The status of one plant, with the runtime's active configuration revision and clock facts that every plant shares.
#[derive(Debug, Clone, PartialEq, Serialize, JsonSchema)]
#[serde(rename_all = "camelCase")]
pub struct PlantStatusView {
    /// The active configuration revision the runtime runs.
    pub active_configuration_revision: String,
    /// The runtime's version.
    pub runtime_version: String,
    /// The plant's simulation time, in nanoseconds, as a decimal text.
    pub simulation_time_ns: String,
    /// Whether the plant runs or has a fault.
    pub operating_state: OperatingState,
    /// The plant's model kind.
    pub model: String,
    /// The command the plant receives, by channel.
    pub command: Vec<f64>,
    /// The input the plant applied, by channel.
    pub applied_input: Vec<f64>,
    /// The output the ideal model gives, by channel.
    pub ideal_output: Vec<f64>,
    /// The measured feedback, by channel.
    pub measured_feedback: Vec<f64>,
    /// The published feedback, by channel.
    pub published_feedback: Vec<f64>,
    /// The plant's state vector.
    pub state: Vec<f64>,
    /// The simulation's time scale.
    pub time_scale: f64,
    /// The seed the simulation's randomness was drawn with, as a decimal text.
    pub effective_seed: String,
    /// The random algorithm's name.
    pub random_algorithm: String,
    /// The random algorithm's version.
    pub random_algorithm_version: u64,
    /// Each actuator's position and flags.
    pub actuators: Vec<PlantActuatorStatus>,
    /// The uncertainties that are active.
    pub active_uncertainties: Vec<String>,
    /// The dropouts that are active.
    pub active_dropouts: Vec<String>,
    /// The events the plant has not yet applied.
    pub pending_event_count: u64,
    /// The delay buffer's depth.
    pub delay_buffer_depth: u64,
    /// The substeps of the last cycle.
    pub last_substeps: u64,
    /// The plant's fault, when it has one.
    pub current_fault: Option<String>,
}

/// What `controlStudio/plantMonitor` answers (specification 35, §0.1a): the plant's summary, unavailable with its reason or
/// available with its status. The page shows the answer; it decides nothing.
#[derive(Debug, Clone, PartialEq, Serialize, JsonSchema)]
#[serde(rename_all = "camelCase")]
pub struct PlantMonitorResult {
    /// Whether the status is available.
    pub availability: PlantAvailability,
    /// The plant's status is a simulation's (specification 34, §20.4).
    pub source: MonitorSource,
    /// The plant the handoff names.
    pub plant_id: String,
    /// The sentence that says why the status is unavailable; absent when it is available.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub reason: Option<String>,
    /// The plant's status; absent when it is unavailable.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub status: Option<PlantStatusView>,
}

/// Answers `controlStudio/plantMonitor` (specification 34, §20.4; specification 35, §0.1a). The runtime is read only when the
/// panel's state allows it: a stopped runtime that does not answer its probe is not read, and a blank endpoint is unreachable.
/// `read` makes the one read of `plant.status` for the connection, with the endpoint trimmed; it answers the runtime's answer, or
/// the text of why the read failed (the refusal code and the failure's words), which decides the reason.
#[must_use]
pub fn answer(
    request: &PlantMonitorRequest,
    read: impl FnOnce(&RuntimeConnection) -> Result<Value, String>,
) -> PlantMonitorResult {
    if request.runtime_stopped && !request.endpoint_reachable {
        return unavailable(&request.plant_id, STOPPED);
    }
    let endpoint = request.connection.endpoint.trim();
    if endpoint.is_empty() {
        return unavailable(&request.plant_id, UNREACHABLE);
    }
    let connection = RuntimeConnection {
        endpoint: endpoint.to_string(),
        token: request.connection.token.clone(),
        runtime_id: request.connection.runtime_id.clone(),
    };
    match read(&connection) {
        Ok(value) => status_answer(&value, request),
        Err(text) => unavailable(&request.plant_id, read_reason(&text)),
    }
}

/// The reason of a failed read, from its text (the refusal code and what the runtime said): a refused token, a `plant.status`
/// the runtime does not support, or otherwise a runtime that cannot be reached.
#[must_use]
pub fn read_reason(text: &str) -> &'static str {
    let text = text.to_ascii_lowercase();
    // "unauthorized" contains "auth", so one marker covers both spellings.
    if text.contains("auth") {
        AUTHORIZATION_REJECTED
    } else if text.contains("unsupported") {
        UNSUPPORTED
    } else {
        UNREACHABLE
    }
}

fn unavailable(plant_id: &str, reason: &str) -> PlantMonitorResult {
    PlantMonitorResult {
        availability: PlantAvailability::Unavailable,
        source: MonitorSource::Simulation,
        plant_id: plant_id.to_string(),
        reason: Some(reason.to_string()),
        status: None,
    }
}

/// The answer of a read that returned `value`: malformed, stale, missing, or the plant's status.
fn status_answer(value: &Value, request: &PlantMonitorRequest) -> PlantMonitorResult {
    let Some(status) = read_status(value, &request.project_id) else {
        return unavailable(&request.plant_id, MALFORMED);
    };
    let stale = request
        .active_configuration_digest
        .as_deref()
        .is_some_and(|digest| !digest.is_empty() && digest != status.revision.as_str());
    if stale {
        return unavailable(&request.plant_id, STALE);
    }
    match status
        .plants
        .into_iter()
        .find(|plant| plant.id == request.plant_id)
    {
        Some(plant) => PlantMonitorResult {
            availability: PlantAvailability::Available,
            source: MonitorSource::Simulation,
            plant_id: request.plant_id.clone(),
            reason: None,
            status: Some(plant.view),
        },
        None => unavailable(&request.plant_id, NO_PLANT),
    }
}

/// The runtime's active revision and each plant's status, decoded.
struct ActiveStatus {
    revision: String,
    plants: Vec<PlantRead>,
}

/// One plant of the answer: its identifier and its status.
struct PlantRead {
    id: String,
    view: PlantStatusView,
}

/// The top-level values that every plant's status shares.
struct Top {
    revision: String,
    runtime_version: String,
    time_scale: f64,
    effective_seed: String,
    random_algorithm: String,
    random_algorithm_version: u64,
}

/// Decodes the runtime's `plant.status` answer for `project_id`, as specification 34, §20.4 names its form: the top-level fields
/// exactly, the project and the operation it names, and every plant with its own exact fields. An answer of another form is not
/// decoded (`None`).
fn read_status(value: &Value, project_id: &str) -> Option<ActiveStatus> {
    let message = value.as_object()?;
    if message.get("schema_version").and_then(Value::as_u64) != Some(1)
        || message.get("contract").and_then(Value::as_str) != Some(CONTRACT)
        || message.get("operation").and_then(Value::as_str) != Some(OPERATION)
        || !has_exactly(message, &STATUS_FIELDS)
        || message.get("available") != Some(&Value::Bool(true))
    {
        return None;
    }
    let project = message.get("project_id").and_then(Value::as_str)?;
    if !project.starts_with(PROJECT_PREFIX) || project != project_id {
        return None;
    }
    let top = Top {
        revision: revision_text(message.get("active_configuration_revision"))?,
        runtime_version: non_empty_text(message.get("runtime_version"))?,
        time_scale: finite(message.get("time_scale")).filter(|scale| *scale > 0.0)?,
        effective_seed: decimal_u64(message.get("effective_seed"))?,
        random_algorithm: non_empty_text(message.get("random_algorithm"))?,
        random_algorithm_version: safe_count(message.get("random_algorithm_version"))?,
    };
    decimal_u128(message.get("simulation_time_ns"))?;
    non_empty_text(message.get("restart_policy"))?;
    let plants = message
        .get("plants")?
        .as_array()?
        .iter()
        .map(|plant| read_plant(plant, &top))
        .collect::<Option<Vec<PlantRead>>>()?;
    Some(ActiveStatus {
        revision: top.revision,
        plants,
    })
}

/// One plant's status, when it has exactly the fields of specification 34, §20.4 and each holds its form.
fn read_plant(value: &Value, top: &Top) -> Option<PlantRead> {
    let plant = value.as_object()?;
    if !has_exactly(plant, &PLANT_FIELDS) {
        return None;
    }
    let id = non_empty_text(plant.get("id"))?;
    let model = non_empty_text(plant.get("model"))?;
    let operating_state = match plant.get("operating_state").and_then(Value::as_str)? {
        "running" => OperatingState::Running,
        "faulted" => OperatingState::Faulted,
        _ => return None,
    };
    plant.get("enabled").and_then(Value::as_bool)?;
    decimal_u128(plant.get("step_ns"))?;
    let simulation_time_ns = decimal_u128(plant.get("simulation_time_ns"))?;
    let actuators = plant
        .get("actuators")?
        .as_array()?
        .iter()
        .map(read_actuator)
        .collect::<Option<Vec<PlantActuatorStatus>>>()?;
    decimal_u128(plant.get("accumulated_remainder_ns"))?;
    let current_fault = match plant.get("current_fault")? {
        Value::Null => None,
        Value::String(text) => Some(text.clone()),
        _ => return None,
    };
    Some(PlantRead {
        id,
        view: PlantStatusView {
            active_configuration_revision: top.revision.clone(),
            runtime_version: top.runtime_version.clone(),
            simulation_time_ns,
            operating_state,
            model,
            command: finite_vector(plant.get("command"))?,
            applied_input: finite_vector(plant.get("applied_input"))?,
            ideal_output: finite_vector(plant.get("ideal_output"))?,
            measured_feedback: finite_vector(plant.get("measured_feedback"))?,
            published_feedback: finite_vector(plant.get("published_feedback"))?,
            state: finite_vector(plant.get("state"))?,
            time_scale: top.time_scale,
            effective_seed: top.effective_seed.clone(),
            random_algorithm: top.random_algorithm.clone(),
            random_algorithm_version: top.random_algorithm_version,
            actuators,
            active_uncertainties: text_vector(plant.get("active_uncertainties"))?,
            active_dropouts: text_vector(plant.get("active_dropouts"))?,
            pending_event_count: safe_count(plant.get("pending_event_count"))?,
            delay_buffer_depth: safe_count(plant.get("delay_buffer_depth"))?,
            last_substeps: safe_count(plant.get("last_substeps"))?,
            current_fault,
        },
    })
}

/// One actuator's flags, when it has exactly its fields and each holds its form.
fn read_actuator(value: &Value) -> Option<PlantActuatorStatus> {
    let actuator = value.as_object()?;
    if !has_exactly(actuator, &ACTUATOR_FIELDS) {
        return None;
    }
    Some(PlantActuatorStatus {
        position: finite(actuator.get("position"))?,
        applied: finite(actuator.get("applied"))?,
        saturation_active: actuator.get("saturation_active").and_then(Value::as_bool)?,
        slew_limit_active: actuator.get("slew_limit_active").and_then(Value::as_bool)?,
        deadband_active: actuator.get("deadband_active").and_then(Value::as_bool)?,
        stiction_active: actuator.get("stiction_active").and_then(Value::as_bool)?,
        stuck: actuator.get("stuck").and_then(Value::as_bool)?,
    })
}

/// Whether the object holds exactly `fields` and no other key.
fn has_exactly(object: &Map<String, Value>, fields: &[&str]) -> bool {
    object.len() == fields.len() && fields.iter().all(|field| object.contains_key(*field))
}

fn non_empty_text(value: Option<&Value>) -> Option<String> {
    value?
        .as_str()
        .filter(|text| !text.is_empty())
        .map(str::to_owned)
}

/// A configuration revision: `sha256:` and at least one more character (specification 34, §20.4).
fn revision_text(value: Option<&Value>) -> Option<String> {
    non_empty_text(value).filter(|revision| {
        revision.starts_with(REVISION_PREFIX) && revision.len() > REVISION_PREFIX.len()
    })
}

/// A canonical decimal text: `0`, or digits with no leading zero.
fn canonical_decimal(value: Option<&Value>) -> Option<&str> {
    let text = value?.as_str()?;
    let digits = text.bytes().all(|byte| byte.is_ascii_digit());
    (text == "0" || (digits && !text.is_empty() && !text.starts_with('0'))).then_some(text)
}

fn decimal_u128(value: Option<&Value>) -> Option<String> {
    canonical_decimal(value)
        .filter(|text| text.parse::<u128>().is_ok())
        .map(str::to_owned)
}

fn decimal_u64(value: Option<&Value>) -> Option<String> {
    canonical_decimal(value)
        .filter(|text| text.parse::<u64>().is_ok())
        .map(str::to_owned)
}

/// A count the page can read exactly: a JSON integer no larger than 2^53 - 1.
fn safe_count(value: Option<&Value>) -> Option<u64> {
    value?.as_u64().filter(|count| *count <= SAFE_INTEGER_MAX)
}

fn finite(value: Option<&Value>) -> Option<f64> {
    value?.as_f64().filter(|number| number.is_finite())
}

fn finite_vector(value: Option<&Value>) -> Option<Vec<f64>> {
    value?
        .as_array()?
        .iter()
        .map(|entry| finite(Some(entry)))
        .collect()
}

fn text_vector(value: Option<&Value>) -> Option<Vec<String>> {
    value?
        .as_array()?
        .iter()
        .map(|entry| entry.as_str().map(str::to_owned))
        .collect()
}

#[cfg(test)]
#[path = "plant_monitor_tests.rs"]
mod tests;
