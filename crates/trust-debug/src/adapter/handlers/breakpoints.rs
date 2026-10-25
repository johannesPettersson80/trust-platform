//! Breakpoint-related requests and location queries.
//! - handle_set_breakpoints: configure source breakpoints
//! - handle_set_exception_breakpoints: ignore exception breakpoints
//! - handle_breakpoint_locations: enumerate valid locations

use serde_json::Value;

use crate::protocol::{
    Breakpoint, BreakpointLocationsArguments, BreakpointLocationsResponseBody, Request,
    SetBreakpointsArguments, SetBreakpointsResponseBody, SourceBreakpoint,
};

use super::super::{AttachBreakpoints, DebugAdapter, DispatchOutcome};

/// The message of a breakpoint set while an attach is pending (specification 13).
const PENDING_ATTACH: &str = "pending until the attach completes";

/// The message of a breakpoint set while a launch is pending (specification 13).
const PENDING_LAUNCH: &str = "pending until the launch completes";

/// The message of a breakpoint the runtime did not resolve: no statement starts at or after its
/// line (specification 13).
pub(in crate::adapter) const NO_STATEMENT: &str = "no statement at or after this line";

impl DebugAdapter {
    pub(in crate::adapter) fn handle_set_breakpoints(
        &mut self,
        request: Request<Value>,
    ) -> DispatchOutcome {
        let Some(args) = request
            .arguments
            .clone()
            .and_then(|value| serde_json::from_value::<SetBreakpointsArguments>(value).ok())
        else {
            return DispatchOutcome {
                responses: vec![self.error_response(&request, "invalid setBreakpoints args")],
                ..DispatchOutcome::default()
            };
        };

        // Until a launch or an attach connects to a runtime, the breakpoints are kept, and the
        // runtime the session starts takes them (specification 13, session lifecycle; specification
        // 44 NX-16).
        if self.remote_session.is_none() {
            let body = self.record_attach_breakpoints(args);
            return DispatchOutcome {
                responses: vec![self.ok_response(&request, Some(body))],
                events: vec![self.debug_output_message(
                    "[trust-debug] setBreakpoints: kept until the session starts",
                )],
                ..DispatchOutcome::default()
            };
        }

        let source_path = args.source.path.as_deref().unwrap_or("<none>");
        let requested = args
            .breakpoints
            .as_ref()
            .map(|items| items.len())
            .or_else(|| args.lines.as_ref().map(|items| items.len()))
            .unwrap_or(0);
        let events = vec![self.debug_output_message(format!(
            "[trust-debug] setBreakpoints: path={source_path} requested={requested}"
        ))];
        let body = self.set_breakpoints(args);
        DispatchOutcome {
            responses: vec![self.ok_response(&request, Some(body))],
            events,
            ..DispatchOutcome::default()
        }
    }

    /// Keeps the breakpoints of one source that a client set while an attach is pending, in place
    /// of the source's earlier ones, and answers them unverified, each with an id of the adapter
    /// (specification 13, session lifecycle).
    fn record_attach_breakpoints(
        &mut self,
        args: SetBreakpointsArguments,
    ) -> SetBreakpointsResponseBody {
        let requested = requested_breakpoints(&args);
        let ids: Vec<u32> = requested
            .iter()
            .map(|_| {
                let id = self.next_breakpoint_id;
                self.next_breakpoint_id = self.next_breakpoint_id.wrapping_add(1);
                id
            })
            .collect();
        let pending = if self.launch_state.has_pending_attach() {
            PENDING_ATTACH
        } else {
            PENDING_LAUNCH
        };
        let breakpoints = requested
            .iter()
            .zip(&ids)
            .map(|(breakpoint, id)| Breakpoint {
                id: Some(*id),
                verified: false,
                message: Some(pending.to_string()),
                source: Some(args.source.clone()),
                line: Some(breakpoint.line),
                column: breakpoint.column,
                end_line: None,
                end_column: None,
            })
            .collect();
        self.attach_breakpoints
            .retain(|kept| kept.args.source.path != args.source.path);
        if !ids.is_empty() {
            self.attach_breakpoints
                .push(AttachBreakpoints { args, ids });
        }
        SetBreakpointsResponseBody { breakpoints }
    }

    /// Sends the breakpoints kept while the attach was pending to the runtime the attach connected
    /// to, each source's lines in ascending order, and returns a `breakpoint` event for each with
    /// the id its pending answer gave it (specification 13, session lifecycle).
    pub(in crate::adapter) fn apply_attach_breakpoints(&mut self) -> Vec<Value> {
        let mut events = Vec::new();
        for AttachBreakpoints { mut args, ids } in core::mem::take(&mut self.attach_breakpoints) {
            let mut ordered: Vec<(SourceBreakpoint, u32)> =
                requested_breakpoints(&args).into_iter().zip(ids).collect();
            ordered.sort_by_key(|(breakpoint, _)| breakpoint.line);
            args.breakpoints = Some(
                ordered
                    .iter()
                    .map(|(breakpoint, _)| breakpoint.clone())
                    .collect(),
            );
            args.lines = None;
            let source = args.source.clone();
            // The runtime answers the lines it resolves in the order asked; a line it does not
            // resolve lies after the file's last statement, so those are the last ones.
            let mut resolved = self.set_breakpoints(args).breakpoints.into_iter();
            for (requested, id) in ordered {
                let breakpoint = match resolved.next() {
                    Some(breakpoint) => Breakpoint {
                        id: Some(id),
                        ..breakpoint
                    },
                    None => Breakpoint {
                        id: Some(id),
                        verified: false,
                        message: Some(NO_STATEMENT.to_string()),
                        source: Some(source.clone()),
                        line: Some(requested.line),
                        column: requested.column,
                        end_line: None,
                        end_column: None,
                    },
                };
                events.push(self.breakpoint_event("changed", breakpoint));
            }
        }
        events
    }

    pub(in crate::adapter) fn handle_set_exception_breakpoints(
        &mut self,
        request: Request<Value>,
    ) -> DispatchOutcome {
        let events =
            vec![self.debug_output_message("[trust-debug] setExceptionBreakpoints ignored")];
        DispatchOutcome {
            responses: vec![self.ok_response::<Value>(&request, None)],
            events,
            should_exit: false,
            stop_gate: None,
        }
    }

    pub(in crate::adapter) fn handle_breakpoint_locations(
        &mut self,
        request: Request<Value>,
    ) -> DispatchOutcome {
        let Some(args) = request
            .arguments
            .clone()
            .and_then(|value| serde_json::from_value::<BreakpointLocationsArguments>(value).ok())
        else {
            return DispatchOutcome {
                responses: vec![self.error_response(&request, "invalid breakpointLocations args")],
                ..DispatchOutcome::default()
            };
        };

        let Some(path) = args.source.path.as_deref() else {
            return DispatchOutcome {
                responses: vec![self.error_response(&request, "source path not provided")],
                ..DispatchOutcome::default()
            };
        };

        let runtime_line = match self.to_runtime_line(args.line) {
            Some(line) => line,
            None => {
                return DispatchOutcome {
                    responses: vec![self.error_response(&request, "invalid line")],
                    ..DispatchOutcome::default()
                };
            }
        };
        let runtime_column = match args.column {
            Some(column) => match self.to_runtime_column(column) {
                Some(col) => Some(col),
                None => {
                    return DispatchOutcome {
                        responses: vec![self.error_response(&request, "invalid column")],
                        ..DispatchOutcome::default()
                    };
                }
            },
            None => None,
        };
        let runtime_end_line = match args.end_line {
            Some(end_line) => match self.to_runtime_line(end_line) {
                Some(end) => Some(end),
                None => {
                    return DispatchOutcome {
                        responses: vec![self.error_response(&request, "invalid end line")],
                        ..DispatchOutcome::default()
                    };
                }
            },
            None => None,
        };
        let runtime_end_column = match args.end_column {
            Some(end_column) => match self.to_runtime_column(end_column) {
                Some(end) => Some(end),
                None => {
                    return DispatchOutcome {
                        responses: vec![self.error_response(&request, "invalid end column")],
                        ..DispatchOutcome::default()
                    };
                }
            },
            None => None,
        };

        let Some(remote) = self.remote_session.as_mut() else {
            let body = BreakpointLocationsResponseBody {
                breakpoints: Vec::new(),
            };
            return DispatchOutcome {
                responses: vec![self.ok_response(&request, Some(body))],
                ..DispatchOutcome::default()
            };
        };
        let response = remote.breakpoint_locations(
            path,
            runtime_line,
            runtime_column,
            runtime_end_line,
            runtime_end_column,
        );
        let body = match response {
            Ok(mut body) => {
                for breakpoint in &mut body.breakpoints {
                    breakpoint.line = self.to_client_line(breakpoint.line);
                    if let Some(column) = breakpoint.column.as_mut() {
                        *column = self.to_client_column(*column);
                    }
                }
                body
            }
            Err(_) => BreakpointLocationsResponseBody {
                breakpoints: Vec::new(),
            },
        };
        DispatchOutcome {
            responses: vec![self.ok_response(&request, Some(body))],
            ..DispatchOutcome::default()
        }
    }
}

/// The breakpoints `args` asks for: its `breakpoints`, or its deprecated `lines`.
pub(in crate::adapter) fn requested_breakpoints(
    args: &SetBreakpointsArguments,
) -> Vec<SourceBreakpoint> {
    if let Some(breakpoints) = &args.breakpoints {
        return breakpoints.clone();
    }
    args.lines
        .iter()
        .flatten()
        .map(|line| SourceBreakpoint {
            line: *line,
            column: None,
            condition: None,
            hit_condition: None,
            log_message: None,
        })
        .collect()
}
