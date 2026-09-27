//! Transport adapter for generic typed parameter transactions.
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
    let result = (|| {
        let metadata = state
            .metadata
            .lock()
            .map_err(|_| "runtime metadata unavailable")?;
        let params = params.ok_or("missing parameters")?;
        if operation == "control.parameters.status" {
            state.debug.parameter_status(params, &metadata)
        } else if operation == "control.parameters.cancel" {
            state.debug.parameter_cancel(params, &metadata)
        } else {
            state.debug.parameter_apply(
                params,
                &metadata,
                operation == "control.parameters.rollback",
            )
        }
    })();
    Some(match result {
        Ok(value) => ControlResponse::ok(id, value),
        Err(error) => ControlResponse::error_with_code(id, error, "invalid_parameter_request"),
    })
}
