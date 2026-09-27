//! Viewer adapter for coherent addressed/symbolic scan inspection.
use super::{ControlResponse, ControlState};
use serde_json::Value;
pub(super) fn dispatch(
    id: u64,
    operation: &str,
    params: Option<Value>,
    state: &ControlState,
) -> Option<ControlResponse> {
    if operation != "control.signals.inspect" {
        return None;
    }
    let result = (|| {
        let metadata = state
            .metadata
            .lock()
            .map_err(|_| "runtime metadata unavailable")?;
        state
            .debug
            .inspect_control_signals(params.ok_or("signal bindings required")?, &metadata)
    })();
    Some(match result {
        Ok(value) => ControlResponse::ok(id, value),
        Err(reason) => ControlResponse::error_with_code(id, reason, "signals_unavailable"),
    })
}
