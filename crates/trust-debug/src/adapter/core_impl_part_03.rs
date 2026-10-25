impl DebugAdapter {
    pub(super) fn dispatch_request(&mut self, request: Request<Value>) -> DispatchOutcome {
        if request.message_type != MessageType::Request {
            return DispatchOutcome::default();
        }

        match request.command.as_str() {
            "initialize" => self.handle_initialize(request),
            "launch" => self.handle_launch(request),
            "attach" => self.handle_attach(request),
            "configurationDone" => self.handle_configuration_done(request),
            "disconnect" => self.handle_disconnect(request),
            "terminate" => self.handle_terminate(request),
            "setBreakpoints" => self.handle_set_breakpoints(request),
            "setExceptionBreakpoints" => self.handle_set_exception_breakpoints(request),
            "breakpointLocations" => self.handle_breakpoint_locations(request),
            "stIoState" => self.handle_io_state(request),
            "stIoWrite" => self.handle_io_write(request),
            "stIoForce" => self.handle_io_force(request),
            "stIoRelease" => self.handle_io_release(request),
            "stReload" => self.handle_reload(request),
            "threads" => self.handle_threads(request),
            "stackTrace" => self.handle_stack_trace(request),
            "scopes" => self.handle_scopes(request),
            "variables" => self.handle_variables(request),
            "setVariable" => self.handle_set_variable(request),
            "setExpression" => self.handle_set_expression(request),
            "continue" => self.handle_continue(request),
            "pause" => self.handle_pause(request),
            "next" => self.handle_next(request),
            "stepIn" => self.handle_step_in(request),
            "stepOut" => self.handle_step_out(request),
            "evaluate" => self.handle_evaluate(request),
            _ => DispatchOutcome {
                responses: vec![self.error_response(&request, "unsupported command")],
                ..DispatchOutcome::default()
            },
        }
    }

    fn next_seq(&self) -> u32 {
        self.next_seq.fetch_add(1, Ordering::Relaxed)
    }

    pub(super) fn ok_response<T>(&self, request: &Request<Value>, body: Option<T>) -> Value
    where
        T: Serialize,
    {
        let body = body
            .map(|payload| serde_json::to_value(payload))
            .transpose()
            .unwrap_or(None);
        let response = Response {
            seq: self.next_seq(),
            message_type: MessageType::Response,
            request_seq: request.seq,
            success: true,
            command: request.command.clone(),
            message: None,
            body,
        };
        serde_json::to_value(response).unwrap_or(Value::Null)
    }

    /// The answer to a request that needs a running program, before a launch or an attach
    /// connected to one.
    pub(super) fn not_connected(&self, request: &Request<Value>) -> DispatchOutcome {
        DispatchOutcome {
            responses: vec![self.error_response(request, NOT_CONNECTED)],
            ..DispatchOutcome::default()
        }
    }

    pub(super) fn error_response(&self, request: &Request<Value>, message: &str) -> Value {
        let response: Response<Value> = Response {
            seq: self.next_seq(),
            message_type: MessageType::Response,
            request_seq: request.seq,
            success: false,
            command: request.command.clone(),
            message: Some(message.to_string()),
            body: None,
        };
        serde_json::to_value(response).unwrap_or(Value::Null)
    }

    pub(super) fn event<T>(&self, name: &str, body: Option<T>) -> Value
    where
        T: Serialize,
    {
        let body = body
            .map(|payload| serde_json::to_value(payload))
            .transpose()
            .unwrap_or(None);
        let event = Event {
            seq: self.next_seq(),
            message_type: MessageType::Event,
            event: name.to_string(),
            body,
        };
        serde_json::to_value(event).unwrap_or(Value::Null)
    }

    pub(super) fn debug_output_message(&self, message: impl Into<String>) -> Value {
        let body = serde_json::json!({ "message": message.into() });
        self.event("trustDebugInternal", Some(body))
    }

    pub(super) fn breakpoint_event(&self, reason: &str, breakpoint: Breakpoint) -> Value {
        let body = BreakpointEventBody {
            reason: reason.to_string(),
            breakpoint,
        };
        self.event("breakpoint", Some(body))
    }

    pub(super) fn start_remote_polling(&mut self) {
        if self.remote_stop_poller.is_some() {
            return;
        }
        let Some(remote) = self.remote_session.as_ref() else {
            return;
        };
        let Some(writer) = self.dap_writer.clone() else {
            return;
        };
        let poller = super::stop_remote::RemoteStopPoller::spawn(
            super::stop_remote::RemoteStopPollerConfig {
                endpoint: remote.endpoint().clone(),
                token: remote.token().map(|value| value.to_string()),
                stop_gate: self.stop_gate.clone(),
                pause_expected: Arc::clone(&self.pause_expected),
                writer,
                logger: self.dap_logger.clone(),
                seq: Arc::clone(&self.next_seq),
                breakpoints: Arc::clone(&self.remote_breakpoints),
                coordinate: self.coordinate,
            },
        );
        self.remote_stop_poller = Some(poller);
    }

    pub(super) fn stop_remote_polling(&mut self) {
        if let Some(poller) = self.remote_stop_poller.take() {
            poller.stop();
        }
    }

    pub(super) fn remote_stop_events(&self, stop: RemoteStop) -> Vec<Value> {
        let thread_id = stop.thread_id.or(Some(1));
        let output = self.debug_output_message(format!(
            "[trust-debug] stopped: reason={} thread_id={}",
            stop.reason,
            thread_id
                .map(|id| id.to_string())
                .unwrap_or_else(|| "<none>".to_string())
        ));
        let stopped = self.event(
            "stopped",
            Some(StoppedEventBody {
                reason: stop.reason,
                thread_id,
                all_threads_stopped: Some(true),
            }),
        );
        vec![output, stopped]
    }
}
