use serde::Deserialize;
use serde_json::{json, Value};

use super::{project_identity, simulation_snapshot, PlantHandlerContext, CONTRACT, SCHEMA_VERSION};
use crate::control::ControlResponse;

#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
struct PlantStatusParams {
    contract: String,
    schema_version: u64,
    project_id: String,
    #[serde(default)]
    plant_id: Option<String>,
}

pub(in crate::control) fn handle_plant_status(
    id: u64,
    params: Option<Value>,
    context: &PlantHandlerContext<'_>,
) -> ControlResponse {
    let params = match serde_json::from_value::<PlantStatusParams>(params.unwrap_or(Value::Null)) {
        Ok(params) => params,
        Err(error) => {
            return ControlResponse::error_with_code(
                id,
                format!("invalid plant.status payload: {error}"),
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
    if params.plant_id.as_deref().is_some_and(str::is_empty) {
        return ControlResponse::error_with_code(
            id,
            "plant.status plant_id must not be empty".to_string(),
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
    let Some(snapshot) = simulation_snapshot(context) else {
        return unavailable(
            id,
            &project_id,
            params.plant_id.as_deref(),
            "runtime_evidence_unavailable",
        );
    };
    let Some(active_revision) = snapshot.active_configuration_revision else {
        return unavailable(
            id,
            &project_id,
            params.plant_id.as_deref(),
            "active_revision_unavailable",
        );
    };
    let plants = snapshot
        .status
        .plants
        .iter()
        .filter(|plant| {
            params
                .plant_id
                .as_deref()
                .is_none_or(|requested| requested == plant.id)
        })
        .map(status_json)
        .collect::<Vec<_>>();
    if params.plant_id.is_some() && plants.is_empty() {
        return unavailable(
            id,
            &project_id,
            params.plant_id.as_deref(),
            "requested_plant_unavailable",
        );
    }
    let simulation_time_ns = snapshot
        .status
        .plants
        .iter()
        .map(|plant| plant.simulated_time.as_nanos())
        .max()
        .unwrap_or(0);
    ControlResponse::ok(
        id,
        json!({
            "contract": CONTRACT,
            "schema_version": SCHEMA_VERSION,
            "operation": "plant.status",
            "project_id": project_id,
            "available": true,
            "active_configuration_revision": active_revision,
            // Decimal strings preserve the full Rust integer across JavaScript's
            // narrower JSON number range.
            "simulation_time_ns": simulation_time_ns.to_string(),
            "effective_seed": snapshot.status.effective_seed.to_string(),
            "random_algorithm": snapshot.status.random_algorithm,
            "random_algorithm_version": snapshot.status.random_algorithm_version,
            "runtime_version": snapshot.trace.runtime_version,
            "time_scale": snapshot.status.time_scale,
            "restart_policy": snapshot.status.restart_policy,
            "plants": plants,
        }),
    )
}

fn unavailable(
    id: u64,
    project_id: &str,
    requested_plant_id: Option<&str>,
    reason_code: &'static str,
) -> ControlResponse {
    ControlResponse::error_with_code_and_result(
        id,
        "active plant runtime evidence is unavailable".to_string(),
        "status_unavailable",
        json!({
            "contract": CONTRACT,
            "schema_version": SCHEMA_VERSION,
            "operation": "plant.status",
            "project_id": project_id,
            "requested_plant_id": requested_plant_id,
            "available": false,
            "reason_code": reason_code,
        }),
    )
}

fn status_json(plant: &crate::simulation::PlantStatus) -> Value {
    let actuators = plant
        .actuators
        .iter()
        .map(|actuator| {
            json!({
                "position": actuator.position,
                "applied": actuator.applied,
                "saturation_active": actuator.saturation_active,
                "slew_limit_active": actuator.slew_limit_active,
                "deadband_active": actuator.deadband_active,
                "stiction_active": actuator.stiction_active,
                "stuck": actuator.stuck,
            })
        })
        .collect::<Vec<_>>();
    json!({
        "id": plant.id,
        "model": plant.model,
        "operating_state": if plant.last_fault.is_some() { "faulted" } else { "running" },
        "enabled": plant.enabled,
        "step_ns": plant.step.as_nanos().to_string(),
        "simulation_time_ns": plant.simulated_time.as_nanos().to_string(),
        "state": plant.state,
        "command": plant.transformed_command,
        "applied_input": plant.applied_input,
        "ideal_output": plant.ideal_output,
        "measured_feedback": plant.measured_feedback,
        "published_feedback": plant.published_feedback,
        "actuators": actuators,
        "active_uncertainties": plant.active_uncertainties,
        "active_dropouts": plant.active_dropouts,
        "pending_event_count": plant.pending_events,
        "delay_buffer_depth": plant.delay_buffer_depth,
        "accumulated_remainder_ns": plant.accumulated_remainder.as_nanos().to_string(),
        "last_substeps": plant.last_substeps,
        "current_fault": plant.last_fault,
    })
}
