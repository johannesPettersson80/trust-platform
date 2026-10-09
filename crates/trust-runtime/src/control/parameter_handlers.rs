//! Transport adapter for generic typed parameter transactions.
use super::experiment_handlers;
use super::transfer_handlers::target_unknown;
use super::{ControlResponse, ControlState};
use serde_json::Value;
pub(super) fn dispatch(
    id: u64,
    operation: &str,
    params: Option<Value>,
    state: &ControlState,
) -> Option<ControlResponse> {
    if !matches!(
        operation,
        "control.parameters.status"
            | "control.parameters.apply"
            | "control.parameters.rollback"
            | "control.parameters.cancel"
    ) {
        return None;
    }
    if matches!(
        operation,
        "control.parameters.apply" | "control.parameters.rollback"
    ) {
        return Some(apply(id, operation, params, state));
    }
    let result = (|| {
        let metadata = state
            .metadata
            .lock()
            .map_err(|_| "runtime metadata unavailable")?;
        let params = params.ok_or("missing parameters")?;
        if operation == "control.parameters.status" {
            state.debug.parameter_status(params, &metadata)
        } else {
            state.debug.parameter_cancel(params, &metadata)
        }
    })();
    Some(match result {
        Ok(value) => ControlResponse::ok(id, value),
        Err(error) => ControlResponse::error_with_code(id, error, "invalid_parameter_request"),
    })
}

/// Applying or rolling back a parameter set writes live values, so it is refused while an experiment is running or
/// restoring. Its physical or simulated target is the runtime's fact from its latest scan (A2.4).
fn apply(id: u64, operation: &str, params: Option<Value>, state: &ControlState) -> ControlResponse {
    let metadata = match state.metadata.lock() {
        Ok(metadata) => metadata,
        Err(_) => {
            return ControlResponse::error_with_code(
                id,
                "runtime metadata unavailable".into(),
                "invalid_parameter_request",
            )
        }
    };
    let Some(params) = params else {
        return ControlResponse::error_with_code(
            id,
            "missing parameters".into(),
            "invalid_parameter_request",
        );
    };
    let Some(simulated) = state.experiments.simulated_target() else {
        return target_unknown(id);
    };
    let rollback = operation == "control.parameters.rollback";
    experiment_handlers::answer_live_write(
        id,
        state,
        || {
            state
                .debug
                .parameter_apply(params, &metadata, rollback, !simulated)
        },
        "invalid_parameter_request",
    )
}
