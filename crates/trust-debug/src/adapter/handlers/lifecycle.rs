//! Adapter lifecycle and reload handlers.
//! - handle_disconnect: tear down session
//! - handle_terminate: stop runtime
//! - handle_reload: change a launch's program online

use serde_json::Value;

use crate::protocol::{
    DisconnectArguments, ReloadArguments, Request, TerminateArguments, TerminatedEventBody,
};

use super::super::{DebugAdapter, DispatchOutcome};

impl DebugAdapter {
    pub(in crate::adapter) fn handle_disconnect(
        &mut self,
        request: Request<Value>,
    ) -> DispatchOutcome {
        let restart = request
            .arguments
            .clone()
            .and_then(|value| serde_json::from_value::<DisconnectArguments>(value).ok())
            .and_then(|args| args.restart);
        self.end_session();
        self.ended(&request, "[trust-debug] disconnect", restart)
    }

    pub(in crate::adapter) fn handle_terminate(
        &mut self,
        request: Request<Value>,
    ) -> DispatchOutcome {
        let restart = request
            .arguments
            .clone()
            .and_then(|value| serde_json::from_value::<TerminateArguments>(value).ok())
            .and_then(|args| args.restart);
        self.end_session();
        self.ended(&request, "[trust-debug] terminate", restart)
    }

    /// Disconnects from the runtime, and stops the launch's program when the session launched it.
    fn end_session(&mut self) {
        self.stop_remote_polling();
        self.remote_session = None;
        if let Ok(mut guard) = self.remote_breakpoints.lock() {
            guard.clear();
        }
        self.stop_simulator();
    }

    /// The answer to a request that ended the session, with its `terminated` event.
    fn ended(
        &mut self,
        request: &Request<Value>,
        message: &str,
        restart: Option<bool>,
    ) -> DispatchOutcome {
        let terminated_event = self.event("terminated", Some(TerminatedEventBody { restart }));
        DispatchOutcome {
            responses: vec![self.ok_response::<Value>(request, None)],
            should_exit: true,
            events: vec![self.debug_output_message(message), terminated_event],
            stop_gate: None,
        }
    }

    /// `stReload`: a launch changes its program online (specification 44 NX-16); an attach to a
    /// runtime the session did not launch takes no reload.
    pub(in crate::adapter) fn handle_reload(&mut self, request: Request<Value>) -> DispatchOutcome {
        if self.simulator.is_none() {
            let message = if self.remote_session.is_some() {
                "reload not supported in attach mode"
            } else {
                super::super::core::NOT_CONNECTED
            };
            return DispatchOutcome {
                responses: vec![self.error_response(&request, message)],
                ..DispatchOutcome::default()
            };
        }
        let args = request
            .arguments
            .clone()
            .and_then(|value| serde_json::from_value::<ReloadArguments>(value).ok())
            .unwrap_or_default();
        let response = match self.reload_compiled(args) {
            Ok(()) => self.ok_response::<Value>(&request, None),
            Err(error) => self.error_response(&request, &error),
        };
        DispatchOutcome {
            responses: vec![response],
            ..DispatchOutcome::default()
        }
    }
}
