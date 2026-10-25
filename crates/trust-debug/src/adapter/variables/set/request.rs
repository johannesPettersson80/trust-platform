//! setVariable handling.

use serde_json::Value;

use crate::protocol::{Request, SetVariableArguments};

use super::super::super::{DebugAdapter, DispatchOutcome};

impl DebugAdapter {
    /// `setVariable`: the runtime of a compiled program takes variable writes and forces through
    /// `setExpression` on an I/O address and the Live Values, not through the Variables view.
    pub(in crate::adapter) fn handle_set_variable(
        &mut self,
        request: Request<Value>,
    ) -> DispatchOutcome {
        if request
            .arguments
            .clone()
            .and_then(|value| serde_json::from_value::<SetVariableArguments>(value).ok())
            .is_none()
        {
            return DispatchOutcome {
                responses: vec![self.error_response(&request, "invalid setVariable args")],
                ..DispatchOutcome::default()
            };
        }
        if self.remote_session.is_none() {
            return self.not_connected(&request);
        }
        DispatchOutcome {
            responses: vec![self.error_response(
                &request,
                "setVariable is not supported for a compiled program; use setExpression",
            )],
            ..DispatchOutcome::default()
        }
    }
}
