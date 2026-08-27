use std::fs::{self, OpenOptions};
use std::io::Write;
use std::path::{Path, PathBuf};

use serde::Deserialize;
use serde_json::{json, Value};
use sha2::{Digest, Sha256};
use toml_edit::{ArrayOfTables, DocumentMut, Item};

use super::{ControlResponse, ControlState, PlantControlService};
use crate::scheduler::{ResourceCommand, ResourceControl, StdClock};

mod remove;
mod status;
mod trace;
pub(super) use status::handle_plant_status;
pub(super) use trace::{handle_plant_trace_clear, handle_plant_trace_read};

const CONTRACT: &str = "trust.plant";
const SCHEMA_VERSION: u64 = 1;
const MAX_TRACE_ROWS_PER_PLANT: u64 = 100_000;

pub(super) struct PlantHandlerContext<'a> {
    pub(super) project_root: Option<&'a Path>,
    pub(super) plant_service: &'a PlantControlService,
    pub(super) resource: Option<&'a ResourceControl<StdClock>>,
}

impl<'a> PlantHandlerContext<'a> {
    pub(super) fn runtime(state: &'a ControlState) -> Self {
        Self {
            project_root: state.project_root.as_deref(),
            plant_service: state.plant_service.as_ref(),
            resource: Some(&state.resource),
        }
    }

    pub(super) fn offline(project_root: &'a Path, plant_service: &'a PlantControlService) -> Self {
        Self {
            project_root: Some(project_root),
            plant_service,
            resource: None,
        }
    }
}

pub(super) fn dispatch(
    id: u64,
    operation: &str,
    params: Option<Value>,
    context: &PlantHandlerContext<'_>,
) -> Option<ControlResponse> {
    Some(match operation {
        "plant.capabilities" => handle_plant_capabilities(id, params, context),
        "plant.read" => handle_plant_read(id, params, context),
        "plant.status" => handle_plant_status(id, params, context),
        "plant.trace.read" => handle_plant_trace_read(id, params, context),
        "plant.trace.clear" => handle_plant_trace_clear(id, params, context),
        "plant.save" => handle_plant_save(id, params, context),
        "plant.remove" => remove::handle_plant_remove(id, params, context),
        "plant.validate" => handle_plant_validate(id, params, context),
        _ => return None,
    })
}

#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
struct PlantReadParams {
    contract: String,
    schema_version: u64,
    #[serde(default)]
    project_id: Option<String>,
}

#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
struct PlantSaveParams {
    contract: String,
    schema_version: u64,
    project_id: String,
    operation_id: String,
    expected_revision: String,
    plant: Value,
}

#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
struct PlantValidateParams {
    contract: String,
    schema_version: u64,
    project_id: String,
    plant: Value,
}

pub(super) fn handle_plant_capabilities(
    id: u64,
    params: Option<Value>,
    context: &PlantHandlerContext<'_>,
) -> ControlResponse {
    let params = match serde_json::from_value::<PlantReadParams>(params.unwrap_or(Value::Null)) {
        Ok(params) => params,
        Err(error) => {
            return ControlResponse::error_with_code(
                id,
                format!("invalid plant.capabilities payload: {error}"),
                "malformed_request",
            );
        }
    };
    if params.contract != CONTRACT || params.schema_version != SCHEMA_VERSION {
        return ControlResponse::error_with_code(
            id,
            "unsupported plant contract schema".to_string(),
            "unsupported_schema",
        );
    }
    let Some(project_root) = context.project_root else {
        return ControlResponse::error_with_code(
            id,
            "plant project root is unavailable".to_string(),
            "io_failure",
        );
    };
    let project_id = project_identity(project_root);
    if params
        .project_id
        .as_deref()
        .is_some_and(|value| value != project_id)
    {
        return ControlResponse::error_with_code(
            id,
            "plant request belongs to a different project".to_string(),
            "malformed_request",
        );
    }
    ControlResponse::ok(
        id,
        json!({
            "contract": CONTRACT,
            "schema_version": SCHEMA_VERSION,
            "operation": "plant.capabilities",
            "project_id": project_id,
            "model_kinds": crate::simulation::SUPPORTED_MODEL_KINDS,
            "limits": {
                "max_trace_rows_per_plant": MAX_TRACE_ROWS_PER_PLANT,
            },
            "operations": [
                "plant.capabilities",
                "plant.read",
                "plant.validate",
                "plant.save",
                "plant.remove",
                "plant.status",
                "plant.trace.read",
                "plant.trace.clear",
            ],
        }),
    )
}

pub(super) fn handle_plant_read(
    id: u64,
    params: Option<Value>,
    context: &PlantHandlerContext<'_>,
) -> ControlResponse {
    let params = match serde_json::from_value::<PlantReadParams>(params.unwrap_or(Value::Null)) {
        Ok(params) => params,
        Err(error) => {
            return ControlResponse::error_with_code(
                id,
                format!("invalid plant.read payload: {error}"),
                "malformed_request",
            );
        }
    };
    if params.contract != CONTRACT || params.schema_version != SCHEMA_VERSION {
        return ControlResponse::error_with_code(
            id,
            "unsupported plant contract schema".to_string(),
            "unsupported_schema",
        );
    }
    let Some(project_root) = context.project_root else {
        return ControlResponse::error_with_code(
            id,
            "plant project root is unavailable".to_string(),
            "io_failure",
        );
    };
    let project_id = project_identity(project_root);
    if params
        .project_id
        .as_deref()
        .is_some_and(|value| value != project_id)
    {
        return ControlResponse::error_with_code(
            id,
            "plant request belongs to a different project".to_string(),
            "malformed_request",
        );
    }
    let path = project_root.join("simulation.toml");
    let bytes = match fs::read(&path) {
        Ok(bytes) => bytes,
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => Vec::new(),
        Err(error) => {
            return ControlResponse::error_with_code(
                id,
                format!("failed to read simulation.toml: {error}"),
                "io_failure",
            );
        }
    };
    let document = if bytes.is_empty() {
        toml::Value::Table(toml::map::Map::new())
    } else {
        match toml::from_str::<toml::Value>(&String::from_utf8_lossy(&bytes)) {
            Ok(document) => document,
            Err(error) => {
                return ControlResponse::error_with_code(
                    id,
                    format!("invalid saved simulation.toml: {error}"),
                    "validation_failed",
                );
            }
        }
    };
    let saved_configuration = serde_json::to_value(&document).unwrap_or_else(|_| json!({}));
    let plants = saved_plant_descriptors(&document);
    ControlResponse::ok(
        id,
        json!({
            "contract": CONTRACT,
            "schema_version": SCHEMA_VERSION,
            "operation": "plant.read",
            "project_id": project_id,
            "configuration_revision": content_revision(&bytes),
            "saved_configuration": saved_configuration,
            "plants": plants,
            "restart_required": false,
        }),
    )
}

pub(super) fn handle_plant_save(
    id: u64,
    params: Option<Value>,
    context: &PlantHandlerContext<'_>,
) -> ControlResponse {
    let params = match serde_json::from_value::<PlantSaveParams>(params.unwrap_or(Value::Null)) {
        Ok(params) => params,
        Err(error) => {
            return ControlResponse::error_with_code(
                id,
                format!("invalid plant.save payload: {error}"),
                "malformed_request",
            );
        }
    };
    if params.contract != CONTRACT || params.schema_version != SCHEMA_VERSION {
        return ControlResponse::error_with_code(
            id,
            "unsupported plant contract schema".to_string(),
            "unsupported_schema",
        );
    }
    if params.operation_id.trim().is_empty() || params.expected_revision.trim().is_empty() {
        return ControlResponse::error_with_code(
            id,
            "plant.save requires non-empty operation_id and expected_revision".to_string(),
            "malformed_request",
        );
    }
    let Some(project_root) = context.project_root else {
        return ControlResponse::error_with_code(
            id,
            "plant project root is unavailable".to_string(),
            "io_failure",
        );
    };
    let project_id = project_identity(project_root);
    if params.project_id != project_id {
        return ControlResponse::error_with_code(
            id,
            "plant request belongs to a different project".to_string(),
            "malformed_request",
        );
    }
    let input_fingerprint = plant_save_fingerprint(&params);
    let operation_id = params.operation_id.clone();
    let outcome =
        context
            .plant_service
            .execute_once(&project_id, &operation_id, &input_fingerprint, || {
                perform_plant_save(id, project_root, &project_id, params, context)
            });
    match outcome {
        Ok(super::plant_service::PlantOperationOutcome::Applied(result))
        | Ok(super::plant_service::PlantOperationOutcome::Replayed(result)) => {
            ControlResponse::ok(id, result)
        }
        Ok(super::plant_service::PlantOperationOutcome::Conflict) => {
            ControlResponse::error_with_code(
                id,
                "operation_id was already used with different plant.save input".to_string(),
                "idempotency_conflict",
            )
        }
        Err(response) => response,
    }
}

fn perform_plant_save(
    id: u64,
    project_root: &Path,
    project_id: &str,
    params: PlantSaveParams,
    context: &PlantHandlerContext<'_>,
) -> Result<Value, ControlResponse> {
    let active_revision =
        simulation_snapshot(context).and_then(|snapshot| snapshot.active_configuration_revision);
    let path = project_root.join("simulation.toml");
    let current = match fs::read(&path) {
        Ok(bytes) => bytes,
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => Vec::new(),
        Err(error) => {
            return Err(ControlResponse::error_with_code(
                id,
                format!("failed to read simulation.toml: {error}"),
                "io_failure",
            ));
        }
    };
    let current_revision = content_revision(&current);
    if params.expected_revision != current_revision {
        let requested_plant_id = params.plant.get("id").and_then(Value::as_str);
        let current_document = toml::from_str::<toml::Value>(&String::from_utf8_lossy(&current))
            .unwrap_or_else(|_| toml::Value::Table(toml::map::Map::new()));
        let current_plant = saved_plant_descriptors(&current_document)
            .into_iter()
            .find(|descriptor| descriptor.get("id").and_then(Value::as_str) == requested_plant_id);
        return Err(ControlResponse::error_with_code_and_result(
            id,
            "saved plant configuration changed since it was read".to_string(),
            "revision_conflict",
            json!({
                "contract": CONTRACT,
                "schema_version": SCHEMA_VERSION,
                "operation": "plant.save",
                "project_id": project_id,
                "configuration_revision": current_revision,
                "current_plant": current_plant,
            }),
        ));
    }
    let current_text = match std::str::from_utf8(&current) {
        Ok(text) => text,
        Err(error) => {
            return Err(ControlResponse::error_with_code(
                id,
                format!("simulation.toml is not UTF-8: {error}"),
                "validation_failed",
            ));
        }
    };
    let (candidate, plant_id) = match render_preserving_candidate(current_text, params.plant) {
        Ok(candidate) => candidate,
        Err(error) => {
            return Err(ControlResponse::error_with_code(
                id,
                error,
                "validation_failed",
            ));
        }
    };
    let configuration = match crate::simulation::SimulationConfig::from_toml_text(
        &candidate,
        "plant.save candidate",
    ) {
        Ok(configuration) => configuration,
        Err(error) => {
            return Err(ControlResponse::error_with_code(
                id,
                error.to_string(),
                "validation_failed",
            ));
        }
    };
    if let Err(error) = atomic_write(&path, candidate.as_bytes(), &params.operation_id) {
        return Err(ControlResponse::error_with_code(
            id,
            format!("failed to save simulation.toml: {error}"),
            "io_failure",
        ));
    }
    let parsed = toml::from_str::<toml::Value>(&candidate)
        .unwrap_or_else(|_| toml::Value::Table(toml::map::Map::new()));
    let new_revision = content_revision(candidate.as_bytes());
    let restart_required = active_revision
        .as_deref()
        .is_some_and(|active| active != new_revision);
    let mut result = json!({
        "contract": CONTRACT,
        "schema_version": SCHEMA_VERSION,
        "operation": "plant.save",
        "operation_id": params.operation_id,
        "project_id": project_id,
        "configuration_revision": new_revision,
        "plants": saved_plant_descriptors(&parsed),
        "affected_plant_ids": [plant_id],
        "restart_required": restart_required,
        "plant_count": configuration.plant_system.plants.len(),
    });
    if let Some(active_revision) = active_revision {
        result
            .as_object_mut()
            .expect("plant.save result is an object")
            .insert(
                "active_configuration_revision".to_string(),
                Value::String(active_revision),
            );
    }
    Ok(result)
}

fn plant_save_fingerprint(params: &PlantSaveParams) -> String {
    let validated_input = json!({
        "contract": params.contract,
        "schema_version": params.schema_version,
        "project_id": params.project_id,
        "operation_id": params.operation_id,
        "expected_revision": params.expected_revision,
        "plant": params.plant,
    });
    format!(
        "sha256:{:x}",
        Sha256::digest(serde_json::to_vec(&validated_input).unwrap_or_default())
    )
}

fn simulation_snapshot(
    context: &PlantHandlerContext<'_>,
) -> Option<crate::simulation::SimulationRuntimeSnapshot> {
    let resource = context.resource?;
    let (sender, receiver) = std::sync::mpsc::channel();
    resource
        .send_command(ResourceCommand::SimulationSnapshot { respond_to: sender })
        .ok()?;
    receiver
        .recv_timeout(std::time::Duration::from_millis(250))
        .ok()
        .flatten()
}

pub(super) fn handle_plant_validate(
    id: u64,
    params: Option<Value>,
    context: &PlantHandlerContext<'_>,
) -> ControlResponse {
    let params = match serde_json::from_value::<PlantValidateParams>(params.unwrap_or(Value::Null))
    {
        Ok(params) => params,
        Err(error) => {
            return ControlResponse::error_with_code(
                id,
                format!("invalid plant.validate payload: {error}"),
                "malformed_request",
            );
        }
    };
    if params.contract != CONTRACT || params.schema_version != SCHEMA_VERSION {
        return ControlResponse::error_with_code(
            id,
            "unsupported plant contract schema".to_string(),
            "unsupported_schema",
        );
    }
    let Some(project_root) = context.project_root else {
        return ControlResponse::error_with_code(
            id,
            "plant project root is unavailable".to_string(),
            "io_failure",
        );
    };
    let project_id = project_identity(project_root);
    if params.project_id != project_id {
        return ControlResponse::error_with_code(
            id,
            "plant request belongs to a different project".to_string(),
            "malformed_request",
        );
    }
    let path = project_root.join("simulation.toml");
    let text = match fs::read_to_string(&path) {
        Ok(text) => text,
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => String::new(),
        Err(error) => {
            return ControlResponse::error_with_code(
                id,
                format!("failed to read simulation.toml: {error}"),
                "io_failure",
            );
        }
    };
    let candidate = match render_candidate(&text, params.plant) {
        Ok(candidate) => candidate,
        Err(error) => {
            return ControlResponse::error_with_code(id, error, "validation_failed");
        }
    };
    if let Err(error) =
        crate::simulation::SimulationConfig::from_toml_text(&candidate, "plant.validate candidate")
    {
        return ControlResponse::error_with_code(id, error.to_string(), "validation_failed");
    }
    ControlResponse::ok(
        id,
        json!({
            "contract": CONTRACT,
            "schema_version": SCHEMA_VERSION,
            "operation": "plant.validate",
            "project_id": project_id,
            "valid": true,
            "issues": [],
            "candidate_revision": content_revision(candidate.as_bytes()),
        }),
    )
}

fn render_candidate(current: &str, plant: Value) -> Result<String, String> {
    let mut document = if current.trim().is_empty() {
        toml::Value::Table(toml::map::Map::new())
    } else {
        toml::from_str::<toml::Value>(current).map_err(|error| error.to_string())?
    };
    let (plant, events, plant_id) = split_plant_candidate(plant)?;
    let root = document
        .as_table_mut()
        .ok_or_else(|| "simulation.toml root must be a table".to_string())?;
    let plants = root
        .entry("plants")
        .or_insert_with(|| toml::Value::Array(Vec::new()))
        .as_array_mut()
        .ok_or_else(|| "plants must be an array of tables".to_string())?;
    if let Some(existing) = plants.iter_mut().find(|existing| {
        existing.get("id").and_then(toml::Value::as_str) == Some(plant_id.as_str())
    }) {
        *existing = plant;
    } else {
        plants.push(plant);
    }
    replace_value_events(root, &plant_id, events)?;
    toml::to_string(&document).map_err(|error| error.to_string())
}

fn render_preserving_candidate(current: &str, plant: Value) -> Result<(String, String), String> {
    let (plant, events, plant_id) = split_plant_candidate(plant)?;
    let plant_table = toml_edit::ser::to_document(&plant)
        .map_err(|error| format!("invalid plant: {error}"))?
        .into_table();
    let mut document = if current.trim().is_empty() {
        DocumentMut::new()
    } else {
        current
            .parse::<DocumentMut>()
            .map_err(|error| format!("invalid saved simulation.toml: {error}"))?
    };
    let root = document.as_table_mut();
    if !root.contains_key("plants") {
        root.insert("plants", Item::ArrayOfTables(ArrayOfTables::new()));
    }
    let plants = root
        .get_mut("plants")
        .and_then(Item::as_array_of_tables_mut)
        .ok_or_else(|| "plants must be an array of tables".to_string())?;
    let matching_index = plants.iter().position(|existing| {
        existing
            .get("id")
            .and_then(Item::as_value)
            .and_then(toml_edit::Value::as_str)
            == Some(plant_id.as_str())
    });
    if let Some(index) = matching_index {
        plants.replace(index, plant_table);
    } else {
        plants.push(plant_table);
    }
    replace_edit_events(root, &plant_id, events)?;
    Ok((document.to_string(), plant_id))
}

fn split_plant_candidate(plant: Value) -> Result<(toml::Value, Vec<toml::Value>, String), String> {
    let mut fields = plant
        .as_object()
        .cloned()
        .ok_or_else(|| "plant candidate must be an object".to_string())?;
    let plant_id = fields
        .get("id")
        .and_then(Value::as_str)
        .filter(|id| !id.is_empty())
        .ok_or_else(|| "plants.id is required".to_string())?
        .to_string();
    let event_values = fields
        .remove("scenario_events")
        .unwrap_or_else(|| Value::Array(Vec::new()));
    let event_values = event_values
        .as_array()
        .ok_or_else(|| "scenario_events must be an array".to_string())?;
    let mut events = Vec::with_capacity(event_values.len());
    for event in event_values {
        let mut event = event
            .as_object()
            .cloned()
            .ok_or_else(|| "each scenario event must be an object".to_string())?;
        if event.contains_key("plant") {
            return Err("scenario event plant is supplied by the enclosing candidate".to_string());
        }
        event.insert("plant".to_string(), Value::String(plant_id.clone()));
        events.push(json_to_toml(Value::Object(event))?);
    }
    Ok((json_to_toml(Value::Object(fields))?, events, plant_id))
}

fn replace_value_events(
    root: &mut toml::map::Map<String, toml::Value>,
    plant_id: &str,
    replacements: Vec<toml::Value>,
) -> Result<(), String> {
    let events = root
        .entry("plant_events")
        .or_insert_with(|| toml::Value::Array(Vec::new()))
        .as_array_mut()
        .ok_or_else(|| "plant_events must be an array of tables".to_string())?;
    events.retain(|event| event.get("plant").and_then(toml::Value::as_str) != Some(plant_id));
    events.extend(replacements);
    Ok(())
}

fn replace_edit_events(
    root: &mut toml_edit::Table,
    plant_id: &str,
    replacements: Vec<toml::Value>,
) -> Result<(), String> {
    let mut events = ArrayOfTables::new();
    if let Some(existing) = root.get("plant_events") {
        let existing = existing
            .as_array_of_tables()
            .ok_or_else(|| "plant_events must be an array of tables".to_string())?;
        for event in existing.iter() {
            let owner = event
                .get("plant")
                .and_then(Item::as_value)
                .and_then(toml_edit::Value::as_str);
            if owner != Some(plant_id) {
                events.push(event.clone());
            }
        }
    }
    for event in replacements {
        let table = toml_edit::ser::to_document(&event)
            .map_err(|error| format!("invalid scenario event: {error}"))?
            .into_table();
        events.push(table);
    }
    root.insert("plant_events", Item::ArrayOfTables(events));
    Ok(())
}

fn atomic_write(path: &Path, bytes: &[u8], operation_id: &str) -> std::io::Result<()> {
    let parent = path.parent().ok_or_else(|| {
        std::io::Error::new(
            std::io::ErrorKind::InvalidInput,
            "destination has no parent",
        )
    })?;
    fs::create_dir_all(parent)?;
    let temporary = temporary_save_path(path, operation_id);
    let result = (|| {
        let mut file = OpenOptions::new()
            .write(true)
            .create_new(true)
            .open(&temporary)?;
        file.write_all(bytes)?;
        file.sync_all()?;
        fs::rename(&temporary, path)?;
        #[cfg(unix)]
        fs::File::open(parent)?.sync_all()?;
        Ok(())
    })();
    if result.is_err() {
        let _ = fs::remove_file(&temporary);
    }
    result
}

fn temporary_save_path(path: &Path, operation_id: &str) -> PathBuf {
    let name = path
        .file_name()
        .and_then(|name| name.to_str())
        .unwrap_or("simulation.toml");
    let operation_hash = Sha256::digest(operation_id.as_bytes());
    path.with_file_name(format!(
        ".{name}.{:x}-{}.tmp",
        operation_hash,
        std::process::id()
    ))
}

fn json_to_toml(value: Value) -> Result<toml::Value, String> {
    match value {
        Value::Null => Err("plant fields cannot be null".to_string()),
        Value::Bool(value) => Ok(toml::Value::Boolean(value)),
        Value::Number(value) => {
            if let Some(integer) = value.as_i64() {
                Ok(toml::Value::Integer(integer))
            } else if let Some(unsigned) = value.as_u64() {
                i64::try_from(unsigned)
                    .map(toml::Value::Integer)
                    .map_err(|_| "plant integer exceeds TOML range".to_string())
            } else {
                value
                    .as_f64()
                    .filter(|number| number.is_finite())
                    .map(toml::Value::Float)
                    .ok_or_else(|| "plant number must be finite".to_string())
            }
        }
        Value::String(value) => Ok(toml::Value::String(value)),
        Value::Array(values) => values
            .into_iter()
            .map(json_to_toml)
            .collect::<Result<Vec<_>, _>>()
            .map(toml::Value::Array),
        Value::Object(values) => values
            .into_iter()
            .map(|(key, value)| json_to_toml(value).map(|value| (key, value)))
            .collect::<Result<toml::map::Map<_, _>, _>>()
            .map(toml::Value::Table),
    }
}

fn saved_plant_descriptors(document: &toml::Value) -> Vec<Value> {
    document
        .get("plants")
        .and_then(toml::Value::as_array)
        .into_iter()
        .flatten()
        .enumerate()
        .filter_map(|(saved_order, plant)| {
            let table = plant.as_table()?;
            let id = table.get("id")?.as_str()?;
            Some(json!({
                "id": id,
                "display_name": table.get("name").and_then(toml::Value::as_str).unwrap_or(id),
                "description": table.get("description").and_then(toml::Value::as_str),
                "model": table.get("model").and_then(toml::Value::as_str),
                "enabled": table.get("enabled").and_then(toml::Value::as_bool).unwrap_or(true),
                "command_sources": table.get("command_sources"),
                "feedback_targets": table.get("feedback_targets"),
                "saved_order": saved_order,
            }))
        })
        .collect()
}

fn content_revision(bytes: &[u8]) -> String {
    format!("sha256:{:x}", Sha256::digest(bytes))
}

pub(super) fn project_identity(project_root: &Path) -> String {
    let canonical = fs::canonicalize(project_root).unwrap_or_else(|_| project_root.to_path_buf());
    format!(
        "project:sha256:{:x}",
        Sha256::digest(canonical.to_string_lossy().as_bytes())
    )
}

pub(super) fn error_envelope(operation: &str, project_root: Option<&Path>) -> Option<Value> {
    let project_root = project_root?;
    Some(json!({
        "contract": CONTRACT,
        "schema_version": SCHEMA_VERSION,
        "operation": operation,
        "project_id": project_identity(project_root),
    }))
}
