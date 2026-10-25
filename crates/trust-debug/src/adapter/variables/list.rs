//! Variables listing, answered by the connected runtime.

use serde_json::Value;

use crate::protocol::{Request, VariablesArguments, VariablesResponseBody};

use super::super::{DebugAdapter, DispatchOutcome};

impl DebugAdapter {
    pub(in crate::adapter) fn handle_variables(
        &mut self,
        request: Request<Value>,
    ) -> DispatchOutcome {
        let Some(args) = request
            .arguments
            .clone()
            .and_then(|value| serde_json::from_value::<VariablesArguments>(value).ok())
        else {
            return DispatchOutcome {
                responses: vec![self.error_response(&request, "invalid variables args")],
                ..DispatchOutcome::default()
            };
        };
        let Some(remote) = self.remote_session.as_mut() else {
            return self.not_connected(&request);
        };
        let variables = remote.variables(&args).unwrap_or_default();
        let body = VariablesResponseBody { variables };
        DispatchOutcome {
            responses: vec![self.ok_response(&request, Some(body))],
            ..DispatchOutcome::default()
        }
    }
}
