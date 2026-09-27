//! Transport boundary for guarded controller mode/state transfers.
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
        "control.transfer.status" | "control.transfer.apply" | "control.transfer.cancel"
    ) {
        return None;
    }
    let result = (|| {
        let metadata = state
            .metadata
            .lock()
            .map_err(|_| "runtime metadata unavailable")?;
        let params = params.ok_or("missing parameters")?;
        if operation == "control.transfer.status" {
            state.debug.transfer_status(params, &metadata)
        } else if operation == "control.transfer.apply" {
            state.debug.transfer_apply(params, &metadata)
        } else {
            state.debug.transfer_cancel(params)
        }
    })();
    Some(match result {
        Ok(value) => ControlResponse::ok(id, value),
        Err(error) => ControlResponse::error_with_code(id, error, "invalid_transfer_request"),
    })
}
