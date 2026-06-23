use std::path::PathBuf;

use crate::debug::DebugBreakpoint;
use serde_json::json;

use super::types::{BreakpointsClearIdParams, BreakpointsParams};
use super::{ControlResponse, ControlState};

pub(super) fn handle_breakpoints_set(
    id: u64,
    params: Option<serde_json::Value>,
    state: &ControlState,
) -> ControlResponse {
    if state.sources.is_empty() {
        return ControlResponse::error(id, "no sources registered".into());
    }
    let params: BreakpointsParams = match params {
        Some(value) => match serde_json::from_value(value) {
            Ok(parsed) => parsed,
            Err(err) => return ControlResponse::error(id, format!("invalid params: {err}")),
        },
        None => return ControlResponse::error(id, "missing params".into()),
    };
    let path = PathBuf::from(params.source);
    let file_id = match state
        .sources
        .file_id_for_path_in_project(&path, state.project_root.as_deref())
    {
        Some(id) => id,
        None => return ControlResponse::error(id, "unknown source path".into()),
    };
    let source_text = match state.sources.source_text(file_id) {
        Some(text) => text,
        None => return ControlResponse::error(id, "source text not loaded".into()),
    };
    let metadata = match state.metadata.lock() {
        Ok(guard) => guard,
        Err(_) => return ControlResponse::error(id, "metadata unavailable".into()),
    };
    let mut breakpoints = Vec::new();
    let mut resolved = Vec::new();
    for line in params.lines {
        if let Some((location, resolved_line, resolved_col)) =
            metadata.resolve_breakpoint_position(source_text, file_id, line, 1)
        {
            breakpoints.push(DebugBreakpoint::new(location));
            resolved.push(json!({"line": resolved_line, "column": resolved_col}));
        }
    }
    state.debug.set_breakpoints_for_file(file_id, breakpoints);
    let generation = state.debug.breakpoint_generation(file_id);
    ControlResponse::ok(
        id,
        json!({
            "status": "ok",
            "file_id": file_id,
            "resolved": resolved,
            "generation": generation,
        }),
    )
}

pub(super) fn handle_breakpoints_clear(
    id: u64,
    params: Option<serde_json::Value>,
    state: &ControlState,
) -> ControlResponse {
    let params: BreakpointsParams = match params {
        Some(value) => match serde_json::from_value(value) {
            Ok(parsed) => parsed,
            Err(err) => return ControlResponse::error(id, format!("invalid params: {err}")),
        },
        None => return ControlResponse::error(id, "missing params".into()),
    };
    let path = PathBuf::from(params.source);
    let file_id = match state
        .sources
        .file_id_for_path_in_project(&path, state.project_root.as_deref())
    {
        Some(id) => id,
        None => return ControlResponse::error(id, "unknown source path".into()),
    };
    state.debug.set_breakpoints_for_file(file_id, Vec::new());
    ControlResponse::ok(id, json!({"status": "cleared"}))
}

pub(super) fn handle_breakpoints_list(id: u64, state: &ControlState) -> ControlResponse {
    let breakpoints = state
        .debug
        .breakpoints()
        .into_iter()
        .map(|bp| {
            json!({
                "file_id": bp.location.file_id,
                "start": bp.location.start,
                "end": bp.location.end,
            })
        })
        .collect::<Vec<_>>();
    ControlResponse::ok(id, json!({ "breakpoints": breakpoints }))
}

pub(super) fn handle_breakpoints_clear_all(id: u64, state: &ControlState) -> ControlResponse {
    state.debug.clear_breakpoints();
    ControlResponse::ok(id, json!({ "status": "cleared" }))
}

pub(super) fn handle_breakpoints_clear_id(
    id: u64,
    params: Option<serde_json::Value>,
    state: &ControlState,
) -> ControlResponse {
    let params: BreakpointsClearIdParams = match params {
        Some(value) => match serde_json::from_value(value) {
            Ok(parsed) => parsed,
            Err(err) => return ControlResponse::error(id, format!("invalid params: {err}")),
        },
        None => return ControlResponse::error(id, "missing params".into()),
    };
    if state.sources.source_text(params.file_id).is_none() {
        return ControlResponse::error(id, "unknown file id".into());
    }
    state
        .debug
        .set_breakpoints_for_file(params.file_id, Vec::new());
    ControlResponse::ok(
        id,
        json!({ "status": "cleared", "file_id": params.file_id }),
    )
}
