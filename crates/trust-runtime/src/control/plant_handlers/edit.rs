//! The plant editor's requests (specification 47 §20.1): `plant.edit` checks and changes an editor
//! draft against the saved project, and `plant.signals` builds the PLC signals a plant can bind to.
//! Neither writes a file or changes runtime state.

use std::fs;

use serde::Deserialize;
use serde_json::{json, Value};

use super::editor;
use super::{
    project_identity, render_candidate, ControlResponse, PlantHandlerContext, CONTRACT,
    SCHEMA_VERSION,
};

#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
struct PlantEditParams {
    contract: String,
    schema_version: u64,
    project_id: String,
    draft: Value,
    #[serde(default)]
    action: Option<Value>,
    #[serde(default)]
    signals: Option<Value>,
}

#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
struct PlantSignalsParams {
    contract: String,
    schema_version: u64,
    project_id: String,
    compiled: Value,
    #[serde(default)]
    live: Option<Value>,
    #[serde(default)]
    communication_schema: Option<Value>,
    #[serde(default)]
    communication_topology: Option<Value>,
}

/// The common checks of an editor request: the contract and schema, a project root, and the
/// producer's project ID; the project ID on success.
fn admit(
    id: u64,
    contract: &str,
    schema_version: u64,
    project_id: &str,
    context: &PlantHandlerContext<'_>,
) -> Result<String, ControlResponse> {
    if contract != CONTRACT || schema_version != SCHEMA_VERSION {
        return Err(ControlResponse::error_with_code(
            id,
            "unsupported plant contract schema".to_string(),
            "unsupported_schema",
        ));
    }
    let Some(project_root) = context.project_root else {
        return Err(ControlResponse::error_with_code(
            id,
            "plant project root is unavailable".to_string(),
            "io_failure",
        ));
    };
    let expected = project_identity(project_root);
    if project_id != expected {
        return Err(ControlResponse::error_with_code(
            id,
            "plant request belongs to a different project".to_string(),
            "malformed_request",
        ));
    }
    Ok(expected)
}

/// `plant.edit`: the draft after the requested edit, with every validation issue, what the editor
/// shows, and the candidate it saves as. When the editor's checks find nothing, the runtime's own
/// validation of the complete resulting `simulation.toml` judges the candidate too.
pub(super) fn handle_plant_edit(
    id: u64,
    params: Option<Value>,
    context: &PlantHandlerContext<'_>,
) -> ControlResponse {
    let params = match serde_json::from_value::<PlantEditParams>(params.unwrap_or(Value::Null)) {
        Ok(params) => params,
        Err(error) => {
            return ControlResponse::error_with_code(
                id,
                format!("invalid plant.edit payload: {error}"),
                "malformed_request",
            );
        }
    };
    let project_id = match admit(
        id,
        &params.contract,
        params.schema_version,
        &params.project_id,
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
    let text = match fs::read_to_string(project_root.join("simulation.toml")) {
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
    let project = saved_project(&text);
    let mut answer = match editor::edit(
        &params.draft,
        params.action.as_ref(),
        params.signals.as_ref(),
        &project,
    ) {
        Ok(answer) => answer,
        Err(error) => {
            return ControlResponse::error_with_code(id, error, "malformed_request");
        }
    };
    if answer.can_save() {
        let judged = render_candidate(&text, answer.candidate.clone()).and_then(|candidate| {
            crate::simulation::SimulationConfig::from_toml_text(&candidate, "plant.edit candidate")
                .map(|_| ())
                .map_err(|error| error.to_string())
        });
        if let Err(reason) = judged {
            answer.add_configuration_issue(reason);
        }
    }
    let mut result = editor::answer_fields(&answer);
    for (key, value) in [
        ("contract", json!(CONTRACT)),
        ("schema_version", json!(SCHEMA_VERSION)),
        ("operation", json!("plant.edit")),
        ("project_id", json!(project_id)),
    ] {
        result.insert(key.to_string(), value);
    }
    ControlResponse::ok(id, Value::Object(result))
}

/// `plant.signals`: the PLC signals a plant can bind to, from the compiled bindings, the live values
/// and the communication schema and topology the caller holds.
pub(super) fn handle_plant_signals(
    id: u64,
    params: Option<Value>,
    context: &PlantHandlerContext<'_>,
) -> ControlResponse {
    let params = match serde_json::from_value::<PlantSignalsParams>(params.unwrap_or(Value::Null)) {
        Ok(params) => params,
        Err(error) => {
            return ControlResponse::error_with_code(
                id,
                format!("invalid plant.signals payload: {error}"),
                "malformed_request",
            );
        }
    };
    let project_id = match admit(
        id,
        &params.contract,
        params.schema_version,
        &params.project_id,
        context,
    ) {
        Ok(project_id) => project_id,
        Err(response) => return response,
    };
    let catalog = editor::build_signal_catalog(
        Some(&params.compiled),
        params.live.as_ref(),
        params.communication_schema.as_ref(),
        params.communication_topology.as_ref(),
    );
    ControlResponse::ok(
        id,
        json!({
            "contract": CONTRACT,
            "schema_version": SCHEMA_VERSION,
            "operation": "plant.signals",
            "project_id": project_id,
            "signals": catalog.to_json(),
        }),
    )
}

/// The saved plants' IDs and feedback targets and the project's default plant step: 10 ms when
/// `simulation.toml` sets none (specification 46 §3.1), unknown when the file does not read or sets
/// no positive whole number.
fn saved_project(text: &str) -> editor::Project {
    let document = if text.trim().is_empty() {
        Some(toml::Value::Table(toml::map::Map::new()))
    } else {
        toml::from_str::<toml::Value>(text).ok()
    };
    let Some(document) = document else {
        return editor::Project {
            saved_ids: Vec::new(),
            saved_feedback: Vec::new(),
            step_ms: None,
        };
    };
    let plants: Vec<&toml::value::Table> = document
        .get("plants")
        .and_then(toml::Value::as_array)
        .into_iter()
        .flatten()
        .filter_map(toml::Value::as_table)
        .collect();
    let saved_ids = plants
        .iter()
        .filter_map(|plant| plant.get("id").and_then(toml::Value::as_str))
        .map(str::to_string)
        .collect();
    let saved_feedback = plants
        .iter()
        .filter_map(|plant| {
            let id = plant.get("id").and_then(toml::Value::as_str)?;
            let targets = plant
                .get("feedback_targets")
                .and_then(toml::Value::as_array)
                .into_iter()
                .flatten()
                .filter_map(toml::Value::as_str)
                .map(str::to_string)
                .collect();
            Some((id.to_string(), targets))
        })
        .collect();
    let step_ms = match document
        .get("simulation")
        .and_then(|simulation| simulation.get("plant_step_ms"))
    {
        None => Some(10.0),
        Some(toml::Value::Integer(step)) if *step > 0 => Some(*step as f64),
        Some(_) => None,
    };
    editor::Project {
        saved_ids,
        saved_feedback,
        step_ms,
    }
}
