use std::path::PathBuf;

use crate::debug::{offset_to_line_col, DebugBreakpoint, SourceLocation};
use serde_json::json;
use trust_runtime_services::debug::replies;

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
            resolved.push(replies::position(resolved_line, resolved_col));
        }
    }
    state.debug.set_breakpoints_for_file(file_id, breakpoints);
    let generation = state.debug.breakpoint_generation(file_id);
    replies::breakpoints_set(id, file_id, &resolved, generation)
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
    replies::breakpoints_cleared(id)
}

pub(super) fn handle_breakpoints_list(id: u64, state: &ControlState) -> ControlResponse {
    let mut breakpoints = Vec::new();
    for breakpoint in state.debug.breakpoints() {
        let Some(text) = state.sources.source_text(breakpoint.location.file_id) else {
            return ControlResponse::error(id, "source text not loaded".into());
        };
        breakpoints.push(list_entry(&breakpoint.location, text));
    }
    ControlResponse::ok(id, json!({ "breakpoints": breakpoints }))
}

/// One entry of `breakpoints.list`: the entry shape that every runtime gives (specification 11
/// §6.13.3), with the statement's byte offsets `start` and `end` added, which a computer counts
/// from its text.
fn list_entry(location: &SourceLocation, text: &str) -> serde_json::Value {
    let (line, column) = offset_to_line_col(text, location.start);
    let (end_line, end_column) = offset_to_line_col(text, location.end);
    let mut entry = replies::breakpoint_entry(location.file_id, line, column, end_line, end_column);
    if let Some(fields) = entry.as_object_mut() {
        fields.insert("start".into(), json!(location.start));
        fields.insert("end".into(), json!(location.end));
    }
    entry
}

pub(super) fn handle_breakpoints_clear_all(id: u64, state: &ControlState) -> ControlResponse {
    state.debug.clear_breakpoints();
    replies::breakpoints_cleared(id)
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
    replies::breakpoints_cleared_for_file(id, params.file_id)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn breakpoints_list_has_the_same_fields_on_a_chip_and_a_computer() {
        // Specification 11 §6.13.3: a computer lists a breakpoint with the five fields a chip gives,
        // zero-based, and its byte offsets `start` and `end` in addition. In the text `x := 1;` and
        // `  y := 2;` (18 bytes), the statement that starts at byte 10 starts at line 1, column 2, and
        // its end, byte 18, is line 2, column 0. Wrong behaviour caught: an entry without the five
        // shared fields, a line or a column that is one-based, a position converted from the wrong
        // byte, and an entry without the byte offsets that a computer keeps.
        let text = "x := 1;\n  y := 2;\n";
        let location = SourceLocation::new(4, 10, 18);

        let entry = list_entry(&location, text);

        assert_eq!(
            entry,
            json!({
                "file_id": 4,
                "line": 1,
                "column": 2,
                "end_line": 2,
                "end_column": 0,
                "start": 10,
                "end": 18,
            })
        );
    }
}
