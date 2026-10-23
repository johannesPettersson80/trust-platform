//! Breakpoint-related requests and location queries.
//! - handle_set_breakpoints: configure source breakpoints
//! - handle_set_exception_breakpoints: ignore exception breakpoints
//! - handle_breakpoint_locations: enumerate valid locations

use serde_json::Value;

use trust_runtime::debug::location_to_line_col;

use crate::protocol::{
    BreakpointLocation, BreakpointLocationsArguments, BreakpointLocationsResponseBody, Request,
    SetBreakpointsArguments,
};

use super::super::{DebugAdapter, DispatchOutcome};

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

        let source_path = args.source.path.as_deref().unwrap_or("<none>");
        let requested = args
            .breakpoints
            .as_ref()
            .map(|items| items.len())
            .or_else(|| args.lines.as_ref().map(|items| items.len()))
            .unwrap_or(0);
        let known_source = if self.remote_session.is_some() {
            true
        } else {
            self.session.source_file_for_path(source_path).is_some()
        };
        let mut events = vec![self.debug_output_message(format!(
            "[trust-debug] setBreakpoints: path={} requested={} known_source={} configured={} pending_launch={}",
            source_path,
            requested,
            known_source,
            self.launch_state.is_configured(),
            self.launch_state.has_pending_launch()
        ))];

        let body = self.set_breakpoints(args);
        if self.remote_session.is_none()
            && let Some(report) = self.session.take_breakpoint_report()
        {
            events.push(self.debug_output_message(report));
        }
        DispatchOutcome {
            responses: vec![self.ok_response(&request, Some(body))],
            events,
            ..DispatchOutcome::default()
        }
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

        if let Some(remote) = self.remote_session.as_mut() {
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
            return DispatchOutcome {
                responses: vec![self.ok_response(&request, Some(body))],
                ..DispatchOutcome::default()
            };
        }

        let Some(source_file) = self.session.source_file_for_path(path) else {
            let body = BreakpointLocationsResponseBody {
                breakpoints: Vec::new(),
            };
            return DispatchOutcome {
                responses: vec![self.ok_response(&request, Some(body))],
                ..DispatchOutcome::default()
            };
        };

        let Some(line) = self.to_runtime_line(args.line) else {
            return DispatchOutcome {
                responses: vec![self.error_response(&request, "invalid line")],
                ..DispatchOutcome::default()
            };
        };
        let column = match args.column {
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
        let end_line = match args.end_line {
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
        let end_column = match args.end_column {
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

        let mut breakpoints = Vec::new();
        if let Some(locations) = self
            .session
            .metadata()
            .statement_locations(source_file.file_id)
        {
            let max_line = end_line.unwrap_or(line);
            for location in locations {
                let (loc_line, loc_col) = location_to_line_col(&source_file.text, location);
                if loc_line < line || loc_line > max_line {
                    continue;
                }
                if let Some(min_col) = column
                    && loc_line == line
                    && loc_col < min_col
                {
                    continue;
                }
                if let Some(max_col) = end_column
                    && loc_line == max_line
                    && loc_col > max_col
                {
                    continue;
                }
                breakpoints.push(BreakpointLocation {
                    line: self.to_client_line(loc_line),
                    column: Some(self.to_client_column(loc_col)),
                    end_line: None,
                    end_column: None,
                });
            }
        }

        let body = BreakpointLocationsResponseBody { breakpoints };
        DispatchOutcome {
            responses: vec![self.ok_response(&request, Some(body))],
            ..DispatchOutcome::default()
        }
    }
}
