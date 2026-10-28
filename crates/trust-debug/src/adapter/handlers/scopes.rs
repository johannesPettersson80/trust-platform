//! Scope enumeration handler.
//! - handle_scopes: the scopes of a frame, from the connected runtime

use serde_json::Value;

use crate::protocol::{Request, ScopesArguments, ScopesResponseBody};

use super::super::{DebugAdapter, DispatchOutcome};

impl DebugAdapter {
    pub(in crate::adapter) fn handle_scopes(&mut self, request: Request<Value>) -> DispatchOutcome {
        let Some(args) = request
            .arguments
            .clone()
            .and_then(|value| serde_json::from_value::<ScopesArguments>(value).ok())
        else {
            return DispatchOutcome {
                responses: vec![self.error_response(&request, "invalid scopes args")],
                ..DispatchOutcome::default()
            };
        };
        let Some(remote) = self.remote_session.as_mut() else {
            return self.not_connected(&request);
        };
        let mut scopes = remote.scopes(args.frame_id).unwrap_or_default();
        for scope in &mut scopes {
            if let Some(line) = scope.line.as_mut() {
                *line = self.to_client_line(*line);
            }
            if let Some(column) = scope.column.as_mut() {
                *column = self.to_client_column(*column);
            }
            if let Some(end_line) = scope.end_line.as_mut() {
                *end_line = self.to_client_line(*end_line);
            }
            if let Some(end_column) = scope.end_column.as_mut() {
                *end_column = self.to_client_column(*end_column);
            }
        }
        let body = ScopesResponseBody { scopes };
        DispatchOutcome {
            responses: vec![self.ok_response(&request, Some(body))],
            ..DispatchOutcome::default()
        }
    }
}
