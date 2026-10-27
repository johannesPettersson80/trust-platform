//! Stack trace request handling.
//! - handle_stack_trace: the connected runtime's frames, with the client's slicing

use serde_json::Value;

use crate::protocol::{Request, StackTraceArguments, StackTraceResponseBody};

use super::super::remote::client_source_path;
use super::super::{DebugAdapter, DispatchOutcome};

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
        let Some(remote) = self.remote_session.as_mut() else {
            return self.not_connected(&request);
        };
        let mut stack_frames = remote.stack_trace().unwrap_or_default();
        let root = self.label_root();
        for frame in &mut stack_frames {
            if let Some(source) = frame.source.as_mut() {
                source.path = source
                    .path
                    .take()
                    .map(|path| client_source_path(root.as_deref(), path));
            }
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
        DispatchOutcome {
            responses: vec![self.ok_response(&request, Some(body))],
            ..DispatchOutcome::default()
        }
    }
}
