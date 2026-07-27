//! Stack trace request handling.
//! - handle_stack_trace: resolve frames and apply client slicing

use serde_json::Value;

use crate::protocol::{Request, StackFrame, StackTraceArguments, StackTraceResponseBody};

use super::super::{DebugAdapter, DispatchOutcome, PausedStateView};

impl DebugAdapter {
    pub(in crate::adapter) fn handle_stack_trace(
        &mut self,
        request: Request<Value>,
    ) -> DispatchOutcome {
        let Some(args) = request
            .arguments
            .clone()
            .and_then(|value| serde_json::from_value::<StackTraceArguments>(value).ok())
        else {
            return DispatchOutcome {
                responses: vec![self.error_response(&request, "invalid stackTrace args")],
                ..DispatchOutcome::default()
            };
        };

        if !self
            .projected_threads()
            .iter()
            .any(|thread| thread.id == args.thread_id)
        {
            return DispatchOutcome {
                responses: vec![self.error_response(&request, "unknown thread id")],
                ..DispatchOutcome::default()
            };
        }

        if let Some(remote) = self.remote_session.as_mut() {
            let mut stack_frames = remote.stack_trace().unwrap_or_default();
            for frame in &mut stack_frames {
                frame.line = self.to_client_line(frame.line);
                frame.column = self.to_client_column(frame.column);
                if let Some(end_line) = frame.end_line.as_mut() {
                    *end_line = self.to_client_line(*end_line);
                }
                if let Some(end_column) = frame.end_column.as_mut() {
                    *end_column = self.to_client_column(*end_column);
                }
            }
            let total_frames = stack_frames.len() as u32;
            let start = usize::try_from(args.start_frame.unwrap_or(0)).unwrap_or(0);
            let levels = args
                .levels
                .and_then(|levels| usize::try_from(levels).ok())
                .unwrap_or(stack_frames.len());
            let sliced = stack_frames
                .drain(start.min(stack_frames.len())..)
                .take(levels)
                .collect::<Vec<_>>();
            let body = StackTraceResponseBody {
                stack_frames: sliced,
                total_frames: Some(total_frames),
            };
            return DispatchOutcome {
                responses: vec![self.ok_response(&request, Some(body))],
                ..DispatchOutcome::default()
            };
        }

        let location = self.current_location();
        let frame_locations = self.session.debug_control().frame_locations();
        let view =
            PausedStateView::new(self.session.debug_control(), self.session.runtime_handle());
        let frames = view
            .with_storage(|storage| storage.frames().to_vec())
            .unwrap_or_default();
        let mut stack_frames: Vec<StackFrame> = if frames.is_empty() {
            let (source, line, column) = location
                .clone()
                .unwrap_or_else(|| (None, self.default_line(), self.default_column()));
            vec![StackFrame {
                id: 0,
                name: "Main".to_string(),
                source,
                line,
                column,
                end_line: None,
                end_column: None,
            }]
        } else {
            frames
                .iter()
                .rev()
                .map(|frame| {
                    let resolved = frame_locations
                        .get(&frame.id)
                        .and_then(|loc| self.location_to_client(loc));
                    let (source, line, column) = resolved.unwrap_or_else(|| {
                        location
                            .clone()
                            .unwrap_or_else(|| (None, self.default_line(), self.default_column()))
                    });
                    StackFrame {
                        id: frame.id.0,
                        name: frame.owner.to_string(),
                        source,
                        line,
                        column,
                        end_line: None,
                        end_column: None,
                    }
                })
                .collect()
        };
        let total_frames = stack_frames.len() as u32;

        let start = usize::try_from(args.start_frame.unwrap_or(0)).unwrap_or(0);
        let levels = args
            .levels
            .and_then(|levels| usize::try_from(levels).ok())
            .unwrap_or(stack_frames.len());

        let sliced = stack_frames
            .drain(start.min(stack_frames.len())..)
            .take(levels)
            .collect::<Vec<_>>();

        let body = StackTraceResponseBody {
            stack_frames: sliced,
            total_frames: Some(total_frames),
        };

        DispatchOutcome {
            responses: vec![self.ok_response(&request, Some(body))],
            ..DispatchOutcome::default()
        }
    }
}
