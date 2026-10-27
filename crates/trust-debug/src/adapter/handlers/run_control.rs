//! Continue/pause/step handlers, answered by the runtime the session is connected to.
//! - handle_continue: resume execution
//! - handle_pause: request pause
//! - handle_next/step_in/step_out: stepping commands

use serde_json::Value;
use std::sync::atomic::Ordering;

use crate::protocol::{
    ContinueArguments, ContinueResponseBody, NextArguments, PauseArguments, Request,
    StepInArguments, StepOutArguments,
};

use super::super::remote::RemoteSession;
use super::super::{DebugAdapter, DispatchOutcome};

impl DebugAdapter {
    pub(in crate::adapter) fn handle_continue(
        &mut self,
        request: Request<Value>,
    ) -> DispatchOutcome {
        if request
            .arguments
            .clone()
            .and_then(|value| serde_json::from_value::<ContinueArguments>(value).ok())
            .is_none()
        {
            return DispatchOutcome {
                responses: vec![self.error_response(&request, "invalid continue args")],
                ..DispatchOutcome::default()
            };
        }
        self.pause_expected.store(false, Ordering::SeqCst);
        self.run_control(
            &request,
            RemoteSession::resume,
            Some(ContinueResponseBody {
                all_threads_continued: Some(true),
            }),
            Vec::new(),
        )
    }

    pub(in crate::adapter) fn handle_pause(&mut self, request: Request<Value>) -> DispatchOutcome {
        if request
            .arguments
            .clone()
            .and_then(|value| serde_json::from_value::<PauseArguments>(value).ok())
            .is_none()
        {
            return DispatchOutcome {
                responses: vec![self.error_response(&request, "invalid pause args")],
                ..DispatchOutcome::default()
            };
        }
        if self.remote_session.is_some() {
            self.pause_expected.store(true, Ordering::SeqCst);
        }
        let events = vec![self.debug_output_message("[trust-debug] pause requested")];
        self.run_control(&request, RemoteSession::pause, None::<Value>, events)
    }

    pub(in crate::adapter) fn handle_next(&mut self, request: Request<Value>) -> DispatchOutcome {
        if request
            .arguments
            .clone()
            .and_then(|value| serde_json::from_value::<NextArguments>(value).ok())
            .is_none()
        {
            return DispatchOutcome {
                responses: vec![self.error_response(&request, "invalid next args")],
                ..DispatchOutcome::default()
            };
        }
        self.run_control(
            &request,
            RemoteSession::step_over,
            None::<Value>,
            Vec::new(),
        )
    }

    pub(in crate::adapter) fn handle_step_in(
        &mut self,
        request: Request<Value>,
    ) -> DispatchOutcome {
        if request
            .arguments
            .clone()
            .and_then(|value| serde_json::from_value::<StepInArguments>(value).ok())
            .is_none()
        {
            return DispatchOutcome {
                responses: vec![self.error_response(&request, "invalid stepIn args")],
                ..DispatchOutcome::default()
            };
        }
        self.run_control(&request, RemoteSession::step_in, None::<Value>, Vec::new())
    }

    pub(in crate::adapter) fn handle_step_out(
        &mut self,
        request: Request<Value>,
    ) -> DispatchOutcome {
        if request
            .arguments
            .clone()
            .and_then(|value| serde_json::from_value::<StepOutArguments>(value).ok())
            .is_none()
        {
            return DispatchOutcome {
                responses: vec![self.error_response(&request, "invalid stepOut args")],
                ..DispatchOutcome::default()
            };
        }
        self.run_control(&request, RemoteSession::step_out, None::<Value>, Vec::new())
    }

    /// Sends one run-control request to the connected runtime and answers `request` with `body`
    /// and `events`; the stop that follows waits for the answer.
    fn run_control<T: serde::Serialize>(
        &mut self,
        request: &Request<Value>,
        send: fn(&mut RemoteSession) -> Result<(), String>,
        body: Option<T>,
        events: Vec<Value>,
    ) -> DispatchOutcome {
        let Some(remote) = self.remote_session.as_mut() else {
            return self.not_connected(request);
        };
        if let Err(error) = send(remote) {
            return DispatchOutcome {
                responses: vec![self.error_response(request, &error)],
                ..DispatchOutcome::default()
            };
        }
        DispatchOutcome {
            responses: vec![self.ok_response(request, body)],
            events,
            stop_gate: Some(self.stop_gate.enter()),
            ..DispatchOutcome::default()
        }
    }
}
