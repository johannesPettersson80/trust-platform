//! The replies of the debug operations and their stable refusals (specification 11 §6.9 and
//! §6.13; specification 13).
//!
//! The reply shapes are written once, here: trust-runtime builds its replies with these functions
//! and a chip builds its replies with them too, so a client reads the same fields from either
//! (specification 34, ARCH-11). Line and column numbers in a reply are zero-based, as the control
//! protocol has always carried them.

use alloc::string::String;

use serde_json::json;

use super::projection::{DebugScope, DebugSource, DebugVariable};
use super::run::StopReason;
use crate::protocol::ControlResponse;

/// The refusal of a step while the debugger does not hold the scan (specification 11 §6.9:
/// `step_in`, `step_over` and `step_out` act only while the debugger holds the scan).
pub const STEP_REQUIRES_HOLD: &str =
    "stepping requires the scan to be held by the debugger (a breakpoint or a debug-mode pause)";

/// The error of a read before any scan has ended (specification 11 §6.13.1).
pub const NO_SNAPSHOT: &str = "no snapshot available";

/// The refusal of a debug request on a program that has no `LINE_TABLE` (specification 11
/// §6.13.3).
pub const NO_LINE_TABLE: &str =
    "the program has no line table, so it cannot be debugged: rebuild the project";

/// The stable code of [`NO_LINE_TABLE`].
pub const NO_LINE_TABLE_CODE: &str = "no_line_table";

/// The refusal of `eval`, `debug.evaluate` and a breakpoint condition or logpoint on a runtime
/// that evaluates no compiled debug expression yet (specification 11 §6.13.2).
pub const EXPRESSIONS_UNAVAILABLE: &str =
    "this runtime evaluates no debug expressions yet: the compiled expression form is not implemented";

/// The stable code of [`EXPRESSIONS_UNAVAILABLE`]: no `expr_version` is implemented.
pub const EXPRESSIONS_UNAVAILABLE_CODE: &str = "expr_version_unsupported";

/// The refusal of a variables listing that would take the variable handle table past its bound
/// (specification 11 §6.13.3).
pub const HANDLES_FULL: &str = "the variable handle table is full: request the scopes again";

/// The stable code of [`HANDLES_FULL`].
pub const HANDLES_FULL_CODE: &str = "variable_handles_full";

/// The error text for a frame the stop does not have.
#[must_use]
pub fn unknown_frame(frame_id: u32) -> String {
    alloc::format!("unknown frame id {frame_id}")
}

/// The refusal of a debug request on a program without a line table.
#[must_use]
pub fn no_line_table(id: u64) -> ControlResponse {
    ControlResponse::error_with_code(id, String::from(NO_LINE_TABLE), NO_LINE_TABLE_CODE)
}

/// The refusal of a listing that would take the variable handle table past its bound.
#[must_use]
pub fn handles_full(id: u64) -> ControlResponse {
    ControlResponse::error_with_code(id, String::from(HANDLES_FULL), HANDLES_FULL_CODE)
}

/// The refusal of a request that carries or evaluates a debug expression.
#[must_use]
pub fn expressions_unavailable(id: u64) -> ControlResponse {
    ControlResponse::error_with_code(
        id,
        String::from(EXPRESSIONS_UNAVAILABLE),
        EXPRESSIONS_UNAVAILABLE_CODE,
    )
}

/// Where a stop stands, in the form a stop reply carries it.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct StopPlace<'a> {
    /// The file's identifier.
    pub file_id: u32,
    /// The statement's zero-based line.
    pub line: u32,
    /// The statement's zero-based column.
    pub column: u32,
    /// The file's path, or its label on a runtime without source text.
    pub path: &'a str,
}

/// One stop: why the scan stopped, on which thread, at which breakpoint generation, and where.
/// The place is present only when the runtime can name the stop's file.
#[must_use]
#[allow(
    clippy::missing_panics_doc,
    reason = "the serde_json::json! conversions of these values cannot fail"
)]
pub fn stop(
    reason: StopReason,
    thread_id: Option<u32>,
    breakpoint_generation: Option<u64>,
    place: Option<StopPlace<'_>>,
) -> serde_json::Value {
    let mut payload = json!({
        "reason": reason.label(),
        "thread_id": thread_id,
        "breakpoint_generation": breakpoint_generation,
    });
    if let (Some(place), Some(object)) = (place, payload.as_object_mut()) {
        object.insert("file_id".into(), json!(place.file_id));
        object.insert("line".into(), json!(place.line));
        object.insert("column".into(), json!(place.column));
        object.insert("path".into(), json!(place.path));
    }
    payload
}

/// One frame of a stack reply.
#[must_use]
#[allow(
    clippy::missing_panics_doc,
    reason = "the serde_json::json! conversions of these values cannot fail"
)]
pub fn stack_frame(
    id: u32,
    name: &str,
    source: &DebugSource,
    line: u32,
    column: u32,
) -> serde_json::Value {
    json!({
        "id": id,
        "name": name,
        "source": source,
        "line": line,
        "column": column,
    })
}

/// `debug.state`: whether the debugger holds the scan, and the latest stop.
#[must_use]
#[allow(
    clippy::missing_panics_doc,
    reason = "the serde_json::json! conversions of these values cannot fail"
)]
pub fn state(id: u64, paused: bool, last_stop: Option<&serde_json::Value>) -> ControlResponse {
    ControlResponse::ok(id, json!({ "paused": paused, "last_stop": last_stop }))
}

/// `debug.stops`: the stops since the last read, oldest first.
#[must_use]
#[allow(
    clippy::missing_panics_doc,
    reason = "the serde_json::json! conversions of these values cannot fail"
)]
pub fn stops(id: u64, stops: &[serde_json::Value]) -> ControlResponse {
    ControlResponse::ok(id, json!({ "stops": stops }))
}

/// `debug.stack`: the frames, innermost first.
#[must_use]
#[allow(
    clippy::missing_panics_doc,
    reason = "the serde_json::json! conversions of these values cannot fail"
)]
pub fn stack(id: u64, stack_frames: &[serde_json::Value]) -> ControlResponse {
    ControlResponse::ok(
        id,
        json!({ "stack_frames": stack_frames, "total_frames": stack_frames.len() }),
    )
}

/// `debug.scopes`: the scopes of one frame.
#[must_use]
#[allow(
    clippy::missing_panics_doc,
    reason = "the serde_json::json! conversions of these values cannot fail"
)]
pub fn scopes(id: u64, scopes: &[DebugScope]) -> ControlResponse {
    ControlResponse::ok(id, json!({ "scopes": scopes }))
}

/// `debug.variables`: the variables of one handle.
#[must_use]
#[allow(
    clippy::missing_panics_doc,
    reason = "the serde_json::json! conversions of these values cannot fail"
)]
pub fn variables(id: u64, variables: &[DebugVariable]) -> ControlResponse {
    ControlResponse::ok(id, json!({ "variables": variables }))
}

/// `debug.breakpoint_locations`: the statement starts in the requested range.
#[must_use]
#[allow(
    clippy::missing_panics_doc,
    reason = "the serde_json::json! conversions of these values cannot fail"
)]
pub fn breakpoint_locations(id: u64, breakpoints: &[serde_json::Value]) -> ControlResponse {
    ControlResponse::ok(id, json!({ "breakpoints": breakpoints }))
}

/// One position of a breakpoint-location or resolved-breakpoint reply.
#[must_use]
#[allow(
    clippy::missing_panics_doc,
    reason = "the serde_json::json! conversions of these values cannot fail"
)]
pub fn position(line: u32, column: u32) -> serde_json::Value {
    json!({ "line": line, "column": column })
}

/// One entry of `breakpoints.list`: the file and the zero-based span of a breakpoint's statement.
/// This is the one entry shape every runtime gives (specification 11 §6.13.3); a computer adds its
/// byte offsets to it, which a runtime without the text cannot give.
#[must_use]
#[allow(
    clippy::missing_panics_doc,
    reason = "the serde_json::json! conversions of these values cannot fail"
)]
pub fn breakpoint_entry(
    file_id: u32,
    line: u32,
    column: u32,
    end_line: u32,
    end_column: u32,
) -> serde_json::Value {
    json!({
        "file_id": file_id,
        "line": line,
        "column": column,
        "end_line": end_line,
        "end_column": end_column,
    })
}

/// `breakpoints.set`: the file, where each requested line resolved, and the generation of the
/// file's breakpoint set.
#[must_use]
#[allow(
    clippy::missing_panics_doc,
    reason = "the serde_json::json! conversions of these values cannot fail"
)]
pub fn breakpoints_set(
    id: u64,
    file_id: u32,
    resolved: &[serde_json::Value],
    generation: Option<u64>,
) -> ControlResponse {
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

/// `breakpoints.clear` and `breakpoints.clear_all`.
#[must_use]
#[allow(
    clippy::missing_panics_doc,
    reason = "the serde_json::json! conversions of these values cannot fail"
)]
pub fn breakpoints_cleared(id: u64) -> ControlResponse {
    ControlResponse::ok(id, json!({ "status": "cleared" }))
}

/// `breakpoints.clear_id`: the breakpoints of one file are cleared.
#[must_use]
#[allow(
    clippy::missing_panics_doc,
    reason = "the serde_json::json! conversions of these values cannot fail"
)]
pub fn breakpoints_cleared_for_file(id: u64, file_id: u32) -> ControlResponse {
    ControlResponse::ok(id, json!({ "status": "cleared", "file_id": file_id }))
}

/// `step_in`, `step_over` and `step_out`: the scan runs on to the step's boundary.
#[must_use]
#[allow(
    clippy::missing_panics_doc,
    reason = "the serde_json::json! conversions of these values cannot fail"
)]
pub fn stepping(id: u64) -> ControlResponse {
    ControlResponse::ok(id, json!({ "status": "stepping" }))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_stop_names_its_place_only_when_it_has_one() {
        // Specification 13, "Stopped Events", and specification 11 §6.13.3: a stop carries its
        // reason label, thread and breakpoint generation, and names its file, line, column and path
        // only when the runtime can place it. A pause on thread 2 has no generation and no place; a
        // breakpoint stop in generation 7 at line 4, column 5 of `a/b.st` (file 3) has both. Wrong
        // behaviour caught: a missing `null` generation, a place that is always present, and a
        // reason spelled other than its stable label.
        assert_eq!(
            stop(StopReason::Pause, Some(2), None, None),
            json!({"reason": "pause", "thread_id": 2, "breakpoint_generation": null})
        );
        let place = StopPlace {
            file_id: 3,
            line: 4,
            column: 5,
            path: "a/b.st",
        };
        assert_eq!(
            stop(StopReason::Breakpoint, Some(1), Some(7), Some(place)),
            json!({
                "reason": "breakpoint",
                "thread_id": 1,
                "breakpoint_generation": 7,
                "file_id": 3,
                "line": 4,
                "column": 5,
                "path": "a/b.st",
            })
        );
    }

    #[test]
    fn a_refusal_carries_its_stable_code_and_text() {
        // Specification 11 §6.13.2 and §6.13.3: the refusal of a program without a line table and the
        // refusal of an expression have their own stable codes and texts. Wrong behaviour caught: a
        // refusal without its code (a client could not tell it from another error).
        let no_table = no_line_table(7);
        assert!(!no_table.ok);
        assert_eq!(no_table.error.as_deref(), Some(NO_LINE_TABLE));
        assert_eq!(no_table.error_code.as_deref(), Some("no_line_table"));
        let no_expressions = expressions_unavailable(8);
        assert_eq!(
            no_expressions.error_code.as_deref(),
            Some("expr_version_unsupported")
        );
    }
}
