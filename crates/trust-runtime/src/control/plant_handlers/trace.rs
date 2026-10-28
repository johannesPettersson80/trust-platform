use serde::Deserialize;
use serde_json::{json, Value};
use sha2::{Digest, Sha256};

use super::{project_identity, simulation_snapshot, PlantHandlerContext, CONTRACT, SCHEMA_VERSION};
use crate::control::ControlResponse;
use crate::scheduler::ResourceCommand;

#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
struct PlantTraceReadParams {
    contract: String,
    schema_version: u64,
    project_id: String,
    plant_id: String,
}

#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
struct PlantTraceClearParams {
    contract: String,
    schema_version: u64,
    project_id: String,
    operation_id: String,
    plant_id: String,
    active_configuration_revision: String,
}

pub(in crate::control) fn handle_plant_trace_read(
    id: u64,
    params: Option<Value>,
    context: &PlantHandlerContext<'_>,
) -> ControlResponse {
    let params = match serde_json::from_value::<PlantTraceReadParams>(params.unwrap_or(Value::Null))
    {
        Ok(params) => params,
        Err(error) => {
            return ControlResponse::error_with_code(
                id,
                format!("invalid plant.trace.read payload: {error}"),
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
    if params.plant_id.is_empty() {
        return ControlResponse::error_with_code(
            id,
            "plant.trace.read plant_id must not be empty".to_string(),
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
        return unavailable(id, &project_id, &params.plant_id);
    };
    if snapshot.active_configuration_revision.is_none()
        || !snapshot
            .status
            .plants
            .iter()
            .any(|plant| plant.id == params.plant_id)
    {
        return unavailable(id, &project_id, &params.plant_id);
    }
    if snapshot.trace.capacity_per_plant == 0 {
        return ControlResponse::error_with_code_and_result(
            id,
            "plant trace collection is disabled".to_string(),
            "trace_disabled",
            json!({
                "contract": CONTRACT,
                "schema_version": SCHEMA_VERSION,
                "operation": "plant.trace.read",
                "project_id": project_id,
                "plant_id": params.plant_id,
                "trace_enabled": false,
            }),
        );
    }
    let stats = snapshot
        .trace
        .plant_stats
        .iter()
        .find(|stats| stats.plant_id == params.plant_id)
        .expect("active plant has trace statistics");
    let rows = snapshot
        .trace
        .rows
        .iter()
        .filter(|row| row.plant_id == params.plant_id)
        .map(|row| {
            json!({
                "sequence": row.sequence.to_string(),
                "plant_id": row.plant_id,
                "model": row.model,
                "simulation_time_ns": row.simulation_time.as_nanos().to_string(),
                "state": row.state,
                "command": row.transformed_command,
                "applied_input": row.applied_input,
                "ideal_output": row.ideal_output,
                "measured_feedback": row.measured_feedback,
                "queued_feedback": row.queued_feedback,
                "fired_events": row.fired_events,
            })
        })
        .collect::<Vec<_>>();
    ControlResponse::ok(
        id,
        json!({
            "contract": CONTRACT,
            "schema_version": SCHEMA_VERSION,
            "operation": "plant.trace.read",
            "project_id": project_id,
            "plant_id": params.plant_id,
            "active_configuration_revision": snapshot.active_configuration_revision,
            "capacity": snapshot.trace.capacity_per_plant,
            "retained_row_count": stats.retained_rows,
            "dropped_row_count": stats.dropped_rows,
            "configuration_digest": snapshot.trace.configuration_digest,
            "effective_seed": snapshot.trace.effective_seed.to_string(),
            "random_algorithm": snapshot.trace.random_algorithm,
            "random_algorithm_version": snapshot.trace.random_algorithm_version,
            "runtime_version": snapshot.trace.runtime_version,
            "restart_policy": snapshot.trace.restart_policy,
            "rows": rows,
        }),
    )
}

pub(in crate::control) fn handle_plant_trace_clear(
    id: u64,
    params: Option<Value>,
    context: &PlantHandlerContext<'_>,
) -> ControlResponse {
    let params =
        match serde_json::from_value::<PlantTraceClearParams>(params.unwrap_or(Value::Null)) {
            Ok(params) => params,
            Err(error) => {
                return ControlResponse::error_with_code(
                    id,
                    format!("invalid plant.trace.clear payload: {error}"),
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
    if params.operation_id.is_empty()
        || params.plant_id.is_empty()
        || params.active_configuration_revision.is_empty()
    {
        return ControlResponse::error_with_code(
            id,
            "plant.trace.clear identifiers and revision must not be empty".to_string(),
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
    let fingerprint = format!(
        "sha256:{}",
        hex::encode(Sha256::digest(
            serde_json::to_vec(&json!({
                "operation": "plant.trace.clear",
                "project_id": params.project_id,
                "operation_id": params.operation_id,
                "plant_id": params.plant_id,
                "active_configuration_revision": params.active_configuration_revision,
            }))
            .unwrap_or_default()
        ))
    );
    let operation_id = params.operation_id.clone();
    let outcome =
        context
            .plant_service
            .execute_once(&project_id, &operation_id, &fingerprint, || {
                perform_clear(id, &project_id, params, context)
            });
    match outcome {
        Ok(crate::control::plant_service::PlantOperationOutcome::Applied(result))
        | Ok(crate::control::plant_service::PlantOperationOutcome::Replayed(result)) => {
            ControlResponse::ok(id, result)
        }
        Ok(crate::control::plant_service::PlantOperationOutcome::Conflict) => {
            ControlResponse::error_with_code(
                id,
                "operation_id was already used with different input".to_string(),
                "idempotency_conflict",
            )
        }
        Err(response) => response,
    }
}

fn perform_clear(
    id: u64,
    project_id: &str,
    params: PlantTraceClearParams,
    context: &PlantHandlerContext<'_>,
) -> Result<Value, ControlResponse> {
    let (sender, receiver) = std::sync::mpsc::channel();
    let Some(resource) = context.resource else {
        return Err(clear_error(
            id,
            project_id,
            &params.plant_id,
            "status_unavailable",
        ));
    };
    if resource
        .send_command(ResourceCommand::SimulationTraceClear {
            plant_id: params.plant_id.clone(),
            active_configuration_revision: params.active_configuration_revision.clone(),
            respond_to: sender,
        })
        .is_err()
    {
        return Err(clear_error(
            id,
            project_id,
            &params.plant_id,
            "status_unavailable",
        ));
    }
    let result = receiver
        .recv_timeout(std::time::Duration::from_millis(250))
        .map_err(|_| clear_error(id, project_id, &params.plant_id, "status_unavailable"))?;
    match result {
        Ok(cleared) => Ok(json!({
            "contract": CONTRACT,
            "schema_version": SCHEMA_VERSION,
            "operation": "plant.trace.clear",
            "project_id": project_id,
            "operation_id": params.operation_id,
            "plant_id": cleared.plant_id,
            "active_configuration_revision": params.active_configuration_revision,
            "prior_row_count": cleared.prior_row_count,
            "prior_dropped_row_count": cleared.prior_dropped_row_count,
        })),
        Err(crate::simulation::PlantTraceClearError::StatusUnavailable) => Err(clear_error(
            id,
            project_id,
            &params.plant_id,
            "status_unavailable",
        )),
        Err(crate::simulation::PlantTraceClearError::RevisionConflict) => Err(clear_error(
            id,
            project_id,
            &params.plant_id,
            "revision_conflict",
        )),
        Err(crate::simulation::PlantTraceClearError::PlantNotFound) => Err(clear_error(
            id,
            project_id,
            &params.plant_id,
            "plant_not_found",
        )),
    }
}

fn clear_error(
    id: u64,
    project_id: &str,
    plant_id: &str,
    error_code: &'static str,
) -> ControlResponse {
    ControlResponse::error_with_code_and_result(
        id,
        format!("plant trace clear failed: {error_code}"),
        error_code,
        json!({
            "contract": CONTRACT,
            "schema_version": SCHEMA_VERSION,
            "operation": "plant.trace.clear",
            "project_id": project_id,
            "plant_id": plant_id,
        }),
    )
}

fn unavailable(id: u64, project_id: &str, plant_id: &str) -> ControlResponse {
    ControlResponse::error_with_code_and_result(
        id,
        "active plant trace evidence is unavailable".to_string(),
        "status_unavailable",
        json!({
            "contract": CONTRACT,
            "schema_version": SCHEMA_VERSION,
            "operation": "plant.trace.read",
            "project_id": project_id,
            "plant_id": plant_id,
            "available": false,
            "reason_code": "runtime_evidence_unavailable",
        }),
    )
}
