//! Adapter core loop + request dispatch.
//! - DebugAdapter::new/session accessors
//! - run/run_with_stdio: protocol loop
//! - dispatch_request/handle_request: route DAP requests
//! - event helpers: output/stopped/terminated

use std::collections::{HashMap, HashSet};
use std::fs::OpenOptions;
use std::io::{self, BufReader, BufWriter};
use std::sync::atomic::{AtomicBool, AtomicU32, Ordering};
use std::sync::{mpsc, Arc, Mutex};
use std::thread;
use std::time::{Duration as StdDuration, Instant};

use serde::Serialize;
use serde_json::Value;

use trust_runtime::debug::{location_to_line_col, DebugLog, DebugStop};
use trust_runtime::error::RuntimeError;
use trust_runtime::io::IoSnapshot;
use trust_runtime::value::Duration;
use trust_runtime::RuntimeMetadata;

use crate::protocol::{
    Breakpoint, BreakpointEventBody, Event, MessageType, OutputEventBody, Request, Response,
    SetBreakpointsArguments, SetBreakpointsResponseBody, Source, StoppedEventBody,
};
use crate::runtime::DebugRuntime;

use super::io::io_state_from_snapshot;
use super::protocol_io::{read_message, write_message_locked, write_protocol_log};
use super::remote::RemoteStop;
use super::stop::StopCoordinator;
use super::util::env_flag;
use super::{CoordinateConverter, DebugAdapter, DispatchOutcome, LaunchState, StopGate};

const IO_EVENT_MIN_INTERVAL: StdDuration = StdDuration::from_millis(150);

impl DebugAdapter {
    #[must_use]
    pub fn new(session: impl DebugRuntime + 'static) -> Self {
        Self {
            session: Box::new(session),
            remote_session: None,
            remote_stop_poller: None,
            remote_breakpoints: Arc::new(Mutex::new(HashMap::new())),
            next_seq: Arc::new(AtomicU32::new(1)),
            coordinate: CoordinateConverter::new(true, true),
            variable_handles: HashMap::new(),
            next_variable_ref: 1,
            watch_cache: HashMap::new(),
            runner: None,
            control_server: None,
            last_io_state: Arc::new(Mutex::new(None)),
            forced_io_addresses: Arc::new(Mutex::new(HashSet::new())),
            launch_state: LaunchState::default(),
            pause_expected: Arc::new(AtomicBool::new(false)),
            stop_gate: StopGate::new(),
            dap_writer: None,
            dap_logger: None,
        }
    }

    pub fn session(&self) -> &dyn DebugRuntime {
        self.session.as_ref()
    }

    pub fn session_mut(&mut self) -> &mut dyn DebugRuntime {
        self.session.as_mut()
    }

    #[must_use]
    pub fn into_session(self) -> Box<dyn DebugRuntime> {
        self.session
    }

    #[must_use]
    pub fn set_breakpoints(&mut self, args: SetBreakpointsArguments) -> SetBreakpointsResponseBody {
        if self.remote_session.is_some() {
            return self.set_breakpoints_remote(args);
        }
        let adjusted = self.to_session_breakpoints(args);
        let response = self.session.set_breakpoints(&adjusted);
        self.to_client_breakpoints(response)
    }

    /// Run a blocking stdio loop that processes DAP requests.
    pub fn run_stdio(&mut self) -> io::Result<()> {
        let stdin = io::stdin();
        let mut reader = BufReader::new(stdin.lock());
        let writer = Arc::new(Mutex::new(BufWriter::new(io::stdout())));
        self.dap_writer = Some(writer.clone());

        fn emit_verbose(
            adapter: &DebugAdapter,
            writer: &Arc<Mutex<BufWriter<io::Stdout>>>,
            dap_log: &Option<Arc<Mutex<BufWriter<std::fs::File>>>>,
            message: String,
        ) -> io::Result<()> {
            let event = adapter.debug_output_message(message);
            let serialized = serde_json::to_string(&event)
                .map_err(|err| io::Error::new(io::ErrorKind::InvalidData, err))?;
            if let Some(logger) = dap_log {
                let _ = write_protocol_log(logger, "->", &serialized);
            }
            write_message_locked(writer, &serialized)
        }

        let dap_log_path = std::env::var("ST_DEBUG_DAP_LOG").ok();
        let dap_log = dap_log_path
            .as_deref()
            .and_then(|path| OpenOptions::new().create(true).append(true).open(path).ok())
            .map(BufWriter::new)
            .map(|writer| Arc::new(Mutex::new(writer)));
        self.dap_logger = dap_log.clone();
        let dap_verbose = env_flag("ST_DEBUG_DAP_VERBOSE");

        let (log_tx, log_rx) = mpsc::channel::<DebugLog>();
        self.session.debug_control().set_log_sender(log_tx);
        let (io_tx, io_rx) = mpsc::channel::<IoSnapshot>();
        self.session.debug_control().set_io_sender(io_tx);
        let (stop_tx, stop_rx) = mpsc::channel::<DebugStop>();
        let stop_control = self.session.debug_control();
        stop_control.set_stop_sender(stop_tx);
        let log_writer = Arc::clone(&writer);
        let log_logger = dap_log.clone();
        let log_seq = Arc::clone(&self.next_seq);
        let log_thread = thread::spawn(move || {
            while let Ok(log) = log_rx.recv() {
                let output = if log.message.ends_with('\n') {
                    log.message
                } else {
                    format!("{}\n", log.message)
                };
                let body = OutputEventBody {
                    output,
                    category: Some("console".to_string()),
                    source: None,
                    line: None,
                    column: None,
                };
                let event = Event {
                    seq: log_seq.fetch_add(1, Ordering::Relaxed),
                    message_type: MessageType::Event,
                    event: "output".to_string(),
                    body: Some(body),
                };
                let serialized = match serde_json::to_string(&event) {
                    Ok(serialized) => serialized,
                    Err(_) => continue,
                };
                if let Some(logger) = &log_logger {
                    let _ = write_protocol_log(logger, "->", &serialized);
                }
                if write_message_locked(&log_writer, &serialized).is_err() {
                    break;
                }
            }
        });
        let io_writer = Arc::clone(&writer);
        let io_logger = dap_log.clone();
        let io_seq = Arc::clone(&self.next_seq);
        let io_state_cache = Arc::clone(&self.last_io_state);
        let forced_io_addresses = Arc::clone(&self.forced_io_addresses);
        let io_thread = thread::spawn(move || {
            let mut last_sent = Instant::now() - IO_EVENT_MIN_INTERVAL;
            while let Ok(snapshot) = io_rx.recv() {
                let mut latest = snapshot;
                while let Ok(next) = io_rx.try_recv() {
                    latest = next;
                }
                let mut body = io_state_from_snapshot(latest);
                if let Ok(forced) = forced_io_addresses.lock() {
                    for entry in body
                        .inputs
                        .iter_mut()
                        .chain(body.outputs.iter_mut())
                        .chain(body.memory.iter_mut())
                    {
                        entry.forced = forced.contains(entry.address.as_str());
                    }
                }
                let mut should_emit = true;
                if let Ok(mut cache) = io_state_cache.lock() {
                    if let Some(previous) = cache.as_ref() {
                        if previous == &body {
                            should_emit = false;
                        }
                    }
                    if should_emit {
                        *cache = Some(body.clone());
                    }
                }
                if !should_emit {
                    continue;
                }
                let elapsed = last_sent.elapsed();
                if elapsed < IO_EVENT_MIN_INTERVAL {
                    thread::sleep(IO_EVENT_MIN_INTERVAL - elapsed);
                }
                let event = Event {
                    seq: io_seq.fetch_add(1, Ordering::Relaxed),
                    message_type: MessageType::Event,
                    event: "stIoState".to_string(),
                    body: Some(body),
                };
                let serialized = match serde_json::to_string(&event) {
                    Ok(serialized) => serialized,
                    Err(_) => continue,
                };
                if let Some(logger) = &io_logger {
                    let _ = write_protocol_log(logger, "->", &serialized);
                }
                if write_message_locked(&io_writer, &serialized).is_err() {
                    break;
                }
                last_sent = Instant::now();
            }
        });
        let stop_thread = StopCoordinator::new(
            self.stop_gate.clone(),
            Arc::clone(&self.pause_expected),
            self.session.debug_control(),
            Arc::clone(&writer),
            dap_log.clone(),
            Arc::clone(&self.next_seq),
        )
        .spawn(stop_rx);

        let mut announced_verbose = false;

        loop {
            let Some(payload) = read_message(&mut reader)? else {
                if dap_verbose {
                    emit_verbose(
                        self,
                        &writer,
                        &dap_log,
                        "[trust-debug][dap] stdin closed".to_string(),
                    )?;
                }
                break;
            };
            if let Some(logger) = &dap_log {
                let _ = write_protocol_log(logger, "<-", &payload);
            }
            if dap_verbose && !announced_verbose {
                let log_hint = match dap_log_path.as_deref() {
                    Some(path) => {
                        format!("[trust-debug] DAP verbose logging enabled; raw log: {path}")
                    }
                    None => "[trust-debug] DAP verbose logging enabled (set ST_DEBUG_DAP_LOG=/path for raw)".to_string(),
                };
                emit_verbose(self, &writer, &dap_log, log_hint)?;
                announced_verbose = true;
            }
            if dap_verbose {
                emit_verbose(
                    self,
                    &writer,
                    &dap_log,
                    format!(
                        "[trust-debug][dap<-] len={} payload={}",
                        payload.len(),
                        payload
                    ),
                )?;
            }

            let request: Request<Value> = match serde_json::from_str(&payload) {
                Ok(request) => request,
                Err(err) => {
                    if dap_verbose {
                        emit_verbose(
                            self,
                            &writer,
                            &dap_log,
                            format!("[trust-debug][dap] invalid json: {err} payload={payload}"),
                        )?;
                    }
                    continue;
                }
            };

            if dap_verbose {
                let actions = self.launch_state.pending_actions();
                emit_verbose(
                    self,
                    &writer,
                    &dap_log,
                    format!(
                        "[trust-debug][dap] dispatch: seq={} command={} configured={} pending_launch={} post_launch_actions={:?}",
                        request.seq,
                        request.command,
                        self.launch_state.is_configured(),
                        self.launch_state.has_pending_launch(),
                        actions
                    ),
                )?;
            }
            let command = request.command.clone();
            let mut outcome = self.dispatch_request(request);
            if let Some(mut timeout_outcome) = self.maybe_force_start_after_timeout(&command) {
                outcome.responses.append(&mut timeout_outcome.responses);
                outcome.events.append(&mut timeout_outcome.events);
                outcome.should_exit |= timeout_outcome.should_exit;
            }
            let _stop_gate = outcome.stop_gate.as_ref();
            if dap_verbose {
                let actions = self.launch_state.pending_actions();
                emit_verbose(
                    self,
                    &writer,
                    &dap_log,
                    format!(
                        "[trust-debug][dap] outcome: responses={} events={} should_exit={} configured={} pending_launch={} post_launch_actions={:?}",
                        outcome.responses.len(),
                        outcome.events.len(),
                        outcome.should_exit,
                        self.launch_state.is_configured(),
                        self.launch_state.has_pending_launch(),
                        actions
                    ),
                )?;
            }
            for response in outcome.responses {
                let serialized = serde_json::to_string(&response)
                    .map_err(|err| io::Error::new(io::ErrorKind::InvalidData, err))?;
                if let Some(logger) = &dap_log {
                    let _ = write_protocol_log(logger, "->", &serialized);
                }
                if dap_verbose {
                    emit_verbose(
                        self,
                        &writer,
                        &dap_log,
                        format!("[trust-debug][dap->] {serialized}"),
                    )?;
                }
                write_message_locked(&writer, &serialized)?;
            }
            for event in outcome.events {
                let serialized = serde_json::to_string(&event)
                    .map_err(|err| io::Error::new(io::ErrorKind::InvalidData, err))?;
                if let Some(logger) = &dap_log {
                    let _ = write_protocol_log(logger, "->", &serialized);
                }
                if dap_verbose {
                    emit_verbose(
                        self,
                        &writer,
                        &dap_log,
                        format!("[trust-debug][dap->] {serialized}"),
                    )?;
                }
                write_message_locked(&writer, &serialized)?;
            }
            let actions = self.launch_state.take_actions();
            if actions.pause_after_launch {
                self.pause_expected.store(true, Ordering::SeqCst);
                self.session.debug_control().pause_entry();
            }
            if actions.start_runner_after_launch && self.runner.is_none() {
                self.start_runner();
                let event = self.debug_output_message("[trust-debug] runner started (post-launch)");
                let serialized = serde_json::to_string(&event)
                    .map_err(|err| io::Error::new(io::ErrorKind::InvalidData, err))?;
                if let Some(logger) = &dap_log {
                    let _ = write_protocol_log(logger, "->", &serialized);
                }
                write_message_locked(&writer, &serialized)?;
            }
            if outcome.should_exit {
                break;
            }
        }

        self.stop_runner();
        self.stop_remote_polling();
        self.session.debug_control().clear_log_sender();
        self.session.debug_control().clear_io_sender();
        self.session.debug_control().clear_stop_sender();
        let _ = log_thread.join();
        let _ = io_thread.join();
        let _ = stop_thread.join();
        Ok(())
    }

    pub(super) fn dispatch_request(&mut self, request: Request<Value>) -> DispatchOutcome {
        if request.message_type != MessageType::Request {
            return DispatchOutcome::default();
        }

        let mut outcome = match request.command.as_str() {
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
            "stVarState" => self.handle_var_state(request),
            "stVarWrite" => self.handle_var_write(request),
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
        };

        outcome.events.extend(self.drain_log_events());
        outcome
    }

    pub(super) fn start_runner(&mut self) {
        if self.runner.is_some() {
            return;
        }
        let runtime = self.session.runtime_handle();
        let control = self.session.debug_control();
        let cycle_time = cycle_time_hint(self.session.metadata());
        let cycle_interval = wall_interval_for_cycle(cycle_time);
        let stop = Arc::new(AtomicBool::new(false));
        let stop_flag = Arc::clone(&stop);
        let handle = thread::spawn(move || loop {
            if stop_flag.load(Ordering::Relaxed) {
                break;
            }
            let cycle_start = Instant::now();
            let mut runtime = match runtime.lock() {
                Ok(guard) => guard,
                Err(poisoned) => poisoned.into_inner(),
            };
            match runtime.execute_cycle() {
                Ok(()) => runtime.advance_time(cycle_time),
                Err(err) => {
                    if !matches!(err, RuntimeError::InvalidControlFlow) {
                        eprintln!("runtime cycle error: {err}");
                    }
                }
            }
            drop(runtime);

            let elapsed = cycle_start.elapsed();
            if elapsed >= cycle_interval {
                continue;
            }
            let deadline = cycle_start + cycle_interval;
            sleep_until_or_stopped(&stop_flag, deadline);
        });
        self.runner = Some(DebugRunner {
            stop,
            handle,
            control,
        });
    }

    pub(super) fn stop_runner(&mut self) {
        if let Some(runner) = self.runner.take() {
            runner.stop();
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

    fn drain_log_events(&self) -> Vec<Value> {
        let logs = self.session.debug_control().drain_logs();
        logs.into_iter().map(|log| self.output_event(log)).collect()
    }

    fn output_event(&self, log: DebugLog) -> Value {
        let (source, line, column) = log
            .location
            .and_then(|location| {
                let source = self.session.source_for_file_id(location.file_id);
                let text = self.session.source_text_for_file_id(location.file_id)?;
                let (line, column) = location_to_line_col(text, &location);
                Some((
                    source,
                    Some(self.to_client_line(line)),
                    Some(self.to_client_column(column)),
                ))
            })
            .unwrap_or((None, None, None));

        let output = if log.message.ends_with('\n') {
            log.message
        } else {
            format!("{}\n", log.message)
        };

        let body = OutputEventBody {
            output,
            category: Some("console".to_string()),
            source,
            line,
            column,
        };

        self.event("output", Some(body))
    }

    pub(super) fn debug_output_message(&self, message: impl Into<String>) -> Value {
        let output = format!("{}\n", message.into());
        let body = OutputEventBody {
            output,
            category: Some("console".to_string()),
            source: None,
            line: None,
            column: None,
        };
        self.event("output", Some(body))
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

    fn to_session_breakpoints(&self, mut args: SetBreakpointsArguments) -> SetBreakpointsArguments {
        let adjust_line = |line: u32| -> u32 {
            if self.coordinate.lines_start_at1() {
                line
            } else {
                line.saturating_add(1)
            }
        };
        let adjust_column = |column: Option<u32>| -> Option<u32> {
            column.map(|column| {
                if self.coordinate.columns_start_at1() {
                    column
                } else {
                    column.saturating_add(1)
                }
            })
        };

        if let Some(breakpoints) = args.breakpoints.as_mut() {
            for breakpoint in breakpoints {
                breakpoint.line = adjust_line(breakpoint.line);
                breakpoint.column = adjust_column(breakpoint.column);
            }
        }

        if let Some(lines) = args.lines.as_mut() {
            for line in lines.iter_mut() {
                *line = adjust_line(*line);
            }
        }

        args
    }

    fn to_client_breakpoints(
        &self,
        mut response: SetBreakpointsResponseBody,
    ) -> SetBreakpointsResponseBody {
        let adjust_line = |line: u32| -> u32 {
            if self.coordinate.lines_start_at1() {
                line
            } else {
                line.saturating_sub(1)
            }
        };
        let adjust_column = |column: u32| -> u32 {
            if self.coordinate.columns_start_at1() {
                column
            } else {
                column.saturating_sub(1)
            }
        };

        for breakpoint in &mut response.breakpoints {
            if let Some(line) = breakpoint.line.as_mut() {
                *line = adjust_line(*line);
            }
            if let Some(column) = breakpoint.column.as_mut() {
                *column = adjust_column(*column);
            }
            if let Some(line) = breakpoint.end_line.as_mut() {
                *line = adjust_line(*line);
            }
            if let Some(column) = breakpoint.end_column.as_mut() {
                *column = adjust_column(*column);
            }
        }

        response
    }

    fn set_breakpoints_remote(
        &mut self,
        args: SetBreakpointsArguments,
    ) -> SetBreakpointsResponseBody {
        let source_path = match args.source.path.as_deref() {
            Some(path) => path,
            None => {
                return SetBreakpointsResponseBody {
                    breakpoints: Vec::new(),
                };
            }
        };
        let mut lines = Vec::new();
        if let Some(breakpoints) = args.breakpoints.as_ref() {
            for breakpoint in breakpoints {
                if let Some(line) = self.to_runtime_line(breakpoint.line) {
                    lines.push(line);
                }
            }
        } else if let Some(list) = args.lines.as_ref() {
            for line in list {
                if let Some(line) = self.to_runtime_line(*line) {
                    lines.push(line);
                }
            }
        }
        if lines.is_empty() {
            if let Some(remote) = self.remote_session.as_mut() {
                let _ = remote.clear_breakpoints(source_path);
            }
            return SetBreakpointsResponseBody {
                breakpoints: Vec::new(),
            };
        }
        let response = if let Some(remote) = self.remote_session.as_mut() {
            remote.set_breakpoints(source_path, lines)
        } else {
            return SetBreakpointsResponseBody {
                breakpoints: Vec::new(),
            };
        };
        match response {
            Ok((mut breakpoints, file_id, generation)) => {
                if let (Some(file_id), Some(generation)) = (file_id, generation) {
                    if let Ok(mut guard) = self.remote_breakpoints.lock() {
                        guard.insert(file_id, generation);
                    }
                }
                for breakpoint in &mut breakpoints {
                    if let Some(line) = breakpoint.line.as_mut() {
                        *line = self.to_client_line(*line);
                    }
                    if let Some(column) = breakpoint.column.as_mut() {
                        *column = self.to_client_column(*column);
                    }
                }
                SetBreakpointsResponseBody { breakpoints }
            }
            Err(_) => SetBreakpointsResponseBody {
                breakpoints: Vec::new(),
            },
        }
    }
    pub(super) fn current_location(&self) -> Option<(Option<Source>, u32, u32)> {
        let location = self.session.debug_control().last_location()?;
        self.location_to_client(&location)
    }

    pub(super) fn location_to_client(
        &self,
        location: &trust_runtime::debug::SourceLocation,
    ) -> Option<(Option<Source>, u32, u32)> {
        let source = self.session.source_for_file_id(location.file_id);
        let text = self.session.source_text_for_file_id(location.file_id)?;
        let (line, column) = location_to_line_col(text, location);
        Some((
            source,
            self.to_client_line(line),
            self.to_client_column(column),
        ))
    }

    pub(super) fn to_client_line(&self, line: u32) -> u32 {
        self.coordinate.to_client_line(line)
    }

    pub(super) fn to_client_column(&self, column: u32) -> u32 {
        self.coordinate.to_client_column(column)
    }

    pub(super) fn to_runtime_line(&self, line: u32) -> Option<u32> {
        self.coordinate.to_runtime_line(line)
    }

    pub(super) fn to_runtime_column(&self, column: u32) -> Option<u32> {
        self.coordinate.to_runtime_column(column)
    }

    pub(super) fn default_line(&self) -> u32 {
        self.coordinate.default_line()
    }

    pub(super) fn default_column(&self) -> u32 {
        self.coordinate.default_column()
    }
}

#[derive(Debug)]
pub(super) struct DebugRunner {
    stop: Arc<AtomicBool>,
    handle: thread::JoinHandle<()>,
    control: trust_runtime::debug::DebugControl,
}

impl DebugRunner {
    pub(super) fn stop(self) {
        self.stop.store(true, Ordering::Relaxed);
        self.control.clear_breakpoints();
        self.control.continue_run();
        let _ = self.handle.join();
    }
}

fn cycle_time_hint(metadata: &RuntimeMetadata) -> Duration {
    metadata
        .tasks()
        .iter()
        .map(|task| task.interval)
        .filter(|interval| interval.as_nanos() > 0)
        .min()
        .unwrap_or_else(|| Duration::from_millis(10))
}

fn wall_interval_for_cycle(cycle_time: Duration) -> StdDuration {
    let nanos = cycle_time.as_nanos();
    if nanos <= 0 {
        return StdDuration::from_millis(10);
    }
    let nanos = u64::try_from(nanos).unwrap_or(u64::MAX);
    StdDuration::from_nanos(nanos)
}

fn sleep_until_or_stopped(stop_flag: &AtomicBool, deadline: Instant) {
    const MAX_SLEEP_CHUNK: StdDuration = StdDuration::from_millis(5);

    while !stop_flag.load(Ordering::Relaxed) {
        let now = Instant::now();
        if now >= deadline {
            break;
        }
        let remaining = deadline.duration_since(now);
        thread::sleep(remaining.min(MAX_SLEEP_CHUNK));
    }
}
