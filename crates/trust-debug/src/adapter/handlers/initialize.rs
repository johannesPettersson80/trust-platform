//! Initialize/launch/configuration handlers.
//! - handle_initialize: client capabilities + adapter options
//! - handle_launch: launch argument handling
//! - handle_configuration_done: apply pending launch actions

use std::time::Instant;

use serde_json::Value;

use crate::protocol::{
    AttachArguments, Capabilities, InitializeArguments, InitializeResponseBody, LaunchArguments,
    Request,
};

use super::super::launch::{
    launch_program_path, launch_stop_on_entry, source_options_from_attach,
    source_options_from_launch,
};
use super::super::remote::attach_from_args;
use super::super::util::is_configuration_request;
use super::super::{
    CoordinateConverter, DebugAdapter, DispatchOutcome, LaunchState, PendingAttach, PendingLaunch,
    PendingStart,
};

const CONFIGURATION_DONE_TIMEOUT: std::time::Duration = std::time::Duration::from_secs(2);

impl DebugAdapter {
    pub(in crate::adapter) fn handle_initialize(
        &mut self,
        request: Request<Value>,
    ) -> DispatchOutcome {
        if self.initialized {
            return DispatchOutcome {
                responses: vec![self.error_response(&request, "already initialized")],
                ..DispatchOutcome::default()
            };
        }
        let args = match request.arguments.clone() {
            Some(value) => match serde_json::from_value::<InitializeArguments>(value) {
                Ok(args) => args,
                Err(_) => {
                    return DispatchOutcome {
                        responses: vec![
                            self.error_response(&request, "invalid initialize arguments")
                        ],
                        ..DispatchOutcome::default()
                    };
                }
            },
            None => InitializeArguments::default(),
        };

        self.initialized = true;
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
        if !self.initialized {
            return DispatchOutcome {
                responses: vec![self.error_response(&request, "initialize required")],
                ..DispatchOutcome::default()
            };
        }
        if self.remote_session.is_some() {
            return DispatchOutcome {
                responses: vec![
                    self.error_response(&request, "launch not available in attach mode")
                ],
                ..DispatchOutcome::default()
            };
        }
        let args = match request.arguments.clone() {
            Some(value) => match serde_json::from_value::<LaunchArguments>(value) {
                Ok(args) => args,
                Err(_) => {
                    return DispatchOutcome {
                        responses: vec![self.error_response(&request, "invalid launch arguments")],
                        ..DispatchOutcome::default()
                    };
                }
            },
            None => LaunchArguments::default(),
        };
        let mut events = Vec::new();
        if !self.launch_state.is_configured() {
            if self.launch_state.has_pending_launch() {
                return DispatchOutcome {
                    responses: vec![self.error_response(&request, "start already pending")],
                    ..DispatchOutcome::default()
                };
            }
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
        if self.simulator.is_some() {
            self.stop_remote_polling();
            self.remote_session = None;
            self.stop_simulator();
        }
        let program = launch_program_path(&args);
        let stop_on_entry = launch_stop_on_entry(&args);
        let source_update = source_options_from_launch(&args);
        self.session.update_source_options(source_update);
        let events = vec![self.debug_output_message(format!(
            "[trust-debug] launch: program={} stopOnEntry={} configurationDone={}",
            program.as_deref().unwrap_or("<none>"),
            stop_on_entry,
            self.launch_state.is_configured()
        ))];
        let Some(program) = program else {
            self.attach_breakpoints.clear();
            return DispatchOutcome {
                responses: vec![self.error_response(&request, "the launch names no program")],
                events,
                ..DispatchOutcome::default()
            };
        };
        self.session.set_program_path(program.clone());
        // The program runs compiled for this computer (specification 44 NX-16).
        self.launch_compiled(&request, &args, &program, stop_on_entry, events)
    }

    pub(in crate::adapter) fn handle_configuration_done(
        &mut self,
        request: Request<Value>,
    ) -> DispatchOutcome {
        if !self.initialized {
            return DispatchOutcome {
                responses: vec![self.error_response(&request, "initialize required")],
                ..DispatchOutcome::default()
            };
        }
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
        if let Some(since) = self.launch_state.pending_since()
            && since.elapsed() >= CONFIGURATION_DONE_TIMEOUT
        {
            force_launch = true;
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
        if !self.initialized {
            return DispatchOutcome {
                responses: vec![self.error_response(&request, "initialize required")],
                ..DispatchOutcome::default()
            };
        }
        if self.remote_session.is_some() {
            return DispatchOutcome {
                responses: vec![self.error_response(&request, "already attached")],
                ..DispatchOutcome::default()
            };
        }
        let args = match request.arguments.clone() {
            Some(value) => match serde_json::from_value::<AttachArguments>(value) {
                Ok(args) => args,
                Err(_) => {
                    return DispatchOutcome {
                        responses: vec![self.error_response(&request, "invalid attach arguments")],
                        ..DispatchOutcome::default()
                    };
                }
            },
            None => AttachArguments::default(),
        };
        let mut events = Vec::new();
        if !self.launch_state.is_configured() {
            if self.launch_state.has_pending_launch() {
                return DispatchOutcome {
                    responses: vec![self.error_response(&request, "start already pending")],
                    ..DispatchOutcome::default()
                };
            }
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
                self.session
                    .update_source_options(source_options_from_attach(&args));
                self.expression_types = None;
                // The project's declarations name the program's tasks, whose threads the client
                // sees; without them the client sees one thread.
                if let Err(error) = self.session.load_project() {
                    events.push(self.debug_output_message(format!(
                        "[trust-debug] attach: the project's tasks are unknown: {error}"
                    )));
                }
                events
                    .push(self.debug_output_message("[trust-debug] attach: connected".to_string()));
                // The breakpoints the client set while the attach was pending reach the runtime
                // before its stops are polled (specification 13, session lifecycle).
                events.extend(self.apply_attach_breakpoints());
                self.start_remote_polling();
                if let Some(state) = state
                    && state.paused
                    && let Some(stop) = state.last_stop
                {
                    events.extend(self.remote_stop_events(stop));
                }
                DispatchOutcome {
                    responses: vec![self.ok_response::<Value>(&request, None)],
                    events,
                    should_exit: false,
                    stop_gate: None,
                }
            }
            Err(err) => {
                self.attach_breakpoints.clear();
                DispatchOutcome {
                    responses: vec![self.error_response(&request, &err.to_string())],
                    events,
                    should_exit: false,
                    stop_gate: None,
                }
            }
        }
    }
}
