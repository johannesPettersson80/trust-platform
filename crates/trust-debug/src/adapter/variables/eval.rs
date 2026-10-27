//! Evaluate request handling.
//! - handle_evaluate: DAP evaluate, answered by the connected runtime

use serde_json::Value;

use crate::protocol::{EvaluateArguments, Request};

use super::super::{DebugAdapter, DispatchOutcome};

impl DebugAdapter {
    /// `evaluate`: the expression compiled by the tool from the project's types and evaluated by
    /// the runtime (specification 11 §6.13.2).
    pub(in crate::adapter) fn handle_evaluate(
        &mut self,
        request: Request<Value>,
    ) -> DispatchOutcome {
        let Some(args) = request
            .arguments
            .clone()
            .and_then(|value| serde_json::from_value::<EvaluateArguments>(value).ok())
        else {
            return DispatchOutcome {
                responses: vec![self.error_response(&request, "invalid evaluate args")],
                ..DispatchOutcome::default()
            };
        };
        if self.remote_session.is_none() {
            return self.not_connected(&request);
        }
        let result = self.compile_for_runtime(&args.expression).and_then(|expr| {
            self.remote_session
                .as_mut()
                .map_or(Err(String::from("not attached")), |remote| {
                    remote.evaluate_compiled(&expr, args.frame_id)
                })
        });
        match result {
            Ok(body) => DispatchOutcome {
                responses: vec![self.ok_response(&request, Some(body))],
                ..DispatchOutcome::default()
            },
            Err(err) => DispatchOutcome {
                responses: vec![self.error_response(&request, &err)],
                ..DispatchOutcome::default()
            },
        }
    }
}
