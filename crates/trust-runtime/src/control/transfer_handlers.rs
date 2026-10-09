//! Transport boundary for guarded controller mode/state transfers.
use super::experiment_handlers;
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
    if operation == "control.transfer.apply" {
        return Some(apply(id, params, state));
    }
    let result = (|| {
        let metadata = state
            .metadata
            .lock()
            .map_err(|_| "runtime metadata unavailable")?;
        let params = params.ok_or("missing parameters")?;
        if operation == "control.transfer.status" {
            state.debug.transfer_status(params, &metadata)
        } else {
            state.debug.transfer_cancel(params)
        }
    })();
    Some(match result {
        Ok(value) => ControlResponse::ok(id, value),
        Err(error) => ControlResponse::error_with_code(id, error, "invalid_transfer_request"),
    })
}

/// A transfer writes live values, so it is refused while an experiment is running or restoring. Its physical or
/// simulated target is the runtime's fact from its latest scan (A2.4), never the request's label.
fn apply(id: u64, params: Option<Value>, state: &ControlState) -> ControlResponse {
    let metadata = match state.metadata.lock() {
        Ok(metadata) => metadata,
        Err(_) => {
            return ControlResponse::error_with_code(
                id,
                "runtime metadata unavailable".into(),
                "invalid_transfer_request",
            )
        }
    };
    let Some(params) = params else {
        return ControlResponse::error_with_code(
            id,
            "missing parameters".into(),
            "invalid_transfer_request",
        );
    };
    let Some(simulated) = state.experiments.simulated_target() else {
        return target_unknown(id);
    };
    experiment_handlers::answer_live_write(
        id,
        state,
        || state.debug.transfer_apply(params, &metadata, !simulated),
        "invalid_transfer_request",
    )
}

/// The refusal of a live write before the runtime's first scan, when its target is not known yet (A2.4).
pub(super) fn target_unknown(id: u64) -> ControlResponse {
    ControlResponse::error_with_code(
        id,
        "no completed runtime scan yet; the runtime's target is not known".into(),
        "io_unavailable",
    )
}
