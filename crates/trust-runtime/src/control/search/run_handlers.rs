use super::{contracts::Identity, run_contracts::RunRequest, snapshot, workload::Purpose};
use crate::control::{ControlResponse, ControlState};
use serde_json::{json, Value};
pub(super) fn dispatch(
    id: u64,
    operation: &str,
    params: Option<Value>,
    state: &ControlState,
) -> Option<ControlResponse> {
    if !matches!(
        operation,
        "control.simulation.run.status"
            | "control.simulation.run.start"
            | "control.simulation.run.cancel"
    ) {
        return None;
    }
    let result = (|| -> Result<Value, String> {
        let params = params.unwrap_or_else(|| json!({}));
        if operation == "control.simulation.run.status"
            && params.as_object().is_some_and(|object| object.is_empty())
        {
            return Ok(match snapshot::load(state) {
                Ok(snapshot) => {
                    json!({"ready":true,"project_revision":snapshot.project_revision,"source_basis":"loaded-runtime-sources","execution":"isolated-simulation","supports_request_id":true})
                }
                Err(reason) => json!({"ready":false,"reason":reason}),
            });
        }
        if operation == "control.simulation.run.start" {
            let request: RunRequest = serde_json::from_value(params).map_err(|e| e.to_string())?;
            request.validate()?;
            return state.searches.start_run(snapshot::load(state)?, request);
        }
        let identity: Identity = serde_json::from_value(params).map_err(|e| e.to_string())?;
        let project = state
            .project_root
            .as_ref()
            .ok_or("active project unavailable")?
            .canonicalize()
            .map_err(|e| e.to_string())?
            .to_string_lossy()
            .into_owned();
        state.searches.inspect_for(
            Purpose::Run,
            &identity.job_id,
            &identity.expected_project_revision,
            &project,
            operation == "control.simulation.run.cancel",
            identity.include_results,
        )
    })();
    Some(match result {
        Ok(value) => ControlResponse::ok(id, value),
        Err(reason) => {
            ControlResponse::error_with_code(id, reason, "invalid_simulation_run_request")
        }
    })
}
