//! `plant.topology`: the simulated plants the Devices & Connections canvas shows (specification 47
//! §20.6). The saved plants of `plant.read` are joined with the owning runtime's `plant.status`
//! answer: the stopped-project session takes the answer the extension got, as it came; the runtime
//! itself takes its own. Live values come only from a current status of the same project, and a
//! plant's operating state and live sample only from its own entry there. Its configuration is the
//! saved table as the plant editor opens it, and its states are those of Section 5.2 that hold,
//! primary first.

use serde::Deserialize;
use serde_json::{json, Map, Value};

use super::editor::is_editor_actuator_field;
use super::{
    admit, handle_plant_status, saved_plants, PlantHandlerContext, CONTRACT, SCHEMA_VERSION,
};
use crate::control::ControlResponse;

#[cfg(test)]
mod tests;

#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
struct PlantTopologyParams {
    contract: String,
    schema_version: u64,
    /// Optional, as for `plant.read`: the request may establish the producer's project ID.
    #[serde(default)]
    project_id: Option<String>,
    /// The owning runtime's `plant.status` answer as the extension got it; absent or null when no
    /// runtime answered. Any other value is a status that is not current. The runtime itself takes
    /// no status: it projects its own.
    #[serde(default)]
    status: Option<Value>,
}

pub(super) fn handle_plant_topology(
    id: u64,
    params: Option<Value>,
    context: &PlantHandlerContext<'_>,
) -> ControlResponse {
    let params = match serde_json::from_value::<PlantTopologyParams>(params.unwrap_or(Value::Null))
    {
        Ok(params) => params,
        Err(error) => {
            return ControlResponse::error_with_code(
                id,
                format!("invalid plant.topology payload: {error}"),
                "malformed_request",
            );
        }
    };
    let project_id = match admit(
        id,
        &params.contract,
        params.schema_version,
        params.project_id.as_deref(),
        context,
    ) {
        Ok(project_id) => project_id,
        Err(response) => return response,
    };
    let Some(project_root) = context.project_root else {
        return ControlResponse::error_with_code(
            id,
            "plant project root is unavailable".to_string(),
            "io_failure",
        );
    };
    // The runtime control surface projects the runtime's own `plant.status` (§20.6).
    let status = if context.resource.is_some() {
        if params.status.is_some() {
            return ControlResponse::error_with_code(
                id,
                "plant.topology on a runtime projects the runtime's own status".to_string(),
                "malformed_request",
            );
        }
        let own = handle_plant_status(
            id,
            Some(json!({
                "contract": CONTRACT,
                "schema_version": SCHEMA_VERSION,
                "project_id": project_id,
            })),
            context,
        );
        if own.ok {
            own.result
        } else {
            None
        }
    } else {
        params.status
    };
    let read = match saved_plants(id, project_root, &project_id) {
        Ok(read) => read,
        Err(response) => return response,
    };
    ControlResponse::ok(
        id,
        json!({
            "contract": CONTRACT,
            "schema_version": SCHEMA_VERSION,
            "operation": "plant.topology",
            "project_id": project_id,
            "configuration_revision": read["configuration_revision"],
            "plants": project(&read, status.as_ref()),
        }),
    )
}

/// The canvas plants of a `plant.read` answer, joined with the runtime's `plant.status` answer.
/// A saved plant whose descriptor is incomplete is left out, and a status entry without a saved
/// plant adds none.
fn project(read: &Value, status: Option<&Value>) -> Vec<Value> {
    let project_id = text(read, "project_id").unwrap_or_default();
    let current = status.filter(|status| is_current(status, project_id));
    let saved_revision = text(read, "configuration_revision");
    let active_revision = current.and_then(|status| text(status, "active_configuration_revision"));
    let saved_configuration = read.get("saved_configuration");
    let configured = tables(saved_configuration.and_then(|saved| saved.get("plants")));
    let live_plants = tables(current.and_then(|status| status.get("plants")));

    tables(read.get("plants"))
        .into_iter()
        .filter_map(|descriptor| {
            let plant = SavedPlant::read(descriptor)?;
            let live = live_plants
                .iter()
                .copied()
                .find(|entry| entry.get("id").and_then(Value::as_str) == Some(plant.id));
            let mut projected = Map::new();
            projected.insert("id".into(), json!(plant.id));
            projected.insert("projectId".into(), json!(project_id));
            projected.insert("displayName".into(), json!(plant.display_name));
            projected.insert("model".into(), json!(plant.model));
            projected.insert("enabled".into(), json!(plant.enabled));
            projected.insert("commandSources".into(), json!(plant.command_sources));
            projected.insert("feedbackTargets".into(), json!(plant.feedback_targets));
            // A current status answers for every saved plant: one it does not name (disabled, or
            // saved after the runtime started) is not running, not unknown.
            let available = current.is_some();
            projected.insert(
                "statusAvailability".into(),
                json!(if available {
                    "available"
                } else {
                    "unavailable"
                }),
            );
            if let Some(revision) = saved_revision {
                projected.insert("savedConfigurationRevision".into(), json!(revision));
            }
            if let Some(revision) = active_revision {
                projected.insert("activeConfigurationRevision".into(), json!(revision));
            }
            if let Some(configuration) = configured
                .iter()
                .find(|table| table.get("id").and_then(Value::as_str) == Some(plant.id))
                .and_then(|table| editor_configuration(table, saved_configuration, plant.id))
            {
                projected.insert("configuration".into(), configuration);
            }
            let operating_state = live
                .and_then(|live| live.get("operating_state"))
                .and_then(Value::as_str)
                .filter(|state| matches!(*state, "running" | "faulted"));
            if let Some(state) = operating_state {
                projected.insert("activeOperatingState".into(), json!(state));
            }
            let sample = live.filter(|live| advanced(live)).and_then(|live| {
                Some((
                    number_list(live.get("command"))?,
                    number_list(live.get("published_feedback"))?,
                ))
            });
            if let Some((command, feedback)) = &sample {
                projected.insert("hasCompletedSample".into(), json!(true));
                projected.insert(
                    "liveSample".into(),
                    json!({ "command": command, "feedback": feedback }),
                );
            }
            projected.insert(
                "states".into(),
                json!(states(&PlantEvidence {
                    enabled: plant.enabled,
                    available,
                    operating_state,
                    completed_sample: sample.is_some(),
                    saved_revision,
                    active_revision,
                })),
            );
            Some(Value::Object(projected))
        })
        .collect()
}

/// Whether a plant's simulation has advanced, so that its status entry holds a completed sample:
/// its `simulation_time_ns`, a decimal string, is above zero. Before its first step a plant holds
/// its starting values, which are not a sample (specification 47 §5.2, §5.3).
fn advanced(live: &Map<String, Value>) -> bool {
    live.get("simulation_time_ns")
        .and_then(Value::as_str)
        .and_then(|nanoseconds| nanoseconds.parse::<u128>().ok())
        .is_some_and(|nanoseconds| nanoseconds > 0)
}

/// A status counts only when it is an available `plant.status` answer of this contract and this
/// project; a missing, refused, malformed or wrong-project status marks every plant unavailable.
fn is_current(status: &Value, project_id: &str) -> bool {
    text(status, "contract") == Some(CONTRACT)
        && status.get("schema_version").and_then(Value::as_u64) == Some(SCHEMA_VERSION)
        && text(status, "operation") == Some("plant.status")
        && text(status, "project_id") == Some(project_id)
        && status.get("available").and_then(Value::as_bool) == Some(true)
}

/// A saved plant's descriptor as the canvas needs it: one that lacks a field, or names an empty
/// ID, display name or model, is not shown (specification 47 §20.1: an empty identifier is
/// invalid).
struct SavedPlant<'a> {
    id: &'a str,
    display_name: &'a str,
    model: &'a str,
    enabled: bool,
    command_sources: Vec<&'a str>,
    feedback_targets: Vec<&'a str>,
}

impl<'a> SavedPlant<'a> {
    fn read(descriptor: &'a Map<String, Value>) -> Option<Self> {
        Some(Self {
            id: named_text(descriptor, "id")?,
            display_name: named_text(descriptor, "display_name")?,
            model: named_text(descriptor, "model")?,
            enabled: descriptor.get("enabled")?.as_bool()?,
            command_sources: text_list(descriptor.get("command_sources"))?,
            feedback_targets: text_list(descriptor.get("feedback_targets"))?,
        })
    }
}

/// What a plant's states are derived from.
struct PlantEvidence<'a> {
    enabled: bool,
    available: bool,
    operating_state: Option<&'a str>,
    completed_sample: bool,
    saved_revision: Option<&'a str>,
    active_revision: Option<&'a str>,
}

/// Every true state of a saved plant, in the order of specification 47 §5.2: the first is the
/// node's state and the others are the inspector's details. `Configured` holds when no other does.
fn states(evidence: &PlantEvidence<'_>) -> Vec<&'static str> {
    let mut states = Vec::new();
    if evidence.available && evidence.operating_state == Some("faulted") {
        states.push("Faulted");
    }
    if let (Some(active), Some(saved)) = (
        named(evidence.active_revision),
        named(evidence.saved_revision),
    ) && active != saved
    {
        states.push("Restart required");
    }
    if !evidence.enabled {
        states.push("Disabled");
    }
    if !evidence.available {
        states.push("Status unavailable");
    }
    if evidence.available && evidence.operating_state == Some("running") {
        states.push(if evidence.completed_sample {
            "Running"
        } else {
            "Waiting for samples"
        });
    }
    if states.is_empty() {
        states.push("Configured");
    }
    states
}

/// A revision that names one: an empty text names none.
fn named(revision: Option<&str>) -> Option<&str> {
    revision.filter(|revision| !revision.is_empty())
}

/// The saved plant table as the plant editor opens it (specification 47 §6): absent when its model
/// values are not numbers or its initial state is not a list of numbers. Rust's `open` edit then
/// makes the defaults explicit; the editor's Save carries `preservedConfiguration`, the saved
/// table, so the fields the editor does not manage keep their saved values (§13.2).
fn editor_configuration(
    table: &Map<String, Value>,
    saved_configuration: Option<&Value>,
    plant_id: &str,
) -> Option<Value> {
    let parameters = numeric_table(table.get("parameters"))?;
    let initial_state_defaulted = !table.contains_key("initial_state");
    let initial_state = if initial_state_defaulted {
        Vec::new()
    } else {
        number_list(table.get("initial_state"))?
    };
    let mut configuration = Map::new();
    configuration.insert(
        "description".into(),
        json!(table
            .get("description")
            .and_then(Value::as_str)
            .unwrap_or_default()),
    );
    configuration.insert("parameters".into(), parameters);
    configuration.insert("initialState".into(), json!(initial_state));
    if !table.contains_key("enabled") {
        configuration.insert("enabledDefaulted".into(), json!(true));
    }
    if initial_state_defaulted {
        configuration.insert("initialStateDefaulted".into(), json!(true));
    }
    configuration.insert(
        "preservedConfiguration".into(),
        Value::Object(table.clone()),
    );
    if let Some(step) = table.get("step_ms").filter(|step| step.is_number()) {
        configuration.insert("stepMs".into(), step.clone());
    }
    for (saved, key) in [
        ("initial_input", "initialInput"),
        ("input_scale", "inputScale"),
        ("input_offset", "inputOffset"),
        ("output_scale", "outputScale"),
        ("output_offset", "outputOffset"),
    ] {
        if let Some(list) = number_list(table.get(saved)) {
            configuration.insert(key.into(), json!(list));
        }
    }
    // The actuator fields the editor manages, as saved (a value that is not a number is the
    // editor's to report). The saved actuator's other keys (its `kind`, minimum on and off times)
    // stay in `preservedConfiguration`, which Save carries (§13.2).
    let actuator: Map<String, Value> = table
        .get("actuator")
        .and_then(Value::as_object)
        .map(|saved| {
            saved
                .iter()
                .filter(|(key, _)| is_editor_actuator_field(key))
                .map(|(key, value)| (key.clone(), value.clone()))
                .collect()
        })
        .unwrap_or_default();
    if !actuator.is_empty() {
        configuration.insert("actuator".into(), Value::Object(actuator));
    }
    if let Some(effects) = table.get("uncertainties").and_then(Value::as_array) {
        // A saved effect is an enabled one: the file holds only the effects the plant applies.
        let effects = effects
            .iter()
            .filter_map(Value::as_object)
            .map(|effect| {
                let mut effect = effect.clone();
                effect.insert("enabled".into(), json!(true));
                Value::Object(effect)
            })
            .collect::<Vec<_>>();
        configuration.insert("uncertainties".into(), Value::Array(effects));
    }
    configuration.insert(
        "scenarioEvents".into(),
        Value::Array(scenario_events(saved_configuration, plant_id)),
    );
    Some(Value::Object(configuration))
}

/// The saved scenario events of one plant as the editor's rows (specification 47 §12). The file
/// holds only included events, so each row is enabled; a time that is not a number is null, which
/// the editor's checks report.
fn scenario_events(saved_configuration: Option<&Value>, plant_id: &str) -> Vec<Value> {
    tables(saved_configuration.and_then(|saved| saved.get("plant_events")))
        .into_iter()
        .filter(|event| event.get("plant").and_then(Value::as_str) == Some(plant_id))
        .map(|event| {
            let mut row = Map::new();
            row.insert("enabled".into(), json!(true));
            row.insert(
                "atMs".into(),
                event
                    .get("at_ms")
                    .filter(|at| at.is_number())
                    .cloned()
                    .unwrap_or(Value::Null),
            );
            row.insert(
                "kind".into(),
                json!(event
                    .get("kind")
                    .and_then(Value::as_str)
                    .unwrap_or_default()),
            );
            if let Some(channel) = event.get("channel").filter(|channel| channel.is_number()) {
                row.insert("channel".into(), channel.clone());
            }
            if let Some(parameter) = event
                .get("parameter")
                .filter(|parameter| parameter.is_string())
            {
                row.insert("parameter".into(), parameter.clone());
            }
            if let Some(value) = event.get("value").filter(|value| value.is_number()) {
                row.insert("value".into(), value.clone());
            }
            Value::Object(row)
        })
        .collect()
}

fn text<'a>(value: &'a Value, key: &str) -> Option<&'a str> {
    value.get(key).and_then(Value::as_str)
}

/// A non-empty text of a table.
fn named_text<'a>(table: &'a Map<String, Value>, key: &str) -> Option<&'a str> {
    table
        .get(key)
        .and_then(Value::as_str)
        .filter(|text| !text.is_empty())
}

/// The tables of a list; entries that are not tables are skipped.
fn tables(value: Option<&Value>) -> Vec<&Map<String, Value>> {
    value
        .and_then(Value::as_array)
        .map(|list| list.iter().filter_map(Value::as_object).collect())
        .unwrap_or_default()
}

fn text_list(value: Option<&Value>) -> Option<Vec<&str>> {
    value?.as_array()?.iter().map(Value::as_str).collect()
}

/// A list of finite numbers, kept as written.
fn number_list(value: Option<&Value>) -> Option<Vec<Value>> {
    let list = value?.as_array()?;
    list.iter().all(is_finite_number).then(|| list.clone())
}

/// A table whose values are finite numbers or lists, nested to any depth, of finite numbers: a
/// model's parameters (scalars, vectors and matrices).
fn numeric_table(value: Option<&Value>) -> Option<Value> {
    let table = value?.as_object()?;
    table
        .values()
        .all(numeric_leaves)
        .then(|| Value::Object(table.clone()))
}

fn numeric_leaves(value: &Value) -> bool {
    match value {
        Value::Array(items) => items.iter().all(numeric_leaves),
        other => is_finite_number(other),
    }
}

fn is_finite_number(value: &Value) -> bool {
    value.as_f64().is_some_and(f64::is_finite)
}
