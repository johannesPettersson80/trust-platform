//! Initialize/launch/configuration handlers.
//! - handle_initialize: client capabilities + adapter options
//! - handle_launch: launch argument handling
//! - handle_configuration_done: apply pending launch actions

use std::path::PathBuf;
use std::time::Instant;

use serde_json::Value;

use trust_runtime::control::ControlEndpoint;

use crate::protocol::{
    AttachArguments, Capabilities, InitializeArguments, InitializeResponseBody, LaunchArguments,
    Request,
};

use super::super::control_bridge::{default_control_endpoint, DebugControlServer};
use super::super::launch::{
    launch_control_auth_token, launch_control_endpoint, launch_program_path, launch_runtime_root,
    launch_stop_on_entry, source_options_from_launch,
};
use super::super::remote::attach_from_args;
use super::super::util::is_configuration_request;
use super::super::{
    CoordinateConverter, DebugAdapter, DispatchOutcome, LaunchActions, LaunchState, PendingAttach,
    PendingLaunch, PendingStart,
};

const CONFIGURATION_DONE_TIMEOUT: std::time::Duration = std::time::Duration::from_secs(2);

impl DebugAdapter {
    pub(in crate::adapter) fn handle_initialize(
        &mut self,
        request: Request<Value>,
    ) -> DispatchOutcome {
        let args = request
            .arguments
            .clone()
            .and_then(|value| serde_json::from_value::<InitializeArguments>(value).ok())
            .unwrap_or_default();

        self.coordinate = CoordinateConverter::new(
            args.lines_start_at1.unwrap_or(true),
            args.columns_start_at1.unwrap_or(true),
        );
        self.launch_state = LaunchState::default();

        let capabilities = Capabilities {
            supports_configuration_done_request: Some(true),
            supports_conditional_breakpoints: Some(true),
            supports_hit_conditional_breakpoints: Some(true),
            supports_log_points: Some(true),
            supports_breakpoint_locations_request: Some(true),
            supports_function_breakpoints: Some(false),
            supports_evaluate_for_hovers: Some(true),
            supports_set_variable: Some(true),
            supports_set_expression: Some(true),
            supports_pause_request: Some(true),
            supports_terminate_request: Some(true),
        };

        let response = self.ok_response(&request, Some(InitializeResponseBody { capabilities }));

        let initialized_event = self.event("initialized", Option::<Value>::None);
        let capabilities_event = self.debug_output_message(
            "[trust-debug] capabilities: pause=true terminate=true".to_string(),
        );
        let debug_event = self.debug_output_message(format!(
            "[trust-debug] initialize: lines_start_at1={} columns_start_at1={}",
            self.coordinate.lines_start_at1(),
            self.coordinate.columns_start_at1()
        ));

        DispatchOutcome {
            responses: vec![response],
            events: vec![initialized_event, capabilities_event, debug_event],
            should_exit: false,
            stop_gate: None,
        }
    }

    pub(in crate::adapter) fn handle_launch(&mut self, request: Request<Value>) -> DispatchOutcome {
        if self.remote_session.is_some() {
            return DispatchOutcome {
                responses: vec![
                    self.error_response(&request, "launch not available in attach mode")
                ],
                ..DispatchOutcome::default()
            };
        }
        let args = request
            .arguments
            .clone()
            .and_then(|value| serde_json::from_value::<LaunchArguments>(value).ok())
            .unwrap_or_default();
        let mut events = Vec::new();
        if !self.launch_state.is_configured() {
            let since = self
                .launch_state
                .pending_since()
                .unwrap_or_else(Instant::now);
            self.launch_state
                .set_pending(PendingStart::Launch(PendingLaunch {
                    request,
                    args,
                    since,
                }));
            events.push(
                self.debug_output_message("[trust-debug] launch deferred until configurationDone"),
            );
            return DispatchOutcome {
                events,
                ..DispatchOutcome::default()
            };
        }

        self.handle_launch_inner(request, args)
    }

    pub(in crate::adapter) fn handle_launch_inner(
        &mut self,
        request: Request<Value>,
        args: LaunchArguments,
    ) -> DispatchOutcome {
        self.launch_state.set_configured();
        if self.runner.is_some() {
            self.stop_runner();
        }
        let program = launch_program_path(&args);
        let stop_on_entry = launch_stop_on_entry(&args);
        let source_update = source_options_from_launch(&args);
        self.session.update_source_options(source_update);
        let mut events = Vec::new();
        events.push(self.debug_output_message(format!(
            "[trust-debug] launch: program={} stopOnEntry={} configurationDone={}",
            program.as_deref().unwrap_or("<none>"),
            stop_on_entry,
            self.launch_state.is_configured()
        )));

        if let Some(program) = program {
            self.session.set_program_path(program.clone());
            match self.session.reload_program(Some(&program)) {
                Ok(updated) => {
                    let mut events = events;
                    events.push(self.debug_output_message(format!(
                        "[trust-debug] reload_program ok; updated_breakpoints={}",
                        updated.len()
                    )));
                    let mut breakpoint_events = updated
                        .into_iter()
                        .map(|breakpoint| self.breakpoint_event("changed", breakpoint))
                        .collect::<Vec<_>>();
                    events.append(&mut breakpoint_events);
                    self.emit_io_state_event_from_runtime(&mut events);
                    let mut actions = LaunchActions::default();
                    if stop_on_entry {
                        actions.pause_after_launch = true;
                        events.push(
                            self.debug_output_message("[trust-debug] stopOnEntry: pause requested"),
                        );
                    }
                    actions.start_runner_after_launch = true;
                    self.launch_state.set_post_launch(actions);
                    events.push(self.debug_output_message(
                        "[trust-debug] runner start scheduled (post-launch)",
                    ));
                    self.ensure_control_server(&args, &mut events);
                    return DispatchOutcome {
                        responses: vec![self.ok_response::<Value>(&request, None)],
                        events,
                        should_exit: false,
                        stop_gate: None,
                    };
                }
                Err(err) => {
                    events.push(self.debug_output_message(format!(
                        "[trust-debug] reload_program error: {}",
                        err
                    )));
                    return DispatchOutcome {
                        responses: vec![self.error_response(&request, &err.to_string())],
                        events,
                        ..DispatchOutcome::default()
                    };
                }
            }
        }

        let mut actions = LaunchActions::default();
        if stop_on_entry {
            actions.pause_after_launch = true;
            events.push(self.debug_output_message("[trust-debug] stopOnEntry: pause requested"));
        }
        actions.start_runner_after_launch = true;
        self.launch_state.set_post_launch(actions);
        events
            .push(self.debug_output_message("[trust-debug] runner start scheduled (post-launch)"));
        self.ensure_control_server(&args, &mut events);
        DispatchOutcome {
            responses: vec![self.ok_response::<Value>(&request, None)],
            events,
            ..DispatchOutcome::default()
        }
    }

    fn ensure_control_server(&mut self, args: &LaunchArguments, events: &mut Vec<Value>) {
        if self.control_server.is_some() {
            return;
        }
        let endpoint = match launch_control_endpoint(args) {
            Some(text) => match ControlEndpoint::parse(&text) {
                Ok(endpoint) => endpoint,
                Err(err) => {
                    events.push(self.debug_output_message(format!(
                        "[trust-debug] control endpoint error: {err}"
                    )));
                    return;
                }
            },
            None => default_control_endpoint(),
        };
        let auth_token = launch_control_auth_token(args);
        let project_root = launch_runtime_root(args).map(PathBuf::from);
        match DebugControlServer::start(self.session.as_ref(), endpoint, auth_token, project_root) {
            Ok(server) => {
                let label = format_control_endpoint(server.endpoint());
                events.push(
                    self.debug_output_message(format!("[trust-debug] control server: {label}")),
                );
                self.control_server = Some(server);
            }
            Err(err) => {
                events.push(self.debug_output_message(format!(
                    "[trust-debug] control server start failed: {err}"
                )));
            }
        }
    }

    pub(in crate::adapter) fn handle_configuration_done(
        &mut self,
        request: Request<Value>,
    ) -> DispatchOutcome {
        let pending = self.launch_state.take_pending();
        self.launch_state.set_configured();
        let mut events =
            vec![self.debug_output_message("[trust-debug] configurationDone received")];
        let mut responses = vec![self.ok_response::<Value>(&request, None)];
        let mut should_exit = false;

        if let Some(pending) = pending {
            let outcome = match pending {
                PendingStart::Launch(pending) => {
                    self.handle_launch_inner(pending.request, pending.args)
                }
                PendingStart::Attach(pending) => {
                    self.handle_attach_inner(pending.request, pending.args)
                }
            };
            responses.extend(outcome.responses);
            events.extend(outcome.events);
            should_exit = outcome.should_exit;
        }

        DispatchOutcome {
            responses,
            events,
            should_exit,
            stop_gate: None,
        }
    }

    pub(in crate::adapter) fn maybe_force_start_after_timeout(
        &mut self,
        command: &str,
    ) -> Option<DispatchOutcome> {
        if self.launch_state.is_configured() || !self.launch_state.has_pending_launch() {
            return None;
        }
        let mut force_launch = false;
        if let Some(since) = self.launch_state.pending_since() {
            if since.elapsed() >= CONFIGURATION_DONE_TIMEOUT {
                force_launch = true;
            }
        }
        if !force_launch && !is_configuration_request(command) {
            force_launch = true;
        }
        if !force_launch {
            return None;
        }
        let pending = self.launch_state.take_pending()?;
        self.launch_state.set_configured();
        let mut outcome = match pending {
            PendingStart::Launch(pending) => {
                self.handle_launch_inner(pending.request, pending.args)
            }
            PendingStart::Attach(pending) => {
                self.handle_attach_inner(pending.request, pending.args)
            }
        };
        outcome.events.insert(
            0,
            self.debug_output_message(format!(
                "[trust-debug] configurationDone missing; proceeding with start after {}",
                command
            )),
        );
        Some(outcome)
    }

    pub(in crate::adapter) fn handle_attach(&mut self, request: Request<Value>) -> DispatchOutcome {
        if self.remote_session.is_some() {
            return DispatchOutcome {
                responses: vec![self.error_response(&request, "already attached")],
                ..DispatchOutcome::default()
            };
        }
        let args = request
            .arguments
            .clone()
            .and_then(|value| serde_json::from_value::<AttachArguments>(value).ok())
            .unwrap_or_default();
        let mut events = Vec::new();
        if !self.launch_state.is_configured() {
            let since = self
                .launch_state
                .pending_since()
                .unwrap_or_else(Instant::now);
            self.launch_state
                .set_pending(PendingStart::Attach(PendingAttach {
                    request,
                    args,
                    since,
                }));
            events.push(
                self.debug_output_message("[trust-debug] attach deferred until configurationDone"),
            );
            return DispatchOutcome {
                events,
                ..DispatchOutcome::default()
            };
        }

        self.handle_attach_inner(request, args)
    }

    fn handle_attach_inner(
        &mut self,
        request: Request<Value>,
        args: AttachArguments,
    ) -> DispatchOutcome {
        let mut events = Vec::new();
        self.launch_state.set_configured();
        match attach_from_args(&args) {
            Ok((remote_session, state)) => {
                self.remote_session = Some(remote_session);
                self.start_remote_polling();
                events
                    .push(self.debug_output_message("[trust-debug] attach: connected".to_string()));
                if let Some(state) = state {
                    if state.paused {
                        if let Some(stop) = state.last_stop {
                            events.extend(self.remote_stop_events(stop));
                        }
                    }
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
                events,
                should_exit: false,
                stop_gate: None,
            },
        }
    }
}

fn format_control_endpoint(endpoint: &ControlEndpoint) -> String {
    match endpoint {
        ControlEndpoint::Tcp(addr) => format!("tcp://{addr}"),
        #[cfg(unix)]
        ControlEndpoint::Unix(path) => format!("unix://{}", path.display()),
    }
}
