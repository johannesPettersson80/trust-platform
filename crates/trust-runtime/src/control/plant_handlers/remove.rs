use std::fs;
use std::path::Path;

use serde::Deserialize;
use serde_json::{json, Value};
use sha2::{Digest, Sha256};
use toml_edit::{ArrayOfTables, DocumentMut, Item};

use super::{
    atomic_write, content_revision, project_identity, saved_plant_descriptors, simulation_snapshot,
    PlantHandlerContext, CONTRACT, SCHEMA_VERSION,
};
use crate::control::plant_service::PlantOperationOutcome;
use crate::control::ControlResponse;

#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
struct PlantRemoveParams {
    contract: String,
    schema_version: u64,
    project_id: String,
    operation_id: String,
    expected_revision: String,
    plant_id: String,
}

pub(super) fn handle_plant_remove(
    id: u64,
    params: Option<Value>,
    context: &PlantHandlerContext<'_>,
) -> ControlResponse {
    let params = match serde_json::from_value::<PlantRemoveParams>(params.unwrap_or(Value::Null)) {
        Ok(params) => params,
        Err(error) => {
            return ControlResponse::error_with_code(
                id,
                format!("invalid plant.remove payload: {error}"),
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
    if params.operation_id.trim().is_empty()
        || params.expected_revision.trim().is_empty()
        || !is_valid_plant_id(&params.plant_id)
    {
        return ControlResponse::error_with_code(
            id,
            "plant.remove requires a valid plant_id and non-empty operation_id and expected_revision"
                .to_string(),
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
    let input_fingerprint = plant_remove_fingerprint(&params);
    let operation_id = params.operation_id.clone();
    let outcome =
        context
            .plant_service
            .execute_once(&project_id, &operation_id, &input_fingerprint, || {
                perform_plant_remove(id, project_root, &project_id, params, context)
            });
    match outcome {
        Ok(PlantOperationOutcome::Applied(result))
        | Ok(PlantOperationOutcome::Replayed(result)) => ControlResponse::ok(id, result),
        Ok(PlantOperationOutcome::Conflict) => ControlResponse::error_with_code(
            id,
            "operation_id was already used with different plant.remove input".to_string(),
            "idempotency_conflict",
        ),
        Err(response) => response,
    }
}

fn perform_plant_remove(
    id: u64,
    project_root: &Path,
    project_id: &str,
    params: PlantRemoveParams,
    context: &PlantHandlerContext<'_>,
) -> Result<Value, ControlResponse> {
    let active_revision =
        simulation_snapshot(context).and_then(|snapshot| snapshot.active_configuration_revision);
    let path = project_root.join("simulation.toml");
    let current = fs::read(&path).map_err(|error| {
        ControlResponse::error_with_code(
            id,
            format!("failed to read simulation.toml: {error}"),
            "io_failure",
        )
    })?;
    let current_revision = content_revision(&current);
    let current_text = std::str::from_utf8(&current).map_err(|error| {
        ControlResponse::error_with_code(
            id,
            format!("simulation.toml is not UTF-8: {error}"),
            "validation_failed",
        )
    })?;
    let current_document = toml::from_str::<toml::Value>(current_text)
        .unwrap_or_else(|_| toml::Value::Table(toml::map::Map::new()));
    let current_plant = saved_plant_descriptors(&current_document)
        .into_iter()
        .find(|descriptor| descriptor.get("id").and_then(Value::as_str) == Some(&params.plant_id));
    if params.expected_revision != current_revision {
        return Err(ControlResponse::error_with_code_and_result(
            id,
            "saved plant configuration changed since it was read".to_string(),
            "revision_conflict",
            json!({
                "contract": CONTRACT,
                "schema_version": SCHEMA_VERSION,
                "operation": "plant.remove",
                "project_id": project_id,
                "configuration_revision": current_revision,
                "current_plant": current_plant,
            }),
        ));
    }
    if current_plant.is_none() {
        return Err(ControlResponse::error_with_code(
            id,
            format!("saved plant '{}' was not found", params.plant_id),
            "plant_not_found",
        ));
    }

    let candidate = render_preserving_removal(current_text, &params.plant_id)
        .map_err(|error| ControlResponse::error_with_code(id, error, "validation_failed"))?;
    let configuration =
        crate::simulation::SimulationConfig::from_toml_text(&candidate, "plant.remove candidate")
            .map_err(|error| {
            ControlResponse::error_with_code(id, error.to_string(), "validation_failed")
        })?;
    atomic_write(&path, candidate.as_bytes(), &params.operation_id).map_err(|error| {
        ControlResponse::error_with_code(
            id,
            format!("failed to save simulation.toml: {error}"),
            "io_failure",
        )
    })?;

    let parsed = toml::from_str::<toml::Value>(&candidate)
        .unwrap_or_else(|_| toml::Value::Table(toml::map::Map::new()));
    let new_revision = content_revision(candidate.as_bytes());
    let restart_required = active_revision
        .as_deref()
        .is_some_and(|active| active != new_revision);
    let mut result = json!({
        "contract": CONTRACT,
        "schema_version": SCHEMA_VERSION,
        "operation": "plant.remove",
        "operation_id": params.operation_id,
        "project_id": project_id,
        "configuration_revision": new_revision,
        "plants": saved_plant_descriptors(&parsed),
        "affected_plant_ids": [params.plant_id],
        "restart_required": restart_required,
        "plant_count": configuration.plant_system.plants.len(),
    });
    if let Some(active_revision) = active_revision {
        result
            .as_object_mut()
            .expect("plant.remove result is an object")
            .insert(
                "active_configuration_revision".to_string(),
                Value::String(active_revision),
            );
    }
    Ok(result)
}

fn render_preserving_removal(current: &str, plant_id: &str) -> Result<String, String> {
    let mut document = current
        .parse::<DocumentMut>()
        .map_err(|error| format!("invalid saved simulation.toml: {error}"))?;
    let root = document.as_table_mut();
    let plants = root
        .get_mut("plants")
        .and_then(Item::as_array_of_tables_mut)
        .ok_or_else(|| "plants must be an array of tables".to_string())?;
    let index = plants
        .iter()
        .position(|plant| {
            plant
                .get("id")
                .and_then(Item::as_value)
                .and_then(toml_edit::Value::as_str)
                == Some(plant_id)
        })
        .ok_or_else(|| format!("saved plant '{plant_id}' was not found"))?;
    plants.remove(index);

    if let Some(existing) = root.get("plant_events") {
        let existing = existing
            .as_array_of_tables()
            .ok_or_else(|| "plant_events must be an array of tables".to_string())?;
        let mut remaining = ArrayOfTables::new();
        for event in existing.iter() {
            let owner = event
                .get("plant")
                .and_then(Item::as_value)
                .and_then(toml_edit::Value::as_str);
            if owner != Some(plant_id) {
                remaining.push(event.clone());
            }
        }
        root.insert("plant_events", Item::ArrayOfTables(remaining));
    }
    Ok(document.to_string())
}

fn plant_remove_fingerprint(params: &PlantRemoveParams) -> String {
    let validated_input = json!({
        "contract": params.contract,
        "schema_version": params.schema_version,
        "project_id": params.project_id,
        "operation_id": params.operation_id,
        "expected_revision": params.expected_revision,
        "plant_id": params.plant_id,
    });
    format!(
        "sha256:{}",
        hex::encode(Sha256::digest(
            serde_json::to_vec(&validated_input).unwrap_or_default()
        ))
    )
}

fn is_valid_plant_id(value: &str) -> bool {
    let mut chars = value.chars();
    chars
        .next()
        .is_some_and(|first| first.is_ascii_alphabetic())
        && chars
            .all(|character| character.is_ascii_alphanumeric() || matches!(character, '_' | '-'))
}
