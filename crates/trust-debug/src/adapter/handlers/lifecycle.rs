//! Adapter lifecycle and reload handlers.
//! - handle_disconnect: tear down session
//! - handle_terminate: stop runtime
//! - handle_reload: update source options

use serde_json::Value;

use trust_runtime::debug::DebugMode;

use crate::protocol::{
    DisconnectArguments, ReloadArguments, Request, TerminateArguments, TerminatedEventBody,
};
use crate::session::SourceOptionsUpdate;

use super::super::{DebugAdapter, DispatchOutcome};

impl DebugAdapter {
    pub(in crate::adapter) fn handle_disconnect(
        &mut self,
        request: Request<Value>,
    ) -> DispatchOutcome {
        let args = request
            .arguments
            .clone()
            .and_then(|value| serde_json::from_value::<DisconnectArguments>(value).ok());

        if self.remote_session.is_some() {
            self.stop_remote_polling();
            self.remote_session = None;
            if let Ok(mut guard) = self.remote_breakpoints.lock() {
                guard.clear();
            }
        } else {
            self.session.debug_control().continue_run();
            self.session.debug_control().clear_watch_expressions();
            self.watch_cache.clear();
        }
        let terminated_event = self.event(
            "terminated",
            Some(TerminatedEventBody {
                restart: args.as_ref().and_then(|value| value.restart),
            }),
        );

        DispatchOutcome {
            responses: vec![self.ok_response::<Value>(&request, None)],
            should_exit: true,
            events: vec![
                self.debug_output_message("[trust-debug] disconnect"),
                terminated_event,
            ],
            stop_gate: None,
        }
    }

    pub(in crate::adapter) fn handle_terminate(
        &mut self,
        request: Request<Value>,
    ) -> DispatchOutcome {
        let args = request
            .arguments
            .clone()
            .and_then(|value| serde_json::from_value::<TerminateArguments>(value).ok());

        if self.remote_session.is_some() {
            self.stop_remote_polling();
            self.remote_session = None;
            if let Ok(mut guard) = self.remote_breakpoints.lock() {
                guard.clear();
            }
        } else {
            self.session.debug_control().continue_run();
            self.session.debug_control().clear_watch_expressions();
            self.watch_cache.clear();
        }
        let terminated_event = self.event(
            "terminated",
            Some(TerminatedEventBody {
                restart: args.as_ref().and_then(|value| value.restart),
            }),
        );

        DispatchOutcome {
            responses: vec![self.ok_response::<Value>(&request, None)],
            should_exit: true,
            events: vec![
                self.debug_output_message("[trust-debug] terminate"),
                terminated_event,
            ],
            stop_gate: None,
        }
    }
    pub(in crate::adapter) fn handle_reload(&mut self, request: Request<Value>) -> DispatchOutcome {
        if self.remote_session.is_some() {
            return DispatchOutcome {
                responses: vec![
                    self.error_response(&request, "reload not supported in attach mode")
                ],
                ..DispatchOutcome::default()
            };
        }
        let args = request
            .arguments
            .clone()
            .and_then(|value| serde_json::from_value::<ReloadArguments>(value).ok())
            .unwrap_or_default();

        match self.reload_with_state(args) {
            Ok((updated, restarted_runner)) => {
                let events = updated
                    .into_iter()
                    .map(|breakpoint| self.breakpoint_event("changed", breakpoint))
                    .collect();
                let mut events = events;
                if !restarted_runner {
                    self.emit_io_state_event_from_runtime(&mut events);
                }
                DispatchOutcome {
                    responses: vec![self.ok_response::<Value>(&request, None)],
                    events,
                    should_exit: false,
                    stop_gate: None,
                }
            }
            Err(err) => DispatchOutcome {
                responses: vec![self.error_response(&request, &err.to_string())],
                ..DispatchOutcome::default()
            },
        }
    }

    fn reload_with_state(
        &mut self,
        args: ReloadArguments,
    ) -> Result<(Vec<crate::protocol::Breakpoint>, bool), trust_runtime::harness::CompileError>
    {
        self.session.update_source_options(SourceOptionsUpdate {
            root: args.runtime_root.clone(),
            include_globs: args.runtime_include_globs.clone(),
            exclude_globs: args.runtime_exclude_globs.clone(),
            ignore_pragmas: args.runtime_ignore_pragmas.clone(),
        });

        let was_running = self.runner.is_some();
        let was_paused = self.session.debug_control().mode() == DebugMode::Paused;
        if was_running {
            self.stop_runner();
        }

        let reload_result = self.session.reload_program(args.program.as_deref());

        if was_running {
            self.start_runner();
            if was_paused {
                self.pause_expected
                    .store(true, std::sync::atomic::Ordering::SeqCst);
                self.session.debug_control().pause();
            }
        }

        reload_result.map(|breakpoints| (breakpoints, was_running))
    }
}
